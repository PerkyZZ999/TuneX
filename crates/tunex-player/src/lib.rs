//! `tunex-player`: audio engine for `TuneX`.
//!
//! [`PlayerEngine`] wraps `GStreamer` `playbin3` behind a stable,
//! UI-thread-friendly API: synchronous control methods plus a bounded event
//! channel for state changes, errors, and end-of-track. The `GStreamer` bus is
//! polled on its own thread; the queue (W-005) advances on `EndOfTrack`.

pub mod engine;
pub mod playback;
pub mod queue;
pub mod uri;

pub use engine::{NextUriProvider, PlayerEngine};
pub use playback::PlaybackController;
pub use queue::{Advance, Queue, QueueItem, Rewind};
pub use uri::path_to_uri;
