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

/// Current schema version (v3: missing flag + file identity for reconcile).
pub const SCHEMA_VERSION: usize = 3;

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

/// Versioned migrations, oldest first. Append-only: never edit a landed
/// migration, always add a new one.
fn migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(V1_SCHEMA), M::up(V2_SCHEMA), M::up(V3_SCHEMA)])
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
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
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
    /// Whether the row is currently flagged missing.
    pub missing: bool,
}

/// Open an in-memory database (tests, throwaway harnesses).
///
/// # Errors
///
/// Returns [`Error::Database`] when the schema migrations fail.
pub fn open_memory() -> Result<Connection> {
    let mut db = Connection::open_in_memory().map_err(|err| db_error(&err))?;
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
    migrations()
        .to_latest(&mut db)
        .map_err(|err| Error::Database(err.to_string()))?;
    Ok(db)
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

/// Register a library root (idempotent).
///
/// # Errors
///
/// Returns [`Error::Database`] when the insert fails.
pub fn add_root(db: &mut Connection, path: &str) -> Result<()> {
    db.execute(
        "INSERT OR IGNORE INTO library_roots(path) VALUES (?1)",
        [path],
    )
    .map_err(|err| db_error(&err))?;
    Ok(())
}

/// Resolve a lookup name to its row id, creating it on first use.
fn get_or_create(db: &Connection, table: &str, name: &str) -> Result<i64, rusqlite::Error> {
    // Table names are internal constants from the call sites below, never
    // user input; dynamic SQL here is safe by construction.
    db.execute(
        &format!("INSERT OR IGNORE INTO {table}(name) VALUES (?1)"),
        [name],
    )?;
    db.query_row(
        &format!("SELECT id FROM {table} WHERE name = ?1"),
        [name],
        |row| row.get(0),
    )
}

/// Human filename for the search index (falls back to the full path when the
/// path has no file name component).
fn filename_of(path: &str) -> String {
    std::path::Path::new(path).file_name().map_or_else(
        || path.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Resolve an optional lookup name (artist/genre) to its row id.
fn resolve_lookup_id(
    transaction: &rusqlite::Transaction<'_>,
    table: &str,
    name: Option<&str>,
) -> Result<Option<i64>> {
    name.map(|name| get_or_create(transaction, table, name))
        .transpose()
        .map_err(|err| db_error(&err))
}

/// Find an album row by title + owner, creating it on first use.
///
/// `owner_id` is `None` for albums with no known artist; `IS` comparison
/// matches `NULL` owners correctly, so both cases share one query pair.
fn find_or_create_album(
    transaction: &rusqlite::Transaction<'_>,
    title: &str,
    owner_id: Option<i64>,
    year: Option<i64>,
) -> Result<Option<i64>> {
    transaction
        .query_row(
            "SELECT id FROM albums WHERE title = ?1 AND artist_id IS ?2",
            rusqlite::params![title, owner_id],
            |row| row.get(0),
        )
        .or_else(|_| {
            transaction.execute(
                "INSERT INTO albums(title, artist_id, year) VALUES (?1, ?2, ?3)",
                rusqlite::params![title, owner_id, year],
            )?;
            transaction.query_row(
                "SELECT id FROM albums WHERE title = ?1 AND artist_id IS ?2",
                rusqlite::params![title, owner_id],
                |row| row.get(0),
            )
        })
        .map(Some)
        .map_err(|err| db_error(&err))
}

/// Resolve an album title + owner to its row id, creating it on first use.
///
/// The album belongs to the album artist when known, else the track artist;
/// compilations resolve under their own name either way.
fn resolve_album_id(
    transaction: &rusqlite::Transaction<'_>,
    title: Option<&str>,
    owner: Option<&str>,
    year: Option<i64>,
) -> Result<Option<i64>> {
    let Some(title) = title else {
        return Ok(None);
    };
    let owner_id = owner
        .map(|name| get_or_create(transaction, "artists", name))
        .transpose()
        .map_err(|err| db_error(&err))?;
    find_or_create_album(transaction, title, owner_id, year)
}

/// Insert a track or refresh it when rescanned, resolving lookup ids and
/// refreshing the search index in the same transaction.
///
/// # Errors
///
/// Returns [`Error::Database`] when any write fails (the transaction rolls
/// back, so tracks and index never diverge).
pub fn upsert_track(db: &mut Connection, track: &NewTrack) -> Result<()> {
    let transaction = db.transaction().map_err(|err| db_error(&err))?;
    let artist_id = resolve_lookup_id(&transaction, "artists", track.artist.as_deref())?;
    let album_owner = track.album_artist.as_deref().or(track.artist.as_deref());
    let album_id = resolve_album_id(
        &transaction,
        track.album.as_deref(),
        album_owner,
        track.year,
    )?;
    let genre_id = resolve_lookup_id(&transaction, "genres", track.genre.as_deref())?;
    transaction
        .execute(
            "INSERT INTO tracks(path, title, stable_key, artist_id, album_id, genre_id,
                                composer, year, track_number, disc_number, duration_ms,
                                file_id, missing)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0)
             ON CONFLICT(path) DO UPDATE SET
                title = excluded.title, stable_key = excluded.stable_key,
                artist_id = excluded.artist_id, album_id = excluded.album_id,
                genre_id = excluded.genre_id, composer = excluded.composer,
                year = excluded.year, track_number = excluded.track_number,
                disc_number = excluded.disc_number, duration_ms = excluded.duration_ms,
                file_id = excluded.file_id, missing = 0",
            rusqlite::params![
                track.path,
                track.title,
                track.stable_key,
                artist_id,
                album_id,
                genre_id,
                track.composer,
                track.year,
                track.track_number,
                track.disc_number,
                track.duration_ms,
                track.file_id,
            ],
        )
        .map_err(|err| db_error(&err))?;
    let id: i64 = transaction
        .query_row(
            "SELECT id FROM tracks WHERE path = ?1",
            [&track.path],
            |row| row.get(0),
        )
        .map_err(|err| db_error(&err))?;
    transaction
        .execute(
            "INSERT OR REPLACE INTO track_search_docs(
                id, title, artist, album, album_artist, composer, genre, filename)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                id,
                track.title.as_deref().unwrap_or(""),
                track.artist.as_deref().unwrap_or(""),
                track.album.as_deref().unwrap_or(""),
                track.album_artist.as_deref().unwrap_or(""),
                track.composer.as_deref().unwrap_or(""),
                track.genre.as_deref().unwrap_or(""),
                filename_of(&track.path),
            ],
        )
        .map_err(|err| db_error(&err))?;
    transaction.commit().map_err(|err| db_error(&err))?;
    Ok(())
}

/// Shared track-list projection: resolved display names over the lookup
/// joins. Callers append their own `WHERE` / `ORDER BY`.
const TRACK_LIST_SELECT: &str =
    "SELECT tracks.id AS id, tracks.path AS path, tracks.title AS title,
                    tracks.stable_key AS stable_key,
                    artists.name AS artist, albums.title AS album, genres.name AS genre,
                    tracks.composer AS composer, tracks.year AS year,
                    tracks.track_number AS track_number, tracks.disc_number AS disc_number,
                    tracks.duration_ms AS duration_ms, tracks.missing AS missing
             FROM tracks
             LEFT JOIN artists ON artists.id = tracks.artist_id
             LEFT JOIN albums ON albums.id = tracks.album_id
             LEFT JOIN genres ON genres.id = tracks.genre_id";

/// All indexed tracks ordered by path, with resolved display names.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_tracks(db: &Connection) -> Result<Vec<TrackRow>> {
    let mut statement = db
        .prepare(&format!("{TRACK_LIST_SELECT} ORDER BY tracks.path"))
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([], TrackRow::from_row)
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<TrackRow>>>()
        .map_err(|err| db_error(&err))
}

/// One artist row for the browse view: name plus collection counts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtistRow {
    /// Artist name.
    pub name: String,
    /// Albums attributed to this artist.
    pub album_count: i64,
    /// Tracks attributed to this artist.
    pub track_count: i64,
}

/// One album row for the browse view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlbumRow {
    /// Database row id (drill-down key for [`list_tracks_in_album`]).
    pub id: i64,
    /// Album title.
    pub title: String,
    /// Attributed artist, if any.
    pub artist: Option<String>,
    /// Release year, if known.
    pub year: Option<i64>,
    /// Indexed tracks on this album.
    pub track_count: i64,
}

/// Every attributed artist with album/track counts, ordered by name.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_artists(db: &Connection) -> Result<Vec<ArtistRow>> {
    let mut statement = db
        .prepare(
            "SELECT artists.name AS name,
                    COUNT(DISTINCT tracks.album_id) AS album_count,
                    COUNT(tracks.id) AS track_count
             FROM artists
             LEFT JOIN tracks ON tracks.artist_id = artists.id
             GROUP BY artists.id
             ORDER BY artists.name COLLATE NOCASE",
        )
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([], |row| {
            Ok(ArtistRow {
                name: row.get("name")?,
                album_count: row.get("album_count")?,
                track_count: row.get("track_count")?,
            })
        })
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<ArtistRow>>>()
        .map_err(|err| db_error(&err))
}

/// Every album with artist, year, and track count, ordered by title.///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_albums(db: &Connection) -> Result<Vec<AlbumRow>> {
    let mut statement = db
        .prepare(
            "SELECT albums.id AS id, albums.title AS title,
                    artists.name AS artist, albums.year AS year,
                    COUNT(tracks.id) AS track_count
             FROM albums
             LEFT JOIN artists ON artists.id = albums.artist_id
             LEFT JOIN tracks ON tracks.album_id = albums.id
             GROUP BY albums.id
             ORDER BY albums.title COLLATE NOCASE",
        )
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([], |row| {
            Ok(AlbumRow {
                id: row.get("id")?,
                title: row.get("title")?,
                artist: row.get("artist")?,
                year: row.get("year")?,
                track_count: row.get("track_count")?,
            })
        })
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<AlbumRow>>>()
        .map_err(|err| db_error(&err))
}

/// Tracks on one album in disc/track order (untagged numbers sort last).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_tracks_in_album(db: &Connection, album_id: i64) -> Result<Vec<TrackRow>> {
    let mut statement = db
        .prepare(&format!(
            "{TRACK_LIST_SELECT}
             WHERE tracks.album_id = ?1
             ORDER BY COALESCE(tracks.disc_number, 1),
                      COALESCE(tracks.track_number, 1000000),
                      tracks.path"
        ))
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([album_id], TrackRow::from_row)
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<TrackRow>>>()
        .map_err(|err| db_error(&err))
}

/// First `limit` tracks by path (songs tab over large libraries stays
/// bounded; full paging and search arrive in S3).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_tracks_capped(db: &Connection, limit: u32) -> Result<Vec<TrackRow>> {
    let mut statement = db
        .prepare(&format!(
            "{TRACK_LIST_SELECT} ORDER BY tracks.path LIMIT ?1"
        ))
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([limit], TrackRow::from_row)
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<TrackRow>>>()
        .map_err(|err| db_error(&err))
}

/// Every indexed row's reconcile identity, ordered by path.
///
/// The scanner loads this once per run and matches walked files against it:
/// same path is a refresh, same `file_id` under a new path is a rename,
/// anything left over is a deletion candidate (flagged missing, never
/// dropped — playlists and history keep pointing at the row).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn track_identities(db: &Connection) -> Result<Vec<TrackIdentity>> {
    let mut statement = db
        .prepare("SELECT id, path, file_id, missing FROM tracks ORDER BY path")
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([], |row| {
            Ok(TrackIdentity {
                id: row.get("id")?,
                path: row.get("path")?,
                file_id: row.get("file_id")?,
                missing: row.get("missing")?,
            })
        })
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<TrackIdentity>>>()
        .map_err(|err| db_error(&err))
}

/// Retarget a row at a new path after a rename: identity (row id, history
/// links) survives, the key and file identity refresh, and any missing flag
/// clears. Tag columns refresh on the following [`upsert_track`].
///
/// # Errors
///
/// Returns [`Error::Database`] when the update fails.
pub fn rename_track(db: &Connection, id: i64, path: &str, stable_key: &str) -> Result<()> {
    db.execute(
        "UPDATE tracks SET path = ?1, stable_key = ?2, missing = 0 WHERE id = ?3",
        rusqlite::params![path, stable_key, id],
    )
    .map_err(|err| db_error(&err))?;
    Ok(())
}

/// Flag or unflag a row as missing without deleting it.
///
/// # Errors
///
/// Returns [`Error::Database`] when the update fails.
pub fn set_missing(db: &Connection, id: i64, missing: bool) -> Result<()> {
    db.execute(
        "UPDATE tracks SET missing = ?1 WHERE id = ?2",
        rusqlite::params![missing, id],
    )
    .map_err(|err| db_error(&err))?;
    Ok(())
}

/// Remove a library root and garbage-collect its track rows (SPEC retention:
/// remove root → stop watch + GC orphans). Referencing playlists and history
/// keep dangling ids — rows are deleted, never rewritten. Returns the number
/// of track rows removed. FTS rows cascade through the `tracks_ad` trigger;
/// emptied artist/album/genre lookup rows linger harmlessly.
///
/// # Errors
///
/// Returns [`Error::Database`] when the deletes fail.
pub fn remove_library_root(db: &Connection, root: &str) -> Result<u64> {
    // `LIKE` metacharacters in real paths (`%`, `_`, `\`) must match
    // literally, or one root could garbage-collect its neighbor.
    let escaped = root
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let prefix = format!("{escaped}/%");
    let removed = db
        .execute(
            "DELETE FROM tracks WHERE path = ?1 OR path LIKE ?2 ESCAPE '\\'",
            rusqlite::params![root, prefix],
        )
        .map_err(|err| db_error(&err))?;
    db.execute("DELETE FROM library_roots WHERE path = ?1", [root])
        .map_err(|err| db_error(&err))?;
    Ok(u64::try_from(removed).unwrap_or(u64::MAX))
}

fn db_error(err: &rusqlite::Error) -> Error {
    Error::Database(err.to_string())
}

/// Prefix search over the FTS5 index, best matches first (BM25).
///
/// Single-term queries only in this slice; the S3 search controller builds
/// multi-term parsing on top. The term is quoted so filename punctuation
/// (dots, dashes) searches literally instead of erroring; results cap at
/// 200 rows per SPEC §11.3.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn search_track_ids(db: &Connection, term: &str) -> Result<Vec<i64>> {
    let mut statement = db
        .prepare(
            "SELECT rowid FROM track_search
             WHERE track_search MATCH ?1 ORDER BY bm25(track_search) LIMIT 200",
        )
        .map_err(|err| db_error(&err))?;
    let query = format!("\"{}\"*", term.replace('"', "\"\""));
    let rows = statement
        .query_map([query], |row| row.get(0))
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
    fn fresh_database_reports_schema_v3() {
        let db = open_memory().expect("in-memory opens");
        assert_eq!(schema_version(&db).expect("version reads"), SCHEMA_VERSION);
        assert_eq!(SCHEMA_VERSION, 3);
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
    fn v1_to_v2_migration_preserves_rows() {
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
        ])
        .to_latest(&mut db)
        .expect("v3 migrates");
        assert_eq!(schema_version(&db).expect("version reads"), 3);
        let tracks = list_tracks(&db).expect("list works");
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title.as_deref(), Some("Old"));
        assert_eq!(tracks[0].artist, None);
        // v3 defaults: surviving rows are present, not missing.
        assert!(!tracks[0].missing);
        // Backfill indexed the surviving row by path.
        assert_eq!(search_track_ids(&db, "old").expect("search works").len(), 1);
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
        let artists = list_artists(&db).expect("artists list");
        assert_eq!(artists.len(), 2);
        let nova = artists
            .iter()
            .find(|row| row.name == "Nova Rae")
            .expect("artist present");
        assert_eq!((nova.album_count, nova.track_count), (1, 2));

        let albums = list_albums(&db).expect("albums list");
        assert_eq!(albums.len(), 2);
        let tapes = albums
            .iter()
            .find(|row| row.title == "Night Tapes")
            .expect("album present");
        assert_eq!(tapes.artist.as_deref(), Some("Nova Rae"));
        assert_eq!(tapes.track_count, 2);

        let songs = list_tracks_in_album(&db, tapes.id).expect("album tracks list");
        assert_eq!(songs.len(), 2);
        assert_eq!(songs[0].title.as_deref(), Some("One"));
        assert_eq!(songs[1].title.as_deref(), Some("Two"));

        let capped = list_tracks_capped(&db, 2).expect("capped list");
        assert_eq!(capped.len(), 2);
        assert!(list_tracks_capped(&db, 0).expect("empty cap").is_empty());
    }

    #[test]
    fn remove_root_collects_only_its_rows() {
        let mut db = open_memory().expect("in-memory opens");
        for path in ["/music/a.flac", "/music/sub/b.flac", "/podcasts/c.flac"] {
            upsert_track(&mut db, &new_track(path)).expect("upsert works");
        }
        // `%` and `_` in real paths must not act as LIKE wildcards.
        upsert_track(&mut db, &new_track("/music/100%_hits/d.flac")).expect("upsert works");
        let removed = remove_library_root(&db, "/music").expect("remove works");
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
