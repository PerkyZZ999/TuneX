//! Durable playback-state store (C14): saved Up Next snapshot, play
//! history, and the recently-played rail query. Owns the restart story at
//! the storage layer; restore policy lives with its callers.

use rusqlite::Connection;

use super::db_read::{ALBUM_LIST_GROUP, ALBUM_LIST_SELECT, album_from_list_row};
use super::{AlbumRow, db_error};
use tunex_core::Result;
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
