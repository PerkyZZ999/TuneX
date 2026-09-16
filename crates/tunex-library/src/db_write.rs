//! Index writes and identity (C14): roots, lookup resolution, track
//! upsert, rename retargeting, missing flags, root removal. Every mutation
//! lands in transactions; reads never touch these paths.

use rusqlite::Connection;

use super::db_read::like_escaped;
use super::{NewTrack, db_error};
use tunex_core::Result;
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
    // INSERT OR IGNORE like `get_or_create` above: two scan connections
    // indexing the same new album concurrently must converge, not fail one
    // upsert with a UNIQUE violation. (The busy timeout makes the loser wait
    // for the winner's commit instead of erroring.)
    transaction
        .execute(
            "INSERT OR IGNORE INTO albums(title, artist_id, year) VALUES (?1, ?2, ?3)",
            rusqlite::params![title, owner_id, year],
        )
        .map_err(|err| db_error(&err))?;
    transaction
        .query_row(
            "SELECT id FROM albums WHERE title = ?1 AND artist_id IS ?2",
            rusqlite::params![title, owner_id],
            |row| row.get(0),
        )
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

/// Retarget a renamed row and refresh its tags in one transaction: path,
/// identity, all tag columns, and the FTS document move together, so a crash
/// can never leave a retargeted path with stale tags or a stale search
/// filename. Prefer this over [`rename_track`] + [`upsert_track`] whenever
/// fresh tags are at hand; `rename_track` stays for path-only moves and
/// documents its own staleness caveat.
///
/// # Errors
///
/// Returns [`Error::Database`] when lookups or the rewrite fail.
pub fn retarget_track(db: &mut Connection, id: i64, track: &NewTrack) -> Result<()> {
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
            "UPDATE tracks SET path = ?1, title = ?2, stable_key = ?3,
                artist_id = ?4, album_id = ?5, genre_id = ?6, composer = ?7,
                year = ?8, track_number = ?9, disc_number = ?10,
                duration_ms = ?11, file_id = ?12, missing = 0,
                indexed_at = strftime('%s','now')
             WHERE id = ?13",
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
                id,
            ],
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

/// Retarget a row at a new path without refreshing tags. Standalone use
/// leaves tags and the FTS filename stale until the next re-upsert — prefer
/// [`retarget_track`] whenever fresh tags are at hand.
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
pub fn remove_library_root(db: &mut Connection, root: &str) -> Result<u64> {
    // `LIKE` metacharacters in real paths (`%`, `_`, `\`) must match
    // literally, or one root could garbage-collect its neighbor.
    let prefix = format!(
        "{root}{sep}%",
        root = like_escaped(root),
        sep = std::path::MAIN_SEPARATOR
    );
    // One transaction: a crash between the track sweep and the root-row
    // delete must not orphan rows or strand a trackless root.
    let transaction = db.transaction().map_err(|err| db_error(&err))?;
    let removed = transaction
        .execute(
            "DELETE FROM tracks WHERE path = ?1 OR path LIKE ?2 ESCAPE '\\'",
            rusqlite::params![root, prefix],
        )
        .map_err(|err| db_error(&err))?;
    transaction
        .execute("DELETE FROM library_roots WHERE path = ?1", [root])
        .map_err(|err| db_error(&err))?;
    transaction.commit().map_err(|err| db_error(&err))?;
    Ok(u64::try_from(removed).unwrap_or(u64::MAX))
}
