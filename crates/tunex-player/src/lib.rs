//! `tunex-player`: audio engine for `TuneX`.
//!
//! [`PlayerEngine`] wraps `GStreamer` `playbin3` behind a stable,
//! UI-thread-friendly API: synchronous control methods plus a bounded event
//! channel for state changes, errors, and end-of-track. The `GStreamer` bus is
//! polled on its own thread; the queue (W-005) advances on `EndOfTrack`.

pub mod engine;
pub mod output;
pub mod playback;
pub mod queue;
pub mod replaygain;
pub mod uri;

pub use engine::{NextUriProvider, PlayerEngine};
pub use output::{AudioOutput, list_audio_outputs};
pub use playback::PlaybackController;
pub use queue::{Advance, Queue, QueueItem, Rewind};
pub use replaygain::{ReplayGainTags, read_replaygain, read_replaygain_uri, replaygain_multiplier};
pub use uri::{path_to_uri, uri_to_path};
