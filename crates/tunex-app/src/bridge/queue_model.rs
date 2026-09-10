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
use std::time::{Duration, Instant};

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};
use tokio::sync::mpsc;

use crate::mpris::{
    MprisCommand, MprisSnapshot, MprisTrack, TRACK_PATH_PREFIX, publish as publish_mpris,
    take_inbox, unmap_loop,
};
use crate::playback::queue_item_from_row;
use tunex_player::{PlaybackController, uri_to_path};

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
    config_path: PathBuf,
    last_len: usize,
    last_cursor: Option<usize>,
    last_error: Option<String>,
    saved_uri: String,
    saved_position_ms: i32,
    saved_at: Option<Instant>,
    /// Inbound MPRIS commands, taken once from the process hub (S5 W-032).
    mpris_rx: Option<mpsc::Receiver<MprisCommand>>,
}

impl Default for QueueModelRust {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
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
                Ok(mut controller) => {
                    let config = tunex_core::load_from(&self.config_path).unwrap_or_default();
                    controller.set_volume(config.volume);
                    controller.set_muted(config.playback.muted);
                    controller.set_shuffle(config.playback.shuffle);
                    controller.set_repeat(config.playback.repeat_mode);
                    self.controller = Some(controller);
                    self.restore_last_track(&config);
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
        self.sync_mpris();
        !unchanged
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
        self.sync_rows();
        uri.clone_into(&mut self.saved_uri);
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
            let title = path.file_name().map_or_else(
                || "Unknown Title".to_owned(),
                |name| name.to_string_lossy().into_owned(),
            );
            return tunex_player::QueueItem::new(uri, &title);
        }
        tunex_player::QueueItem::new(uri, "Unknown Title")
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
        self.maybe_persist_session(true);
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
        self.saved_uri = item.uri;
        self.saved_position_ms = position;
        self.saved_at = Some(Instant::now());
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

    /// Known duration in milliseconds (engine, else current row, else 0).
    pub fn duration_ms(&self) -> i32 {
        self.rust().duration_ms()
    }

    /// Title of the playing row (empty when idle).
    pub fn current_title(&self) -> QString {
        self.rust().current_title()
    }

    /// Artist of the playing row (empty when idle).
    pub fn current_artist(&self) -> QString {
        self.rust().current_artist()
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
    use super::QueueModelRust;
    use super::qobject::QueueRoles;
    use crate::bridge::test_support::seeded_index;
    use cxx_qt_lib::{QString, QVariant};

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
    fn current_track_reads_from_playing_row() {
        let mut model = QueueModelRust::default();
        model.rows.push((
            QString::from("Midnight"),
            QString::from("Nova Rae"),
            QString::from("Night Tapes"),
            273_000,
            true,
            7,
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
