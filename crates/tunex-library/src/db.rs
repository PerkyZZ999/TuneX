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
                                file_id, missing, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0, strftime('%s','now'))
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
    /// A track on this album to read artwork from (`None` for an album with
    /// no indexed tracks). Any track resolves the same folder art, and
    /// embedded covers on an album agree in practice — the grid needs one
    /// source, not all of them.
    pub art_source: Option<String>,
}

/// Songs-tab / folder-track browse order (V1-basic; L-006 advanced filters
/// stay out of scope). Keys match the IA labels the UI shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TrackSort {
    /// Title A–Z, then path.
    #[default]
    Title,
    /// Artist A–Z, then title.
    Artist,
    /// Album A–Z, then disc/track.
    Album,
    /// Year newest-first; untagged years last.
    Date,
}

impl TrackSort {
    /// Parse a UI sort key; unknown values fall back to [`TrackSort::Title`].
    #[must_use]
    pub fn from_key(key: &str) -> Self {
        match key {
            "artist" => Self::Artist,
            "album" => Self::Album,
            "date" => Self::Date,
            _ => Self::Title,
        }
    }

    /// Stable UI key for this order.
    #[must_use]
    pub fn as_key(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Artist => "artist",
            Self::Album => "album",
            Self::Date => "date",
        }
    }
}

/// Albums-tab browse order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlbumSort {
    /// Title A–Z.
    #[default]
    Title,
    /// Artist A–Z, then title.
    Artist,
    /// Year newest-first; untagged years last.
    Date,
}

impl AlbumSort {
    /// Parse a UI sort key; unknown values fall back to [`AlbumSort::Title`].
    #[must_use]
    pub fn from_key(key: &str) -> Self {
        match key {
            "artist" => Self::Artist,
            "date" => Self::Date,
            _ => Self::Title,
        }
    }

    /// Stable UI key for this order.
    #[must_use]
    pub fn as_key(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Artist => "artist",
            Self::Date => "date",
        }
    }
}

/// Artists-tab browse order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ArtistSort {
    /// Name A–Z.
    #[default]
    Name,
    /// Song count, most first, then name.
    Songs,
}

impl ArtistSort {
    /// Parse a UI sort key; unknown values fall back to [`ArtistSort::Name`].
    #[must_use]
    pub fn from_key(key: &str) -> Self {
        match key {
            "songs" => Self::Songs,
            _ => Self::Name,
        }
    }

    /// Stable UI key for this order.
    #[must_use]
    pub fn as_key(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Songs => "songs",
        }
    }
}

/// One folder that directly contains indexed tracks (browse, not library-root
/// management).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderRow {
    /// Absolute directory path (drill-down key for [`list_tracks_in_folder`]).
    pub path: String,
    /// Last path component, for the row title.
    pub name: String,
    /// Indexed tracks whose parent directory is this folder.
    pub track_count: i64,
}

/// Shared album-list projection: attributed artist plus track counts.
/// Callers append their own `WHERE` before [`ALBUM_LIST_TAIL`].
const ALBUM_LIST_SELECT: &str = "SELECT albums.id AS id, albums.title AS title,
                    artists.name AS artist, albums.year AS year,
                    COUNT(tracks.id) AS track_count,
                    MIN(tracks.path) AS art_source
             FROM albums
             LEFT JOIN artists ON artists.id = albums.artist_id
             LEFT JOIN tracks ON tracks.album_id = albums.id";

/// Grouping shared by every album-list query (order is appended per sort).
const ALBUM_LIST_GROUP: &str = "GROUP BY albums.id";

/// Grouping + title order: search hydrates groups then reorders in memory.
const ALBUM_LIST_TAIL: &str = "GROUP BY albums.id ORDER BY albums.title COLLATE NOCASE";

/// Shared artist-list projection: album/track counts per name. Callers append
/// their own `WHERE` before [`ARTIST_LIST_TAIL`].
const ARTIST_LIST_SELECT: &str = "SELECT artists.name AS name,
                    COUNT(DISTINCT tracks.album_id) AS album_count,
                    COUNT(tracks.id) AS track_count
             FROM artists
             LEFT JOIN tracks ON tracks.artist_id = artists.id";

/// Grouping shared by every artist-list query (order is appended per sort).
const ARTIST_LIST_GROUP: &str = "GROUP BY artists.id";

/// Grouping + name order: search hydrates groups then reorders in memory.
const ARTIST_LIST_TAIL: &str = "GROUP BY artists.id ORDER BY artists.name COLLATE NOCASE";

/// Static `ORDER BY` for the songs browse (never interpolates user text).
fn track_order_sql(sort: TrackSort) -> &'static str {
    match sort {
        TrackSort::Title => "ORDER BY COALESCE(tracks.title, '') COLLATE NOCASE, tracks.path",
        TrackSort::Artist => {
            "ORDER BY COALESCE(artists.name, '') COLLATE NOCASE, COALESCE(tracks.title, '') COLLATE NOCASE, tracks.path"
        }
        TrackSort::Album => {
            "ORDER BY COALESCE(albums.title, '') COLLATE NOCASE, COALESCE(tracks.disc_number, 1), COALESCE(tracks.track_number, 1000000), tracks.path"
        }
        TrackSort::Date => {
            "ORDER BY COALESCE(tracks.year, 0) DESC, COALESCE(tracks.title, '') COLLATE NOCASE, tracks.path"
        }
    }
}

/// Static `ORDER BY` for the albums browse (never interpolates user text).
fn album_order_sql(sort: AlbumSort) -> &'static str {
    match sort {
        AlbumSort::Title => "ORDER BY albums.title COLLATE NOCASE",
        AlbumSort::Artist => {
            "ORDER BY COALESCE(artists.name, '') COLLATE NOCASE, albums.title COLLATE NOCASE"
        }
        AlbumSort::Date => "ORDER BY COALESCE(albums.year, 0) DESC, albums.title COLLATE NOCASE",
    }
}

/// Static `ORDER BY` for the artists browse (never interpolates user text).
fn artist_order_sql(sort: ArtistSort) -> &'static str {
    match sort {
        ArtistSort::Name => "ORDER BY artists.name COLLATE NOCASE",
        ArtistSort::Songs => "ORDER BY track_count DESC, artists.name COLLATE NOCASE",
    }
}

/// Every attributed artist with album/track counts, ordered by [`ArtistSort`].
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_artists(db: &Connection, sort: ArtistSort) -> Result<Vec<ArtistRow>> {
    let mut statement = db
        .prepare(&format!(
            "{ARTIST_LIST_SELECT} {ARTIST_LIST_GROUP} {}",
            artist_order_sql(sort)
        ))
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

/// Every album with artist, year, and track count, ordered by [`AlbumSort`].
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_albums(db: &Connection, sort: AlbumSort) -> Result<Vec<AlbumRow>> {
    let mut statement = db
        .prepare(&format!(
            "{ALBUM_LIST_SELECT} {ALBUM_LIST_GROUP} {}",
            album_order_sql(sort)
        ))
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([], |row| {
            Ok(AlbumRow {
                id: row.get("id")?,
                title: row.get("title")?,
                artist: row.get("artist")?,
                year: row.get("year")?,
                track_count: row.get("track_count")?,
                art_source: row.get("art_source")?,
            })
        })
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<AlbumRow>>>()
        .map_err(|err| db_error(&err))
}

/// One album by row id (landing-page header).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn album_by_id(db: &Connection, album_id: i64) -> Result<Option<AlbumRow>> {
    let mut statement = db
        .prepare(&format!(
            "{ALBUM_LIST_SELECT} WHERE albums.id = ?1 {ALBUM_LIST_GROUP}"
        ))
        .map_err(|err| db_error(&err))?;
    let mut rows = statement
        .query_map([album_id], album_from_list_row)
        .map_err(|err| db_error(&err))?;
    rows.next().transpose().map_err(|err| db_error(&err))
}

/// Total duration of an album's indexed tracks, in milliseconds.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn album_duration_ms(db: &Connection, album_id: i64) -> Result<i64> {
    db.query_row(
        "SELECT COALESCE(SUM(duration_ms), 0) FROM tracks WHERE album_id = ?1",
        [album_id],
        |row| row.get(0),
    )
    .map_err(|err| db_error(&err))
}

/// Albums attributed to one artist, optionally excluding one album id
/// (“more by this artist”).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_albums_for_artist(
    db: &Connection,
    artist: &str,
    exclude_id: Option<i64>,
) -> Result<Vec<AlbumRow>> {
    let sql = if exclude_id.is_some() {
        format!(
            "{ALBUM_LIST_SELECT}
             WHERE artists.name = ?1 AND albums.id != ?2
             {ALBUM_LIST_GROUP}
             ORDER BY COALESCE(albums.year, 0) DESC, albums.title COLLATE NOCASE"
        )
    } else {
        format!(
            "{ALBUM_LIST_SELECT}
             WHERE artists.name = ?1
             {ALBUM_LIST_GROUP}
             ORDER BY COALESCE(albums.year, 0) DESC, albums.title COLLATE NOCASE"
        )
    };
    let mut statement = db.prepare(&sql).map_err(|err| db_error(&err))?;
    let mapped = if let Some(exclude) = exclude_id {
        statement.query_map(rusqlite::params![artist, exclude], album_from_list_row)
    } else {
        statement.query_map(rusqlite::params![artist], album_from_list_row)
    }
    .map_err(|err| db_error(&err))?;
    mapped
        .collect::<rusqlite::Result<Vec<AlbumRow>>>()
        .map_err(|err| db_error(&err))
}

fn album_from_list_row(row: &Row<'_>) -> rusqlite::Result<AlbumRow> {
    Ok(AlbumRow {
        id: row.get("id")?,
        title: row.get("title")?,
        artist: row.get("artist")?,
        year: row.get("year")?,
        track_count: row.get("track_count")?,
        art_source: row.get("art_source")?,
    })
}

/// One artist by name (landing-page header).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn artist_by_name(db: &Connection, name: &str) -> Result<Option<ArtistRow>> {
    let mut statement = db
        .prepare(&format!(
            "{ARTIST_LIST_SELECT} WHERE artists.name = ?1 {ARTIST_LIST_GROUP}"
        ))
        .map_err(|err| db_error(&err))?;
    let mut rows = statement
        .query_map([name], |row| {
            Ok(ArtistRow {
                name: row.get("name")?,
                album_count: row.get("album_count")?,
                track_count: row.get("track_count")?,
            })
        })
        .map_err(|err| db_error(&err))?;
    rows.next().transpose().map_err(|err| db_error(&err))
}

/// Display name used when a genre or composer tag is missing.
pub const UNKNOWN_FACET: &str = "Unknown";

/// One genre or composer group for the library rail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FacetRow {
    /// Display name (`Unknown` when untagged).
    pub name: String,
    /// Indexed tracks in this group.
    pub track_count: i64,
}

/// Every genre with a track count. Untagged rows group as [`UNKNOWN_FACET`].
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_genres(db: &Connection) -> Result<Vec<FacetRow>> {
    list_facets(
        db,
        "SELECT COALESCE(NULLIF(genres.name, ''), 'Unknown') AS name,
                COUNT(tracks.id) AS track_count
         FROM tracks
         LEFT JOIN genres ON genres.id = tracks.genre_id
         GROUP BY 1
         ORDER BY name COLLATE NOCASE",
    )
}

/// Every composer with a track count. Untagged rows group as [`UNKNOWN_FACET`].
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_composers(db: &Connection) -> Result<Vec<FacetRow>> {
    list_facets(
        db,
        "SELECT CASE WHEN tracks.composer IS NULL OR TRIM(tracks.composer) = ''
                     THEN 'Unknown' ELSE tracks.composer END AS name,
                COUNT(tracks.id) AS track_count
         FROM tracks
         GROUP BY 1
         ORDER BY name COLLATE NOCASE",
    )
}

fn list_facets(db: &Connection, sql: &str) -> Result<Vec<FacetRow>> {
    let mut statement = db.prepare(sql).map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([], |row| {
            Ok(FacetRow {
                name: row.get("name")?,
                track_count: row.get("track_count")?,
            })
        })
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<FacetRow>>>()
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

/// Tracks by one artist in album order (release year, album title, then
/// disc/track), for artist enqueue. Untagged years sort last.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_tracks_for_artist(db: &Connection, artist: &str) -> Result<Vec<TrackRow>> {
    let mut statement = db
        .prepare(&format!(
            "{TRACK_LIST_SELECT}
             WHERE artists.name = ?1
             ORDER BY COALESCE(tracks.year, 9999),
                      albums.title COLLATE NOCASE,
                      COALESCE(tracks.disc_number, 1),
                      COALESCE(tracks.track_number, 1000000),
                      tracks.path"
        ))
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([artist], TrackRow::from_row)
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<TrackRow>>>()
        .map_err(|err| db_error(&err))
}

/// Tracks in one genre group (untagged rows live under [`UNKNOWN_FACET`]).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_tracks_for_genre(db: &Connection, name: &str) -> Result<Vec<TrackRow>> {
    let sql = if name == UNKNOWN_FACET {
        format!(
            "{TRACK_LIST_SELECT}
             WHERE tracks.genre_id IS NULL OR genres.name IS NULL OR TRIM(genres.name) = ''
             ORDER BY COALESCE(tracks.title, '') COLLATE NOCASE, tracks.path"
        )
    } else {
        format!(
            "{TRACK_LIST_SELECT}
             WHERE genres.name = ?1
             ORDER BY COALESCE(tracks.title, '') COLLATE NOCASE, tracks.path"
        )
    };
    query_named_tracks(db, &sql, name == UNKNOWN_FACET, name)
}

/// Tracks credited to one composer (untagged rows live under [`UNKNOWN_FACET`]).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_tracks_for_composer(db: &Connection, name: &str) -> Result<Vec<TrackRow>> {
    let sql = if name == UNKNOWN_FACET {
        format!(
            "{TRACK_LIST_SELECT}
             WHERE tracks.composer IS NULL OR TRIM(tracks.composer) = ''
             ORDER BY COALESCE(tracks.title, '') COLLATE NOCASE, tracks.path"
        )
    } else {
        format!(
            "{TRACK_LIST_SELECT}
             WHERE tracks.composer = ?1
             ORDER BY COALESCE(tracks.title, '') COLLATE NOCASE, tracks.path"
        )
    };
    query_named_tracks(db, &sql, name == UNKNOWN_FACET, name)
}

fn query_named_tracks(
    db: &Connection,
    sql: &str,
    untagged: bool,
    name: &str,
) -> Result<Vec<TrackRow>> {
    let mut statement = db.prepare(sql).map_err(|err| db_error(&err))?;
    let rows = if untagged {
        statement.query_map([], TrackRow::from_row)
    } else {
        statement.query_map([name], TrackRow::from_row)
    }
    .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<TrackRow>>>()
        .map_err(|err| db_error(&err))
}

/// First `limit` tracks ordered by [`TrackSort`] (songs tab over large
/// libraries stays bounded; full paging stays with search).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_tracks_capped(db: &Connection, limit: u32, sort: TrackSort) -> Result<Vec<TrackRow>> {
    let mut statement = db
        .prepare(&format!(
            "{TRACK_LIST_SELECT} {} LIMIT ?1",
            track_order_sql(sort)
        ))
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([limit], TrackRow::from_row)
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<TrackRow>>>()
        .map_err(|err| db_error(&err))
}

/// Distinct parent directories of indexed tracks, ordered by display name.
///
/// This is the Folders *browse* list (tracks grouped by the folder they live
/// in). Library-root add/remove stays on [`crate::db::remove_library_root`]
/// / config, not here.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_track_folders(db: &Connection) -> Result<Vec<FolderRow>> {
    let mut statement = db
        .prepare("SELECT path FROM tracks")
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|err| db_error(&err))?;
    let mut counts = std::collections::BTreeMap::<String, i64>::new();
    for path in rows {
        let path = path.map_err(|err| db_error(&err))?;
        let Some(parent) = std::path::Path::new(&path).parent() else {
            continue;
        };
        let Some(parent) = parent.to_str() else {
            continue;
        };
        if parent.is_empty() {
            continue;
        }
        *counts.entry(parent.to_owned()).or_insert(0) += 1;
    }
    let mut rows: Vec<FolderRow> = counts
        .into_iter()
        .map(|(path, track_count)| FolderRow {
            name: folder_display_name(&path),
            path,
            track_count,
        })
        .collect();
    rows.sort_by_key(|row| (row.name.to_lowercase(), row.path.clone()));
    Ok(rows)
}

/// Tracks whose parent directory is `folder`, ordered by [`TrackSort`] and
/// capped at `limit`. Nested subfolders are listed separately.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_tracks_in_folder(
    db: &Connection,
    folder: &str,
    limit: u32,
    sort: TrackSort,
) -> Result<Vec<TrackRow>> {
    let prefix = format!("{}/%", like_escaped(folder));
    let nested = format!("{}/%/%", like_escaped(folder));
    let mut statement = db
        .prepare(&format!(
            "{TRACK_LIST_SELECT}
             WHERE tracks.path LIKE ?1 ESCAPE '\\'
               AND tracks.path NOT LIKE ?2 ESCAPE '\\'
             {} LIMIT ?3",
            track_order_sql(sort)
        ))
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map(rusqlite::params![prefix, nested, limit], TrackRow::from_row)
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<TrackRow>>>()
        .map_err(|err| db_error(&err))
}

/// Last path component for a folder row title.
fn folder_display_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_owned()
}

/// Escape `\`, `%`, and `_` so a `LIKE` prefix matches the path literally.
fn like_escaped(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
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
        .prepare("SELECT id, path, file_id, stable_key, missing FROM tracks ORDER BY path")
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([], |row| {
            Ok(TrackIdentity {
                id: row.get("id")?,
                path: row.get("path")?,
                file_id: row.get("file_id")?,
                stable_key: row.get("stable_key")?,
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

/// Delete one indexed track by row id. Playlist links keep the id and
/// resolve as dangling (D-009). FTS rows cascade through `tracks_ad`.
/// Returns whether a row was removed.
///
/// # Errors
///
/// Returns [`Error::Database`] when the delete fails.
pub fn delete_track(db: &Connection, id: i64) -> Result<bool> {
    let removed = db
        .execute("DELETE FROM tracks WHERE id = ?1", [id])
        .map_err(|err| db_error(&err))?;
    Ok(removed > 0)
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
    let prefix = format!("{}/{}", like_escaped(root), "%");
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

/// Prefix search over the FTS5 index, best matches first (BM25).
///
/// Single-term queries only; the S3 search module builds multi-term parsing
/// on top. The term is quoted so filename punctuation (dots, dashes)
/// searches literally instead of erroring; results cap at [`GROUP_LIMIT`]
/// rows per SPEC §11.3.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn search_track_ids(db: &Connection, term: &str) -> Result<Vec<i64>> {
    matched_ids(db, &super::search::match_query([term]))
}

/// Hydrate track ids to rows with resolved display names. Returned in index
/// order — callers needing rank order reorder in memory.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub(crate) fn tracks_by_ids(db: &Connection, ids: &[i64]) -> Result<Vec<TrackRow>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut statement = db
        .prepare(&format!(
            "{TRACK_LIST_SELECT} WHERE tracks.id IN ({})",
            placeholders(ids.len())
        ))
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map(
            rusqlite::params_from_iter(ids.iter().copied()),
            TrackRow::from_row,
        )
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<TrackRow>>>()
        .map_err(|err| db_error(&err))
}

/// One indexed track by row id (Up Next enqueue from row identity).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn track_by_id(db: &Connection, id: i64) -> Result<Option<TrackRow>> {
    Ok(tracks_by_ids(db, &[id])?.pop())
}

/// One indexed track by canonical path (session restore).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn track_by_path(db: &Connection, path: &str) -> Result<Option<TrackRow>> {
    let mut statement = db
        .prepare(&format!("{TRACK_LIST_SELECT} WHERE tracks.path = ?1"))
        .map_err(|err| db_error(&err))?;
    let mut rows = statement
        .query_map([path], TrackRow::from_row)
        .map_err(|err| db_error(&err))?;
    rows.next().transpose().map_err(|err| db_error(&err))
}

/// Album rows for the given titles that own at least one of `track_ids`
/// (same-titled albums by different artists all resolve, but only when their
/// own tracks matched — a shared title never drags in an unrelated album).
/// Returned in title order — callers needing rank order reorder in memory.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub(crate) fn albums_for_tracks(
    db: &Connection,
    titles: &[String],
    track_ids: &[i64],
) -> Result<Vec<AlbumRow>> {
    if titles.is_empty() || track_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut statement = db
        .prepare(&format!(
            "{ALBUM_LIST_SELECT}
             WHERE albums.title IN ({}) AND albums.id IN (
                SELECT album_id FROM tracks WHERE tracks.id IN ({})
             )
             {ALBUM_LIST_TAIL}",
            placeholders(titles.len()),
            placeholders(track_ids.len())
        ))
        .map_err(|err| db_error(&err))?;
    // Mixed bindings (titles then ids) go through `Value`: the two IN-lists
    // share one positional parameter sequence.
    let params: Vec<rusqlite::types::Value> = titles
        .iter()
        .map(|title| rusqlite::types::Value::Text(title.clone()))
        .chain(track_ids.iter().copied().map(rusqlite::types::Value::from))
        .collect();
    let rows = statement
        .query_map(rusqlite::params_from_iter(params), |row| {
            Ok(AlbumRow {
                id: row.get("id")?,
                title: row.get("title")?,
                artist: row.get("artist")?,
                year: row.get("year")?,
                track_count: row.get("track_count")?,
                art_source: row.get("art_source")?,
            })
        })
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<AlbumRow>>>()
        .map_err(|err| db_error(&err))
}

/// Artist rows for the given names (names are unique, so at most one row per
/// name). Returned in name order — callers needing rank order reorder.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub(crate) fn artists_by_names(db: &Connection, names: &[String]) -> Result<Vec<ArtistRow>> {
    if names.is_empty() {
        return Ok(Vec::new());
    }
    let mut statement = db
        .prepare(&format!(
            "{ARTIST_LIST_SELECT} WHERE artists.name IN ({}) {ARTIST_LIST_TAIL}",
            placeholders(names.len())
        ))
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map(
            rusqlite::params_from_iter(names.iter().map(String::as_str)),
            |row| {
                Ok(ArtistRow {
                    name: row.get("name")?,
                    album_count: row.get("album_count")?,
                    track_count: row.get("track_count")?,
                })
            },
        )
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<ArtistRow>>>()
        .map_err(|err| db_error(&err))
}

/// One persisted Up Next row (URI plus display fields).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedQueueItem {
    /// Playback URI (`file://` for local tracks).
    pub uri: String,
    /// Library row id when the item came from the index.
    pub track_id: Option<i64>,
    /// Display title.
    pub title: String,
    /// Display artist, when tagged.
    pub artist: Option<String>,
    /// Display album, when tagged.
    pub album: Option<String>,
    /// Tagged duration, when known.
    pub duration_ms: Option<i64>,
}

/// Saved Up Next: ordered items, cursor, and paused position.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SavedQueue {
    /// Items in play order.
    pub items: Vec<SavedQueueItem>,
    /// Cursor into `items` (0 when empty).
    pub cursor: usize,
    /// Paused position of the current item, milliseconds.
    pub position_ms: u64,
}

/// Replace the persisted Up Next snapshot.
///
/// # Errors
///
/// Returns [`Error::Database`] when the write fails.
pub fn save_playback_queue(
    db: &mut Connection,
    items: &[SavedQueueItem],
    cursor: usize,
    position_ms: u64,
) -> Result<()> {
    let transaction = db.transaction().map_err(|err| db_error(&err))?;
    transaction
        .execute("DELETE FROM playback_queue", [])
        .map_err(|err| db_error(&err))?;
    {
        let mut insert = transaction
            .prepare(
                "INSERT INTO playback_queue(position, uri, track_id, title, artist, album, duration_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .map_err(|err| db_error(&err))?;
        for (position, item) in items.iter().enumerate() {
            insert
                .execute(rusqlite::params![
                    i64::try_from(position).unwrap_or(i64::MAX),
                    item.uri,
                    item.track_id,
                    item.title,
                    item.artist,
                    item.album,
                    item.duration_ms,
                ])
                .map_err(|err| db_error(&err))?;
        }
    }
    transaction
        .execute(
            "INSERT INTO playback_queue_meta(id, cursor, position_ms) VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET cursor = excluded.cursor, position_ms = excluded.position_ms",
            rusqlite::params![
                i64::try_from(cursor).unwrap_or(0),
                i64::try_from(position_ms).unwrap_or(0),
            ],
        )
        .map_err(|err| db_error(&err))?;
    transaction.commit().map_err(|err| db_error(&err))?;
    Ok(())
}

/// Load the persisted Up Next snapshot (empty when never saved).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn load_playback_queue(db: &Connection) -> Result<SavedQueue> {
    let mut statement = db
        .prepare(
            "SELECT uri, track_id, title, artist, album, duration_ms
             FROM playback_queue ORDER BY position",
        )
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([], |row| {
            Ok(SavedQueueItem {
                uri: row.get(0)?,
                track_id: row.get(1)?,
                title: row.get(2)?,
                artist: row.get(3)?,
                album: row.get(4)?,
                duration_ms: row.get(5)?,
            })
        })
        .map_err(|err| db_error(&err))?;
    let items = rows
        .collect::<rusqlite::Result<Vec<SavedQueueItem>>>()
        .map_err(|err| db_error(&err))?;
    let meta: rusqlite::Result<(i64, i64)> = db.query_row(
        "SELECT cursor, position_ms FROM playback_queue_meta WHERE id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );
    let (cursor, position_ms) = match meta {
        Ok((cursor, position_ms)) => (
            usize::try_from(cursor).unwrap_or(0),
            u64::try_from(position_ms).unwrap_or(0),
        ),
        Err(rusqlite::Error::QueryReturnedNoRows) => (0, 0),
        Err(err) => return Err(db_error(&err)),
    };
    Ok(SavedQueue {
        items,
        cursor,
        position_ms,
    })
}

/// Append one play. Restores must not call this (same rule as notifications).
///
/// # Errors
///
/// Returns [`Error::Database`] when the insert fails.
pub fn record_play(db: &Connection, track_id: Option<i64>, uri: &str) -> Result<()> {
    db.execute(
        "INSERT INTO play_history(track_id, uri, played_at)
         VALUES (?1, ?2, strftime('%s','now'))",
        rusqlite::params![track_id, uri],
    )
    .map_err(|err| db_error(&err))?;
    Ok(())
}

/// Distinct albums from play history, newest first, capped.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_recently_played_albums(db: &Connection, limit: i64) -> Result<Vec<AlbumRow>> {
    let sql = format!(
        "{ALBUM_LIST_SELECT}
         INNER JOIN play_history ON play_history.track_id = tracks.id
         {ALBUM_LIST_GROUP}
         ORDER BY MAX(play_history.played_at) DESC
         LIMIT ?1"
    );
    let mut statement = db.prepare(&sql).map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([limit], album_from_list_row)
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<AlbumRow>>>()
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
        let artists = list_artists(&db, ArtistSort::Name).expect("artists list");
        assert_eq!(artists.len(), 2);
        let nova = artists
            .iter()
            .find(|row| row.name == "Nova Rae")
            .expect("artist present");
        assert_eq!((nova.album_count, nova.track_count), (1, 2));

        let albums = list_albums(&db, AlbumSort::Title).expect("albums list");
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

        let capped = list_tracks_capped(&db, 2, TrackSort::Title).expect("capped list");
        assert_eq!(capped.len(), 2);
        assert!(
            list_tracks_capped(&db, 0, TrackSort::Title)
                .expect("empty cap")
                .is_empty()
        );
        let by_title = list_tracks_capped(&db, 10, TrackSort::Title).expect("title sort");
        let titles: Vec<_> = by_title
            .iter()
            .filter_map(|row| row.title.as_deref())
            .collect();
        assert_eq!(titles, ["One", "Solo", "Two"]);
        let by_artist = list_tracks_capped(&db, 10, TrackSort::Artist).expect("artist sort");
        assert_eq!(by_artist[0].artist.as_deref(), Some("Nova Rae"));
        assert_eq!(by_artist[2].artist.as_deref(), Some("Solo Act"));

        let albums_by_artist = list_albums(&db, AlbumSort::Artist).expect("album artist sort");
        assert_eq!(albums_by_artist[0].artist.as_deref(), Some("Nova Rae"));
        let artists_by_songs = list_artists(&db, ArtistSort::Songs).expect("artist song sort");
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
        let in_tapes =
            list_tracks_in_folder(&db, &tapes.path, 10, TrackSort::Title).expect("folder tracks");
        let titles: Vec<_> = in_tapes
            .iter()
            .filter_map(|row| row.title.as_deref())
            .collect();
        assert_eq!(titles, ["One", "Two"]);
        let live = folders
            .iter()
            .find(|row| row.name == "Live")
            .expect("nested folder");
        let in_live =
            list_tracks_in_folder(&db, &live.path, 10, TrackSort::Title).expect("nested tracks");
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
        let rows = list_tracks_capped(&db, 10, TrackSort::Date).expect("date sort");
        let titles: Vec<_> = rows.iter().filter_map(|row| row.title.as_deref()).collect();
        assert_eq!(titles, ["New", "Old", "None"]);
        let albums = list_albums(&db, AlbumSort::Date).expect("album date sort");
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
