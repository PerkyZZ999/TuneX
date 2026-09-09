//! `tunex-library`: background library engine for `TuneX`.
//!
//! Covers library roots, recursive scanning, metadata extraction, the `SQLite`
//! index with `FTS5` search, the artwork pipeline, and playlists. All heavy work
//! runs on `tokio` workers and reports back as `tunex_core::AppEvent`s; the Qt
//! thread is never blocked and never touched from here.
//!
//! Wiring to `tunex-core` types lands with the first behavior (S1 W-002+);
//! this scaffold intentionally declares no dependencies yet.
