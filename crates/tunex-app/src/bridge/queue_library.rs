//! Library-to-queue enqueue entry points (C13): cursor/index helpers plus
//! every enqueue-by-id/album/artist/playlist path. Row sync stays in
//! `queue_model`; these only resolve ids and hand rows to the controller.

use super::queue_model::{QueueItem, QueueModelRust};
use crate::playback::queue_item_from_row;
use tunex_player::PlaybackController;

impl QueueModelRust {
    /// Cursor position (-1 when idle).
    pub(crate) fn current_index(&self) -> i32 {
        self.controller
            .as_ref()
            .and_then(PlaybackController::current_index)
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1)
    }

    /// Library track id at a queue row (`-1` when missing/dangling).
    pub(crate) fn track_id_at(&self, row: i32) -> i32 {
        usize::try_from(row)
            .ok()
            .and_then(|index| self.rows.get(index))
            .map_or(-1, |row| row.5)
    }

    /// Last failure, if any (cleared by the next success).
    pub(crate) fn error_message(&self) -> Option<String> {
        self.last_error.clone()
    }

    /// Open the library index for enqueue-by-id entry points.
    pub(crate) fn open_index(&mut self) -> Option<rusqlite::Connection> {
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
    pub(crate) fn resolve_track(
        &mut self,
        db: &rusqlite::Connection,
        track_id: i64,
    ) -> Option<QueueItem> {
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
    pub(crate) fn do_enqueue_track(&mut self, track_id: i64) -> i32 {
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
    pub(crate) fn do_play_track_next(&mut self, track_id: i64) -> i32 {
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
    pub(crate) fn do_play_track_now(&mut self, track_id: i64) {
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
    pub(crate) fn do_enqueue_album(&mut self, album_id: i64) -> i32 {
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
    pub(crate) fn do_enqueue_folder(
        &mut self,
        folder: &str,
        sort_key: &str,
        descending: bool,
    ) -> i32 {
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
    pub(crate) fn do_enqueue_artist(&mut self, artist: &str) -> i32 {
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
    pub(crate) fn do_enqueue_playlist(&mut self, playlist_id: i64) -> i32 {
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
    pub(crate) fn do_enqueue_library_list(
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
    pub(crate) fn do_play_track_ids(&mut self, ids: &[i64]) -> i32 {
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
    pub(crate) fn do_play_track_ids_next(&mut self, ids: &[i64]) -> i32 {
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
    pub(crate) fn do_enqueue_track_ids_at(&mut self, ids: &[i64], position: usize) -> i32 {
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
    pub(crate) fn do_move_items(&mut self, rows: &[usize], to: usize) {
        if !self.ensure_controller() {
            return;
        }
        if let Some(controller) = &mut self.controller {
            controller.move_items(rows, to);
        }
        self.sync_rows();
    }

    /// Enqueue tracks by database id, one model reset at the end.
    pub(crate) fn do_enqueue_track_ids(&mut self, ids: &[i64]) -> i32 {
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
    pub(crate) fn do_enqueue_album_ids(&mut self, album_ids: &[i64]) -> i32 {
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
    pub(crate) fn do_enqueue_artist_names(&mut self, names: &[String]) -> i32 {
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
