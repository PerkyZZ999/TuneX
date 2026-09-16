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

/// `ReplayGain` application (L-001). Missing tags stay at unity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayGainMode {
    /// Leave tagged gains unused.
    #[default]
    Off,
    /// Apply per-track gain.
    Track,
    /// Apply album gain, falling back to track, then unity.
    Album,
}

/// Now Playing artwork well (S16 W-072). Reduce-motion forces Artwork.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualizerMode {
    /// Embedded or folder art, or the monogram placeholder.
    #[default]
    Artwork,
    /// Spectrum bars from the analysis tap.
    Spectrum,
    /// Oscilloscope of recent PCM.
    Waveform,
    /// `ShaderEffect` driven by the same spectrum bands.
    Visualizer,
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
    fn replaygain_defaults_off() {
        assert_eq!(ReplayGainMode::default(), ReplayGainMode::Off);
    }

    #[test]
    fn state_changed_event_carries_state() {
        let event = PlayerEvent::StateChanged(PlaybackState::Playing);
        assert!(matches!(
            event,
            PlayerEvent::StateChanged(PlaybackState::Playing)
        ));
    }

    // The snake_case spellings below are load-bearing: they persist in
    // config.toml, so a rename or variant reorder must fail loudly here
    // instead of silently breaking existing configs on load.
    #[test]
    fn repeat_mode_serde_spellings_are_stable() {
        for (mode, spelling) in [
            (RepeatMode::Off, "\"off\""),
            (RepeatMode::All, "\"all\""),
            (RepeatMode::One, "\"one\""),
        ] {
            assert_eq!(serde_json::to_string(&mode).unwrap(), spelling);
            assert_eq!(serde_json::from_str::<RepeatMode>(spelling).unwrap(), mode);
        }
    }

    #[test]
    fn replaygain_mode_serde_spellings_are_stable() {
        for (mode, spelling) in [
            (ReplayGainMode::Off, "\"off\""),
            (ReplayGainMode::Track, "\"track\""),
            (ReplayGainMode::Album, "\"album\""),
        ] {
            assert_eq!(serde_json::to_string(&mode).unwrap(), spelling);
            assert_eq!(
                serde_json::from_str::<ReplayGainMode>(spelling).unwrap(),
                mode
            );
        }
    }

    #[test]
    fn visualizer_mode_serde_spellings_are_stable() {
        for (mode, spelling) in [
            (VisualizerMode::Artwork, "\"artwork\""),
            (VisualizerMode::Spectrum, "\"spectrum\""),
            (VisualizerMode::Waveform, "\"waveform\""),
            (VisualizerMode::Visualizer, "\"visualizer\""),
        ] {
            assert_eq!(serde_json::to_string(&mode).unwrap(), spelling);
            assert_eq!(
                serde_json::from_str::<VisualizerMode>(spelling).unwrap(),
                mode
            );
        }
    }

    #[test]
    fn unknown_variant_rejects_loudly() {
        assert!(serde_json::from_str::<RepeatMode>("\"shuffle\"").is_err());
    }
}
