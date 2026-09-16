//! SQLite library index: versioned schema, WAL mode, indexed track rows.
//!
//! Migrations run from day one (`rusqlite_migration`): every schema change is
//! a numbered migration, so S2+ tables arrive without breaking S1 databases.
//! Only index data lives here — never audio.
//!
//! Write discipline: every mutation goes through the helpers below inside one
//! transaction, so `tracks`, lookup tables, and the FTS index never diverge.
//! SQLite triggers backstop deletions cascading into the search index.

use rusqlite::{Connection, Row};
use rusqlite_migration::{M, Migrations};
use tunex_core::{Error, Result};

#[path = "db_queue.rs"]
mod db_queue;
#[path = "db_read.rs"]
mod db_read;
#[path = "db_write.rs"]
mod db_write;

pub use db_queue::{
    SavedQueue, SavedQueueItem, list_recently_played_albums, load_playback_queue, record_play,
    save_playback_queue,
};
pub use db_read::{
    AlbumRow, AlbumSort, ArtistRow, ArtistSort, FacetRow, FolderRow, SortDir, TrackSort,
    UNKNOWN_FACET, album_by_id, album_duration_ms, artist_by_name, list_albums,
    list_albums_for_artist, list_artists, list_composers, list_genres, list_track_folders,
    list_tracks, list_tracks_capped, list_tracks_for_artist, list_tracks_for_composer,
    list_tracks_for_genre, list_tracks_in_album, list_tracks_in_folder, search_track_ids,
    track_by_id, track_by_path, track_identities,
};
pub(crate) use db_read::{albums_for_tracks, artists_by_names, tracks_by_ids};
pub use db_write::{
    add_root, delete_track, remove_library_root, rename_track, retarget_track, set_missing,
    upsert_track,
};

/// Current schema version (v7: play history, smart playlists, `indexed_at`).
pub const SCHEMA_VERSION: usize = 7;

/// v1 DDL, frozen: roots + tracks skeleton (landed in S1, never edited).
const V1_SCHEMA: &str = "CREATE TABLE library_roots(
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE
        );
        CREATE TABLE tracks(
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            title TEXT,
            stable_key TEXT NOT NULL
        );
        CREATE INDEX idx_tracks_stable_key ON tracks(stable_key);";

/// v2 DDL: metadata columns, lookup tables, folders, scan state, and the
/// FTS5 external-content search index with sync triggers (SPEC §11.3).
const V2_SCHEMA: &str = "
        CREATE TABLE artists(
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        );
        CREATE TABLE albums(
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            artist_id INTEGER REFERENCES artists(id),
            year INTEGER,
            UNIQUE(title, artist_id)
        );
        CREATE TABLE genres(
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        );
        CREATE TABLE folders(
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            root_id INTEGER NOT NULL REFERENCES library_roots(id)
        );
        CREATE TABLE scan_state(
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        ALTER TABLE tracks ADD COLUMN artist_id INTEGER REFERENCES artists(id);
        ALTER TABLE tracks ADD COLUMN album_id INTEGER REFERENCES albums(id);
        ALTER TABLE tracks ADD COLUMN genre_id INTEGER REFERENCES genres(id);
        ALTER TABLE tracks ADD COLUMN composer TEXT;
        ALTER TABLE tracks ADD COLUMN year INTEGER;
        ALTER TABLE tracks ADD COLUMN track_number INTEGER;
        ALTER TABLE tracks ADD COLUMN disc_number INTEGER;
        ALTER TABLE tracks ADD COLUMN duration_ms INTEGER;
        CREATE TABLE track_search_docs(
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL DEFAULT '',
            artist TEXT NOT NULL DEFAULT '',
            album TEXT NOT NULL DEFAULT '',
            album_artist TEXT NOT NULL DEFAULT '',
            composer TEXT NOT NULL DEFAULT '',
            genre TEXT NOT NULL DEFAULT '',
            filename TEXT NOT NULL DEFAULT ''
        );
        CREATE VIRTUAL TABLE track_search USING fts5(
            title, artist, album, album_artist, composer, genre, filename,
            content='track_search_docs', content_rowid='id',
            tokenize='unicode61'
        );
        CREATE TRIGGER track_search_ai AFTER INSERT ON track_search_docs BEGIN
            INSERT INTO track_search(rowid, title, artist, album, album_artist, composer, genre, filename)
            VALUES (new.id, new.title, new.artist, new.album, new.album_artist, new.composer, new.genre, new.filename);
        END;
        CREATE TRIGGER track_search_ad AFTER DELETE ON track_search_docs BEGIN
            INSERT INTO track_search(track_search, rowid, title, artist, album, album_artist, composer, genre, filename)
            VALUES ('delete', old.id, old.title, old.artist, old.album, old.album_artist, old.composer, old.genre, old.filename);
        END;
        CREATE TRIGGER track_search_au AFTER UPDATE ON track_search_docs BEGIN
            INSERT INTO track_search(track_search, rowid, title, artist, album, album_artist, composer, genre, filename)
            VALUES ('delete', old.id, old.title, old.artist, old.album, old.album_artist, old.composer, old.genre, old.filename);
            INSERT INTO track_search(rowid, title, artist, album, album_artist, composer, genre, filename)
            VALUES (new.id, new.title, new.artist, new.album, new.album_artist, new.composer, new.genre, new.filename);
        END;
        CREATE TRIGGER tracks_ad AFTER DELETE ON tracks BEGIN
            DELETE FROM track_search_docs WHERE id = old.id;
        END;
        INSERT INTO track_search_docs(id, title, filename)
            SELECT id, COALESCE(title, ''), path FROM tracks;";

/// v3 DDL: reconcile support — `missing` flags vanished files (SPEC §11.1:
/// mark first, garbage-collect later) and `file_id` (`dev:ino`) lets rescans
/// recognize renames instead of duplicating rows (D-009 inode assist).
const V3_SCHEMA: &str = "
        ALTER TABLE tracks ADD COLUMN missing INTEGER NOT NULL DEFAULT 0;
        ALTER TABLE tracks ADD COLUMN file_id TEXT;
        CREATE INDEX idx_tracks_missing ON tracks(missing);";

/// v4 DDL: playlists with ordered entries. Names are unique (rename
/// conflicts error explicitly); entries carry a surrogate id so the same
/// track can repeat. `track_id` deliberately has NO foreign key: deleted
/// tracks leave entries dangling-as-missing (D-009) instead of cascading.
const V4_SCHEMA: &str = "
        CREATE TABLE playlists(
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        );
        CREATE TABLE playlist_tracks(
            id INTEGER PRIMARY KEY,
            playlist_id INTEGER NOT NULL REFERENCES playlists(id),
            track_id INTEGER NOT NULL,
            position INTEGER NOT NULL
        );
        CREATE INDEX idx_playlist_tracks_lookup ON playlist_tracks(playlist_id, position);";

/// v5 DDL: cover the two foreign keys every grouped query joins on.
///
/// Without them the album and artist projections ([`ALBUM_LIST_SELECT`],
/// [`ARTIST_LIST_SELECT`]) scan the whole `tracks` table once per group, so
/// their cost is `groups x tracks` rather than `matched rows`. Measured on a
/// 50k-track index (S6 W-041): search p95 fell from ~1.0 s to under 18 ms,
/// because a search hydrates up to 200 album groups and 200 artist groups and
/// was paying 200 full scans for each.
const V5_SCHEMA: &str = "
        CREATE INDEX idx_tracks_album_id ON tracks(album_id);
        CREATE INDEX idx_tracks_artist_id ON tracks(artist_id);";

/// v6 DDL: ordered Up Next snapshot so the queue survives restart (L-012).
const V6_SCHEMA: &str = "
        CREATE TABLE playback_queue (
            position INTEGER PRIMARY KEY,
            uri TEXT NOT NULL,
            track_id INTEGER,
            title TEXT NOT NULL,
            artist TEXT,
            album TEXT,
            duration_ms INTEGER
        );
        CREATE TABLE playback_queue_meta (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            cursor INTEGER NOT NULL DEFAULT 0,
            position_ms INTEGER NOT NULL DEFAULT 0
        );";

/// v7 DDL: play history, favorites stub, smart-playlist rules, and first-index time.
const V7_SCHEMA: &str = "
        ALTER TABLE tracks ADD COLUMN indexed_at INTEGER NOT NULL DEFAULT 0;
        CREATE TABLE play_history (
            id INTEGER PRIMARY KEY,
            track_id INTEGER,
            uri TEXT NOT NULL,
            played_at INTEGER NOT NULL
        );
        CREATE INDEX idx_play_history_played ON play_history(played_at DESC);
        CREATE TABLE favorites (
            track_id INTEGER PRIMARY KEY
        );
        CREATE TABLE smart_playlists (
            playlist_id INTEGER PRIMARY KEY REFERENCES playlists(id) ON DELETE CASCADE,
            rule_kind TEXT NOT NULL,
            rule_value TEXT NOT NULL DEFAULT '',
            exclude_missing INTEGER NOT NULL DEFAULT 1
        );";

/// Versioned migrations, oldest first. Append-only: never edit a landed
/// migration, always add a new one.
fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(V1_SCHEMA),
        M::up(V2_SCHEMA),
        M::up(V3_SCHEMA),
        M::up(V4_SCHEMA),
        M::up(V5_SCHEMA),
        M::up(V6_SCHEMA),
        M::up(V7_SCHEMA),
    ])
}

/// One indexed track row with resolved display names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackRow {
    /// Database row id (internal; renames keep history via `stable_key`).
    pub id: i64,
    /// Absolute filesystem path.
    pub path: String,
    /// Title when known.
    pub title: Option<String>,
    /// Stable identity (`path + mtime + size`); survives renames via reconcile.
    pub stable_key: String,
    /// Resolved artist name.
    pub artist: Option<String>,
    /// Resolved album title.
    pub album: Option<String>,
    /// Resolved genre name.
    pub genre: Option<String>,
    /// Composer credit.
    pub composer: Option<String>,
    /// Release year.
    pub year: Option<i64>,
    /// Track number within the disc.
    pub track_number: Option<i64>,
    /// Disc number within the release.
    pub disc_number: Option<i64>,
    /// Duration in milliseconds.
    pub duration_ms: Option<i64>,
    /// File vanished from disk but the row is kept (playlists/history links
    /// survive; a later rescan clears the flag when the file returns).
    pub missing: bool,
}

impl TrackRow {
    pub(crate) fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            path: row.get("path")?,
            title: row.get("title")?,
            stable_key: row.get("stable_key")?,
            artist: row.get("artist")?,
            album: row.get("album")?,
            genre: row.get("genre")?,
            composer: row.get("composer")?,
            year: row.get("year")?,
            track_number: row.get("track_number")?,
            disc_number: row.get("disc_number")?,
            duration_ms: row.get("duration_ms")?,
            missing: row.get("missing")?,
        })
    }
}

/// A track to index: write-side counterpart of [`TrackRow`] with unresolved
/// names (ids are resolved inside the upsert transaction).
#[derive(Clone, Debug, Default)]
pub struct NewTrack {
    /// Absolute filesystem path (identity for upserts).
    pub path: String,
    /// Stable identity (`stable_key` from the scanner).
    pub stable_key: String,
    /// Title when tagged.
    pub title: Option<String>,
    /// Artist name when tagged.
    pub artist: Option<String>,
    /// Album title when tagged.
    pub album: Option<String>,
    /// Album artist when tagged (may differ from track artist on compilations).
    pub album_artist: Option<String>,
    /// Composer credit when tagged.
    pub composer: Option<String>,
    /// Genre name when tagged.
    pub genre: Option<String>,
    /// Release year.
    pub year: Option<i64>,
    /// Track number within the disc.
    pub track_number: Option<i64>,
    /// Disc number within the release.
    pub disc_number: Option<i64>,
    /// Duration in milliseconds.
    pub duration_ms: Option<i64>,
    /// Filesystem identity (`dev:ino` on Unix, `None` elsewhere); rescans
    /// match renames through this instead of duplicating rows.
    pub file_id: Option<String>,
}

/// Minimal row identity for reconcile: everything the scanner needs to tell
/// a refresh apart from a rename apart from a deletion, without loading
/// display data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackIdentity {
    /// Database row id.
    pub id: i64,
    /// Last indexed absolute path.
    pub path: String,
    /// Last seen filesystem identity.
    pub file_id: Option<String>,
    /// Identity key as last indexed (`path + mtime + size`), so a rescan can
    /// tell an untouched file from an edited one without reading its tags.
    pub stable_key: String,
    /// Whether the row is currently flagged missing.
    pub missing: bool,
    /// `true` when the indexed title is missing or blank. Untouched-file
    /// skip must not apply: a one-time tag miss would otherwise stick.
    pub incomplete: bool,
}

/// Open an in-memory database (tests, throwaway harnesses).
///
/// # Errors
///
/// Returns [`Error::Database`] when the schema migrations fail.
pub fn open_memory() -> Result<Connection> {
    let mut db = Connection::open_in_memory().map_err(|err| db_error(&err))?;
    // Same contention story as file databases (tests share nothing, but any
    // future multi-connection memory use must wait, not fail instantly).
    db.busy_timeout(BUSY_TIMEOUT)
        .map_err(|err| db_error(&err))?;
    migrations()
        .to_latest(&mut db)
        .map_err(|err| Error::Database(err.to_string()))?;
    Ok(db)
}

/// Open (creating parents for) a file database with migrations applied and
/// WAL mode enabled for concurrent readers.
///
/// # Errors
///
/// Returns [`Error::Io`] when directories cannot be created and
/// [`Error::Database`] when SQLite or the migrations fail.
pub fn open_file(path: &std::path::Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let mut db = Connection::open(path).map_err(|err| db_error(&err))?;
    db.pragma_update(None, "journal_mode", "WAL")
        .map_err(|err| db_error(&err))?;
    // WAL keeps readers off a writer's back, but two writers still collide,
    // and the scan worker writes continuously while the UI reads and writes.
    // Without a timeout SQLite fails a contended write immediately instead of
    // waiting the moment or two the other write needs.
    db.busy_timeout(BUSY_TIMEOUT)
        .map_err(|err| db_error(&err))?;
    migrate(&mut db)?;
    Ok(db)
}

/// How long a connection waits on another connection's write lock.
const BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// Bring one connection's schema up to date, tolerating a concurrent winner.
///
/// `rusqlite_migration` reads `user_version` outside the transaction that
/// applies the DDL, so two connections opening the same out-of-date index —
/// exactly what the app and its scan worker do on the first start after an
/// upgrade — can both decide to migrate, and the loser then replays DDL that
/// already landed. Its transaction rolls back untouched, so the only thing
/// left to decide is whether the schema ended up current; if it did, the
/// other connection did the work and this one has nothing left to do.
///
/// This leans on every migration being DDL, which fails when replayed. A
/// migration whose replay would *succeed* (inserting rows, say) would commit
/// twice over, so keep the ladder declarative.
fn migrate(db: &mut Connection) -> Result<()> {
    if schema_version(db)? >= SCHEMA_VERSION {
        return Ok(());
    }
    let Err(err) = migrations().to_latest(db) else {
        return Ok(());
    };
    if schema_version(db)? == SCHEMA_VERSION {
        return Ok(());
    }
    Err(Error::Database(err.to_string()))
}

/// Applied schema version (equals [`SCHEMA_VERSION`] on fresh databases).
///
/// # Errors
///
/// Returns [`Error::Database`] when the pragma cannot be read.
pub fn schema_version(db: &Connection) -> Result<usize> {
    let version: i64 = db
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|err| db_error(&err))?;
    Ok(usize::try_from(version).unwrap_or(0))
}

pub(crate) fn db_error(err: &rusqlite::Error) -> Error {
    Error::Database(err.to_string())
}

/// Per-group result cap: every search group (tracks, albums, artists) returns
/// at most this many rows (SPEC §11.3). Raising it grows per-keystroke query
/// and hydration work linearly; the UI never pages past it in V1.
pub(crate) const GROUP_LIMIT: u32 = 200;

/// `?, ?, …` placeholders for an IN-list of `count` bindings. Callers skip
/// the query entirely when the list is empty (`IN ()` is invalid SQL).
fn placeholders(count: usize) -> String {
    vec!["?"; count].join(", ")
}

/// Run one FTS5 MATCH expression against the search index, best matches
/// first (BM25), capped at [`GROUP_LIMIT`] rows.
///
/// The expression must already be quoted literal-by-literal (see
/// [`match_query`](super::search::match_query)): raw user text would parse
/// as FTS5 syntax and error on punctuation.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub(crate) fn matched_ids(db: &Connection, expression: &str) -> Result<Vec<i64>> {
    let mut statement = db
        .prepare(
            "SELECT rowid FROM track_search
             WHERE track_search MATCH ?1 ORDER BY bm25(track_search) LIMIT ?2",
        )
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map(rusqlite::params![expression, GROUP_LIMIT], |row| row.get(0))
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<i64>>>()
        .map_err(|err| db_error(&err))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_track(path: &str) -> NewTrack {
        NewTrack {
            path: path.to_owned(),
            stable_key: format!("{path}:0:0"),
            ..Default::default()
        }
    }

    #[test]
    fn fresh_database_reports_schema_v7() {
        let db = open_memory().expect("in-memory opens");
        assert_eq!(schema_version(&db).expect("version reads"), SCHEMA_VERSION);
        assert_eq!(SCHEMA_VERSION, 7);
    }

    #[test]
    fn file_database_uses_wal_mode() {
        let dir = std::env::temp_dir().join(format!("tunex-db-{}", std::process::id()));
        let path = dir.join("library.db");
        let db = open_file(&path).expect("file opens");
        let mode: String = db
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .expect("pragma reads");
        assert_eq!(mode.to_lowercase(), "wal");
        assert_eq!(schema_version(&db).expect("version reads"), SCHEMA_VERSION);
        drop(db);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn old_databases_migrate_to_latest() {
        let mut db = Connection::open_in_memory().expect("in-memory opens");
        Migrations::new(vec![M::up(super::V1_SCHEMA)])
            .to_latest(&mut db)
            .expect("v1 applies");
        db.execute(
            "INSERT INTO tracks(path, title, stable_key) VALUES ('/m/old.flac', 'Old', 'k')",
            [],
        )
        .expect("v1 row inserts");
        Migrations::new(vec![M::up(super::V1_SCHEMA), M::up(super::V2_SCHEMA)])
            .to_latest(&mut db)
            .expect("v2 migrates");
        Migrations::new(vec![
            M::up(super::V1_SCHEMA),
            M::up(super::V2_SCHEMA),
            M::up(super::V3_SCHEMA),
            M::up(super::V4_SCHEMA),
            M::up(super::V5_SCHEMA),
            M::up(super::V6_SCHEMA),
            M::up(super::V7_SCHEMA),
        ])
        .to_latest(&mut db)
        .expect("v3+v4+v5+v6+v7 migrate");
        assert_eq!(schema_version(&db).expect("version reads"), 7);
        let tracks = list_tracks(&db).expect("list works");
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title.as_deref(), Some("Old"));
        assert_eq!(tracks[0].artist, None);
        // v3 defaults: surviving rows are present, not missing.
        assert!(!tracks[0].missing);
        // Backfill indexed the surviving row by path.
        assert_eq!(search_track_ids(&db, "old").expect("search works").len(), 1);
        // v4 arrives empty alongside the preserved rows.
        assert!(
            crate::list_playlists(&db)
                .expect("playlists list")
                .is_empty()
        );
        // v5 covers the grouped-query joins on an already-populated table.
        let indexes: Vec<String> = db
            .prepare("SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'tracks'")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get(0))?
                    .collect::<rusqlite::Result<Vec<String>>>()
            })
            .expect("index list reads");
        assert!(indexes.iter().any(|name| name == "idx_tracks_album_id"));
        assert!(indexes.iter().any(|name| name == "idx_tracks_artist_id"));
    }

    #[test]
    fn playback_queue_round_trips() {
        let mut db = open_memory().expect("in-memory opens");
        let items = vec![
            SavedQueueItem {
                uri: "file:///a.flac".to_owned(),
                track_id: Some(1),
                title: "A".to_owned(),
                artist: Some("Nova".to_owned()),
                album: Some("Tapes".to_owned()),
                duration_ms: Some(1200),
            },
            SavedQueueItem {
                uri: "file:///b.flac".to_owned(),
                track_id: None,
                title: "B".to_owned(),
                artist: None,
                album: None,
                duration_ms: None,
            },
        ];
        save_playback_queue(&mut db, &items, 1, 1500).expect("save");
        let loaded = load_playback_queue(&db).expect("load");
        assert_eq!(loaded.cursor, 1);
        assert_eq!(loaded.position_ms, 1500);
        assert_eq!(loaded.items, items);
        save_playback_queue(&mut db, &[], 0, 0).expect("clear");
        let empty = load_playback_queue(&db).expect("empty");
        assert!(empty.items.is_empty());
    }

    #[test]
    fn play_history_powers_recently_played_albums() {
        let mut db = open_memory().expect("in-memory opens");
        let mut first = new_track("/music/a.flac");
        first.artist = Some("Nova".to_owned());
        first.album = Some("Tapes".to_owned());
        upsert_track(&mut db, &first).expect("upsert");
        let mut second = new_track("/music/b.flac");
        second.artist = Some("Nova".to_owned());
        second.album = Some("Harbor".to_owned());
        upsert_track(&mut db, &second).expect("upsert");
        let tracks = list_tracks(&db).expect("list");
        record_play(&db, Some(tracks[0].id), "file:///music/a.flac").expect("history");
        record_play(&db, Some(tracks[1].id), "file:///music/b.flac").expect("history");
        let recent = list_recently_played_albums(&db, 20).expect("recent");
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].title, "Harbor");
        assert_eq!(recent[1].title, "Tapes");
    }

    #[test]
    fn concurrent_opens_migrate_an_out_of_date_index_once() {
        // The app and its scan worker open the same file index at the same
        // time, so the first start after an upgrade runs this race for real.
        let dir = std::env::temp_dir().join(format!("tunex-race-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("setup works");
        // The window between reading `user_version` and applying the DDL is
        // microseconds wide, so one round proves nothing: repeat the whole
        // out-of-date-index scenario often enough to land inside it.
        for round in 0..40 {
            race_one_upgrade(&dir.join(format!("library-{round}.db")));
        }
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    /// An index left at the schema version before the newest migration.
    fn write_out_of_date_index(path: &std::path::Path) {
        let mut old = Connection::open(path).expect("file opens");
        old.pragma_update(None, "journal_mode", "WAL")
            .expect("wal enables");
        Migrations::new(vec![
            M::up(super::V1_SCHEMA),
            M::up(super::V2_SCHEMA),
            M::up(super::V3_SCHEMA),
            M::up(super::V4_SCHEMA),
        ])
        .to_latest(&mut old)
        .expect("v4 applies");
    }

    /// One v4 index opened by four connections at once; every open must come
    /// back at the current schema version.
    fn race_one_upgrade(path: &std::path::Path) {
        write_out_of_date_index(path);

        let ready = std::sync::Arc::new(std::sync::Barrier::new(4));
        let openers: Vec<_> = (0..4)
            .map(|_| {
                let path = path.to_path_buf();
                let ready = std::sync::Arc::clone(&ready);
                std::thread::spawn(move || {
                    ready.wait();
                    open_file(&path).map(|db| schema_version(&db))
                })
            })
            .collect();
        for opener in openers {
            let version = opener
                .join()
                .expect("opener thread finishes")
                .expect("a concurrent open still succeeds")
                .expect("version reads");
            assert_eq!(version, SCHEMA_VERSION);
        }
    }

    #[test]
    fn rename_and_missing_helpers_keep_row_identity() {
        let mut db = open_memory().expect("in-memory opens");
        upsert_track(&mut db, &new_track("/music/a.flac")).expect("upsert works");
        let id = list_tracks(&db).expect("list works")[0].id;
        rename_track(&db, id, "/music/b.flac", "k2").expect("rename works");
        let tracks = list_tracks(&db).expect("list works");
        assert_eq!(tracks.len(), 1);
        assert_eq!(
            tracks[0].id, id,
            "rename keeps the row (history links survive)"
        );
        assert_eq!(tracks[0].path, "/music/b.flac");
        set_missing(&db, id, true).expect("flag works");
        assert!(list_tracks(&db).expect("list works")[0].missing);
        let identities = track_identities(&db).expect("identities read");
        assert_eq!(identities.len(), 1);
        assert!(identities[0].missing);
        // Rescanning the returned file clears the flag.
        let mut returned = new_track("/music/b.flac");
        returned.stable_key = "k3".to_owned();
        upsert_track(&mut db, &returned).expect("re-upsert works");
        assert!(!list_tracks(&db).expect("list works")[0].missing);
    }

    #[test]
    fn delete_track_drops_the_index_row_and_search_hit() {
        let mut db = open_memory().expect("in-memory opens");
        let mut track = new_track("/music/gone.flac");
        track.title = Some("Gone".to_owned());
        upsert_track(&mut db, &track).expect("upsert works");
        let id = list_tracks(&db).expect("list works")[0].id;
        assert_eq!(
            search_track_ids(&db, "gone").expect("search works").len(),
            1
        );
        assert!(delete_track(&db, id).expect("delete works"));
        assert!(list_tracks(&db).expect("list works").is_empty());
        assert!(
            search_track_ids(&db, "gone")
                .expect("search after delete")
                .is_empty(),
            "FTS cascade must drop the hit"
        );
        assert!(
            !delete_track(&db, id).expect("second delete is a no-op"),
            "missing ids return false"
        );
    }

    #[test]
    fn roots_are_idempotent() {
        let mut db = open_memory().expect("in-memory opens");
        add_root(&mut db, "/music").expect("first add works");
        add_root(&mut db, "/music").expect("second add is a no-op");
        let count: i64 = db
            .query_row("SELECT COUNT(*) FROM library_roots", [], |row| row.get(0))
            .expect("count reads");
        assert_eq!(count, 1);
    }

    #[test]
    fn upsert_resolves_names_and_searches() {
        let mut db = open_memory().expect("in-memory opens");
        let track = NewTrack {
            path: "/music/midnight.flac".to_owned(),
            stable_key: "k1".to_owned(),
            title: Some("Midnight".to_owned()),
            artist: Some("Nova Rae".to_owned()),
            album: Some("Night Tapes".to_owned()),
            genre: Some("Ambient".to_owned()),
            ..Default::default()
        };
        upsert_track(&mut db, &track).expect("upsert works");
        let tracks = list_tracks(&db).expect("list works");
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].artist.as_deref(), Some("Nova Rae"));
        assert_eq!(tracks[0].album.as_deref(), Some("Night Tapes"));
        assert_eq!(tracks[0].genre.as_deref(), Some("Ambient"));
        assert_eq!(search_track_ids(&db, "mid").expect("search works").len(), 1);
        assert_eq!(
            search_track_ids(&db, "nova").expect("search works").len(),
            1
        );
        assert!(
            search_track_ids(&db, "zzz")
                .expect("search works")
                .is_empty()
        );
        // Rescan refreshes instead of duplicating.
        upsert_track(&mut db, &track).expect("re-upsert works");
        assert_eq!(list_tracks(&db).expect("list works").len(), 1);
    }

    #[test]
    fn track_by_id_resolves_single_rows() {
        let mut db = open_memory().expect("in-memory opens");
        let mut track = new_track("/music/solo.flac");
        track.title = Some("Solo".to_owned());
        upsert_track(&mut db, &track).expect("upsert works");
        let id = list_tracks(&db).expect("list works")[0].id;
        let found = track_by_id(&db, id).expect("lookup works");
        assert_eq!(found.and_then(|row| row.title).as_deref(), Some("Solo"));
        assert!(track_by_id(&db, id + 1000).expect("lookup works").is_none());
        let by_path = track_by_path(&db, "/music/solo.flac").expect("path lookup works");
        assert_eq!(by_path.and_then(|row| row.title).as_deref(), Some("Solo"));
        assert!(
            track_by_path(&db, "/music/absent.flac")
                .expect("missing path lookup works")
                .is_none()
        );
    }

    #[test]
    fn browse_lists_aggregate_and_drill_down() {
        let mut db = open_memory().expect("in-memory opens");
        for (path, title, artist, album, number) in [
            ("a1.flac", "One", "Nova Rae", "Night Tapes", 1),
            ("a2.flac", "Two", "Nova Rae", "Night Tapes", 2),
            ("b1.flac", "Solo", "Solo Act", "Only", 1),
        ] {
            upsert_track(
                &mut db,
                &NewTrack {
                    path: format!("/music/{path}"),
                    stable_key: path.to_owned(),
                    title: Some(title.to_owned()),
                    artist: Some(artist.to_owned()),
                    album: Some(album.to_owned()),
                    track_number: Some(number),
                    ..Default::default()
                },
            )
            .expect("upsert works");
        }
        let artists = list_artists(&db, ArtistSort::Name, SortDir::Asc).expect("artists list");
        assert_eq!(artists.len(), 2);
        let nova = artists
            .iter()
            .find(|row| row.name == "Nova Rae")
            .expect("artist present");
        assert_eq!((nova.album_count, nova.track_count), (1, 2));

        let albums = list_albums(&db, AlbumSort::Title, SortDir::Asc).expect("albums list");
        assert_eq!(albums.len(), 2);
        let tapes = albums
            .iter()
            .find(|row| row.title == "Night Tapes")
            .expect("album present");
        assert_eq!(tapes.artist.as_deref(), Some("Nova Rae"));
        assert_eq!(tapes.track_count, 2);
        assert_eq!(
            tapes.art_source.as_deref(),
            Some("/music/a1.flac"),
            "the album carries a track to read artwork from"
        );

        let songs = list_tracks_in_album(&db, tapes.id).expect("album tracks list");
        assert_eq!(songs.len(), 2);
        assert_eq!(songs[0].title.as_deref(), Some("One"));
        assert_eq!(songs[1].title.as_deref(), Some("Two"));

        let capped =
            list_tracks_capped(&db, 2, TrackSort::Title, SortDir::Asc).expect("capped list");
        assert_eq!(capped.len(), 2);
        assert!(
            list_tracks_capped(&db, 0, TrackSort::Title, SortDir::Asc)
                .expect("empty cap")
                .is_empty()
        );
        let by_title =
            list_tracks_capped(&db, 10, TrackSort::Title, SortDir::Asc).expect("title sort");
        let titles: Vec<_> = by_title
            .iter()
            .filter_map(|row| row.title.as_deref())
            .collect();
        assert_eq!(titles, ["One", "Solo", "Two"]);
        let by_title_desc =
            list_tracks_capped(&db, 10, TrackSort::Title, SortDir::Desc).expect("title desc");
        let titles_desc: Vec<_> = by_title_desc
            .iter()
            .filter_map(|row| row.title.as_deref())
            .collect();
        assert_eq!(titles_desc, ["Two", "Solo", "One"]);
        let by_artist =
            list_tracks_capped(&db, 10, TrackSort::Artist, SortDir::Asc).expect("artist sort");
        assert_eq!(by_artist[0].artist.as_deref(), Some("Nova Rae"));
        assert_eq!(by_artist[2].artist.as_deref(), Some("Solo Act"));

        let albums_by_artist =
            list_albums(&db, AlbumSort::Artist, SortDir::Asc).expect("album artist sort");
        assert_eq!(albums_by_artist[0].artist.as_deref(), Some("Nova Rae"));
        let artists_by_songs =
            list_artists(&db, ArtistSort::Songs, SortDir::Desc).expect("artist song sort");
        assert_eq!(artists_by_songs[0].name, "Nova Rae");
        assert_eq!(artists_by_songs[0].track_count, 2);

        let loaded = album_by_id(&db, tapes.id)
            .expect("album by id")
            .expect("present");
        assert_eq!(loaded.title, "Night Tapes");
        assert_eq!(album_duration_ms(&db, tapes.id).expect("duration"), 0);
        let more = list_albums_for_artist(&db, "Nova Rae", Some(tapes.id)).expect("more by");
        assert!(more.is_empty());
        let all_nova = list_albums_for_artist(&db, "Nova Rae", None).expect("artist albums");
        assert_eq!(all_nova.len(), 1);
        assert_eq!(
            artist_by_name(&db, "Nova Rae")
                .expect("artist by name")
                .expect("present")
                .track_count,
            2
        );
    }

    #[test]
    fn genres_and_composers_group_unknown() {
        let mut db = open_memory().expect("in-memory opens");
        upsert_track(
            &mut db,
            &NewTrack {
                path: "/music/tagged.flac".to_owned(),
                stable_key: "t".to_owned(),
                title: Some("Tagged".to_owned()),
                genre: Some("Ambient".to_owned()),
                composer: Some("Novo".to_owned()),
                ..Default::default()
            },
        )
        .expect("tagged upsert");
        upsert_track(
            &mut db,
            &NewTrack {
                path: "/music/bare.flac".to_owned(),
                stable_key: "b".to_owned(),
                title: Some("Bare".to_owned()),
                ..Default::default()
            },
        )
        .expect("bare upsert");
        let genres = list_genres(&db).expect("genres");
        assert_eq!(genres.len(), 2);
        assert!(
            genres
                .iter()
                .any(|row| row.name == "Ambient" && row.track_count == 1)
        );
        assert!(
            genres
                .iter()
                .any(|row| row.name == UNKNOWN_FACET && row.track_count == 1)
        );
        let composers = list_composers(&db).expect("composers");
        assert!(composers.iter().any(|row| row.name == "Novo"));
        assert!(composers.iter().any(|row| row.name == UNKNOWN_FACET));
        assert_eq!(
            list_tracks_for_genre(&db, "Ambient")
                .expect("genre tracks")
                .len(),
            1
        );
        assert_eq!(
            list_tracks_for_genre(&db, UNKNOWN_FACET)
                .expect("unknown genre")
                .len(),
            1
        );
        assert_eq!(
            list_tracks_for_composer(&db, "Novo")
                .expect("composer tracks")
                .len(),
            1
        );
    }

    #[test]
    fn folders_group_direct_children_only() {
        let mut db = open_memory().expect("in-memory opens");
        for (path, title) in [
            ("/music/Night Tapes/one.flac", "One"),
            ("/music/Night Tapes/two.flac", "Two"),
            ("/music/Only/solo.flac", "Solo"),
            ("/music/Night Tapes/Live/three.flac", "Three"),
        ] {
            upsert_track(
                &mut db,
                &NewTrack {
                    path: path.to_owned(),
                    stable_key: title.to_owned(),
                    title: Some(title.to_owned()),
                    artist: Some("Nova Rae".to_owned()),
                    album: Some("Night Tapes".to_owned()),
                    ..Default::default()
                },
            )
            .expect("upsert works");
        }
        let folders = list_track_folders(&db).expect("folders list");
        let names: Vec<_> = folders.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(names, ["Live", "Night Tapes", "Only"]);
        let tapes = folders
            .iter()
            .find(|row| row.name == "Night Tapes")
            .expect("album folder");
        assert_eq!(tapes.track_count, 2, "nested Live tracks stay in Live");
        let in_tapes = list_tracks_in_folder(&db, &tapes.path, 10, TrackSort::Title, SortDir::Asc)
            .expect("folder tracks");
        let titles: Vec<_> = in_tapes
            .iter()
            .filter_map(|row| row.title.as_deref())
            .collect();
        assert_eq!(titles, ["One", "Two"]);
        let live = folders
            .iter()
            .find(|row| row.name == "Live")
            .expect("nested folder");
        let in_live = list_tracks_in_folder(&db, &live.path, 10, TrackSort::Title, SortDir::Asc)
            .expect("nested tracks");
        assert_eq!(in_live.len(), 1);
        assert_eq!(in_live[0].title.as_deref(), Some("Three"));
    }

    #[test]
    fn artist_tracks_list_in_album_order() {
        let mut db = open_memory().expect("in-memory opens");
        for (path, title, artist, album, year, number) in [
            ("b2.flac", "Two", "Nova Rae", "Night Tapes", 2024, 2),
            ("b1.flac", "One", "Nova Rae", "Night Tapes", 2024, 1),
            ("solo.flac", "Solo", "Solo Act", "Only", 2020, 1),
            ("day.flac", "Day", "Nova Rae", "Day Tapes", 2022, 1),
        ] {
            upsert_track(
                &mut db,
                &NewTrack {
                    path: format!("/music/{path}"),
                    stable_key: path.to_owned(),
                    title: Some(title.to_owned()),
                    artist: Some(artist.to_owned()),
                    album: Some(album.to_owned()),
                    year: Some(year),
                    track_number: Some(number),
                    ..Default::default()
                },
            )
            .expect("upsert works");
        }
        let rows = list_tracks_for_artist(&db, "Nova Rae").expect("artist tracks list");
        let titles: Vec<&str> = rows
            .iter()
            .map(|row| row.title.as_deref().unwrap_or("<unknown>"))
            .collect();
        assert_eq!(
            titles,
            ["Day", "One", "Two"],
            "year, then album, then track number"
        );
        assert!(
            list_tracks_for_artist(&db, "Nobody")
                .expect("unknown artist lists")
                .is_empty()
        );
    }

    #[test]
    fn date_sort_puts_newest_first_and_unknown_last() {
        let mut db = open_memory().expect("in-memory opens");
        for (path, title, album, year) in [
            ("/music/old.flac", "Old", "Old Album", Some(2010)),
            ("/music/new.flac", "New", "New Album", Some(2024)),
            ("/music/none.flac", "None", "None Album", None),
        ] {
            upsert_track(
                &mut db,
                &NewTrack {
                    path: path.to_owned(),
                    stable_key: title.to_owned(),
                    title: Some(title.to_owned()),
                    album: Some(album.to_owned()),
                    year,
                    ..Default::default()
                },
            )
            .expect("upsert works");
        }
        let rows = list_tracks_capped(&db, 10, TrackSort::Date, SortDir::Desc).expect("date sort");
        let titles: Vec<_> = rows.iter().filter_map(|row| row.title.as_deref()).collect();
        assert_eq!(titles, ["New", "Old", "None"]);
        let albums = list_albums(&db, AlbumSort::Date, SortDir::Desc).expect("album date sort");
        assert_eq!(albums[0].year, Some(2024));
        assert_eq!(TrackSort::from_key("nope"), TrackSort::Title);
        assert_eq!(AlbumSort::from_key("date").as_key(), "date");
        assert_eq!(ArtistSort::from_key("songs").as_key(), "songs");
    }

    #[test]
    fn remove_root_collects_only_its_rows() {
        let mut db = open_memory().expect("in-memory opens");
        for path in ["/music/a.flac", "/music/sub/b.flac", "/podcasts/c.flac"] {
            upsert_track(&mut db, &new_track(path)).expect("upsert works");
        }
        // `%` and `_` in real paths must not act as LIKE wildcards.
        upsert_track(&mut db, &new_track("/music/100%_hits/d.flac")).expect("upsert works");
        let removed = remove_library_root(&mut db, "/music").expect("remove works");
        assert_eq!(removed, 3);
        let tracks = list_tracks(&db).expect("list works");
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].path, "/podcasts/c.flac");
        // Search rows cascade through the trigger; survivors still search.
        assert!(
            search_track_ids(&db, "b.flac")
                .expect("search works")
                .is_empty()
        );
        assert_eq!(
            search_track_ids(&db, "c.flac").expect("search works").len(),
            1
        );
    }

    #[test]
    fn deleting_a_track_clears_its_search_row() {
        let mut db = open_memory().expect("in-memory opens");
        let mut track = new_track("/music/gone.flac");
        track.title = Some("Gone".to_owned());
        upsert_track(&mut db, &track).expect("upsert works");
        assert_eq!(
            search_track_ids(&db, "gone").expect("search works").len(),
            1
        );
        db.execute("DELETE FROM tracks WHERE path = '/music/gone.flac'", [])
            .expect("delete works");
        assert!(
            search_track_ids(&db, "gone")
                .expect("search works")
                .is_empty()
        );
    }
}
