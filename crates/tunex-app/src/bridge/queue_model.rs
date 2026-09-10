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
use std::time::Duration;

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

use crate::playback::queue_item_from_row;
use tunex_player::PlaybackController;

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
            _ => "Unknown",
        };
        write!(f, "QueueRoles::{name}")
    }
}

/// Queue row: display strings plus duration, now-playing flag, row identity.
type QueueRow = (QString, QString, QString, i32, bool, i32);

/// Up Next row store plus the playback controller.
#[derive(Debug)]
pub struct QueueModelRust {
    rows: Vec<QueueRow>,
    controller: Option<PlaybackController>,
    index_path: PathBuf,
    last_len: usize,
    last_cursor: Option<usize>,
    last_error: Option<String>,
}

impl Default for QueueModelRust {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            controller: None,
            index_path: tunex_core::library_db_path(),
            last_len: 0,
            last_cursor: None,
            last_error: None,
        }
    }
}

impl QueueModelRust {
    /// Drop all rows.
    fn drop_rows(&mut self) {
        self.rows.clear();
    }

    /// Current row count.
    fn row_count(&self) -> i32 {
        i32::try_from(self.rows.len()).unwrap_or(i32::MAX)
    }

    /// Data for one row/role; invalid variant when out of range or the role
    /// is unknown.
    fn row_data(&self, row: usize, role: qobject::QueueRoles) -> QVariant {
        if let Some((title, artist, album, duration_ms, is_current, track_id)) = self.rows.get(row)
        {
            return match role {
                qobject::QueueRoles::Title => QVariant::from(title),
                qobject::QueueRoles::Artist => QVariant::from(artist),
                qobject::QueueRoles::Album => QVariant::from(album),
                qobject::QueueRoles::DurationMs => QVariant::from(duration_ms),
                qobject::QueueRoles::IsCurrent => QVariant::from(is_current),
                qobject::QueueRoles::TrackId => QVariant::from(track_id),
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
                Ok(controller) => self.controller = Some(controller),
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
                ));
            }
        }
        self.rows = rows;
        self.last_len = controller.queue_len();
        self.last_cursor = cursor;
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
            }
        }
        let Some(controller) = &self.controller else {
            return false;
        };
        if controller.queue_len() == self.last_len && controller.current_index() == self.last_cursor
        {
            return false;
        }
        self.sync_rows();
        true
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
        controller.is_shuffle()
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
        self.controller.as_mut().map_or(0, |controller| {
            let next = match controller.repeat_mode() {
                tunex_core::RepeatMode::Off => tunex_core::RepeatMode::All,
                tunex_core::RepeatMode::All => tunex_core::RepeatMode::One,
                tunex_core::RepeatMode::One => tunex_core::RepeatMode::Off,
            };
            controller.set_repeat(next);
            repeat_to_int(next)
        })
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

    /// Cursor position (-1 when idle).
    fn current_index(&self) -> i32 {
        self.controller
            .as_ref()
            .and_then(PlaybackController::current_index)
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1)
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
        let Some(db) = self.open_index() else {
            return 0;
        };
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
}

/// Queue item type for [`resolve_track`](QueueModelRust::resolve_track).
/// (Alias keeps the signature readable.)
type QueueItem = tunex_player::QueueItem;

impl qobject::QueueModel {
    /// Drain controller events; emits model reset and returns true exactly
    /// when rows changed (length or cursor moved).
    #[must_use]
    pub fn poll(mut self: Pin<&mut Self>) -> bool {
        let changed = self.as_mut().rust_mut().poll_queue();
        if !changed {
            return false;
        }
        // SAFETY: reset pair strictly paired on this single path.
        // Rows already rebuilt by `poll_queue`; the reset only notifies.
        unsafe {
            self.as_mut().begin_reset_model_queue();
            self.as_mut().end_reset_model_queue();
        };
        true
    }

    /// Start or resume playback.
    pub fn play(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().do_play();
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_queue();
            self.as_mut().end_reset_model_queue();
        };
    }

    /// Pause, holding position.
    pub fn pause(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().do_pause();
    }

    /// Toggle play/pause from the panel transport.
    pub fn play_pause(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().do_play_pause();
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_queue();
            self.as_mut().end_reset_model_queue();
        };
    }

    /// Step to the next track (stopping at a bare end).
    pub fn next_track(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().do_next();
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_queue();
            self.as_mut().end_reset_model_queue();
        };
    }

    /// Step back, honoring the restart threshold (`position_ms` past it
    /// restarts the current track instead).
    pub fn previous_track(mut self: Pin<&mut Self>, position_ms: i32) {
        self.as_mut().rust_mut().do_previous(position_ms);
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_queue();
            self.as_mut().end_reset_model_queue();
        };
    }

    /// Play the entry at `index` now (Up Next direct play). Out-of-range
    /// indices are ignored; unloadable entries surface text.
    pub fn play_at(mut self: Pin<&mut Self>, index: i32) {
        self.as_mut().rust_mut().do_play_at(index);
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_queue();
            self.as_mut().end_reset_model_queue();
        };
    }

    /// Remove the entry at `index` (ignored when out of range).
    pub fn remove_at(mut self: Pin<&mut Self>, index: i32) {
        self.as_mut().rust_mut().do_remove(index);
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_queue();
            self.as_mut().end_reset_model_queue();
        };
    }

    /// Move an entry (ignored when out of range).
    pub fn move_item(mut self: Pin<&mut Self>, from: i32, to: i32) {
        self.as_mut().rust_mut().do_move(from, to);
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_queue();
            self.as_mut().end_reset_model_queue();
        };
    }

    /// Empty the queue (the loaded track keeps playing).
    pub fn clear_queue(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().do_clear_queue();
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_queue();
            self.as_mut().end_reset_model_queue();
        };
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

    /// Cursor position (-1 when idle).
    pub fn current_index(&self) -> i32 {
        self.rust().current_index()
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
            // SAFETY: reset pair strictly paired on this single path.
            unsafe {
                self.as_mut().begin_reset_model_queue();
                self.as_mut().end_reset_model_queue();
            };
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
            // SAFETY: reset pair strictly paired on this single path.
            unsafe {
                self.as_mut().begin_reset_model_queue();
                self.as_mut().end_reset_model_queue();
            };
        }
        added
    }

    /// Play one library track immediately (inserted after the cursor).
    /// Failures surface through `errorText` and leave the queue untouched.
    pub fn play_track_now(mut self: Pin<&mut Self>, track_id: i32) {
        self.as_mut()
            .rust_mut()
            .do_play_track_now(i64::from(track_id));
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_queue();
            self.as_mut().end_reset_model_queue();
        };
    }

    /// Enqueue one album in disc/track order; returns the number enqueued.
    #[must_use]
    pub fn enqueue_album(mut self: Pin<&mut Self>, album_id: i32) -> i32 {
        let added = self
            .as_mut()
            .rust_mut()
            .do_enqueue_album(i64::from(album_id));
        if added > 0 {
            // SAFETY: reset pair strictly paired on this single path.
            unsafe {
                self.as_mut().begin_reset_model_queue();
                self.as_mut().end_reset_model_queue();
            };
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
            // SAFETY: reset pair strictly paired on this single path.
            unsafe {
                self.as_mut().begin_reset_model_queue();
                self.as_mut().end_reset_model_queue();
            };
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
            // SAFETY: reset pair strictly paired on this single path.
            unsafe {
                self.as_mut().begin_reset_model_queue();
                self.as_mut().end_reset_model_queue();
            };
        }
        added
    }

    /// Row count override for `QAbstractListModel`.
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
        roles
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

#[cfg(test)]
mod tests {
    use super::QueueModelRust;
    use super::qobject::QueueRoles;
    use crate::bridge::test_support::seeded_index;
    use cxx_qt_lib::{QString, QVariant};

    /// Model pointed at a scratch seeded index (never the real home dir).
    fn model_with_seeded_library(
        case: &str,
    ) -> (QueueModelRust, crate::bridge::test_support::Guard) {
        let (guard, path) = seeded_index(case);
        let model = QueueModelRust {
            index_path: path,
            ..Default::default()
        };
        (model, guard)
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
        let tapes = tunex_library::list_albums(&db)
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
    fn queue_ops_reorder_and_remove_rows() {
        let (mut model, _guard) = model_with_seeded_library("queue-ops");
        let db = tunex_library::open_file(&model.index_path).expect("seed opens");
        let tapes = tunex_library::list_albums(&db)
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
        let mut model = QueueModelRust::default();
        assert!(!model.is_shuffle());
        assert!(model.do_toggle_shuffle());
        assert!(model.is_shuffle());
        assert_eq!(model.repeat_mode(), 0);
        assert_eq!(model.do_cycle_repeat(), 1);
        assert_eq!(model.do_cycle_repeat(), 2);
        assert_eq!(model.do_cycle_repeat(), 0);
        assert_eq!(model.playback_state(), 0, "idle engine reads stopped");
        assert_eq!(model.current_index(), -1, "idle cursor reads -1");
        assert_eq!(model.position_ms(), 0);
    }
}
