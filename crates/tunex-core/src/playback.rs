//! Playback vocabulary: states and engine events shared by `tunex-player`,
//! the queue, MPRIS, and QML player models.
//!
//! Position is deliberately *not* an event — the UI polls it. Events fire
//! only on discrete transitions (state, track end, errors, duration).

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Repeat behavior for the queue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    /// Play through once and stop.
    #[default]
    Off,
    /// Repeat the queue.
    All,
    /// Repeat the current track.
    One,
}

/// Application-level playback states (SPEC §6).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaybackState {
    /// Nothing loaded, or stopped after playback.
    #[default]
    Stopped,
    /// Track accepted, pipeline prerolling.
    Loading,
    /// Audio flowing.
    Playing,
    /// Paused with position held.
    Paused,
}

/// Discrete notifications from the player engine to the rest of the app.
#[derive(Clone, Debug, PartialEq)]
pub enum PlayerEvent {
    /// Pipeline state settled into a new application state.
    StateChanged(PlaybackState),
    /// Current track reached end-of-stream (queue advances on this).
    EndOfTrack,
    /// Unrecoverable pipeline failure with a human-readable message.
    PlaybackError(String),
    /// Track duration became known (stream discovery completed).
    DurationChanged(Duration),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_stopped() {
        assert_eq!(PlaybackState::default(), PlaybackState::Stopped);
    }

    #[test]
    fn state_changed_event_carries_state() {
        let event = PlayerEvent::StateChanged(PlaybackState::Playing);
        assert!(matches!(
            event,
            PlayerEvent::StateChanged(PlaybackState::Playing)
        ));
    }
}
