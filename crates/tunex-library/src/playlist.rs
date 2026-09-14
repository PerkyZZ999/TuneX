//! Local playlists over the library index (R-012).
//!
//! Playlists link tracks by row id inside one transaction per mutation, so
//! lists and entries never diverge. Names are unique (conflicts error
//! explicitly); entries carry a surrogate id so repeats stay distinct and
//! positions stay dense (0-based, renumbered on every structural change).
//! Deleted tracks leave entries dangling-as-missing (D-009): the entry
//! survives with no resolved row, and the UI renders it dimmed instead of
//! dropping it.

use rusqlite::Connection;
use tunex_core::{Error, Result};

use super::db::TrackRow;

/// One playlist with its entry count (dangling entries count — the UI dims
/// them in place rather than hiding them).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Playlist {
    /// Database row id.
    pub id: i64,
    /// Display name (unique).
    pub name: String,
    /// Ordered entries, including dangling ones.
    pub track_count: i64,
}

/// One ordered playlist entry with its resolved track, if any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaylistEntry {
    /// Entry row id (stable across renumbers; distinguishes repeats).
    pub id: i64,
    /// Linked track row id (dangling when the track is gone).
    pub track_id: i64,
    /// Zero-based position within the playlist.
    pub position: i64,
    /// Resolved track (`None` when the track row is gone; `missing` rows
    /// still resolve so the UI can dim them with a badge).
    pub track: Option<TrackRow>,
}

/// Create a playlist with a unique, non-blank name; returns its row id.
///
/// # Errors
///
/// Returns [`Error::Database`] when the name is blank or taken, or the
/// insert fails.
pub fn create_playlist(db: &Connection, name: &str) -> Result<i64> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Database(
            "playlist name must not be blank".to_owned(),
        ));
    }
    if playlist_id_by_name(db, name)?.is_some() {
        return Err(Error::Database(format!(
            "a playlist named \"{name}\" already exists"
        )));
    }
    db.execute("INSERT INTO playlists(name) VALUES (?1)", [name])
        .map_err(|err| db_error(&err))?;
    Ok(db.last_insert_rowid())
}

/// Rename a playlist (names stay unique and non-blank).
///
/// # Errors
///
/// Returns [`Error::Database`] when the playlist is gone, the name is blank
/// or taken, or the update fails.
pub fn rename_playlist(db: &Connection, id: i64, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Database(
            "playlist name must not be blank".to_owned(),
        ));
    }
    require_playlist(db, id)?;
    if playlist_id_by_name(db, name)?.is_some_and(|owner| owner != id) {
        return Err(Error::Database(format!(
            "a playlist named \"{name}\" already exists"
        )));
    }
    db.execute(
        "UPDATE playlists SET name = ?1 WHERE id = ?2",
        rusqlite::params![name, id],
    )
    .map_err(|err| db_error(&err))?;
    Ok(())
}

/// Delete a playlist and all its entries in one transaction.
///
/// # Errors
///
/// Returns [`Error::Database`] when the playlist is gone or the delete fails.
pub fn delete_playlist(db: &mut Connection, id: i64) -> Result<()> {
    require_playlist(db, id)?;
    let transaction = db.transaction().map_err(|err| db_error(&err))?;
    transaction
        .execute("DELETE FROM smart_playlists WHERE playlist_id = ?1", [id])
        .map_err(|err| db_error(&err))?;
    transaction
        .execute("DELETE FROM playlist_tracks WHERE playlist_id = ?1", [id])
        .map_err(|err| db_error(&err))?;
    transaction
        .execute("DELETE FROM playlists WHERE id = ?1", [id])
        .map_err(|err| db_error(&err))?;
    transaction.commit().map_err(|err| db_error(&err))?;
    Ok(())
}

/// Every playlist by name with entry counts.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_playlists(db: &Connection) -> Result<Vec<Playlist>> {
    let mut statement = db
        .prepare(
            "SELECT playlists.id AS id, playlists.name AS name,
                    COUNT(playlist_tracks.id) AS track_count
             FROM playlists
             LEFT JOIN playlist_tracks
                ON playlist_tracks.playlist_id = playlists.id
             GROUP BY playlists.id
             ORDER BY playlists.name COLLATE NOCASE",
        )
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([], |row| {
            Ok(Playlist {
                id: row.get("id")?,
                name: row.get("name")?,
                track_count: row.get("track_count")?,
            })
        })
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<Playlist>>>()
        .map_err(|err| db_error(&err))
}

/// Append a track to a playlist. The track must exist (dangling arises
/// later, through deletion — never at insert time).
///
/// # Errors
///
/// Returns [`Error::Database`] when the playlist or track is gone, or the
/// insert fails.
pub fn add_to_playlist(db: &mut Connection, playlist_id: i64, track_id: i64) -> Result<()> {
    require_playlist(db, playlist_id)?;
    if smart_rule(db, playlist_id)?.is_some() {
        return Err(Error::Database(
            "smart playlists rebuild from rules — edit the rule instead".to_owned(),
        ));
    }
    require_track(db, track_id)?;
    let position: i64 = db
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_tracks WHERE playlist_id = ?1",
            [playlist_id],
            |row| row.get(0),
        )
        .map_err(|err| db_error(&err))?;
    db.execute(
        "INSERT INTO playlist_tracks(playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
        rusqlite::params![playlist_id, track_id, position],
    )
    .map_err(|err| db_error(&err))?;
    Ok(())
}

/// Remove one entry by entry id (repeats stay distinct) and renumber.
///
/// # Errors
///
/// Returns [`Error::Database`] when the entry is gone or the update fails.
pub fn remove_from_playlist(db: &mut Connection, playlist_id: i64, entry_id: i64) -> Result<()> {
    require_playlist(db, playlist_id)?;
    let removed = db
        .execute(
            "DELETE FROM playlist_tracks WHERE id = ?1 AND playlist_id = ?2",
            rusqlite::params![entry_id, playlist_id],
        )
        .map_err(|err| db_error(&err))?;
    if removed == 0 {
        return Err(Error::Database("playlist entry is gone".to_owned()));
    }
    renumber(db, playlist_id)
}

/// Move one entry to `to` (clamped into range) and renumber.
///
/// # Errors
///
/// Returns [`Error::Database`] when the playlist or entry is gone, or the
/// update fails.
pub fn move_entry(db: &mut Connection, playlist_id: i64, entry_id: i64, to: usize) -> Result<()> {
    require_playlist(db, playlist_id)?;
    let mut ids: Vec<i64> = db
        .prepare("SELECT id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position")
        .map_err(|err| db_error(&err))?
        .query_map([playlist_id], |row| row.get(0))
        .map_err(|err| db_error(&err))?
        .collect::<rusqlite::Result<Vec<i64>>>()
        .map_err(|err| db_error(&err))?;
    let Some(from) = ids.iter().position(|id| *id == entry_id) else {
        return Err(Error::Database("playlist entry is gone".to_owned()));
    };
    let entry = ids.remove(from);
    let to = to.min(ids.len());
    ids.insert(to, entry);
    let transaction = db.transaction().map_err(|err| db_error(&err))?;
    for (position, id) in ids.iter().enumerate() {
        transaction
            .execute(
                "UPDATE playlist_tracks SET position = ?1 WHERE id = ?2",
                rusqlite::params![i64::try_from(position).unwrap_or(i64::MAX), id],
            )
            .map_err(|err| db_error(&err))?;
    }
    transaction.commit().map_err(|err| db_error(&err))?;
    Ok(())
}

/// Ordered entries with resolved tracks (`None` where dangling).
///
/// # Errors
///
/// Returns [`Error::Database`] when the playlist is gone or the query fails.
pub fn list_entries(db: &Connection, playlist_id: i64) -> Result<Vec<PlaylistEntry>> {
    require_playlist(db, playlist_id)?;
    let mut statement = db
        .prepare(
            "SELECT playlist_tracks.id AS entry_id,
                    playlist_tracks.track_id AS entry_track_id,
                    playlist_tracks.position AS entry_position,
                    tracks.id AS id, tracks.path AS path, tracks.title AS title,
                    tracks.stable_key AS stable_key,
                    artists.name AS artist, albums.title AS album, genres.name AS genre,
                    tracks.composer AS composer, tracks.year AS year,
                    tracks.track_number AS track_number, tracks.disc_number AS disc_number,
                    tracks.duration_ms AS duration_ms, tracks.missing AS missing
             FROM playlist_tracks
             LEFT JOIN tracks ON tracks.id = playlist_tracks.track_id
             LEFT JOIN artists ON artists.id = tracks.artist_id
             LEFT JOIN albums ON albums.id = tracks.album_id
             LEFT JOIN genres ON genres.id = tracks.genre_id
             WHERE playlist_tracks.playlist_id = ?1
             ORDER BY playlist_tracks.position",
        )
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([playlist_id], |row| {
            let id: i64 = row.get("entry_id")?;
            let track_id: i64 = row.get("entry_track_id")?;
            let position: i64 = row.get("entry_position")?;
            let present: Option<i64> = row.get("id")?;
            let track = if present.is_none() {
                None
            } else {
                Some(TrackRow::from_row(row)?)
            };
            Ok(PlaylistEntry {
                id,
                track_id,
                position,
                track,
            })
        })
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<PlaylistEntry>>>()
        .map_err(|err| db_error(&err))
}

/// Renumber one playlist's positions dense from zero (structural changes only
/// ever call this inside their own transaction discipline — positions are
/// never left gapped on success).
fn renumber(db: &Connection, playlist_id: i64) -> Result<()> {
    let ids: Vec<i64> = db
        .prepare("SELECT id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position")
        .map_err(|err| db_error(&err))?
        .query_map([playlist_id], |row| row.get(0))
        .map_err(|err| db_error(&err))?
        .collect::<rusqlite::Result<Vec<i64>>>()
        .map_err(|err| db_error(&err))?;
    for (position, id) in ids.iter().enumerate() {
        db.execute(
            "UPDATE playlist_tracks SET position = ?1 WHERE id = ?2",
            rusqlite::params![i64::try_from(position).unwrap_or(i64::MAX), id],
        )
        .map_err(|err| db_error(&err))?;
    }
    Ok(())
}

/// Row id behind a playlist name, if any.
fn playlist_id_by_name(db: &Connection, name: &str) -> Result<Option<i64>> {
    use rusqlite::OptionalExtension as _;
    db.query_row("SELECT id FROM playlists WHERE name = ?1", [name], |row| {
        row.get(0)
    })
    .optional()
    .map_err(|err| db_error(&err))
}

/// Fail explicitly when the playlist is gone (UI ids are always live, but
/// headless callers and races deserve words, not silent no-ops).
fn require_playlist(db: &Connection, id: i64) -> Result<()> {
    let present: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM playlists WHERE id = ?1)",
            [id],
            |row| row.get(0),
        )
        .map_err(|err| db_error(&err))?;
    if present {
        Ok(())
    } else {
        Err(Error::Database("playlist is gone".to_owned()))
    }
}

/// Fail explicitly when the track row is gone.
fn require_track(db: &Connection, id: i64) -> Result<()> {
    let present: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM tracks WHERE id = ?1)",
            [id],
            |row| row.get(0),
        )
        .map_err(|err| db_error(&err))?;
    if present {
        Ok(())
    } else {
        Err(Error::Database("track is gone".to_owned()))
    }
}

/// Stored smart-playlist rule (L-004). Evaluated in SQL on open/play.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmartRule {
    /// `added_days` / `never_played` / `artist` / `genre` / `composer`.
    pub kind: String,
    /// Days as digits, or an exact artist/genre/composer name.
    pub value: String,
    /// Drop missing files from the generated list.
    pub exclude_missing: bool,
}

/// Create a smart playlist and evaluate it once.
///
/// # Errors
///
/// Returns [`Error::Database`] when the name is blank/taken or evaluation fails.
pub fn create_smart_playlist(db: &mut Connection, name: &str, rule: &SmartRule) -> Result<i64> {
    match rule.kind.as_str() {
        "added_days" | "artist" | "genre" | "composer" if rule.value.trim().is_empty() => {
            return Err(Error::Database(
                "smart-playlist rule needs a value".to_owned(),
            ));
        }
        "added_days" | "never_played" | "artist" | "genre" | "composer" => {}
        other => {
            return Err(Error::Database(format!(
                "unknown smart-playlist rule {other}"
            )));
        }
    }
    let id = create_playlist(db, name)?;
    db.execute(
        "INSERT INTO smart_playlists(playlist_id, rule_kind, rule_value, exclude_missing)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![id, rule.kind, rule.value, i64::from(rule.exclude_missing)],
    )
    .map_err(|err| db_error(&err))?;
    evaluate_smart_playlist(db, id)?;
    Ok(id)
}

/// Whether this playlist is generated from a stored rule.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn smart_rule(db: &Connection, playlist_id: i64) -> Result<Option<SmartRule>> {
    let mut statement = db
        .prepare(
            "SELECT rule_kind, rule_value, exclude_missing FROM smart_playlists WHERE playlist_id = ?1",
        )
        .map_err(|err| db_error(&err))?;
    let mut rows = statement
        .query_map([playlist_id], |row| {
            Ok(SmartRule {
                kind: row.get(0)?,
                value: row.get(1)?,
                exclude_missing: row.get::<_, i64>(2)? != 0,
            })
        })
        .map_err(|err| db_error(&err))?;
    rows.next().transpose().map_err(|err| db_error(&err))
}

/// Rebuild smart-playlist entries from the stored rule. No-op for user lists.
///
/// # Errors
///
/// Returns [`Error::Database`] when the rule is unknown or SQL fails.
pub fn evaluate_smart_playlist(db: &mut Connection, playlist_id: i64) -> Result<()> {
    let Some(rule) = smart_rule(db, playlist_id)? else {
        return Ok(());
    };
    db.execute(
        "DELETE FROM playlist_tracks WHERE playlist_id = ?1",
        [playlist_id],
    )
    .map_err(|err| db_error(&err))?;
    let missing = if rule.exclude_missing {
        " AND tracks.missing = 0"
    } else {
        ""
    };
    let sql = match rule.kind.as_str() {
        "added_days" => format!(
            "INSERT INTO playlist_tracks(playlist_id, track_id, position)
             SELECT ?1, tracks.id, 0
             FROM tracks
             WHERE tracks.indexed_at >= strftime('%s','now') - (?2 * 86400)
               AND tracks.indexed_at > 0{missing}"
        ),
        "never_played" => format!(
            "INSERT INTO playlist_tracks(playlist_id, track_id, position)
             SELECT ?1, tracks.id, 0
             FROM tracks
             LEFT JOIN play_history ON play_history.track_id = tracks.id
             WHERE play_history.id IS NULL{missing}"
        ),
        "artist" => format!(
            "INSERT INTO playlist_tracks(playlist_id, track_id, position)
             SELECT ?1, tracks.id, 0
             FROM tracks
             JOIN artists ON artists.id = tracks.artist_id
             WHERE artists.name = ?2{missing}"
        ),
        "genre" => format!(
            "INSERT INTO playlist_tracks(playlist_id, track_id, position)
             SELECT ?1, tracks.id, 0
             FROM tracks
             JOIN genres ON genres.id = tracks.genre_id
             WHERE genres.name = ?2{missing}"
        ),
        "composer" => format!(
            "INSERT INTO playlist_tracks(playlist_id, track_id, position)
             SELECT ?1, tracks.id, 0
             FROM tracks
             WHERE tracks.composer = ?2{missing}"
        ),
        other => {
            return Err(Error::Database(format!(
                "unknown smart-playlist rule {other}"
            )));
        }
    };
    if rule.kind == "never_played" {
        db.execute(&sql, rusqlite::params![playlist_id])
            .map_err(|err| db_error(&err))?;
    } else {
        db.execute(&sql, rusqlite::params![playlist_id, rule.value])
            .map_err(|err| db_error(&err))?;
    }
    renumber(db, playlist_id)
}

fn db_error(err: &rusqlite::Error) -> Error {
    Error::Database(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::super::db::{NewTrack, open_file, open_memory, upsert_track};
    use super::*;

    fn seed_track(db: &mut Connection, path: &str, title: &str) -> i64 {
        upsert_track(
            db,
            &NewTrack {
                path: path.to_owned(),
                stable_key: path.to_owned(),
                title: Some(title.to_owned()),
                ..Default::default()
            },
        )
        .expect("seed works");
        super::super::db::list_tracks(db)
            .expect("list works")
            .iter()
            .find(|row| row.path == path)
            .expect("seed present")
            .id
    }

    fn entry_titles(entries: &[PlaylistEntry]) -> Vec<String> {
        entries
            .iter()
            .map(|entry| {
                entry
                    .track
                    .as_ref()
                    .and_then(|row| row.title.clone())
                    .unwrap_or_else(|| "<dangling>".to_owned())
            })
            .collect()
    }

    #[test]
    fn create_lists_and_renames() {
        let db = open_memory().expect("in-memory opens");
        assert!(list_playlists(&db).expect("list works").is_empty());
        let id = create_playlist(&db, "Evening").expect("create works");
        assert_eq!(
            create_playlist(&db, "  Morning  ").expect("create works") - id,
            1
        );
        let names: Vec<String> = list_playlists(&db)
            .expect("list works")
            .into_iter()
            .map(|playlist| playlist.name)
            .collect();
        assert_eq!(names, ["Evening", "Morning"], "trimmed, name-ordered");
        rename_playlist(&db, id, "Night").expect("rename works");
        let renamed = list_playlists(&db).expect("list works");
        assert!(renamed.iter().any(|playlist| playlist.name == "Night"));
    }

    #[test]
    fn blank_and_duplicate_names_error_explicitly() {
        let db = open_memory().expect("in-memory opens");
        assert!(create_playlist(&db, "   ").is_err());
        create_playlist(&db, "Mix").expect("create works");
        let dupe = create_playlist(&db, "Mix").expect_err("duplicate names fail");
        assert!(
            dupe.to_string().contains("already exists"),
            "explicit: {dupe}"
        );
        let other = create_playlist(&db, "Other").expect("create works");
        let clash = rename_playlist(&db, other, "Mix").expect_err("rename clash fails");
        assert!(
            clash.to_string().contains("already exists"),
            "explicit: {clash}"
        );
        assert!(rename_playlist(&db, other, "  ").is_err());
        assert!(rename_playlist(&db, 999_999, "Ghost").is_err());
    }

    #[test]
    fn delete_removes_list_and_entries() {
        let mut db = open_memory().expect("in-memory opens");
        let track = seed_track(&mut db, "/m/a.flac", "A");
        let id = create_playlist(&db, "Temp").expect("create works");
        add_to_playlist(&mut db, id, track).expect("add works");
        delete_playlist(&mut db, id).expect("delete works");
        assert!(list_playlists(&db).expect("list works").is_empty());
        assert!(
            delete_playlist(&mut db, id).is_err(),
            "double delete fails loudly"
        );
    }

    #[test]
    fn entries_add_remove_and_reorder() {
        let mut db = open_memory().expect("in-memory opens");
        let a = seed_track(&mut db, "/m/a.flac", "A");
        let b = seed_track(&mut db, "/m/b.flac", "B");
        let c = seed_track(&mut db, "/m/c.flac", "C");
        let id = create_playlist(&db, "Order").expect("create works");
        for track in [a, b, c, a] {
            add_to_playlist(&mut db, id, track).expect("add works");
        }
        let entries = list_entries(&db, id).expect("entries list");
        assert_eq!(entry_titles(&entries), ["A", "B", "C", "A"]);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.position)
                .collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        // Repeats stay distinct: removing the first A keeps the last.
        remove_from_playlist(&mut db, id, entries[0].id).expect("remove works");
        let entries = list_entries(&db, id).expect("entries list");
        assert_eq!(entry_titles(&entries), ["B", "C", "A"]);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.position)
                .collect::<Vec<_>>(),
            [0, 1, 2],
            "positions stay dense"
        );
        move_entry(&mut db, id, entries[2].id, 0).expect("move works");
        assert_eq!(
            entry_titles(&list_entries(&db, id).expect("entries list")),
            ["A", "B", "C"]
        );
        move_entry(&mut db, id, entries[2].id, 99).expect("clamped move works");
        assert_eq!(
            entry_titles(&list_entries(&db, id).expect("entries list")),
            ["B", "C", "A"],
            "overshoot clamps to the end"
        );
        assert!(remove_from_playlist(&mut db, id, 999_999).is_err());
        assert!(move_entry(&mut db, id, 999_999, 0).is_err());
        assert!(add_to_playlist(&mut db, 999_999, a).is_err());
        assert!(add_to_playlist(&mut db, id, 999_999).is_err());
    }

    #[test]
    fn deleted_tracks_dangle_while_missing_ones_resolve() {
        let mut db = open_memory().expect("in-memory opens");
        let gone = seed_track(&mut db, "/m/gone.flac", "Gone");
        let flagged = seed_track(&mut db, "/m/flagged.flac", "Flagged");
        let id = create_playlist(&db, "Fragile").expect("create works");
        add_to_playlist(&mut db, id, gone).expect("add works");
        add_to_playlist(&mut db, id, flagged).expect("add works");
        db.execute("DELETE FROM tracks WHERE id = ?1", [gone])
            .expect("delete works");
        super::super::db::set_missing(&db, flagged, true).expect("flag works");
        let entries = list_entries(&db, id).expect("entries list");
        assert_eq!(entries.len(), 2, "dangling entries survive");
        assert!(entries[0].track.is_none(), "deleted track dangles");
        assert_eq!(entries[0].track_id, gone, "dangling keeps its link");
        assert!(
            entries[1].track.as_ref().is_some_and(|row| row.missing),
            "missing-flagged rows still resolve"
        );
        let playlists = list_playlists(&db).expect("list works");
        assert_eq!(playlists[0].track_count, 2, "counts include dangling");
    }

    #[test]
    fn renames_keep_playlist_links_by_id() {
        let mut db = open_memory().expect("in-memory opens");
        let track = seed_track(&mut db, "/m/old.flac", "Old");
        let id = create_playlist(&db, "Stable").expect("create works");
        add_to_playlist(&mut db, id, track).expect("add works");
        super::super::db::rename_track(&db, track, "/m/new.flac", "k2").expect("rename works");
        let entries = list_entries(&db, id).expect("entries list");
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].track.as_ref().expect("link survives").path,
            "/m/new.flac"
        );
    }

    #[test]
    fn playlists_survive_restart() {
        let dir =
            std::env::temp_dir().join(format!("tunex-playlist-restart-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("library.db");
        let id = {
            let mut db = open_file(&path).expect("file opens");
            let track = seed_track(&mut db, "/m/a.flac", "A");
            let id = create_playlist(&db, "Keep").expect("create works");
            add_to_playlist(&mut db, id, track).expect("add works");
            id
        };
        let db = open_file(&path).expect("reopen works");
        let playlists = list_playlists(&db).expect("list works");
        assert_eq!(playlists.len(), 1);
        assert_eq!(playlists[0].name, "Keep");
        let entries = list_entries(&db, id).expect("entries list");
        assert_eq!(entry_titles(&entries), ["A"]);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn smart_never_played_excludes_history() {
        let mut db = open_memory().expect("in-memory opens");
        let played = seed_track(&mut db, "/m/a.flac", "Heard");
        let _fresh = seed_track(&mut db, "/m/b.flac", "Fresh");
        super::super::db::record_play(&db, Some(played), "file:///m/a.flac").expect("history");
        let id = create_smart_playlist(
            &mut db,
            "Unheard",
            &SmartRule {
                kind: "never_played".to_owned(),
                value: String::new(),
                exclude_missing: true,
            },
        )
        .expect("create smart");
        let titles = entry_titles(&list_entries(&db, id).expect("entries"));
        assert_eq!(titles, ["Fresh"]);
    }
}
