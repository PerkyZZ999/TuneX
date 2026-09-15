//! `QueueModel` rows and transport (bridge in [`super::models`]).
//!
//! S3 W-022 Up Next model: the single `QObject` owning the
//! [`PlaybackController`] — queue rows plus transport in one place, so no
//! state transfer is needed between siblings. The controller constructs
//! lazily (`GStreamer` may fail); until then the model is empty with surfaced
//! text, never a crash. Enqueue entry points resolve library rows by id
//! through bounded indexed queries (no scan, no decode on this path).
//!
//! All row logic lives on the plain [`QueueModelRust`] struct (fully
//! unit-tested); the `impl` below only pairs Qt model notifications around
//! it (see the S1 proof model for the pairing pattern).

use super::models::qobject;

use core::pin::Pin;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};
use tokio::sync::mpsc;

use crate::art::ArtCore;
use crate::mpris::{
    MprisCommand, MprisSnapshot, MprisTrack, TRACK_PATH_PREFIX, publish as publish_mpris,
    take_inbox, unmap_loop,
};
use crate::playback::queue_item_from_row;
use tunex_core::{ReplayGainMode, VisualizerMode, matching_preset, preset_bands};
use tunex_player::{PlaybackController, list_audio_outputs, uri_to_path};

/// How often the same-track position is flushed to disk.
/// Frequent enough that a crash loses at most a couple of seconds; rare
/// enough not to rewrite `config.toml` on every poll tick.
const SESSION_SAVE_INTERVAL: Duration = Duration::from_secs(2);

/// Human-readable `Debug` for the generated roles enum (`#[derive]` is not
/// permitted on `#[qenum]` items, so this is written by hand).
impl std::fmt::Debug for qobject::QueueRoles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Variants are associated constants on the generated type; compare
        // their discriminants rather than matching by value.
        let name = match self.repr {
            repr if repr == qobject::QueueRoles::Title.repr => "Title",
            repr if repr == qobject::QueueRoles::Artist.repr => "Artist",
            repr if repr == qobject::QueueRoles::Album.repr => "Album",
            repr if repr == qobject::QueueRoles::DurationMs.repr => "DurationMs",
            repr if repr == qobject::QueueRoles::IsCurrent.repr => "IsCurrent",
            repr if repr == qobject::QueueRoles::TrackId.repr => "TrackId",
            repr if repr == qobject::QueueRoles::Missing.repr => "Missing",
            _ => "Unknown",
        };
        write!(f, "QueueRoles::{name}")
    }
}

/// Queue row: display strings plus duration, now-playing flag, row identity.
type QueueRow = (QString, QString, QString, i32, bool, i32, bool);

/// How the row list moved since the view last heard about it.
///
/// A reset rebuilds every delegate, which cancels the row's own enter and
/// exit animations (DESIGN.md gives them 160 ms) and drops whatever the view
/// was doing — so a change that is really one row says so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowChange {
    /// Nothing to announce.
    None,
    /// Exactly this row appeared.
    Inserted(usize),
    /// Exactly this row went away.
    Removed(usize),
    /// Anything wider: the view rebuilds.
    Reset,
}

/// Classify `now` against what the view was last told (`published`).
///
/// Only the single-row cases are narrowed; a cursor move (which rewrites the
/// now-playing flag on two rows) and bulk enqueues stay resets.
fn row_change(published: &[QueueRow], now: &[QueueRow]) -> RowChange {
    if published == now {
        return RowChange::None;
    }
    if now.len() == published.len() + 1 {
        if let Some(at) = single_extra(published, now) {
            return RowChange::Inserted(at);
        }
    }
    if published.len() == now.len() + 1 {
        if let Some(at) = single_extra(now, published) {
            return RowChange::Removed(at);
        }
    }
    RowChange::Reset
}

/// Index of the one row `longer` has that `shorter` does not, when the rest
/// match in order; `None` when more than one row differs.
fn single_extra(shorter: &[QueueRow], longer: &[QueueRow]) -> Option<usize> {
    let at = shorter
        .iter()
        .zip(longer)
        .position(|(old, new)| old != new)
        .unwrap_or(shorter.len());
    (shorter[at..] == longer[at + 1..]).then_some(at)
}

/// Up Next row store plus the playback controller.
#[derive(Debug)]
pub struct QueueModelRust {
    rows: Vec<QueueRow>,
    /// Lazy cover lookups for the playing track (S6 W-040): the mini-player,
    /// Now Playing and MPRIS all read the answer.
    art: ArtCore,
    /// The rows the view has been told about, so a change can be announced
    /// as what it is (see [`row_change`]).
    published: Vec<QueueRow>,
    controller: Option<PlaybackController>,
    index_path: PathBuf,
    config_path: PathBuf,
    last_len: usize,
    last_cursor: Option<usize>,
    last_error: Option<String>,
    saved_uri: String,
    saved_position_ms: i32,
    saved_at: Option<Instant>,
    /// Inbound MPRIS commands, taken once from the process hub (S5 W-032).
    mpris_rx: Option<mpsc::Receiver<MprisCommand>>,
    /// URI of the last toasted track (S5 W-034). Restores seed it silently;
    /// only later advances pop a notification.
    last_notified_uri: Option<String>,
    outputs: Arc<Mutex<Vec<(String, String)>>>,
    lyrics_uri: String,
    lyrics_plain: String,
    lyrics_lines: Vec<(i32, String)>,
    /// True while the `TuneX` window is focused (QML `Window.active`).
    window_active: bool,
}

impl Default for QueueModelRust {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            art: ArtCore::default(),
            published: Vec::new(),
            controller: None,
            index_path: tunex_core::library_db_path(),
            config_path: tunex_core::config_file(),
            last_len: 0,
            last_cursor: None,
            last_error: None,
            saved_uri: String::new(),
            saved_position_ms: 0,
            saved_at: None,
            mpris_rx: None,
            last_notified_uri: None,
            outputs: Arc::new(Mutex::new(vec![(String::new(), "System".to_owned())])),
            lyrics_uri: String::new(),
            lyrics_plain: String::new(),
            lyrics_lines: Vec::new(),
            window_active: true,
        }
    }
}

impl QueueModelRust {
    /// Drop all rows.
    fn drop_rows(&mut self) {
        self.rows.clear();
    }

    /// How the rows moved since the view last heard about them.
    fn pending_change(&self) -> RowChange {
        row_change(&self.published, &self.rows)
    }

    /// Put the rows back to the state the view last saw, returning the
    /// current ones. Queue mutations rebuild rows as they go, so announcing
    /// an insert or removal means briefly showing the old shape again — see
    /// `apply_rows`.
    fn rewind_rows(&mut self) -> Vec<QueueRow> {
        std::mem::replace(&mut self.rows, self.published.clone())
    }

    /// Put back the rows taken by [`rewind_rows`](Self::rewind_rows).
    fn restore_rows(&mut self, rows: Vec<QueueRow>) {
        self.rows = rows;
    }

    /// Record the rows as announced; call it once the matching model signal
    /// has been emitted.
    fn mark_published(&mut self) {
        self.published.clone_from(&self.rows);
    }

    /// Current row count.
    fn row_count(&self) -> i32 {
        i32::try_from(self.rows.len()).unwrap_or(i32::MAX)
    }

    /// Data for one row/role; invalid variant when out of range or the role
    /// is unknown.
    fn row_data(&self, row: usize, role: qobject::QueueRoles) -> QVariant {
        if let Some((title, artist, album, duration_ms, is_current, track_id, missing)) =
            self.rows.get(row)
        {
            return match role {
                qobject::QueueRoles::Title => QVariant::from(title),
                qobject::QueueRoles::Artist => QVariant::from(artist),
                qobject::QueueRoles::Album => QVariant::from(album),
                qobject::QueueRoles::DurationMs => QVariant::from(duration_ms),
                qobject::QueueRoles::IsCurrent => QVariant::from(is_current),
                qobject::QueueRoles::TrackId => QVariant::from(track_id),
                qobject::QueueRoles::Missing => QVariant::from(missing),
                _ => QVariant::default(),
            };
        }
        QVariant::default()
    }

    /// Construct the controller on first use. `GStreamer` failures degrade to
    /// surfaced text (rows stay empty, transport reports the failure).
    fn ensure_controller(&mut self) -> bool {
        if self.controller.is_none() {
            match PlaybackController::new() {
                Ok(mut controller) => {
                    let config = tunex_core::load_from(&self.config_path).unwrap_or_default();
                    controller.set_volume(config.volume);
                    controller.set_muted(config.playback.muted);
                    controller.set_shuffle(config.playback.shuffle);
                    controller.set_repeat(config.playback.repeat_mode);
                    controller.set_replaygain_mode(config.playback.replaygain);
                    controller.set_crossfade(Duration::from_secs(u64::from(
                        config.playback.crossfade_secs,
                    )));
                    if !config.playback.output_device.is_empty()
                        && let Err(err) =
                            controller.set_output_device(&config.playback.output_device)
                    {
                        tracing::warn!(
                            name = "queue.output_restore_failed",
                            error = %err,
                            "stored output device unavailable; using system default"
                        );
                    }
                    self.controller = Some(controller);
                    if let Some(live) = &mut self.controller {
                        live.set_equalizer(config.playback.eq_enabled, config.playback.eq_bands);
                    }
                    self.restore_session(&config);
                }
                Err(err) => {
                    tracing::warn!(name = "queue.engine_failed", error = %err, "audio unavailable");
                    self.last_error = Some(err.to_string());
                    return false;
                }
            }
        }
        true
    }

    /// Rebuild display rows from the controller snapshot.
    fn sync_rows(&mut self) {
        let Some(controller) = &self.controller else {
            return;
        };
        let cursor = controller.current_index();
        let mut rows = Vec::new();
        for index in 0..controller.queue_len() {
            if let Some(item) = controller.queue_item(index) {
                rows.push((
                    QString::from(&item.title),
                    QString::from(item.artist.as_deref().unwrap_or("Unknown Artist")),
                    QString::from(item.album.as_deref().unwrap_or("Unknown Album")),
                    item.duration
                        .and_then(|duration| i32::try_from(duration.as_millis()).ok())
                        .unwrap_or(0),
                    Some(index) == cursor,
                    item.track_id
                        .and_then(|id| i32::try_from(id).ok())
                        .unwrap_or(-1),
                    queue_uri_missing(&item.uri),
                ));
            }
        }
        self.rows = rows;
        self.last_len = controller.queue_len();
        self.last_cursor = cursor;
        self.persist_queue();
    }

    /// Drain controller events and refresh rows when the queue shape (length
    /// or cursor) moved. Returns true exactly when views must rebuild.
    fn poll_queue(&mut self) -> bool {
        if !self.ensure_controller() {
            return false;
        }
        let drained = self
            .controller
            .as_mut()
            .map(PlaybackController::poll)
            .unwrap_or_default();
        for event in &drained {
            if let tunex_core::PlayerEvent::PlaybackError(message) = event {
                self.last_error = Some(message.clone());
                let notify = self.notify_config();
                if crate::notify::should_notify_error(notify.enabled, notify.playback_errors) {
                    crate::notify::post("Playback error".to_owned(), message.clone());
                }
            }
        }
        let unchanged = {
            let Some(controller) = &self.controller else {
                return false;
            };
            controller.queue_len() == self.last_len
                && controller.current_index() == self.last_cursor
        };
        if !unchanged {
            self.sync_rows();
        }
        self.maybe_persist_session(false);
        let art_landed = self.sync_current_art();
        self.sync_mpris();
        self.maybe_notify_track();
        self.maybe_load_lyrics();
        !unchanged || art_landed
    }

    /// Load sidecar or embedded lyrics when the current URI changes.
    fn maybe_load_lyrics(&mut self) {
        let uri = self
            .controller
            .as_ref()
            .and_then(PlaybackController::current_item)
            .map(|item| item.uri.clone())
            .unwrap_or_default();
        if uri == self.lyrics_uri {
            return;
        }
        self.lyrics_uri.clone_from(&uri);
        self.lyrics_plain.clear();
        self.lyrics_lines.clear();
        let Some(path) = uri_to_path(&uri) else {
            return;
        };
        let lyrics = tunex_library::load_lyrics(&path);
        self.lyrics_plain = lyrics.plain;
        self.lyrics_lines = lyrics
            .lines
            .into_iter()
            .map(|line| {
                (
                    line.time_ms
                        .and_then(|ms| i32::try_from(ms).ok())
                        .unwrap_or(-1),
                    line.text,
                )
            })
            .collect();
    }

    /// Toast on track advances and nothing else. Restores seed
    /// `last_notified_uri` silently, so a startup resume never pops a toast;
    /// stops and repeats keep quiet by [`crate::notify::should_notify`].
    fn maybe_notify_track(&mut self) {
        let current = self
            .controller
            .as_ref()
            .and_then(PlaybackController::current_item);
        let current_uri = current.as_ref().map(|item| item.uri.clone());
        let notify = self.notify_config();
        if crate::notify::should_notify(
            self.last_notified_uri.as_deref(),
            current_uri.as_deref(),
            self.window_active,
            notify.enabled,
            notify.track_change,
        ) {
            if let Some(item) = &current {
                crate::notify::post(
                    item.title.clone(),
                    crate::notify::track_body(item.artist.as_deref(), item.album.as_deref()),
                );
                self.record_play_history(item);
            }
        }
        self.last_notified_uri = current_uri;
    }

    /// Append play history on URI advance (same rule as notifications).
    fn record_play_history(&self, item: &tunex_player::QueueItem) {
        if !self.index_path.is_file() {
            return;
        }
        let Ok(db) = tunex_library::open_file(&self.index_path) else {
            return;
        };
        if let Err(err) = tunex_library::record_play(&db, item.track_id, &item.uri) {
            tracing::debug!(
                name = "queue.history_failed",
                error = %err,
                "play history not recorded"
            );
        }
    }

    /// Restore the SQLite queue when present, otherwise the last-track path.
    fn restore_session(&mut self, config: &tunex_core::TunexConfig) {
        if self.restore_saved_queue() {
            return;
        }
        self.restore_last_track(config);
    }

    /// Restore the ordered queue from the index (paused, never auto-play).
    fn restore_saved_queue(&mut self) -> bool {
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
    fn restore_last_track(&mut self, config: &tunex_core::TunexConfig) {
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
    fn item_for_uri(&self, uri: &str) -> tunex_player::QueueItem {
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

    /// Start or resume playback; refreshes rows on success.
    fn do_play(&mut self) {
        if !self.ensure_controller() {
            return;
        }
        let outcome = self.controller.as_mut().map(PlaybackController::play);
        match outcome {
            Some(Ok(())) | None => {
                self.last_error = None;
                self.sync_rows();
                self.maybe_persist_session(true);
            }
            Some(Err(err)) => self.last_error = Some(err.to_string()),
        }
    }

    /// Pause, holding position.
    fn do_pause(&mut self) {
        if !self.ensure_controller() {
            return;
        }
        if let Some(controller) = &mut self.controller {
            if let Err(err) = controller.pause() {
                self.last_error = Some(err.to_string());
            }
        }
        self.maybe_persist_session(true);
    }

    /// Toggle play/pause from the panel transport.
    fn do_play_pause(&mut self) {
        let playing = self
            .controller
            .as_ref()
            .is_some_and(|controller| controller.state() == tunex_core::PlaybackState::Playing);
        if playing {
            self.do_pause();
        } else {
            self.do_play();
        }
    }

    /// Step to the next track (stopping at a bare end); refreshes rows.
    fn do_next(&mut self) {
        if !self.ensure_controller() {
            return;
        }
        let outcome = self.controller.as_mut().map(PlaybackController::next_track);
        match outcome {
            Some(Ok(())) | None => {
                self.last_error = None;
                self.sync_rows();
                self.maybe_persist_session(true);
            }
            Some(Err(err)) => self.last_error = Some(err.to_string()),
        }
    }

    /// Step back, honoring the restart threshold; refreshes rows.
    fn do_previous(&mut self, position_ms: i32) {
        let position =
            u64::try_from(position_ms.max(0)).map_or(Duration::ZERO, Duration::from_millis);
        if !self.ensure_controller() {
            return;
        }
        let outcome = self
            .controller
            .as_mut()
            .map(|controller| controller.previous_track(position));
        match outcome {
            Some(Ok(())) | None => {
                self.last_error = None;
                self.sync_rows();
                self.maybe_persist_session(true);
            }
            Some(Err(err)) => self.last_error = Some(err.to_string()),
        }
    }

    /// Play the entry at `index` now; refreshes rows. Out-of-range indices
    /// are ignored; unloadable entries surface text.
    fn do_play_at(&mut self, index: i32) {
        if !self.ensure_controller() {
            return;
        }
        let Ok(at) = usize::try_from(index) else {
            return;
        };
        let outcome = self
            .controller
            .as_mut()
            .map(|controller| controller.play_at(at));
        match outcome {
            Some(Ok(())) => {
                self.last_error = None;
                self.sync_rows();
                self.maybe_persist_session(true);
            }
            Some(Err(err)) => self.last_error = Some(err.to_string()),
            None => {}
        }
    }

    /// Remove the entry at `index` and refresh rows (ignored out of range).
    fn do_remove(&mut self, index: i32) {
        if !self.ensure_controller() {
            return;
        }
        if let Ok(at) = usize::try_from(index) {
            if let Some(controller) = &mut self.controller {
                controller.remove_at(at);
            }
        }
        self.sync_rows();
    }

    /// Move an entry and refresh rows (ignored out of range).
    fn do_move(&mut self, from: i32, to: i32) {
        if !self.ensure_controller() {
            return;
        }
        if let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) {
            if let Some(controller) = &mut self.controller {
                controller.move_item(from, to);
            }
        }
        self.sync_rows();
    }

    /// Move an entry to play next (ignored out of range).
    fn do_play_next_at(&mut self, index: i32) {
        if !self.ensure_controller() {
            return;
        }
        if let Ok(at) = usize::try_from(index) {
            if let Some(controller) = &mut self.controller {
                controller.play_next_at(at);
            }
        }
        self.sync_rows();
    }

    /// Move an entry to the end (ignored out of range).
    fn do_move_to_end(&mut self, index: i32) {
        if !self.ensure_controller() {
            return;
        }
        if let Ok(at) = usize::try_from(index) {
            let last = self
                .controller
                .as_ref()
                .map_or(0, PlaybackController::queue_len);
            if last == 0 {
                return;
            }
            if let Some(controller) = &mut self.controller {
                controller.move_item(at, last.saturating_sub(1));
            }
        }
        self.sync_rows();
    }

    /// Empty the queue (the loaded track keeps playing) and refresh rows.
    fn do_clear_queue(&mut self) {
        if !self.ensure_controller() {
            return;
        }
        if let Some(controller) = &mut self.controller {
            controller.clear_queue();
        }
        self.sync_rows();
    }

    /// Toggle shuffle; returns the new state.
    fn do_toggle_shuffle(&mut self) -> bool {
        if !self.ensure_controller() {
            return false;
        }
        let Some(controller) = self.controller.as_mut() else {
            return false;
        };
        controller.set_shuffle(!controller.is_shuffle());
        let shuffle = controller.is_shuffle();
        self.write_audio_config();
        shuffle
    }

    /// Whether shuffle is on.
    fn is_shuffle(&self) -> bool {
        self.controller
            .as_ref()
            .is_some_and(PlaybackController::is_shuffle)
    }

    /// Cycle repeat off → all → one; returns the new mode (0/1/2).
    fn do_cycle_repeat(&mut self) -> i32 {
        if !self.ensure_controller() {
            return 0;
        }
        let mode = self.controller.as_mut().map_or(0, |controller| {
            let next = match controller.repeat_mode() {
                tunex_core::RepeatMode::Off => tunex_core::RepeatMode::All,
                tunex_core::RepeatMode::All => tunex_core::RepeatMode::One,
                tunex_core::RepeatMode::One => tunex_core::RepeatMode::Off,
            };
            controller.set_repeat(next);
            repeat_to_int(next)
        });
        self.write_audio_config();
        mode
    }

    /// Repeat mode as 0 (off), 1 (all), 2 (one).
    fn repeat_mode(&self) -> i32 {
        self.controller
            .as_ref()
            .map_or(0, |controller| repeat_to_int(controller.repeat_mode()))
    }

    /// Playback state as 0 (stopped), 1 (loading), 2 (playing), 3 (paused).
    fn playback_state(&self) -> i32 {
        self.controller
            .as_ref()
            .map_or(0, |controller| state_to_int(controller.state()))
    }

    /// Current pipeline position in milliseconds (0 when unknown).
    fn position_ms(&self) -> i32 {
        self.controller
            .as_ref()
            .and_then(PlaybackController::position)
            .and_then(|position| i32::try_from(position.as_millis()).ok())
            .unwrap_or(0)
    }

    /// Playing row, when the cursor is on a synced entry.
    fn current_row(&self) -> Option<&QueueRow> {
        self.rows.iter().find(|row| row.4)
    }

    /// Known duration: engine first, then the playing row, else 0.
    fn duration_ms(&self) -> i32 {
        let from_engine = self
            .controller
            .as_ref()
            .and_then(PlaybackController::duration)
            .and_then(|duration| i32::try_from(duration.as_millis()).ok())
            .unwrap_or(0);
        if from_engine > 0 {
            return from_engine;
        }
        self.current_row().map_or(0, |row| row.3)
    }

    /// Title of the playing row (empty when idle).
    fn current_title(&self) -> QString {
        self.current_row()
            .map(|row| row.0.clone())
            .unwrap_or_default()
    }

    /// Artist of the playing row (empty when idle).
    fn current_artist(&self) -> QString {
        self.current_row()
            .map(|row| row.1.clone())
            .unwrap_or_default()
    }

    /// Unsynced or joined LRC text.
    fn lyrics_plain(&self) -> QString {
        QString::from(self.lyrics_plain.as_str())
    }

    fn lyrics_line_count(&self) -> i32 {
        i32::try_from(self.lyrics_lines.len()).unwrap_or(i32::MAX)
    }

    fn lyrics_line_at(&self, row: i32) -> QString {
        usize::try_from(row)
            .ok()
            .and_then(|index| self.lyrics_lines.get(index))
            .map(|line| QString::from(line.1.as_str()))
            .unwrap_or_default()
    }

    fn lyrics_time_at(&self, row: i32) -> i32 {
        usize::try_from(row)
            .ok()
            .and_then(|index| self.lyrics_lines.get(index))
            .map_or(-1, |line| line.0)
    }

    fn lyrics_active_index(&self, position_ms: i32) -> i32 {
        let position = u64::try_from(position_ms.max(0)).unwrap_or(0);
        let mut active = -1;
        for (index, (time, _)) in self.lyrics_lines.iter().enumerate() {
            if *time >= 0 && u64::try_from(*time).unwrap_or(0) <= position {
                active = i32::try_from(index).unwrap_or(i32::MAX);
            }
        }
        active
    }

    fn lyrics_synced(&self) -> bool {
        self.lyrics_lines.iter().any(|(time, _)| *time >= 0)
    }

    fn reload_index_path(&mut self) {
        self.persist_queue();
        self.index_path = tunex_core::library_db_path();
        if !self.restore_saved_queue() {
            if let Some(controller) = self.controller.as_mut() {
                controller.clear_queue();
            }
            self.lyrics_uri.clear();
            self.lyrics_plain.clear();
            self.lyrics_lines.clear();
        }
        self.sync_rows();
    }

    /// Cached cover of the playing track, as a path into the art cache.
    ///
    /// The 512px thumbnail is the biggest the cache keeps, and Now Playing
    /// draws artwork at 320px or more.
    fn current_art_path(&self) -> Option<&std::path::Path> {
        let track_id = self.current_item()?.track_id?;
        Some(self.art.art(track_id)?.thumb512.as_path())
    }

    /// Cached cover of the playing track as a QML-loadable URL, empty while
    /// unresolved and for tracks without one.
    fn current_art_url(&self) -> QString {
        self.current_art_path()
            .map(|path| QString::from(&format!("file://{}", path.display())))
            .unwrap_or_default()
    }

    /// Currently playing queue entry, straight from the controller.
    fn current_item(&self) -> Option<tunex_player::QueueItem> {
        self.controller
            .as_ref()
            .and_then(PlaybackController::current_item)
    }

    /// Ask for the playing track's cover and take whatever has landed.
    ///
    /// Called from the poll, so a track change resolves its art within one
    /// tick without any view having to ask.
    fn sync_current_art(&mut self) -> bool {
        let Some(item) = self.current_item() else {
            return false;
        };
        let (Some(track_id), Some(path)) = (item.track_id, uri_to_path(&item.uri)) else {
            return false;
        };
        self.art.request(track_id, &path);
        !self.art.poll().is_empty()
    }

    /// Output volume as 0–100 (100 when the engine is not up yet).
    fn volume_pct(&self) -> i32 {
        self.controller.as_ref().map_or(100, |controller| {
            let scaled = (controller.volume().clamp(0.0, 1.0) * 100.0).round();
            // Clamped to 0..=100 before the integer cast.
            #[expect(clippy::cast_possible_truncation, reason = "volume percent is 0..=100")]
            let pct = scaled.clamp(0.0, 100.0) as i32;
            pct
        })
    }

    /// Set output volume (clamped 0–100) and persist it to config.
    fn set_volume_pct(&mut self, pct: i32) {
        if !self.ensure_controller() {
            return;
        }
        let clamped = u8::try_from(pct.clamp(0, 100)).unwrap_or(100);
        let volume = f32::from(clamped) / 100.0;
        if let Some(controller) = &mut self.controller {
            controller.set_volume(volume);
        }
        self.write_audio_config();
    }

    /// Whether output is muted.
    fn is_muted(&self) -> bool {
        self.controller
            .as_ref()
            .is_some_and(PlaybackController::muted)
    }

    /// Mute or unmute and persist.
    fn set_muted(&mut self, muted: bool) {
        if !self.ensure_controller() {
            return;
        }
        if let Some(controller) = &mut self.controller {
            controller.set_muted(muted);
        }
        self.write_audio_config();
    }

    /// Seek to an absolute position. Negative values clamp to zero; failures
    /// surface as `errorText` (nothing loaded, out of range, refused).
    fn seek_ms(&mut self, position_ms: i32) {
        if !self.ensure_controller() {
            return;
        }
        let millis = u64::try_from(position_ms.max(0)).unwrap_or(0);
        let position = Duration::from_millis(millis);
        if let Some(controller) = &mut self.controller {
            match controller.seek(position) {
                Ok(()) => self.last_error = None,
                Err(err) => self.last_error = Some(err.to_string()),
            }
        }
        // Scrub fires this every pixel; coalesce with the 2 s session window.
        self.maybe_persist_session(false);
    }

    /// Snapshot engine truth for the MPRIS thread. Honest when idle
    /// (stopped, no track, capabilities denied — never fabricated).
    fn mpris_snapshot(&self) -> MprisSnapshot {
        let Some(controller) = &self.controller else {
            return MprisSnapshot::default();
        };
        let cursor = controller.current_index();
        let len = controller.queue_len();
        let position_us = controller.position().map_or(0, duration_to_us);
        let length_us = controller.duration().map(duration_to_us);
        // Clients read the cover from the same cache the UI draws.
        let art_url = self
            .current_art_path()
            .map(|path| format!("file://{}", path.display()));
        let track = controller.current_item().map(|item| {
            let suffix = item.track_id.map_or_else(
                || format!("q{}", cursor.unwrap_or(0)),
                |id| format!("db{id}"),
            );
            MprisTrack {
                id_path: format!("{TRACK_PATH_PREFIX}{suffix}"),
                uri: item.uri,
                title: item.title,
                artist: item.artist,
                album: item.album,
                length_us,
                art_url,
            }
        });
        let state = controller.state();
        MprisSnapshot {
            status: state,
            position_us,
            volume: controller.volume(),
            shuffle: controller.is_shuffle(),
            repeat: controller.repeat_mode(),
            track,
            capabilities: crate::mpris::MprisCapabilities {
                next: len > 0
                    && (controller.repeat_mode() == tunex_core::RepeatMode::All
                        || cursor.is_some_and(|at| at + 1 < len)),
                previous: cursor.is_some(),
                play: true,
                pause: matches!(
                    state,
                    tunex_core::PlaybackState::Playing | tunex_core::PlaybackState::Paused
                ),
                seek: length_us.is_some_and(|length| length > 0),
            },
        }
    }

    /// Execute one inbound MPRIS command. Every arm reuses the panel paths
    /// (`do_*`), so bus clients and the UI can never diverge.
    fn apply_mpris_command(&mut self, command: &MprisCommand) {
        if self.apply_transport_command(command) {
            return;
        }
        match command {
            MprisCommand::Seek { offset_us } => {
                let base_us = self
                    .controller
                    .as_ref()
                    .and_then(PlaybackController::position)
                    .map_or(0, duration_to_us);
                self.seek_ms(us_to_ms_saturating(base_us.saturating_add(*offset_us)));
            }
            MprisCommand::SetPosition { position_us, .. } => {
                self.seek_ms(us_to_ms_saturating(*position_us));
            }
            MprisCommand::SetVolume(volume) => {
                #[expect(clippy::cast_possible_truncation, reason = "volume percent is 0..=100")]
                let pct = (volume.clamp(0.0, 1.0) * 100.0).round() as i32;
                self.set_volume_pct(pct);
            }
            MprisCommand::SetShuffle(shuffle) => {
                if !self.ensure_controller() {
                    return;
                }
                if let Some(controller) = &mut self.controller {
                    controller.set_shuffle(*shuffle);
                }
                self.write_audio_config();
            }
            MprisCommand::SetLoop(loop_status) => {
                if !self.ensure_controller() {
                    return;
                }
                if let Some(controller) = &mut self.controller {
                    controller.set_repeat(unmap_loop(*loop_status));
                }
                self.write_audio_config();
            }
            // Transport variants never reach here (handled above).
            MprisCommand::Play
            | MprisCommand::Pause
            | MprisCommand::PlayPause
            | MprisCommand::Stop
            | MprisCommand::Next
            | MprisCommand::Previous => {}
        }
    }

    /// Transport half of [`Self::apply_mpris_command`]; true when handled.
    fn apply_transport_command(&mut self, command: &MprisCommand) -> bool {
        match command {
            MprisCommand::Play => self.do_play(),
            MprisCommand::Pause => self.do_pause(),
            MprisCommand::PlayPause => self.do_play_pause(),
            MprisCommand::Stop => {
                if !self.ensure_controller() {
                    return true;
                }
                if let Some(controller) = &mut self.controller {
                    if let Err(err) = controller.stop() {
                        self.last_error = Some(err.to_string());
                    }
                }
                self.maybe_persist_session(true);
            }
            MprisCommand::Next => self.do_next(),
            MprisCommand::Previous => {
                let position = self
                    .controller
                    .as_ref()
                    .and_then(PlaybackController::position)
                    .unwrap_or(Duration::ZERO);
                let millis = i32::try_from(position.as_millis()).unwrap_or(i32::MAX);
                self.do_previous(millis);
            }
            _ => return false,
        }
        true
    }

    /// Drain inbound MPRIS commands onto the controller, then publish engine
    /// truth for the D-Bus thread. The first call takes the process inbox;
    /// without `spawn` (tests, early startup) only the snapshot builds.
    fn sync_mpris(&mut self) {
        if self.mpris_rx.is_none() {
            self.mpris_rx = take_inbox();
        }
        let commands: Vec<MprisCommand> = self
            .mpris_rx
            .as_mut()
            .map(|rx| {
                let mut commands = Vec::new();
                while let Ok(command) = rx.try_recv() {
                    commands.push(command);
                }
                commands
            })
            .unwrap_or_default();
        for command in &commands {
            self.apply_mpris_command(command);
        }
        publish_mpris(self.mpris_snapshot());
    }

    /// Whether glass should fall back to opaque surfaces.
    fn reduce_transparency(&self) -> bool {
        tunex_core::load_from(&self.config_path)
            .unwrap_or_default()
            .appearance
            .reduce_transparency
    }

    /// Whether overlay motion should be instant.
    fn reduce_motion(&self) -> bool {
        tunex_core::load_from(&self.config_path)
            .unwrap_or_default()
            .appearance
            .reduce_motion
    }

    /// Persist the opaque-surface preference.
    fn set_reduce_transparency(&self, enabled: bool) {
        if let Err(err) = tunex_core::update(&self.config_path, |config| {
            config.appearance.reduce_transparency = enabled;
        }) {
            tracing::warn!(
                name = "queue.appearance_persist_failed",
                error = %err,
                "reduce-transparency not saved"
            );
        }
    }

    /// Persist the instant-motion preference.
    fn set_reduce_motion(&self, enabled: bool) {
        if let Err(err) = tunex_core::update(&self.config_path, |config| {
            config.appearance.reduce_motion = enabled;
        }) {
            tracing::warn!(
                name = "queue.appearance_persist_failed",
                error = %err,
                "reduce-motion not saved"
            );
        }
    }

    fn notify_config(&self) -> tunex_core::NotifyConfig {
        tunex_core::load_from(&self.config_path)
            .unwrap_or_default()
            .notify
    }

    fn set_window_active(&mut self, active: bool) {
        self.window_active = active;
    }

    fn notifications_enabled(&self) -> bool {
        self.notify_config().enabled
    }

    fn set_notifications_enabled(&self, enabled: bool) {
        if let Err(err) = tunex_core::update(&self.config_path, |config| {
            config.notify.enabled = enabled;
        }) {
            tracing::warn!(
                name = "queue.notify_persist_failed",
                error = %err,
                "notifications master not saved"
            );
        }
    }

    fn notify_track_change(&self) -> bool {
        self.notify_config().track_change
    }

    fn set_notify_track_change(&self, enabled: bool) {
        if let Err(err) = tunex_core::update(&self.config_path, |config| {
            config.notify.track_change = enabled;
        }) {
            tracing::warn!(
                name = "queue.notify_persist_failed",
                error = %err,
                "track-change toast pref not saved"
            );
        }
    }

    fn notify_playback_errors(&self) -> bool {
        self.notify_config().playback_errors
    }

    fn set_notify_playback_errors(&self, enabled: bool) {
        if let Err(err) = tunex_core::update(&self.config_path, |config| {
            config.notify.playback_errors = enabled;
        }) {
            tracing::warn!(
                name = "queue.notify_persist_failed",
                error = %err,
                "error toast pref not saved"
            );
        }
    }

    fn replaygain_mode(&self) -> i32 {
        match tunex_core::load_from(&self.config_path)
            .unwrap_or_default()
            .playback
            .replaygain
        {
            ReplayGainMode::Off => 0,
            ReplayGainMode::Track => 1,
            ReplayGainMode::Album => 2,
        }
    }

    fn set_replaygain_mode(&mut self, mode: i32) {
        let mode = match mode {
            1 => ReplayGainMode::Track,
            2 => ReplayGainMode::Album,
            _ => ReplayGainMode::Off,
        };
        if self.ensure_controller()
            && let Some(controller) = &mut self.controller
        {
            controller.set_replaygain_mode(mode);
        }
        if let Err(err) = tunex_core::update(&self.config_path, |config| {
            config.playback.replaygain = mode;
        }) {
            tracing::warn!(
                name = "queue.replaygain_persist_failed",
                error = %err,
                "replaygain not saved"
            );
        }
    }

    fn do_cycle_replaygain(&mut self) -> i32 {
        let next = (self.replaygain_mode() + 1) % 3;
        self.set_replaygain_mode(next);
        next
    }

    fn crossfade_secs(&self) -> i32 {
        i32::from(
            tunex_core::load_from(&self.config_path)
                .unwrap_or_default()
                .playback
                .crossfade_secs,
        )
    }

    fn set_crossfade_secs(&mut self, secs: i32) {
        let secs = u8::try_from(secs.clamp(0, 12)).unwrap_or(0);
        if self.ensure_controller()
            && let Some(controller) = &mut self.controller
        {
            controller.set_crossfade(Duration::from_secs(u64::from(secs)));
        }
        if let Err(err) = tunex_core::update(&self.config_path, |config| {
            config.playback.crossfade_secs = secs;
        }) {
            tracing::warn!(
                name = "queue.crossfade_persist_failed",
                error = %err,
                "crossfade not saved"
            );
        }
    }

    fn playback_config(&self) -> tunex_core::PlaybackConfig {
        tunex_core::load_from(&self.config_path)
            .unwrap_or_default()
            .playback
    }

    fn apply_live_eq(&mut self, enabled: bool, bands: [f32; 10]) {
        if self.ensure_controller()
            && let Some(controller) = &mut self.controller
        {
            controller.set_equalizer(enabled, bands);
        }
    }

    fn persist_eq(&self, enabled: bool, preset: &str, bands: [f32; 10]) {
        if let Err(err) = tunex_core::update(&self.config_path, |config| {
            config.playback.eq_enabled = enabled;
            preset.clone_into(&mut config.playback.eq_preset);
            config.playback.eq_bands = bands;
        }) {
            tracing::warn!(
                name = "queue.eq_persist_failed",
                error = %err,
                "equalizer not saved"
            );
        }
    }

    fn eq_enabled(&self) -> bool {
        self.playback_config().eq_enabled
    }

    fn set_eq_enabled(&mut self, enabled: bool) {
        let playback = self.playback_config();
        self.apply_live_eq(enabled, playback.eq_bands);
        self.persist_eq(enabled, &playback.eq_preset, playback.eq_bands);
    }

    fn eq_preset(&self) -> String {
        let preset = self.playback_config().eq_preset;
        if preset.is_empty() {
            "flat".to_owned()
        } else {
            preset
        }
    }

    fn set_eq_preset(&mut self, name: &str) {
        let Some(bands) = preset_bands(name) else {
            return;
        };
        let enabled = self.playback_config().eq_enabled;
        self.apply_live_eq(enabled, bands);
        self.persist_eq(enabled, name, bands);
    }

    fn eq_band(&self, index: i32) -> f32 {
        usize::try_from(index)
            .ok()
            .and_then(|slot| self.playback_config().eq_bands.get(slot).copied())
            .unwrap_or(0.0)
    }

    fn set_eq_band(&mut self, index: i32, gain: f32) {
        let Ok(slot) = usize::try_from(index) else {
            return;
        };
        if slot >= 10 {
            return;
        }
        let mut playback = self.playback_config();
        playback.eq_bands[slot] = tunex_core::clamp_gain(gain);
        let preset = matching_preset(playback.eq_bands);
        self.apply_live_eq(playback.eq_enabled, playback.eq_bands);
        self.persist_eq(playback.eq_enabled, preset, playback.eq_bands);
    }

    #[expect(clippy::unused_self, reason = "cxx-qt invokable on the QObject")]
    fn eq_band_label(&self, index: i32) -> String {
        usize::try_from(index)
            .ok()
            .and_then(|slot| tunex_core::EQ_BAND_LABELS.get(slot).copied())
            .unwrap_or("")
            .to_owned()
    }

    fn reset_eq(&mut self) {
        self.set_eq_preset("flat");
    }

    fn eq_missing(&self) -> bool {
        self.controller
            .as_ref()
            .is_some_and(PlaybackController::equalizer_missing)
    }

    fn spectrum_csv(&self) -> String {
        self.controller
            .as_ref()
            .map_or_else(String::new, PlaybackController::spectrum_csv)
    }

    fn waveform_csv(&self) -> String {
        self.controller
            .as_ref()
            .map_or_else(String::new, PlaybackController::waveform_csv)
    }

    fn visualizer_mode(&self) -> i32 {
        match tunex_core::load_from(&self.config_path)
            .unwrap_or_default()
            .appearance
            .visualizer
        {
            VisualizerMode::Artwork => 0,
            VisualizerMode::Spectrum => 1,
            VisualizerMode::Waveform => 2,
            VisualizerMode::Visualizer => 3,
        }
    }

    fn set_visualizer_mode(&self, mode: i32) {
        let visualizer = match mode {
            1 => VisualizerMode::Spectrum,
            2 => VisualizerMode::Waveform,
            3 => VisualizerMode::Visualizer,
            _ => VisualizerMode::Artwork,
        };
        if let Err(err) = tunex_core::update(&self.config_path, |config| {
            config.appearance.visualizer = visualizer;
        }) {
            tracing::warn!(
                name = "queue.visualizer_persist_failed",
                error = %err,
                "visualizer mode not saved"
            );
        }
    }

    fn visualizer_fps(&self) -> i32 {
        i32::from(
            tunex_core::load_from(&self.config_path)
                .unwrap_or_default()
                .appearance
                .visualizer_fps,
        )
    }

    fn set_visualizer_fps(&self, fps: i32) {
        let fps = u8::try_from(fps.clamp(5, 30)).unwrap_or(20);
        if let Err(err) = tunex_core::update(&self.config_path, |config| {
            config.appearance.visualizer_fps = fps;
        }) {
            tracing::warn!(
                name = "queue.visualizer_persist_failed",
                error = %err,
                "visualizer fps not saved"
            );
        }
    }

    fn output_device(&self) -> String {
        tunex_core::load_from(&self.config_path)
            .unwrap_or_default()
            .playback
            .output_device
    }

    fn set_output_device_id(&mut self, id: &str) {
        if self.ensure_controller()
            && let Some(controller) = &mut self.controller
            && let Err(err) = controller.set_output_device(id)
        {
            self.last_error = Some(err.to_string());
            return;
        }
        if let Err(err) = tunex_core::update(&self.config_path, |config| {
            id.clone_into(&mut config.playback.output_device);
        }) {
            tracing::warn!(
                name = "queue.output_persist_failed",
                error = %err,
                "output device not saved"
            );
        }
    }

    fn refresh_outputs(&self) {
        let cache = Arc::clone(&self.outputs);
        std::thread::Builder::new()
            .name("tunex-outputs".to_owned())
            .spawn(move || {
                let list = list_audio_outputs()
                    .into_iter()
                    .map(|device| (device.id, device.label))
                    .collect();
                if let Ok(mut guard) = cache.lock() {
                    *guard = list;
                }
            })
            .ok();
    }

    fn output_count(&self) -> i32 {
        self.outputs
            .lock()
            .map_or(1, |guard| i32::try_from(guard.len()).unwrap_or(i32::MAX))
    }

    fn output_id_at(&self, index: i32) -> String {
        let Ok(index) = usize::try_from(index) else {
            return String::new();
        };
        self.outputs
            .lock()
            .ok()
            .and_then(|guard| guard.get(index).map(|(id, _)| id.clone()))
            .unwrap_or_default()
    }

    fn output_label_at(&self, index: i32) -> String {
        let Ok(index) = usize::try_from(index) else {
            return String::new();
        };
        self.outputs
            .lock()
            .ok()
            .and_then(|guard| guard.get(index).map(|(_, label)| label.clone()))
            .unwrap_or_default()
    }

    fn do_cycle_output(&mut self) -> i32 {
        let count = self.output_count();
        if count <= 0 {
            return 0;
        }
        let current = self.output_device();
        let mut at = 0_i32;
        for index in 0..count {
            if self.output_id_at(index) == current {
                at = index;
                break;
            }
        }
        let next = (at + 1) % count;
        let id = self.output_id_at(next);
        self.set_output_device_id(&id);
        next
    }

    /// Write volume, mute, shuffle/repeat, and last-track into settings.
    fn write_audio_config(&self) {
        let Some(controller) = &self.controller else {
            return;
        };
        let mut config = tunex_core::load_from(&self.config_path).unwrap_or_default();
        // Pipeline volume is 0..=1; f32 is the config field width.
        #[expect(clippy::cast_possible_truncation, reason = "volume is 0..=1")]
        let volume = controller.volume().clamp(0.0, 1.0) as f32;
        config.volume = volume;
        config.playback.muted = controller.muted();
        config.playback.shuffle = controller.is_shuffle();
        config.playback.repeat_mode = controller.repeat_mode();
        if let Some(item) = controller.current_item() {
            config.playback.last_uri = Some(item.uri);
            config.playback.last_position_ms =
                u64::try_from(self.position_ms().max(0)).unwrap_or(0);
        }
        if let Err(err) = tunex_core::save_to(&self.config_path, &config) {
            tracing::warn!(
                name = "queue.session_persist_failed",
                error = %err,
                "playback session not saved"
            );
        }
    }

    /// Persist last-track + position. `force` writes immediately (pause/seek);
    /// otherwise position ticks are coalesced to ~2 s.
    fn maybe_persist_session(&mut self, force: bool) {
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
    fn persist_queue(&self) {
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

    /// Cursor position (-1 when idle).
    fn current_index(&self) -> i32 {
        self.controller
            .as_ref()
            .and_then(PlaybackController::current_index)
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1)
    }

    /// Library track id at a queue row (`-1` when missing/dangling).
    fn track_id_at(&self, row: i32) -> i32 {
        usize::try_from(row)
            .ok()
            .and_then(|index| self.rows.get(index))
            .map_or(-1, |row| row.5)
    }

    /// Last failure, if any (cleared by the next success).
    fn error_message(&self) -> Option<String> {
        self.last_error.clone()
    }

    /// Open the library index for enqueue-by-id entry points.
    fn open_index(&mut self) -> Option<rusqlite::Connection> {
        if !self.index_path.is_file() {
            self.last_error = Some("Your library is empty — add a music folder first.".to_owned());
            return None;
        }
        match tunex_library::open_file(&self.index_path) {
            Ok(db) => Some(db),
            Err(err) => {
                tracing::warn!(name = "queue.index_failed", error = %err, "index unreadable");
                self.last_error = Some("The music library cannot be read.".to_owned());
                None
            }
        }
    }

    /// Resolve one library track to a playable item (shared by the
    /// track entry points): missing/unplayable rows surface text.
    fn resolve_track(&mut self, db: &rusqlite::Connection, track_id: i64) -> Option<QueueItem> {
        let row = match tunex_library::track_by_id(db, track_id) {
            Ok(row) => row,
            Err(err) => {
                tracing::warn!(name = "queue.lookup_failed", error = %err, "track lookup failed");
                self.last_error = Some("That track is no longer in the library.".to_owned());
                return None;
            }
        };
        let Some(row) = row else {
            self.last_error = Some("That track is no longer in the library.".to_owned());
            return None;
        };
        if row.missing {
            self.last_error = Some("That file is missing from disk.".to_owned());
            return None;
        }
        let Some(item) = queue_item_from_row(&row) else {
            self.last_error = Some("That file cannot be played.".to_owned());
            return None;
        };
        Some(item)
    }

    /// Enqueue one library track by row id. Returns 1 on success, 0 with
    /// surfaced text when unavailable.
    fn do_enqueue_track(&mut self, track_id: i64) -> i32 {
        if !self.ensure_controller() {
            return 0;
        }
        let Some(db) = self.open_index() else {
            return 0;
        };
        let Some(item) = self.resolve_track(&db, track_id) else {
            return 0;
        };
        if let Some(controller) = &mut self.controller {
            controller.enqueue(item);
        }
        self.sync_rows();
        self.last_error = None;
        1
    }

    /// Insert one library track to play next. Same contract as
    /// [`do_enqueue_track`](Self::do_enqueue_track).
    fn do_play_track_next(&mut self, track_id: i64) -> i32 {
        if !self.ensure_controller() {
            return 0;
        }
        let Some(db) = self.open_index() else {
            return 0;
        };
        let Some(item) = self.resolve_track(&db, track_id) else {
            return 0;
        };
        if let Some(controller) = &mut self.controller {
            controller.enqueue_next(item);
        }
        self.sync_rows();
        self.last_error = None;
        1
    }

    /// Play one library track immediately. Failures surface as text and leave
    /// the queue untouched.
    fn do_play_track_now(&mut self, track_id: i64) {
        if !self.ensure_controller() {
            return;
        }
        let Some(db) = self.open_index() else {
            return;
        };
        let Some(item) = self.resolve_track(&db, track_id) else {
            return;
        };
        if let Some(controller) = &mut self.controller {
            if let Err(err) = controller.play_now(item) {
                self.last_error = Some(err.to_string());
                return;
            }
        }
        self.sync_rows();
        self.last_error = None;
        self.maybe_persist_session(true);
    }

    /// Enqueue one album in disc/track order. Returns the number enqueued.
    fn do_enqueue_album(&mut self, album_id: i64) -> i32 {
        if !self.ensure_controller() {
            return 0;
        }
        let Some(db) = self.open_index() else {
            return 0;
        };
        let count = self
            .controller
            .as_mut()
            .map(|controller| crate::playback::enqueue_album(controller, &db, album_id));
        if let Some(Ok(count)) = count {
            self.sync_rows();
            self.last_error = None;
            i32::try_from(count).unwrap_or(i32::MAX)
        } else {
            self.last_error = Some("That album is no longer in the library.".to_owned());
            0
        }
    }

    /// Enqueue one folder's direct child tracks. Returns the number enqueued.
    fn do_enqueue_folder(&mut self, folder: &str, sort_key: &str, descending: bool) -> i32 {
        if !self.ensure_controller() {
            return 0;
        }
        let Some(db) = self.open_index() else {
            return 0;
        };
        let sort = tunex_library::TrackSort::from_key(sort_key);
        let dir = tunex_library::SortDir::from_descending(descending);
        let rows = match tunex_library::list_tracks_in_folder(&db, folder, u32::MAX, sort, dir) {
            Ok(rows) => rows,
            Err(err) => {
                tracing::warn!(name = "queue.folder_failed", error = %err, "folder lookup failed");
                self.last_error = Some("That folder is no longer in the library.".to_owned());
                return 0;
            }
        };
        let mut added = 0;
        if let Some(controller) = &mut self.controller {
            for row in &rows {
                if row.missing {
                    continue;
                }
                if let Some(item) = queue_item_from_row(row) {
                    controller.enqueue(item);
                    added += 1;
                }
            }
        }
        self.sync_rows();
        self.last_error = None;
        added
    }

    /// Enqueue one artist in album order. Returns the number enqueued.
    fn do_enqueue_artist(&mut self, artist: &str) -> i32 {
        if !self.ensure_controller() {
            return 0;
        }
        let Some(db) = self.open_index() else {
            return 0;
        };
        let count = self
            .controller
            .as_mut()
            .map(|controller| crate::playback::enqueue_artist(controller, &db, artist));
        if let Some(Ok(count)) = count {
            self.sync_rows();
            self.last_error = None;
            i32::try_from(count).unwrap_or(i32::MAX)
        } else {
            self.last_error = Some("That artist is no longer in the library.".to_owned());
            0
        }
    }

    /// Enqueue one playlist in entry order, skipping dangling and missing
    /// entries. Returns the number enqueued.
    fn do_enqueue_playlist(&mut self, playlist_id: i64) -> i32 {
        if !self.ensure_controller() {
            return 0;
        }
        let Some(mut db) = self.open_index() else {
            return 0;
        };
        if let Err(err) = tunex_library::evaluate_smart_playlist(&mut db, playlist_id) {
            tracing::debug!(
                name = "queue.smart_eval_failed",
                error = %err,
                "smart playlist not rebuilt"
            );
        }
        let entries = match tunex_library::list_entries(&db, playlist_id) {
            Ok(entries) => entries,
            Err(err) => {
                tracing::warn!(name = "queue.playlist_failed", error = %err, "entries lookup failed");
                self.last_error = Some("That playlist is no longer in the library.".to_owned());
                return 0;
            }
        };
        let mut added = 0;
        if let Some(controller) = &mut self.controller {
            for entry in &entries {
                let playable = entry.track.as_ref().filter(|row| !row.missing);
                if let Some(item) = playable.and_then(queue_item_from_row) {
                    controller.enqueue(item);
                    added += 1;
                }
            }
        }
        self.sync_rows();
        self.last_error = None;
        added
    }

    /// Enqueue one library browse list (Play all). Returns the number added.
    fn do_enqueue_library_list(
        &mut self,
        kind: &str,
        key: &str,
        sort_key: &str,
        descending: bool,
    ) -> i32 {
        if !self.ensure_controller() {
            return 0;
        }
        let Some(db) = self.open_index() else {
            return 0;
        };
        let count = self.controller.as_mut().map(|controller| {
            crate::playback::enqueue_library_list(
                controller,
                &db,
                kind,
                key,
                sort_key,
                descending,
                crate::bridge::library_track_model::SONGS_CAP,
            )
        });
        match count {
            Some(Ok(count)) if count > 0 => {
                self.sync_rows();
                self.last_error = None;
                i32::try_from(count).unwrap_or(i32::MAX)
            }
            Some(Ok(_)) => {
                self.last_error = Some("Nothing playable in that list.".to_owned());
                0
            }
            Some(Err(err)) => {
                tracing::warn!(name = "queue.play_all_failed", error = %err, "library list lookup failed");
                self.last_error = Some("That list is no longer in the library.".to_owned());
                0
            }
            None => 0,
        }
    }

    /// Play ids now: first starts, the rest insert immediately after it.
    fn do_play_track_ids(&mut self, ids: &[i64]) -> i32 {
        if ids.is_empty() {
            return 0;
        }
        if !self.ensure_controller() {
            return 0;
        }
        let Some(db) = self.open_index() else {
            return 0;
        };
        let mut items = Vec::new();
        for track_id in ids {
            if let Some(item) = self.resolve_track(&db, *track_id) {
                items.push(item);
            }
        }
        if items.is_empty() {
            self.last_error = Some("Nothing playable in that list.".to_owned());
            return 0;
        }
        let first = items.remove(0);
        let added = i32::try_from(items.len() + 1).unwrap_or(i32::MAX);
        if let Some(controller) = &mut self.controller {
            if let Err(err) = controller.play_now(first) {
                self.last_error = Some(err.to_string());
                return 0;
            }
            for item in items.into_iter().rev() {
                controller.enqueue_next(item);
            }
        }
        self.sync_rows();
        self.last_error = None;
        self.maybe_persist_session(true);
        added
    }

    /// Insert ids after the cursor, preserving list order.
    fn do_play_track_ids_next(&mut self, ids: &[i64]) -> i32 {
        if ids.is_empty() {
            return 0;
        }
        if !self.ensure_controller() {
            return 0;
        }
        let Some(db) = self.open_index() else {
            return 0;
        };
        let mut items = Vec::new();
        for track_id in ids {
            if let Some(item) = self.resolve_track(&db, *track_id) {
                items.push(item);
            }
        }
        if items.is_empty() {
            self.last_error = Some("Nothing playable in that list.".to_owned());
            return 0;
        }
        let added = i32::try_from(items.len()).unwrap_or(i32::MAX);
        if let Some(controller) = &mut self.controller {
            for item in items.into_iter().rev() {
                controller.enqueue_next(item);
            }
        }
        self.sync_rows();
        self.last_error = None;
        self.maybe_persist_session(true);
        added
    }

    /// Insert tracks by database id at `position` (clamped into range),
    /// preserving list order. Returns the number inserted; failures surface
    /// as text and leave the queue untouched.
    fn do_enqueue_track_ids_at(&mut self, ids: &[i64], position: usize) -> i32 {
        if ids.is_empty() {
            return 0;
        }
        if !self.ensure_controller() {
            return 0;
        }
        let Some(db) = self.open_index() else {
            return 0;
        };
        let mut items = Vec::new();
        for track_id in ids {
            if let Some(item) = self.resolve_track(&db, *track_id) {
                items.push(item);
            }
        }
        if items.is_empty() {
            self.last_error = Some("Nothing playable in that list.".to_owned());
            return 0;
        }
        let mut added = 0;
        if let Some(controller) = &mut self.controller {
            let at = position.min(controller.queue_len());
            for (offset, item) in items.into_iter().enumerate() {
                controller.insert_at(at + offset, item);
                added += 1;
            }
        }
        self.sync_rows();
        self.last_error = None;
        added
    }

    /// Move queue rows (comma positions from QML) to `to` as one block.
    fn do_move_items(&mut self, rows: &[usize], to: usize) {
        if !self.ensure_controller() {
            return;
        }
        if let Some(controller) = &mut self.controller {
            controller.move_items(rows, to);
        }
        self.sync_rows();
    }

    /// Enqueue tracks by database id, one model reset at the end.
    fn do_enqueue_track_ids(&mut self, ids: &[i64]) -> i32 {
        if ids.is_empty() {
            return 0;
        }
        if !self.ensure_controller() {
            return 0;
        }
        let Some(db) = self.open_index() else {
            return 0;
        };
        let mut items = Vec::new();
        for track_id in ids {
            if let Some(item) = self.resolve_track(&db, *track_id) {
                items.push(item);
            }
        }
        let mut added = 0;
        if let Some(controller) = &mut self.controller {
            for item in items {
                controller.enqueue(item);
                added += 1;
            }
        }
        if added > 0 {
            self.sync_rows();
            self.last_error = None;
        } else {
            self.last_error = Some("Nothing playable in that list.".to_owned());
        }
        added
    }

    /// Enqueue albums by id, concatenating each album's disc order.
    fn do_enqueue_album_ids(&mut self, album_ids: &[i64]) -> i32 {
        if album_ids.is_empty() {
            return 0;
        }
        if !self.ensure_controller() {
            return 0;
        }
        let Some(db) = self.open_index() else {
            return 0;
        };
        let count = self
            .controller
            .as_mut()
            .map(|controller| crate::playback::enqueue_album_ids(controller, &db, album_ids));
        match count {
            Some(Ok(count)) if count > 0 => {
                self.sync_rows();
                self.last_error = None;
                i32::try_from(count).unwrap_or(i32::MAX)
            }
            Some(Ok(_)) => {
                self.last_error = Some("Nothing playable in that list.".to_owned());
                0
            }
            Some(Err(err)) => {
                tracing::warn!(name = "queue.albums_failed", error = %err, "album list lookup failed");
                self.last_error = Some("Those albums are no longer in the library.".to_owned());
                0
            }
            None => 0,
        }
    }

    /// Enqueue artists by name, concatenating each artist's album order.
    fn do_enqueue_artist_names(&mut self, names: &[String]) -> i32 {
        if names.is_empty() {
            return 0;
        }
        if !self.ensure_controller() {
            return 0;
        }
        let Some(db) = self.open_index() else {
            return 0;
        };
        let count = self
            .controller
            .as_mut()
            .map(|controller| crate::playback::enqueue_artist_names(controller, &db, names));
        match count {
            Some(Ok(count)) if count > 0 => {
                self.sync_rows();
                self.last_error = None;
                i32::try_from(count).unwrap_or(i32::MAX)
            }
            Some(Ok(_)) => {
                self.last_error = Some("Nothing playable in that list.".to_owned());
                0
            }
            Some(Err(err)) => {
                tracing::warn!(name = "queue.artists_failed", error = %err, "artist list lookup failed");
                self.last_error = Some("Those artists are no longer in the library.".to_owned());
                0
            }
            None => 0,
        }
    }
}

/// Comma-separated library ids from QML Play all on a visible model.
fn parse_id_list(csv: &str) -> Vec<i64> {
    csv.split(',')
        .filter_map(|part| {
            let part = part.trim();
            if part.is_empty() {
                None
            } else {
                part.parse().ok()
            }
        })
        .collect()
}

/// Newline-separated artist names from QML Play all on a visible model.
fn parse_name_list(block: &str) -> Vec<String> {
    block
        .split('\n')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// Queue item type for [`resolve_track`](QueueModelRust::resolve_track).
/// (Alias keeps the signature readable.)
type QueueItem = tunex_player::QueueItem;

impl qobject::QueueModel {
    /// Announce the current rows with the narrowest signal that fits: one
    /// row appearing or leaving says so, anything wider resets.
    ///
    /// A single-row signal is what lets Up Next animate the row in or out
    /// (DESIGN.md motion budget) and leaves the other delegates — and the
    /// scroll position — alone. Queue mutations rebuild the rows as they go,
    /// so the rows are walked back to the shape the view last saw for the
    /// "about to change" half of the pair, which is where Qt expects it.
    fn apply_rows(mut self: Pin<&mut Self>) {
        let change = self.rust().pending_change();
        match change {
            RowChange::None => return,
            RowChange::Inserted(at) | RowChange::Removed(at) => {
                let inserting = matches!(change, RowChange::Inserted(_));
                let row = i32::try_from(at).unwrap_or(i32::MAX);
                let parent = QModelIndex::default();
                let announced = self.as_mut().rust_mut().rewind_rows();
                // SAFETY: each pair opens and closes on this single path,
                // with the rows carrying the old shape at the open and the
                // new shape before the close.
                unsafe {
                    if inserting {
                        self.as_mut().begin_insert_rows_queue(&parent, row, row);
                    } else {
                        self.as_mut().begin_remove_rows_queue(&parent, row, row);
                    }
                    self.as_mut().rust_mut().restore_rows(announced);
                    if inserting {
                        self.as_mut().end_insert_rows_queue();
                    } else {
                        self.as_mut().end_remove_rows_queue();
                    }
                }
            }
            // SAFETY: reset pair strictly paired on this single path.
            // Rows are already rebuilt; the reset only notifies.
            RowChange::Reset => unsafe {
                self.as_mut().begin_reset_model_queue();
                self.as_mut().end_reset_model_queue();
            },
        }
        self.as_mut().rust_mut().mark_published();
    }

    /// Drain controller events; announces the row change and returns true
    /// exactly when rows changed (length or cursor moved).
    #[must_use]
    pub fn poll(mut self: Pin<&mut Self>) -> bool {
        let changed = self.as_mut().rust_mut().poll_queue();
        if !changed {
            return false;
        }
        self.as_mut().apply_rows();
        true
    }

    /// Start or resume playback.
    pub fn play(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().do_play();
        self.as_mut().apply_rows();
    }

    /// Pause, holding position.
    pub fn pause(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().do_pause();
    }

    /// Toggle play/pause from the panel transport.
    pub fn play_pause(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().do_play_pause();
        self.as_mut().apply_rows();
    }

    /// Step to the next track (stopping at a bare end).
    pub fn next_track(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().do_next();
        self.as_mut().apply_rows();
    }

    /// Step back, honoring the restart threshold (`position_ms` past it
    /// restarts the current track instead).
    pub fn previous_track(mut self: Pin<&mut Self>, position_ms: i32) {
        self.as_mut().rust_mut().do_previous(position_ms);
        self.as_mut().apply_rows();
    }

    /// Play the entry at `index` now (Up Next direct play). Out-of-range
    /// indices are ignored; unloadable entries surface text.
    pub fn play_at(mut self: Pin<&mut Self>, index: i32) {
        self.as_mut().rust_mut().do_play_at(index);
        self.as_mut().apply_rows();
    }

    /// Remove the entry at `index` (ignored when out of range).
    pub fn remove_at(mut self: Pin<&mut Self>, index: i32) {
        self.as_mut().rust_mut().do_remove(index);
        self.as_mut().apply_rows();
    }

    /// Move an entry (ignored when out of range).
    pub fn move_item(mut self: Pin<&mut Self>, from: i32, to: i32) {
        self.as_mut().rust_mut().do_move(from, to);
        self.as_mut().apply_rows();
    }

    /// Move an already-queued entry so it plays next.
    pub fn play_next_at(mut self: Pin<&mut Self>, index: i32) {
        self.as_mut().rust_mut().do_play_next_at(index);
        self.as_mut().apply_rows();
    }

    /// Move an entry to the end of Up Next.
    pub fn move_to_end(mut self: Pin<&mut Self>, index: i32) {
        self.as_mut().rust_mut().do_move_to_end(index);
        self.as_mut().apply_rows();
    }

    /// Empty the queue (the loaded track keeps playing).
    pub fn clear_queue(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().do_clear_queue();
        self.as_mut().apply_rows();
    }

    /// Drop all rows without touching the controller (view-only reset; rows
    /// rebuild on the next poll or queue op).
    pub fn clear(mut self: Pin<&mut Self>) {
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_queue();
            self.as_mut().rust_mut().drop_rows();
            self.as_mut().end_reset_model_queue();
        };
        self.as_mut().rust_mut().mark_published();
    }

    /// Toggle shuffle; returns the new state.
    #[must_use]
    pub fn toggle_shuffle(mut self: Pin<&mut Self>) -> bool {
        self.as_mut().rust_mut().do_toggle_shuffle()
    }

    /// Whether shuffle is on.
    pub fn is_shuffle(&self) -> bool {
        self.rust().is_shuffle()
    }

    /// Cycle repeat off → all → one; returns the new mode (0/1/2).
    #[must_use]
    pub fn cycle_repeat(mut self: Pin<&mut Self>) -> i32 {
        self.as_mut().rust_mut().do_cycle_repeat()
    }

    /// Repeat mode as 0 (off), 1 (all), 2 (one).
    pub fn repeat_mode(&self) -> i32 {
        self.rust().repeat_mode()
    }

    /// Playback state as 0 (stopped), 1 (loading), 2 (playing), 3 (paused).
    pub fn playback_state(&self) -> i32 {
        self.rust().playback_state()
    }

    /// Current pipeline position in milliseconds (0 when unknown).
    pub fn position_ms(&self) -> i32 {
        self.rust().position_ms()
    }

    /// Known duration in milliseconds (engine, else current row, else 0).
    pub fn duration_ms(&self) -> i32 {
        self.rust().duration_ms()
    }

    /// Cached cover of the playing track as a `file://` URL, empty while
    /// unresolved and for tracks that have none.
    pub fn current_art_url(&self) -> QString {
        self.rust().current_art_url()
    }

    /// Title of the playing row (empty when idle).
    pub fn current_title(&self) -> QString {
        self.rust().current_title()
    }

    /// Artist of the playing row (empty when idle).
    pub fn current_artist(&self) -> QString {
        self.rust().current_artist()
    }

    /// Joined lyrics text (sidecar LRC or embedded unsynced).
    pub fn lyrics_plain(&self) -> QString {
        self.rust().lyrics_plain()
    }

    /// Number of synced lyric lines (0 for unsynced-only).
    pub fn lyrics_line_count(&self) -> i32 {
        self.rust().lyrics_line_count()
    }

    /// Lyric text at `row`.
    pub fn lyrics_line_at(&self, row: i32) -> QString {
        self.rust().lyrics_line_at(row)
    }

    /// Lyric start time at `row` (`-1` when unsynced).
    pub fn lyrics_time_at(&self, row: i32) -> i32 {
        self.rust().lyrics_time_at(row)
    }

    /// Synced line for `positionMs` (`-1` when none).
    pub fn lyrics_active_index(&self, position_ms: i32) -> i32 {
        self.rust().lyrics_active_index(position_ms)
    }

    /// Whether loaded lyrics carry timestamps.
    pub fn lyrics_synced(&self) -> bool {
        self.rust().lyrics_synced()
    }

    /// Re-read the active profile's index path and restore that profile's queue.
    pub fn reload_index(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().reload_index_path();
        self.as_mut().apply_rows();
    }

    /// Output volume as 0–100.
    pub fn volume_pct(&self) -> i32 {
        self.rust().volume_pct()
    }

    /// Set output volume (clamped 0–100) and persist it.
    pub fn set_volume_pct(mut self: Pin<&mut Self>, pct: i32) {
        self.as_mut().rust_mut().set_volume_pct(pct);
    }

    /// Whether output is muted.
    pub fn is_muted(&self) -> bool {
        self.rust().is_muted()
    }

    /// Mute or unmute (independent of the volume level) and persist.
    pub fn set_muted(mut self: Pin<&mut Self>, muted: bool) {
        self.as_mut().rust_mut().set_muted(muted);
    }

    /// Seek to an absolute position in milliseconds.
    pub fn seek_ms(mut self: Pin<&mut Self>, position_ms: i32) {
        self.as_mut().rust_mut().seek_ms(position_ms);
    }

    /// Whether glass should fall back to opaque surfaces.
    pub fn reduce_transparency(&self) -> bool {
        self.rust().reduce_transparency()
    }

    /// Whether overlay motion should be instant.
    pub fn reduce_motion(&self) -> bool {
        self.rust().reduce_motion()
    }

    /// Persist the opaque-surface preference.
    pub fn set_reduce_transparency(self: Pin<&mut Self>, enabled: bool) {
        self.rust().set_reduce_transparency(enabled);
    }

    /// Persist the instant-motion preference.
    pub fn set_reduce_motion(self: Pin<&mut Self>, enabled: bool) {
        self.rust().set_reduce_motion(enabled);
    }

    /// Whether the `TuneX` window is the active window.
    pub fn set_window_active(mut self: Pin<&mut Self>, active: bool) {
        self.as_mut().rust_mut().set_window_active(active);
    }

    /// Master notification switch.
    pub fn notifications_enabled(&self) -> bool {
        self.rust().notifications_enabled()
    }

    /// Persist the master notification switch.
    pub fn set_notifications_enabled(self: Pin<&mut Self>, enabled: bool) {
        self.rust().set_notifications_enabled(enabled);
    }

    /// Track-change toast preference.
    pub fn notify_track_change(&self) -> bool {
        self.rust().notify_track_change()
    }

    /// Persist track-change toasts.
    pub fn set_notify_track_change(self: Pin<&mut Self>, enabled: bool) {
        self.rust().set_notify_track_change(enabled);
    }

    /// Playback-error toast preference.
    pub fn notify_playback_errors(&self) -> bool {
        self.rust().notify_playback_errors()
    }

    /// Persist playback-error toasts.
    pub fn set_notify_playback_errors(self: Pin<&mut Self>, enabled: bool) {
        self.rust().set_notify_playback_errors(enabled);
    }

    /// `ReplayGain` mode as 0 (off), 1 (track), 2 (album).
    pub fn replaygain_mode(&self) -> i32 {
        self.rust().replaygain_mode()
    }

    /// Cycle `ReplayGain` off → track → album.
    #[must_use]
    pub fn cycle_replaygain(mut self: Pin<&mut Self>) -> i32 {
        self.as_mut().rust_mut().do_cycle_replaygain()
    }

    /// Crossfade length in seconds.
    pub fn crossfade_secs(&self) -> i32 {
        self.rust().crossfade_secs()
    }

    /// Set crossfade length in seconds (0–12).
    pub fn set_crossfade_secs(mut self: Pin<&mut Self>, secs: i32) {
        self.as_mut().rust_mut().set_crossfade_secs(secs);
    }

    /// Current output device id (empty = System).
    pub fn output_device(&self) -> QString {
        QString::from(self.rust().output_device().as_str())
    }

    /// Refresh the device list on a worker.
    pub fn refresh_outputs(self: Pin<&mut Self>) {
        self.rust().refresh_outputs();
    }

    /// How many output devices are listed.
    pub fn output_count(&self) -> i32 {
        self.rust().output_count()
    }

    /// Device id at `index`.
    pub fn output_id_at(&self, index: i32) -> QString {
        QString::from(self.rust().output_id_at(index).as_str())
    }

    /// Device label at `index`.
    pub fn output_label_at(&self, index: i32) -> QString {
        QString::from(self.rust().output_label_at(index).as_str())
    }

    /// Cycle the output device.
    #[must_use]
    pub fn cycle_output(mut self: Pin<&mut Self>) -> i32 {
        self.as_mut().rust_mut().do_cycle_output()
    }

    pub fn eq_enabled(&self) -> bool {
        self.rust().eq_enabled()
    }

    pub fn set_eq_enabled(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().rust_mut().set_eq_enabled(enabled);
    }

    pub fn eq_preset(&self) -> QString {
        QString::from(self.rust().eq_preset().as_str())
    }

    pub fn set_eq_preset(mut self: Pin<&mut Self>, name: &QString) {
        self.as_mut().rust_mut().set_eq_preset(&name.to_string());
    }

    pub fn eq_band(&self, index: i32) -> f32 {
        self.rust().eq_band(index)
    }

    pub fn set_eq_band(mut self: Pin<&mut Self>, index: i32, gain: f32) {
        self.as_mut().rust_mut().set_eq_band(index, gain);
    }

    pub fn eq_band_label(&self, index: i32) -> QString {
        QString::from(self.rust().eq_band_label(index).as_str())
    }

    pub fn reset_eq(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().reset_eq();
    }

    pub fn eq_missing(&self) -> bool {
        self.rust().eq_missing()
    }

    pub fn spectrum_csv(&self) -> QString {
        QString::from(self.rust().spectrum_csv().as_str())
    }

    pub fn waveform_csv(&self) -> QString {
        QString::from(self.rust().waveform_csv().as_str())
    }

    pub fn visualizer_mode(&self) -> i32 {
        self.rust().visualizer_mode()
    }

    pub fn set_visualizer_mode(self: Pin<&mut Self>, mode: i32) {
        self.rust().set_visualizer_mode(mode);
    }

    pub fn visualizer_fps(&self) -> i32 {
        self.rust().visualizer_fps()
    }

    pub fn set_visualizer_fps(self: Pin<&mut Self>, fps: i32) {
        self.rust().set_visualizer_fps(fps);
    }

    /// Cursor position (-1 when idle).
    pub fn current_index(&self) -> i32 {
        self.rust().current_index()
    }

    /// Library track id at a queue row (`-1` when missing).
    pub fn track_id_at(&self, row: i32) -> i32 {
        self.rust().track_id_at(row)
    }

    /// Last failure, or empty when clear.
    pub fn error_text(&self) -> QString {
        self.rust()
            .error_message()
            .map(QString::from)
            .unwrap_or_default()
    }

    /// Enqueue one library track by row id; returns 1 (0 + error text when
    /// unavailable).
    #[must_use]
    pub fn enqueue_track(mut self: Pin<&mut Self>, track_id: i32) -> i32 {
        let added = self
            .as_mut()
            .rust_mut()
            .do_enqueue_track(i64::from(track_id));
        if added > 0 {
            self.as_mut().apply_rows();
        }
        added
    }

    /// Insert one library track to play next; returns 1 (0 + error text
    /// when unavailable).
    #[must_use]
    pub fn play_track_next(mut self: Pin<&mut Self>, track_id: i32) -> i32 {
        let added = self
            .as_mut()
            .rust_mut()
            .do_play_track_next(i64::from(track_id));
        if added > 0 {
            self.as_mut().apply_rows();
        }
        added
    }

    /// Play one library track immediately (inserted after the cursor).
    /// Failures surface through `errorText` and leave the queue untouched.
    pub fn play_track_now(mut self: Pin<&mut Self>, track_id: i32) {
        self.as_mut()
            .rust_mut()
            .do_play_track_now(i64::from(track_id));
        self.as_mut().apply_rows();
    }

    /// Enqueue one album in disc/track order; returns the number enqueued.
    #[must_use]
    pub fn enqueue_album(mut self: Pin<&mut Self>, album_id: i32) -> i32 {
        let added = self
            .as_mut()
            .rust_mut()
            .do_enqueue_album(i64::from(album_id));
        if added > 0 {
            self.as_mut().apply_rows();
        }
        added
    }

    /// Enqueue one folder's direct child tracks; returns the number enqueued.
    #[must_use]
    pub fn enqueue_folder(
        mut self: Pin<&mut Self>,
        folder: &QString,
        sort_key: &QString,
        descending: bool,
    ) -> i32 {
        let added = self.as_mut().rust_mut().do_enqueue_folder(
            &folder.to_string(),
            &sort_key.to_string(),
            descending,
        );
        if added > 0 {
            self.as_mut().apply_rows();
        }
        added
    }

    /// Enqueue one artist in album order; returns the number enqueued.
    #[must_use]
    pub fn enqueue_artist(mut self: Pin<&mut Self>, artist: &QString) -> i32 {
        let added = self
            .as_mut()
            .rust_mut()
            .do_enqueue_artist(&artist.to_string());
        if added > 0 {
            self.as_mut().apply_rows();
        }
        added
    }

    /// Enqueue one playlist in entry order (dangling and missing entries
    /// skipped); returns the number enqueued.
    #[must_use]
    pub fn enqueue_playlist(mut self: Pin<&mut Self>, playlist_id: i32) -> i32 {
        let added = self
            .as_mut()
            .rust_mut()
            .do_enqueue_playlist(i64::from(playlist_id));
        if added > 0 {
            self.as_mut().apply_rows();
        }
        added
    }

    /// Enqueue the visible library list for `kind`. Returns the number added.
    #[must_use]
    pub fn enqueue_library_list(
        mut self: Pin<&mut Self>,
        kind: &QString,
        key: &QString,
        sort_key: &QString,
        descending: bool,
    ) -> i32 {
        let added = self.as_mut().rust_mut().do_enqueue_library_list(
            &kind.to_string(),
            &key.to_string(),
            &sort_key.to_string(),
            descending,
        );
        if added > 0 {
            self.as_mut().apply_rows();
        }
        added
    }

    /// Enqueue library tracks by comma-separated row ids.
    #[must_use]
    pub fn enqueue_track_ids(mut self: Pin<&mut Self>, ids: &QString) -> i32 {
        let parsed = parse_id_list(&ids.to_string());
        let added = self.as_mut().rust_mut().do_enqueue_track_ids(&parsed);
        if added > 0 {
            self.as_mut().apply_rows();
        }
        added
    }

    /// Insert library tracks by comma-separated row ids at `position`
    /// (clamped into range), preserving list order. Returns the number added.
    #[must_use]
    pub fn enqueue_track_ids_at(mut self: Pin<&mut Self>, ids: &QString, position: i32) -> i32 {
        let parsed = parse_id_list(&ids.to_string());
        let at = usize::try_from(position.max(0)).unwrap_or(usize::MAX);
        let added = self
            .as_mut()
            .rust_mut()
            .do_enqueue_track_ids_at(&parsed, at);
        if added > 0 {
            self.as_mut().apply_rows();
        }
        added
    }

    /// Move queue rows (comma-separated positions) to `to` as one block.
    pub fn move_items(mut self: Pin<&mut Self>, rows: &QString, to: i32) {
        let parsed = parse_id_list(&rows.to_string());
        let rows = parsed
            .into_iter()
            .filter_map(|row| usize::try_from(row).ok())
            .collect::<Vec<_>>();
        let to = usize::try_from(to.max(0)).unwrap_or(usize::MAX);
        self.as_mut().rust_mut().do_move_items(&rows, to);
        self.as_mut().apply_rows();
    }

    /// Play comma-separated library ids now; the rest play next in order.
    #[must_use]
    pub fn play_track_ids(mut self: Pin<&mut Self>, ids: &QString) -> i32 {
        let parsed = parse_id_list(&ids.to_string());
        let added = self.as_mut().rust_mut().do_play_track_ids(&parsed);
        if added > 0 {
            self.as_mut().apply_rows();
        }
        added
    }

    /// Insert comma-separated library ids to play next, in order.
    #[must_use]
    pub fn play_track_ids_next(mut self: Pin<&mut Self>, ids: &QString) -> i32 {
        let parsed = parse_id_list(&ids.to_string());
        let added = self.as_mut().rust_mut().do_play_track_ids_next(&parsed);
        if added > 0 {
            self.as_mut().apply_rows();
        }
        added
    }

    /// Enqueue albums by comma-separated ids.
    #[must_use]
    pub fn enqueue_album_ids(mut self: Pin<&mut Self>, ids: &QString) -> i32 {
        let parsed = parse_id_list(&ids.to_string());
        let added = self.as_mut().rust_mut().do_enqueue_album_ids(&parsed);
        if added > 0 {
            self.as_mut().apply_rows();
        }
        added
    }

    /// Enqueue artists by newline-separated names.
    #[must_use]
    pub fn enqueue_artist_names(mut self: Pin<&mut Self>, names: &QString) -> i32 {
        let parsed = parse_name_list(&names.to_string());
        let added = self.as_mut().rust_mut().do_enqueue_artist_names(&parsed);
        if added > 0 {
            self.as_mut().apply_rows();
        }
        added
    }
    pub fn row_count_queue(&self, _parent: &QModelIndex) -> i32 {
        self.rust().row_count()
    }

    /// Role data override for `QAbstractListModel`.
    pub fn data_queue(&self, index: &QModelIndex, role: i32) -> QVariant {
        let row = usize::try_from(index.row()).unwrap_or(usize::MAX);
        self.rust()
            .row_data(row, qobject::QueueRoles { repr: role })
    }

    /// Role-name table override; without it QML sees no custom roles.
    /// The Qt virtual signature requires the receiver although no instance
    /// state participates.
    pub fn role_names_queue(&self) -> QHash<QHashPair_i32_QByteArray> {
        let _ = self;
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(qobject::QueueRoles::Title.repr, QByteArray::from("title"));
        roles.insert(qobject::QueueRoles::Artist.repr, QByteArray::from("artist"));
        roles.insert(qobject::QueueRoles::Album.repr, QByteArray::from("album"));
        roles.insert(
            qobject::QueueRoles::DurationMs.repr,
            QByteArray::from("durationMs"),
        );
        roles.insert(
            qobject::QueueRoles::IsCurrent.repr,
            QByteArray::from("isCurrent"),
        );
        roles.insert(
            qobject::QueueRoles::TrackId.repr,
            QByteArray::from("trackId"),
        );
        roles.insert(
            qobject::QueueRoles::Missing.repr,
            QByteArray::from("missing"),
        );
        roles
    }
}

fn queue_uri_missing(uri: &str) -> bool {
    tunex_player::uri_to_path(uri).is_none_or(|path| !path.is_file())
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

/// Repeat mode as 0 (off), 1 (all), 2 (one) for QML.
fn repeat_to_int(mode: tunex_core::RepeatMode) -> i32 {
    match mode {
        tunex_core::RepeatMode::Off => 0,
        tunex_core::RepeatMode::All => 1,
        tunex_core::RepeatMode::One => 2,
    }
}

/// Playback state as 0 (stopped), 1 (loading), 2 (playing), 3 (paused).
fn state_to_int(state: tunex_core::PlaybackState) -> i32 {
    match state {
        tunex_core::PlaybackState::Stopped => 0,
        tunex_core::PlaybackState::Loading => 1,
        tunex_core::PlaybackState::Playing => 2,
        tunex_core::PlaybackState::Paused => 3,
    }
}

/// Saturating `Duration` → microseconds for MPRIS `Time`.
fn duration_to_us(duration: Duration) -> i64 {
    i64::try_from(duration.as_micros()).unwrap_or(i64::MAX)
}

/// Saturating microseconds → whole milliseconds for engine seeks.
fn us_to_ms_saturating(micros: i64) -> i32 {
    i32::try_from(micros.max(0) / 1000).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::qobject::QueueRoles;
    use super::{QueueModelRust, QueueRow, RowChange, parse_id_list, row_change};
    use crate::bridge::test_support::seeded_index;
    use cxx_qt_lib::{QString, QVariant};

    /// Queue row carrying just what the diff looks at: identity and the
    /// now-playing flag.
    fn row(track_id: i32, current: bool) -> QueueRow {
        (
            QString::from(format!("Track {track_id}")),
            QString::from("Artist"),
            QString::from("Album"),
            1_000,
            current,
            track_id,
            false,
        )
    }

    /// Model pointed at a scratch seeded index (never the real home dir).
    fn model_with_seeded_library(
        case: &str,
    ) -> (QueueModelRust, crate::bridge::test_support::Guard) {
        let (guard, path) = seeded_index(case);
        let config_path = guard.0.join("config.toml");
        let model = QueueModelRust {
            index_path: path,
            config_path,
            ..Default::default()
        };
        (model, guard)
    }

    /// Seeded index plus a real fixture file so restore/play paths have audio.
    fn model_with_playable_track(
        case: &str,
    ) -> (QueueModelRust, crate::bridge::test_support::Guard, i64) {
        let (model, guard) = model_with_seeded_library(case);
        let wav = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/sine.wav");
        let dest = guard.0.join("sine.wav");
        std::fs::copy(&wav, &dest).expect("copy fixture");
        let mut db = tunex_library::open_file(&model.index_path).expect("seed opens");
        tunex_library::upsert_track(
            &mut db,
            &tunex_library::NewTrack {
                path: dest.to_string_lossy().into_owned(),
                stable_key: "sine.wav".to_owned(),
                title: Some("Sine".to_owned()),
                artist: Some("Fixture".to_owned()),
                album: Some("Tones".to_owned()),
                ..Default::default()
            },
        )
        .expect("upsert sine");
        let id = tunex_library::list_tracks(&db)
            .expect("list works")
            .into_iter()
            .find(|track| track.path.ends_with("sine.wav"))
            .expect("sine indexed")
            .id;
        (model, guard, id)
    }

    #[test]
    fn row_data_returns_every_role() {
        let mut model = QueueModelRust::default();
        model.rows.push((
            QString::from("Midnight"),
            QString::from("Nova Rae"),
            QString::from("Night Tapes"),
            273_000,
            true,
            7,
            false,
        ));
        assert_eq!(model.row_count(), 1);
        assert_ne!(model.row_data(0, QueueRoles::Title), QVariant::default());
        assert_ne!(model.row_data(0, QueueRoles::Artist), QVariant::default());
        assert_ne!(model.row_data(0, QueueRoles::Album), QVariant::default());
        assert_ne!(
            model.row_data(0, QueueRoles::DurationMs),
            QVariant::default()
        );
        assert_ne!(
            model.row_data(0, QueueRoles::IsCurrent),
            QVariant::default()
        );
        assert_ne!(model.row_data(0, QueueRoles::TrackId), QVariant::default());
        assert_eq!(
            model.row_data(0, QueueRoles { repr: i32::MAX }),
            QVariant::default()
        );
        assert_eq!(model.row_data(99, QueueRoles::Title), QVariant::default());
    }

    #[test]
    fn enqueue_track_syncs_one_row() {
        let (mut model, _guard) = model_with_seeded_library("queue-track");
        let db = tunex_library::open_file(&model.index_path).expect("seed opens");
        let id = tunex_library::list_tracks(&db).expect("list works")[0].id;
        assert_eq!(model.do_enqueue_track(id), 1);
        assert_eq!(model.row_count(), 1);
        assert!(model.error_message().is_none());
        assert!(!model.poll_queue(), "no drift without playback");
    }

    #[test]
    fn enqueue_track_ids_batches_every_seeded_row() {
        let (mut model, _guard) = model_with_seeded_library("queue-ids");
        let db = tunex_library::open_file(&model.index_path).expect("seed opens");
        let ids: Vec<i64> = tunex_library::list_tracks(&db)
            .expect("list works")
            .into_iter()
            .map(|track| track.id)
            .collect();
        assert!(!ids.is_empty());
        let added = model.do_enqueue_track_ids(&ids);
        assert_eq!(added, i32::try_from(ids.len()).expect("fits"));
        assert_eq!(model.row_count(), added);
        assert!(model.error_message().is_none());
    }

    #[test]
    fn parse_id_list_skips_junk() {
        assert_eq!(parse_id_list("1, 2,x,3,"), vec![1, 2, 3]);
        assert!(parse_id_list("").is_empty());
    }

    #[test]
    fn insert_ids_at_position_keeps_order() {
        let (mut model, _guard) = model_with_seeded_library("queue-insert-at");
        let db = tunex_library::open_file(&model.index_path).expect("seed opens");
        let ids: Vec<i64> = tunex_library::list_tracks(&db)
            .expect("list works")
            .into_iter()
            .map(|track| track.id)
            .collect();
        assert!(ids.len() >= 3);
        assert_eq!(model.do_enqueue_track_ids(&ids[..2]), 2);
        assert_eq!(model.do_enqueue_track_ids_at(&ids[2..3], 1), 1);
        assert_eq!(model.row_count(), 3);
        let at = |row| i64::from(model.track_id_at(row));
        assert_eq!((at(0), at(1), at(2)), (ids[0], ids[2], ids[1]));
        assert!(model.error_message().is_none());
        // Overshoot clamps to the end instead of failing.
        assert_eq!(model.do_enqueue_track_ids_at(&ids[2..3], 99), 1);
        assert_eq!(
            model.track_id_at(3),
            i32::try_from(ids[2]).expect("seed ids fit")
        );
    }

    #[test]
    fn move_items_block_keeps_count() {
        let (mut model, _guard) = model_with_seeded_library("queue-move-items");
        let db = tunex_library::open_file(&model.index_path).expect("seed opens");
        let ids: Vec<i64> = tunex_library::list_tracks(&db)
            .expect("list works")
            .into_iter()
            .map(|track| track.id)
            .collect();
        assert!(ids.len() >= 3);
        assert_eq!(
            model.do_enqueue_track_ids(&ids),
            i32::try_from(ids.len()).expect("fits")
        );
        model.do_move_items(&[0, 2], 3);
        assert_eq!(model.row_count(), 3, "a move never copies");
        let at = |row| i64::from(model.track_id_at(row));
        assert_eq!((at(0), at(1), at(2)), (ids[1], ids[0], ids[2]));
    }

    #[test]
    fn enqueue_unknown_track_surfaces_text() {
        let (mut model, _guard) = model_with_seeded_library("queue-unknown");
        assert_eq!(model.do_enqueue_track(999_999), 0);
        assert!(model.error_message().is_some_and(|text| !text.is_empty()));
        assert_eq!(model.row_count(), 0);
    }

    #[test]
    fn enqueue_without_index_surfaces_empty_library() {
        let missing =
            std::env::temp_dir().join(format!("tunex-queue-noindex-{}", std::process::id()));
        let mut model = QueueModelRust {
            index_path: missing.join("library.db"),
            config_path: missing.join("config.toml"),
            ..Default::default()
        };
        assert_eq!(model.do_enqueue_track(1), 0);
        assert!(
            model
                .error_message()
                .is_some_and(|text| text.contains("empty"))
        );
    }

    #[test]
    fn enqueue_album_and_artist_collect_in_order() {
        let (mut model, _guard) = model_with_seeded_library("queue-groups");
        let db = tunex_library::open_file(&model.index_path).expect("seed opens");
        let tapes = tunex_library::list_albums(
            &db,
            tunex_library::AlbumSort::Title,
            tunex_library::SortDir::Asc,
        )
        .expect("albums list")
        .into_iter()
        .find(|album| album.title == "Night Tapes")
        .expect("album present");
        assert_eq!(model.do_enqueue_album(tapes.id), 2);
        assert_eq!(model.row_count(), 2);
        assert_eq!(model.do_enqueue_artist("Nova Rae"), 2);
        assert_eq!(model.row_count(), 4);
    }

    #[test]
    fn enqueue_folder_collects_direct_children() {
        let (mut model, _guard) = model_with_seeded_library("queue-folder");
        assert_eq!(model.do_enqueue_folder("/music", "title", false), 3);
        assert_eq!(model.row_count(), 3);
        assert_eq!(model.do_enqueue_folder("/music/missing", "title", false), 0);
    }

    #[test]
    fn queue_ops_reorder_and_remove_rows() {
        let (mut model, _guard) = model_with_seeded_library("queue-ops");
        let db = tunex_library::open_file(&model.index_path).expect("seed opens");
        let tapes = tunex_library::list_albums(
            &db,
            tunex_library::AlbumSort::Title,
            tunex_library::SortDir::Asc,
        )
        .expect("albums list")
        .into_iter()
        .find(|album| album.title == "Night Tapes")
        .expect("album present");
        assert_eq!(model.do_enqueue_album(tapes.id), 2);
        model.do_move(0, 1);
        assert_eq!(
            model.rows[0].0,
            QString::from("Two"),
            "move reorders display rows"
        );
        model.do_remove(0);
        assert_eq!(model.row_count(), 1);
        model.do_remove(99);
        assert_eq!(model.row_count(), 1, "out-of-range remove ignored");
        model.do_clear_queue();
        assert_eq!(model.row_count(), 0);
    }

    #[test]
    fn enqueue_playlist_plays_entries_in_order_skipping_dangling() {
        let (mut model, _guard) = model_with_seeded_library("queue-playlist");
        let db = tunex_library::open_file(&model.index_path).expect("seed opens");
        let id = tunex_library::create_playlist(&db, "Mix").expect("create works");
        let tracks = tunex_library::list_tracks(&db).expect("list works");
        let mut db = db;
        for row in &tracks {
            tunex_library::add_to_playlist(&mut db, id, row.id).expect("add works");
        }
        drop(db);
        assert_eq!(model.do_enqueue_playlist(id), 3);
        assert_eq!(model.row_count(), 3);
        // Delete a track row: its entry dangles and is skipped next time.
        let db = tunex_library::open_file(&model.index_path).expect("reopen works");
        db.execute("DELETE FROM tracks WHERE path LIKE '%a1.flac'", [])
            .expect("delete works");
        drop(db);
        model.do_clear_queue();
        assert_eq!(model.do_enqueue_playlist(id), 2);
    }

    #[test]
    fn shuffle_and_repeat_toggle_state() {
        let (mut model, _guard) = model_with_seeded_library("queue-shuffle");
        assert!(!model.is_shuffle());
        assert_eq!(model.volume_pct(), 100, "idle volume reads full");
        assert!(!model.is_muted());
        assert_eq!(model.duration_ms(), 0);
        assert!(model.do_toggle_shuffle());
        assert!(model.is_shuffle());
        assert_eq!(model.repeat_mode(), 0);
        assert_eq!(model.do_cycle_repeat(), 1);
        assert_eq!(model.do_cycle_repeat(), 2);
        assert_eq!(model.do_cycle_repeat(), 0);
        let saved = tunex_core::load_from(&model.config_path).expect("modes saved");
        assert!(saved.playback.shuffle);
        assert_eq!(saved.playback.repeat_mode, tunex_core::RepeatMode::Off);
        assert_eq!(model.playback_state(), 0, "idle engine reads stopped");
        assert_eq!(model.current_index(), -1, "idle cursor reads -1");
        assert_eq!(model.position_ms(), 0);
    }

    #[test]
    fn replaygain_crossfade_and_output_persist() {
        let (mut model, _guard) = model_with_seeded_library("queue-enrichment");
        assert_eq!(model.replaygain_mode(), 0);
        assert_eq!(model.do_cycle_replaygain(), 1);
        assert_eq!(model.do_cycle_replaygain(), 2);
        model.set_crossfade_secs(8);
        assert_eq!(model.crossfade_secs(), 8);
        model.set_crossfade_secs(99);
        assert_eq!(model.crossfade_secs(), 12);
        assert_eq!(model.output_count(), 1);
        assert!(model.output_id_at(0).is_empty());
        assert_eq!(model.output_label_at(0), "System");
        model.set_output_device_id("");
        let saved = tunex_core::load_from(&model.config_path).expect("enrichment saved");
        assert_eq!(saved.playback.replaygain, tunex_core::ReplayGainMode::Album);
        assert_eq!(saved.playback.crossfade_secs, 12);
        assert!(saved.playback.output_device.is_empty());
    }

    #[test]
    fn current_track_reads_from_playing_row() {
        let mut model = QueueModelRust::default();
        model.rows.push((
            QString::from("Midnight"),
            QString::from("Nova Rae"),
            QString::from("Night Tapes"),
            273_000,
            true,
            7,
            false,
        ));
        assert_eq!(model.current_title(), QString::from("Midnight"));
        assert_eq!(model.current_artist(), QString::from("Nova Rae"));
        assert_eq!(model.duration_ms(), 273_000);
    }

    #[test]
    fn volume_clamps_and_persists() {
        let (mut model, _guard) = model_with_seeded_library("queue-volume");
        model.set_volume_pct(150);
        assert_eq!(model.volume_pct(), 100);
        model.set_volume_pct(-4);
        assert_eq!(model.volume_pct(), 0);
        let loaded = tunex_core::load_from(&model.config_path).expect("volume saved");
        assert!((loaded.volume - 0.0).abs() < f32::EPSILON);
        model.set_muted(true);
        assert!(model.is_muted());
        let loaded = tunex_core::load_from(&model.config_path).expect("mute saved");
        assert!(loaded.playback.muted);
    }

    #[test]
    fn seek_without_track_surfaces_error() {
        let (mut model, _guard) = model_with_seeded_library("queue-seek");
        model.seek_ms(1_000);
        let message = model.error_message().expect("seek error surfaced");
        assert!(
            message.contains("nothing loaded"),
            "unexpected seek error: {message}"
        );
    }

    #[test]
    fn appearance_flags_read_from_config() {
        let (model, _guard) = model_with_seeded_library("queue-appear");
        assert!(!model.reduce_transparency());
        assert!(!model.reduce_motion());
        let mut config = tunex_core::TunexConfig::default();
        config.appearance.reduce_transparency = true;
        config.appearance.reduce_motion = true;
        tunex_core::save_to(&model.config_path, &config).expect("appearance saved");
        assert!(model.reduce_transparency());
        assert!(model.reduce_motion());
    }

    #[test]
    fn appearance_flags_persist_from_setters() {
        let (model, _guard) = model_with_seeded_library("queue-appear-set");
        model.set_reduce_motion(true);
        model.set_reduce_transparency(true);
        assert!(model.reduce_motion());
        assert!(model.reduce_transparency());
        let loaded = tunex_core::load_from(&model.config_path).expect("appearance saved");
        assert!(loaded.appearance.reduce_motion);
        assert!(loaded.appearance.reduce_transparency);
    }

    #[test]
    fn notify_prefs_default_on_and_persist() {
        let (mut model, _guard) = model_with_seeded_library("queue-notify");
        assert!(model.notifications_enabled());
        assert!(model.notify_track_change());
        assert!(model.notify_playback_errors());
        assert!(model.window_active);
        model.set_window_active(false);
        assert!(!model.window_active);
        model.set_notifications_enabled(false);
        model.set_notify_track_change(false);
        model.set_notify_playback_errors(false);
        let loaded = tunex_core::load_from(&model.config_path).expect("notify saved");
        assert!(!loaded.notify.enabled);
        assert!(!loaded.notify.track_change);
        assert!(!loaded.notify.playback_errors);
    }

    #[test]
    fn equalizer_and_visualizer_persist() {
        let (mut model, _guard) = model_with_seeded_library("queue-eq-viz");
        model.set_eq_enabled(true);
        model.set_eq_preset("rock");
        model.set_visualizer_mode(1);
        model.set_visualizer_fps(24);
        let loaded = tunex_core::load_from(&model.config_path).expect("eq saved");
        assert!(loaded.playback.eq_enabled);
        assert_eq!(loaded.playback.eq_preset, "rock");
        assert_eq!(
            loaded.appearance.visualizer,
            tunex_core::VisualizerMode::Spectrum
        );
        assert_eq!(loaded.appearance.visualizer_fps, 24);
    }

    #[test]
    fn mpris_snapshot_without_engine_is_honest() {
        let model = QueueModelRust::default();
        let snapshot = model.mpris_snapshot();
        assert_eq!(snapshot.status, tunex_core::PlaybackState::Stopped);
        assert!(snapshot.track.is_none());
        assert!(!snapshot.capabilities.play);
        assert!(!snapshot.capabilities.pause);
        assert!(!snapshot.capabilities.seek);
        assert!(!snapshot.capabilities.next);
        assert!(!snapshot.capabilities.previous);
    }

    #[test]
    fn mpris_set_volume_drives_engine_and_persists() {
        let (mut model, _guard) = model_with_seeded_library("queue-mpris-volume");
        model.apply_mpris_command(&crate::mpris::MprisCommand::SetVolume(0.5));
        let volume = model
            .controller
            .as_ref()
            .map(tunex_player::PlaybackController::volume)
            .expect("engine constructed");
        assert!((volume - 0.5).abs() < f64::EPSILON);
        let loaded = tunex_core::load_from(&model.config_path).expect("volume saved");
        assert!((loaded.volume - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn mpris_play_pause_on_empty_queue_stays_stopped() {
        let (mut model, _guard) = model_with_seeded_library("queue-mpris-idle");
        // The offscreen DoD smoke leans on this: no track, no lie.
        model.apply_mpris_command(&crate::mpris::MprisCommand::PlayPause);
        assert_eq!(
            model.mpris_snapshot().status,
            tunex_core::PlaybackState::Stopped
        );
        model.apply_mpris_command(&crate::mpris::MprisCommand::Stop);
        assert_eq!(
            model.mpris_snapshot().status,
            tunex_core::PlaybackState::Stopped
        );
    }

    #[test]
    fn last_track_persists_after_play() {
        let (mut model, _guard, id) = model_with_playable_track("queue-session-save");
        model.do_play_track_now(id);
        model.do_pause();
        let loaded = tunex_core::load_from(&model.config_path).expect("session saved");
        let uri = loaded.playback.last_uri.expect("last uri written");
        assert!(uri.contains("sine.wav"), "unexpected last uri: {uri}");
    }

    #[test]
    fn saved_queue_restores_paused_without_autoplay() {
        let (mut model, guard, id) = model_with_playable_track("queue-restore-full");
        model.do_play_track_now(id);
        model.do_pause();
        let saved = {
            let db = tunex_library::open_file(&model.index_path).expect("index opens");
            tunex_library::load_playback_queue(&db).expect("queue saved")
        };
        assert_eq!(saved.items.len(), 1);
        let config_path = model.config_path.clone();
        let index_path = model.index_path.clone();
        drop(model);

        let mut restored = QueueModelRust {
            index_path,
            config_path,
            ..Default::default()
        };
        restored.poll_queue();
        assert_eq!(restored.row_count(), 1);
        assert!(restored.error_message().is_none());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let _ = restored.poll_queue();
            let state = restored.playback_state();
            assert_ne!(state, 2, "restore must not auto-play");
            if state == 1 || state == 3 {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "restore did not reach paused/loading"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        drop(guard);
    }

    #[test]
    fn last_track_restores_paused_on_new_model() {
        let (model, guard, _id) = model_with_playable_track("queue-session-restore");
        let dest = guard.0.join("sine.wav");
        let uri = tunex_player::path_to_uri(&dest).expect("fixture uri");
        let mut config = tunex_core::TunexConfig::default();
        config.playback.last_uri = Some(uri.clone());
        config.playback.last_position_ms = 1_500;
        tunex_core::save_to(&model.config_path, &config).expect("session seeded");
        let config_path = model.config_path.clone();
        let index_path = model.index_path.clone();
        drop(model);

        let mut restored = QueueModelRust {
            index_path,
            config_path,
            ..Default::default()
        };
        restored.poll_queue();
        assert_eq!(restored.row_count(), 1, "restored one track");
        assert!(
            restored.error_message().is_none(),
            "missing file would toast"
        );
        assert!(
            restored
                .last_notified_uri
                .as_deref()
                .is_some_and(|uri| uri.contains("sine.wav")),
            "restore seeds the toast cursor silently (no startup toast)"
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let _ = restored.poll_queue();
            let state = restored.playback_state();
            assert_ne!(state, 2, "restore must not auto-play");
            let paused = state == 1 || state == 3;
            if paused && restored.position_ms() >= 1_400 {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "restored position never settled (state={state} pos={})",
                restored.position_ms()
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    #[test]
    fn row_change_announces_nothing_when_the_rows_match() {
        let rows = vec![row(1, true), row(2, false)];
        assert_eq!(row_change(&rows, &rows), RowChange::None);
    }

    #[test]
    fn row_change_announces_an_appended_row() {
        let before = vec![row(1, true)];
        let after = vec![row(1, true), row(2, false)];
        assert_eq!(row_change(&before, &after), RowChange::Inserted(1));
    }

    #[test]
    fn row_change_announces_a_row_inserted_mid_queue() {
        // "Play next" lands directly after the playing track.
        let before = vec![row(1, true), row(3, false)];
        let after = vec![row(1, true), row(2, false), row(3, false)];
        assert_eq!(row_change(&before, &after), RowChange::Inserted(1));
    }

    #[test]
    fn row_change_announces_a_removed_row() {
        let before = vec![row(1, true), row(2, false), row(3, false)];
        let after = vec![row(1, true), row(3, false)];
        assert_eq!(row_change(&before, &after), RowChange::Removed(1));
    }

    #[test]
    fn row_change_resets_when_the_cursor_moves() {
        // Two rows rewrite their now-playing flag, which is wider than one
        // row: the view rebuilds rather than animating anything.
        let before = vec![row(1, true), row(2, false)];
        let after = vec![row(1, false), row(2, true)];
        assert_eq!(row_change(&before, &after), RowChange::Reset);
    }

    #[test]
    fn row_change_resets_when_a_whole_album_arrives() {
        let before = vec![row(1, true)];
        let after = vec![row(1, true), row(2, false), row(3, false)];
        assert_eq!(row_change(&before, &after), RowChange::Reset);
    }

    #[test]
    fn restored_track_reports_the_row_change_so_up_next_rebuilds() {
        // `poll` only emits the model reset when `poll_queue` reports a
        // change, so a restore that quietly filled the rows left Up Next
        // showing its empty state while the mini-player played the track.
        let (model, guard, _id) = model_with_playable_track("queue-session-restore-reset");
        let dest = guard.0.join("sine.wav");
        let uri = tunex_player::path_to_uri(&dest).expect("fixture uri");
        let mut config = tunex_core::TunexConfig::default();
        config.playback.last_uri = Some(uri);
        config.playback.last_position_ms = 1_500;
        tunex_core::save_to(&model.config_path, &config).expect("session seeded");
        let config_path = model.config_path.clone();
        let index_path = model.index_path.clone();
        drop(model);

        let mut restored = QueueModelRust {
            index_path,
            config_path,
            ..Default::default()
        };
        assert!(
            restored.poll_queue(),
            "the poll that restores a track must report rows changed"
        );
    }

    #[test]
    fn missing_last_track_skips_restore_without_error() {
        let (mut model, _guard) = model_with_seeded_library("queue-session-missing");
        let mut config = tunex_core::TunexConfig::default();
        config.playback.last_uri = Some("file:///nonexistent-tunex-restore-w030.flac".to_owned());
        config.playback.last_position_ms = 800;
        tunex_core::save_to(&model.config_path, &config).expect("missing uri seeded");
        model.poll_queue();
        assert!(
            model.error_message().is_none(),
            "missing last track must not toast: {:?}",
            model.error_message()
        );
        assert_eq!(model.row_count(), 0);
    }
}
