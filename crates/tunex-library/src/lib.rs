//! `tunex-library`: background library engine for `TuneX`.
//!
//! Covers library roots, recursive scan, metadata extraction, the SQLite
//! index with FTS5 search, the artwork pipeline, and playlists. All heavy work
//! runs on `tokio` workers and reports back as `tunex_core::AppEvent`s; the Qt
//! thread is never blocked and never touched from here.
//!
//! S1 exposes the index foundation (`db`, `scan`); S2 grows it into the v2
//! schema (lookup tables + FTS5) with metadata extraction. The scanner
//! worker, artwork pipeline, and playlists arrive in later S2/S3 items.

pub mod db;
pub mod metadata;
pub mod scan;

pub use db::{
    NewTrack, TrackIdentity, TrackRow, add_root, list_tracks, open_file, open_memory, rename_track,
    schema_version, search_track_ids, set_missing, track_identities, upsert_track,
};
pub use metadata::{EmbeddedArtwork, FileMetadata, read_metadata};
pub use scan::{
    SUPPORTED_EXTENSIONS, ScanProgress, ScanStats, collect_media_files, file_id, is_supported,
    scan_folder, scan_folder_live, scan_folder_with_callback, stable_key,
};
