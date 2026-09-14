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
pub mod display;
pub mod enrich;
pub mod lyrics;
pub mod metadata;
pub mod playlist;
pub mod scan;
pub mod search;
pub mod watch;

pub use artwork::{
    CACHE_BUDGET_BYTES, CachedArt, MAX_ART_BYTES, MAX_ART_DIMENSION, THUMB_SIZES, artwork_key,
    cached_art, default_cache_dir, find_folder_art, load_remote_art, resolve_track_art,
    store_artwork, store_artwork_with_budget, store_remote_art,
};

pub use db::{
    AlbumRow, AlbumSort, ArtistRow, ArtistSort, FacetRow, FolderRow, NewTrack, SavedQueue,
    SavedQueueItem, SortDir, TrackIdentity, TrackRow, TrackSort, UNKNOWN_FACET, add_root,
    album_by_id, album_duration_ms, artist_by_name, delete_track, list_albums,
    list_albums_for_artist, list_artists, list_composers, list_genres, list_recently_played_albums,
    list_track_folders, list_tracks, list_tracks_capped, list_tracks_for_artist,
    list_tracks_for_composer, list_tracks_for_genre, list_tracks_in_album, list_tracks_in_folder,
    load_playback_queue, open_file, open_memory, record_play, remove_library_root, rename_track,
    save_playback_queue, schema_version, search_track_ids, set_missing, track_by_id, track_by_path,
    track_identities, upsert_track,
};
pub use display::{UNKNOWN_ARTIST, UNKNOWN_TITLE, display_title_artist};
pub use enrich::{
    Enrichment, cover_art_url, fill_missing, lookup_recording, parse_recording_search,
    run_missing_pass,
};
pub use lyrics::{LyricLine, Lyrics, active_line, load_lyrics, parse_lrc};
pub use metadata::{EmbeddedArtwork, FileMetadata, TagEdit, read_metadata, write_tags};
pub use playlist::{
    Playlist, PlaylistEntry, SmartRule, add_to_playlist, create_playlist, create_smart_playlist,
    delete_playlist, evaluate_smart_playlist, list_entries, list_playlists, move_entry,
    remove_from_playlist, rename_playlist, smart_rule,
};
pub use scan::{
    SUPPORTED_EXTENSIONS, ScanProgress, ScanStats, collect_media_files, file_id, is_supported,
    scan_folder, scan_folder_live, scan_folder_with_callback, stable_key,
};
pub use search::{SearchResults, search_library};
pub use watch::{DEBOUNCE_WINDOW, LibraryWatcher, watch_roots};
