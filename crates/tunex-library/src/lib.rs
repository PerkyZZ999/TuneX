//! `tunex-library`: background library engine for `TuneX`.
//!
//! Covers library roots, recursive scan, metadata extraction, the SQLite
//! index with FTS5 search, the artwork pipeline, and playlists. All heavy work
//! runs on `tokio` workers and reports back as `tunex_core::AppEvent`s; the Qt
//! thread is never blocked and never touched from here.
//!
//! S1 exposes the index foundation (`db`, `scan`); S2 grows it into the v2
//! schema (lookup tables + FTS5) with metadata extraction. The scanner
//! worker, artwork, and playlists arrive in later S2/S3 items.

pub mod artwork;
pub mod db;
pub mod metadata;
pub mod playlist;
pub mod scan;
pub mod search;
pub mod watch;

pub use artwork::{
    CACHE_BUDGET_BYTES, CachedArt, MAX_ART_BYTES, MAX_ART_DIMENSION, THUMB_SIZES, artwork_key,
    cached_art, default_cache_dir, find_folder_art, resolve_track_art, store_artwork,
    store_artwork_with_budget,
};

pub use db::{
    AlbumRow, ArtistRow, NewTrack, TrackIdentity, TrackRow, add_root, list_albums, list_artists,
    list_tracks, list_tracks_capped, list_tracks_for_artist, list_tracks_in_album, open_file,
    open_memory, remove_library_root, rename_track, schema_version, search_track_ids, set_missing,
    track_by_id, track_identities, upsert_track,
};
pub use metadata::{EmbeddedArtwork, FileMetadata, read_metadata};
pub use playlist::{
    Playlist, PlaylistEntry, add_to_playlist, create_playlist, delete_playlist, list_entries,
    list_playlists, move_entry, remove_from_playlist, rename_playlist,
};
pub use scan::{
    SUPPORTED_EXTENSIONS, ScanProgress, ScanStats, collect_media_files, file_id, is_supported,
    scan_folder, scan_folder_live, scan_folder_with_callback, stable_key,
};
pub use search::{SearchResults, search_library};
pub use watch::{DEBOUNCE_WINDOW, LibraryWatcher, watch_roots};
