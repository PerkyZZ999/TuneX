//! `tunex-library`: background library engine for `TuneX`.
//!
//! Covers library roots, recursive scan, metadata extraction, the SQLite
//! index with FTS5 search, the artwork pipeline, and playlists. All heavy work
//! runs on `tokio` workers and reports back as `tunex_core::AppEvent`s; the Qt
//! thread is never blocked and never touched from here.
//!
//! S1 exposes the index foundation (`db`, `scan`); metadata, search, artwork,
//! and playlists arrive slice by slice in S2/S3.

pub mod db;
pub mod scan;

pub use db::{TrackRow, add_root, list_tracks, open_file, open_memory, schema_version};
pub use scan::{
    SUPPORTED_EXTENSIONS, ScanStats, collect_media_files, is_supported, scan_folder, stable_key,
};
