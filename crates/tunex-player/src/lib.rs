//! `tunex-player`: audio engine for `TuneX`.
//!
//! Owns the `PlayerEngine` abstraction over `GStreamer` `playbin3` (gapless via
//! `about-to-finish` preloading) plus the queue with shuffle/repeat modes.
//! Emits playback state as `tunex_core::AppEvent`s; the `GStreamer` bus is polled
//! on its own thread.
//!
//! Wiring to `tunex-core` types and `GStreamer` lands with the first behavior
//! (S1 W-004+); this scaffold intentionally declares no dependencies yet.
