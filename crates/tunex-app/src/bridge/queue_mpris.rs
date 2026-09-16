//! MPRIS snapshot and command application (C13): engine truth published
//! for the D-Bus thread, inbound commands drained onto the controller.

use std::time::Duration;

use super::queue_model::{QueueModelRust, duration_to_us, us_to_ms_saturating};
use crate::mpris::{
    MprisCommand, MprisSnapshot, MprisTrack, TRACK_PATH_PREFIX, publish as publish_mpris,
    take_inbox, unmap_loop,
};
use tunex_player::PlaybackController;

impl QueueModelRust {
    /// Snapshot engine truth for the MPRIS thread. Honest when idle
    /// (stopped, no track, capabilities denied — never fabricated).
    pub(crate) fn mpris_snapshot(&self) -> MprisSnapshot {
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
    pub(crate) fn apply_mpris_command(&mut self, command: &MprisCommand) {
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
    pub(crate) fn apply_transport_command(&mut self, command: &MprisCommand) -> bool {
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
    pub(crate) fn sync_mpris(&mut self) {
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
}
