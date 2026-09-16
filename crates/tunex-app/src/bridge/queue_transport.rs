//! Queue transport operations (C13): play/pause/seek-adjacent controls,
//! queue mutation, shuffle/repeat/output cycling. Thin forwarding over the
//! playback controller plus row sync; snapshots and policy helpers stay in
//! `queue_model`.

use std::time::Duration;

use super::queue_model::{QueueModelRust, repeat_to_int};
use tunex_player::PlaybackController;

impl QueueModelRust {
    /// Construct the controller on first use. `GStreamer` failures degrade to
    /// surfaced text (rows stay empty, transport reports the failure).
    pub(crate) fn ensure_controller(&mut self) -> bool {
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

    /// Start or resume playback; refreshes rows on success.
    pub(crate) fn do_play(&mut self) {
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
    pub(crate) fn do_pause(&mut self) {
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
    pub(crate) fn do_play_pause(&mut self) {
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
    pub(crate) fn do_next(&mut self) {
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
    pub(crate) fn do_previous(&mut self, position_ms: i32) {
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
    pub(crate) fn do_play_at(&mut self, index: i32) {
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
    pub(crate) fn do_remove(&mut self, index: i32) {
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
    pub(crate) fn do_move(&mut self, from: i32, to: i32) {
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
    pub(crate) fn do_play_next_at(&mut self, index: i32) {
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
    pub(crate) fn do_move_to_end(&mut self, index: i32) {
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
    pub(crate) fn do_clear_queue(&mut self) {
        if !self.ensure_controller() {
            return;
        }
        if let Some(controller) = &mut self.controller {
            controller.clear_queue();
        }
        self.sync_rows();
    }

    /// Toggle shuffle; returns the new state.
    pub(crate) fn do_toggle_shuffle(&mut self) -> bool {
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
    pub(crate) fn is_shuffle(&self) -> bool {
        self.controller
            .as_ref()
            .is_some_and(PlaybackController::is_shuffle)
    }

    /// Cycle repeat off → all → one; returns the new mode (0/1/2).
    pub(crate) fn do_cycle_repeat(&mut self) -> i32 {
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
}
