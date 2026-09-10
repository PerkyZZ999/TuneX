//! SQLite library index: versioned schema, WAL mode, minimal track rows.
//!
//! Migrations run from day one (`rusqlite_migration`): every schema change is
//! a numbered migration, so S2+ tables arrive without breaking S1 databases.
//! Only index data lives here — never audio.

use rusqlite::{Connection, Row};
use rusqlite_migration::{M, Migrations};
use tunex_core::{Error, Result};

/// Current schema version (v1: roots + tracks skeleton).
pub const SCHEMA_VERSION: usize = 1;

/// Versioned migrations, oldest first. Append-only: never edit a landed
/// migration, always add a new one.
fn migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(
        "CREATE TABLE library_roots(
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE
        );
        CREATE TABLE tracks(
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            title TEXT,
            stable_key TEXT NOT NULL
        );
        CREATE INDEX idx_tracks_stable_key ON tracks(stable_key);",
    )])
}

/// One indexed track row (metadata columns arrive with S2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackRow {
    /// Database row id (internal; renames keep history via `stable_key`).
    pub id: i64,
    /// Absolute filesystem path.
    pub path: String,
    /// Title when known; `None` until metadata extraction lands (S2).
    pub title: Option<String>,
    /// Stable identity (`path + mtime + size`); survives renames via reconcile.
    pub stable_key: String,
}

impl TrackRow {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            path: row.get("path")?,
            title: row.get("title")?,
            stable_key: row.get("stable_key")?,
        })
    }
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
pub fn add_root(db: &Connection, path: &str) -> Result<()> {
    db.execute(
        "INSERT OR IGNORE INTO library_roots(path) VALUES (?1)",
        [path],
    )
    .map_err(|err| db_error(&err))?;
    Ok(())
}

/// Insert a track or refresh its stable key when the file changed
/// (mini-reconcile; full watcher-driven reconcile lands in S2).
///
/// # Errors
///
/// Returns [`Error::Database`] when the upsert fails.
pub fn upsert_track(
    db: &Connection,
    path: &str,
    title: Option<&str>,
    stable_key: &str,
) -> Result<()> {
    db.execute(
        "INSERT INTO tracks(path, title, stable_key) VALUES (?1, ?2, ?3)
         ON CONFLICT(path) DO UPDATE SET stable_key = excluded.stable_key",
        rusqlite::params![path, title, stable_key],
    )
    .map_err(|err| db_error(&err))?;
    Ok(())
}

/// All indexed tracks ordered by path.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_tracks(db: &Connection) -> Result<Vec<TrackRow>> {
    let mut statement = db
        .prepare("SELECT id, path, title, stable_key FROM tracks ORDER BY path")
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([], TrackRow::from_row)
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<TrackRow>>>()
        .map_err(|err| db_error(&err))
}

fn db_error(err: &rusqlite::Error) -> Error {
    Error::Database(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_database_reports_schema_v1() {
        let db = open_memory().expect("in-memory opens");
        assert_eq!(schema_version(&db).expect("version reads"), SCHEMA_VERSION);
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
    fn roots_are_idempotent() {
        let db = open_memory().expect("in-memory opens");
        add_root(&db, "/music").expect("first add works");
        add_root(&db, "/music").expect("second add is a no-op");
        let count: i64 = db
            .query_row("SELECT COUNT(*) FROM library_roots", [], |row| row.get(0))
            .expect("count reads");
        assert_eq!(count, 1);
    }

    #[test]
    fn upsert_refreshes_changed_keys() {
        let db = open_memory().expect("in-memory opens");
        upsert_track(&db, "/music/a.flac", None, "k1").expect("insert works");
        upsert_track(&db, "/music/a.flac", None, "k2").expect("reinsert works");
        let tracks = list_tracks(&db).expect("list works");
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].stable_key, "k2");
        assert_eq!(tracks[0].title, None);
    }
}
