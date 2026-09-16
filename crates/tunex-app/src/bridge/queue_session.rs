//! Saved-queue session persistence (C13): restore on start, persist on
//! change, plus the saved-row conversions. Owns the restart story; the
//! player owns the live queue.

use std::time::{Duration, Instant};

use super::queue_model::QueueModelRust;
use crate::playback::queue_item_from_row;
use tunex_player::{PlaybackController, uri_to_path};

/// How often the same-track position is flushed to disk.
/// Frequent enough that a crash loses at most a couple of seconds; rare
/// enough not to rewrite `config.toml` on every poll tick.
const SESSION_SAVE_INTERVAL: Duration = Duration::from_secs(2);

impl QueueModelRust {
    /// Restore the SQLite queue when present, otherwise the last-track path.
    pub(crate) fn restore_session(&mut self, config: &tunex_core::TunexConfig) {
        if self.restore_saved_queue() {
            return;
        }
        self.restore_last_track(config);
    }

    /// Restore the ordered queue from the index (paused, never auto-play).
    pub(crate) fn restore_saved_queue(&mut self) -> bool {
        if !self.index_path.is_file() {
            return false;
        }
        let Ok(db) = tunex_library::open_file(&self.index_path) else {
            return false;
        };
        let Ok(saved) = tunex_library::load_playback_queue(&db) else {
            return false;
        };
        if saved.items.is_empty() {
            return false;
        }
        let items: Vec<tunex_player::QueueItem> =
            saved.items.into_iter().map(saved_to_item).collect();
        let position = Duration::from_millis(saved.position_ms);
        let Some(controller) = self.controller.as_mut() else {
            return false;
        };
        if let Err(err) = controller.restore_queue(items, saved.cursor, position) {
            tracing::debug!(
                name = "queue.restore_skipped",
                error = %err,
                "saved queue not restored"
            );
            return false;
        }
        self.last_notified_uri = self
            .controller
            .as_ref()
            .and_then(PlaybackController::current_item)
            .map(|item| item.uri);
        if let Some(item) = self
            .controller
            .as_ref()
            .and_then(PlaybackController::current_item)
        {
            item.uri.clone_into(&mut self.saved_uri);
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "UI position is i32 milliseconds"
        )]
        let saved_ms = saved.position_ms.min(i32::MAX as u64) as i32;
        self.saved_position_ms = saved_ms;
        self.saved_at = Some(Instant::now());
        true
    }

    /// Best-effort last-track restore: load paused at the saved position when
    /// the file still exists. Missing files are skipped, never an error toast.
    pub(crate) fn restore_last_track(&mut self, config: &tunex_core::TunexConfig) {
        let Some(uri) = config
            .playback
            .last_uri
            .as_deref()
            .filter(|uri| !uri.is_empty())
        else {
            return;
        };
        let item = self.item_for_uri(uri);
        let position = Duration::from_millis(config.playback.last_position_ms);
        let Some(controller) = self.controller.as_mut() else {
            return;
        };
        if let Err(err) = controller.restore_paused(item, position) {
            tracing::debug!(
                name = "queue.restore_skipped",
                error = %err,
                "last track not restored"
            );
            return;
        }
        // Deliberately no `sync_rows` here: leaving the restored entry for
        // the caller's own change check is what makes that poll report a
        // change, and a change is what emits the model reset Up Next needs.
        uri.clone_into(&mut self.saved_uri);
        // Seed the toast cursor silently: resuming is not a track change.
        self.last_notified_uri = self
            .controller
            .as_ref()
            .and_then(PlaybackController::current_item)
            .map(|item| item.uri);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "UI position is i32 milliseconds"
        )]
        let saved = config.playback.last_position_ms.min(i32::MAX as u64) as i32;
        self.saved_position_ms = saved;
        self.saved_at = Some(Instant::now());
    }

    /// Hydrate a restored URI from the index when possible (filename fallback).
    pub(crate) fn item_for_uri(&self, uri: &str) -> tunex_player::QueueItem {
        if let Some(path) = uri_to_path(uri) {
            if let Ok(db) = tunex_library::open_file(&self.index_path) {
                if let Ok(Some(row)) = tunex_library::track_by_path(&db, &path.to_string_lossy()) {
                    if let Some(item) = queue_item_from_row(&row) {
                        return item;
                    }
                }
            }
            let (title, artist) =
                tunex_library::display_title_artist(&path.to_string_lossy(), None, None);
            let mut item = tunex_player::QueueItem::new(uri, &title);
            if artist != tunex_library::UNKNOWN_ARTIST {
                item = item.with_artist(&artist);
            }
            return item;
        }
        tunex_player::QueueItem::new(uri, tunex_library::UNKNOWN_TITLE)
    }

    /// Persist last-track + position. `force` writes immediately (pause/seek);
    /// otherwise position ticks are coalesced to ~2 s.
    pub(crate) fn maybe_persist_session(&mut self, force: bool) {
        let Some(item) = self
            .controller
            .as_ref()
            .and_then(PlaybackController::current_item)
        else {
            return;
        };
        if self
            .controller
            .as_ref()
            .is_some_and(PlaybackController::restore_pending)
        {
            return;
        }
        let position = self.position_ms();
        let uri_changed = item.uri != self.saved_uri;
        let pos_changed = position != self.saved_position_ms;
        if !force && !uri_changed && !pos_changed {
            return;
        }
        let elapsed = self
            .saved_at
            .is_none_or(|at| at.elapsed() >= SESSION_SAVE_INTERVAL);
        if !force && !uri_changed && !elapsed {
            return;
        }
        self.write_audio_config();
        self.persist_queue();
        self.saved_uri = item.uri;
        self.saved_position_ms = position;
        self.saved_at = Some(Instant::now());
    }

    /// Write ordered URIs + cursor to the library index (not TOML).
    pub(crate) fn persist_queue(&self) {
        let Some(controller) = &self.controller else {
            return;
        };
        if controller.restore_pending() {
            return;
        }
        if !self.index_path.is_file() {
            return;
        }
        let items: Vec<tunex_library::SavedQueueItem> = controller
            .queue_items()
            .into_iter()
            .map(item_to_saved)
            .collect();
        let cursor = controller.current_index().unwrap_or(0);
        let position_ms = u64::try_from(self.position_ms().max(0)).unwrap_or(0);
        let Ok(mut db) = tunex_library::open_file(&self.index_path) else {
            return;
        };
        if let Err(err) = tunex_library::save_playback_queue(&mut db, &items, cursor, position_ms) {
            tracing::warn!(
                name = "queue.persist_failed",
                error = %err,
                "up next not saved"
            );
        }
    }
}

fn saved_to_item(row: tunex_library::SavedQueueItem) -> tunex_player::QueueItem {
    let mut item = tunex_player::QueueItem::new(&row.uri, &row.title);
    if let Some(track_id) = row.track_id {
        item = item.with_track_id(track_id);
    }
    if let Some(artist) = row.artist {
        item = item.with_artist(artist);
    }
    if let Some(album) = row.album {
        item = item.with_album(album);
    }
    if let Some(duration_ms) = row.duration_ms.filter(|ms| *ms > 0) {
        item = item.with_duration(Duration::from_millis(
            u64::try_from(duration_ms).unwrap_or(0),
        ));
    }
    item
}

fn item_to_saved(item: tunex_player::QueueItem) -> tunex_library::SavedQueueItem {
    tunex_library::SavedQueueItem {
        uri: item.uri,
        track_id: item.track_id,
        title: item.title,
        artist: item.artist,
        album: item.album,
        duration_ms: item
            .duration
            .and_then(|duration| i64::try_from(duration.as_millis()).ok()),
    }
}
