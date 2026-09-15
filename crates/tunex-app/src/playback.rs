//! Library-to-queue bridging: index rows become playable queue items.
//!
//! [`PlaybackController`](tunex_player::PlaybackController) speaks URIs;
//! these helpers translate [`TrackRow`]s (paths, tags, durations) into
//! enriched [`QueueItem`](tunex_player::QueueItem)s and enqueue whole
//! albums and artists in play order. Missing files are skipped at enqueue
//! time (with a warning) — the queue holds playable tracks only; dangling
//! playlist entries are a W-023 concern, not a queue one.

use rusqlite::Connection;
use tunex_core::Result;
use tunex_library::{
    AlbumSort, ArtistSort, SortDir, TrackRow, TrackSort, display_title_artist, list_albums,
    list_artists, list_composers, list_genres, list_track_folders, list_tracks_capped,
    list_tracks_for_artist, list_tracks_for_composer, list_tracks_for_genre, list_tracks_in_album,
    list_tracks_in_folder,
};
use tunex_player::{PlaybackController, QueueItem, path_to_uri};

/// Translate one index row into an enriched queue item.
///
/// Title and artist fall back to the file stem (`Artist - Title` when both
/// tags are missing). Parsed names are never written as tags. Returns `None`
/// when the path cannot become a playback URI.
#[must_use]
pub fn queue_item_from_row(row: &TrackRow) -> Option<QueueItem> {
    let uri = path_to_uri(std::path::Path::new(&row.path)).ok()?;
    let (title, artist) =
        display_title_artist(&row.path, row.title.as_deref(), row.artist.as_deref());
    let mut item = QueueItem::new(&uri, &title).with_track_id(row.id);
    if artist != tunex_library::UNKNOWN_ARTIST {
        item = item.with_artist(&artist);
    }
    if let Some(album) = &row.album {
        item = item.with_album(album);
    }
    if let Some(duration_ms) = row.duration_ms {
        if let Ok(duration) = u64::try_from(duration_ms) {
            item = item.with_duration(std::time::Duration::from_millis(duration));
        }
    }
    Some(item)
}

/// Enqueue rows in order, skipping missing files and unconvertible paths.
/// Returns the number enqueued.
pub fn enqueue_rows(controller: &mut PlaybackController, rows: &[TrackRow]) -> usize {
    let mut enqueued = 0;
    for row in rows {
        if row.missing {
            tracing::debug!(
                name = "playback.enqueue_skipped",
                path = %row.path,
                "missing file skipped at enqueue"
            );
            continue;
        }
        if let Some(item) = queue_item_from_row(row) {
            controller.enqueue(item);
            enqueued += 1;
        } else {
            tracing::warn!(
                name = "playback.enqueue_failed",
                path = %row.path,
                "path cannot become a playback URI"
            );
        }
    }
    enqueued
}

/// Concatenate one track list per group, in group order (albums in a
/// Play-all sweep, artists in a genre drill, ids in a multi-enqueue).
fn concat_group_tracks<G>(
    groups: &[G],
    mut tracks_for: impl FnMut(&G) -> Result<Vec<TrackRow>>,
) -> Result<Vec<TrackRow>> {
    let mut rows = Vec::new();
    for group in groups {
        rows.extend(tracks_for(group)?);
    }
    Ok(rows)
}

/// Enqueue one album's tracks in disc/track order. Returns the number
/// enqueued (missing files skipped).
///
/// # Errors
///
/// Returns [`Error::Database`](tunex_core::Error::Database) when the album
/// query fails.
pub fn enqueue_album(
    controller: &mut PlaybackController,
    db: &Connection,
    album_id: i64,
) -> Result<usize> {
    Ok(enqueue_rows(
        controller,
        &list_tracks_in_album(db, album_id)?,
    ))
}

/// Enqueue one artist's tracks in album order. Returns the number enqueued
/// (missing files skipped).
///
/// # Errors
///
/// Returns [`Error::Database`](tunex_core::Error::Database) when the artist
/// query fails.
pub fn enqueue_artist(
    controller: &mut PlaybackController,
    db: &Connection,
    artist: &str,
) -> Result<usize> {
    Ok(enqueue_rows(
        controller,
        &list_tracks_for_artist(db, artist)?,
    ))
}

/// Rows for one library browse list, in the same order the UI shows.
///
/// `songs` is capped at `songs_cap` (the songs tab). Other kinds follow the
/// browse models: albums/artists in their sort, folders/genres/composers in
/// display order, `genre`/`composer` as one drilled group.
///
/// # Errors
///
/// Returns [`Error::Database`](tunex_core::Error::Database) when a lookup
/// fails.
pub fn library_list_rows(
    db: &Connection,
    kind: &str,
    key: &str,
    sort_key: &str,
    descending: bool,
    songs_cap: u32,
) -> Result<Vec<TrackRow>> {
    let dir = SortDir::from_descending(descending);
    match kind {
        "songs" => list_tracks_capped(db, songs_cap, TrackSort::from_key(sort_key), dir),
        "albums" => {
            let albums = list_albums(db, AlbumSort::from_key(sort_key), dir)?;
            concat_group_tracks(&albums, |album| list_tracks_in_album(db, album.id))
        }
        "artists" => {
            let artists = list_artists(db, ArtistSort::from_key(sort_key), dir)?;
            concat_group_tracks(&artists, |artist| list_tracks_for_artist(db, &artist.name))
        }
        "folders" => {
            let folders = list_track_folders(db)?;
            let sort = TrackSort::from_key(sort_key);
            concat_group_tracks(&folders, |folder| {
                list_tracks_in_folder(db, &folder.path, u32::MAX, sort, dir)
            })
        }
        "genres" => concat_group_tracks(&list_genres(db)?, |facet| {
            list_tracks_for_genre(db, &facet.name)
        }),
        "composers" => concat_group_tracks(&list_composers(db)?, |facet| {
            list_tracks_for_composer(db, &facet.name)
        }),
        "genre" => list_tracks_for_genre(db, key),
        "composer" => list_tracks_for_composer(db, key),
        _ => Ok(Vec::new()),
    }
}

/// Enqueue the currently shown library list. Returns the number enqueued.
///
/// # Errors
///
/// Returns [`Error::Database`](tunex_core::Error::Database) when a lookup
/// fails.
pub fn enqueue_library_list(
    controller: &mut PlaybackController,
    db: &Connection,
    kind: &str,
    key: &str,
    sort_key: &str,
    descending: bool,
    songs_cap: u32,
) -> Result<usize> {
    Ok(enqueue_rows(
        controller,
        &library_list_rows(db, kind, key, sort_key, descending, songs_cap)?,
    ))
}

/// Enqueue albums by database id, in the given order.
///
/// # Errors
///
/// Returns [`Error::Database`](tunex_core::Error::Database) when a lookup
/// fails.
pub fn enqueue_album_ids(
    controller: &mut PlaybackController,
    db: &Connection,
    album_ids: &[i64],
) -> Result<usize> {
    let rows = concat_group_tracks(album_ids, |album_id| list_tracks_in_album(db, *album_id))?;
    Ok(enqueue_rows(controller, &rows))
}

/// Enqueue artists by name, in the given order.
///
/// # Errors
///
/// Returns [`Error::Database`](tunex_core::Error::Database) when a lookup
/// fails.
pub fn enqueue_artist_names(
    controller: &mut PlaybackController,
    db: &Connection,
    names: &[String],
) -> Result<usize> {
    let rows = concat_group_tracks(names, |name| list_tracks_for_artist(db, name))?;
    Ok(enqueue_rows(controller, &rows))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tunex_library::{NewTrack, open_file, upsert_track};

    /// Scratch file index (parallel-safe).
    fn scratch_db(case: &str) -> (Connection, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("tunex-enqueue-{case}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("setup works");
        let mut db = open_file(&dir.join("library.db")).expect("index opens");
        for (name, title, artist, album, duration_ms) in [
            (
                "a1.flac",
                Some("One"),
                Some("Nova Rae"),
                Some("Night Tapes"),
                Some(273_000),
            ),
            (
                "a2.flac",
                Some("Two"),
                Some("Nova Rae"),
                Some("Night Tapes"),
                Some(195_000),
            ),
            ("b1.flac", None, None, None, None),
        ] {
            upsert_track(
                &mut db,
                &NewTrack {
                    path: format!("/music/{name}"),
                    stable_key: name.to_owned(),
                    title: title.map(str::to_owned),
                    artist: artist.map(str::to_owned),
                    album: album.map(str::to_owned),
                    duration_ms,
                    ..Default::default()
                },
            )
            .expect("seed works");
        }
        (db, dir)
    }

    #[test]
    fn tagged_row_converts_fully() {
        let (db, _dir) = scratch_db("tagged");
        let rows = tunex_library::list_tracks(&db).expect("list works");
        let tagged = rows
            .iter()
            .find(|row| row.path.ends_with("a1.flac"))
            .expect("seed present");
        let item = queue_item_from_row(tagged).expect("tagged row converts");
        assert_eq!(item.uri, "file:///music/a1.flac");
        assert_eq!(item.title, "One");
        assert_eq!(item.track_id, Some(tagged.id));
        assert_eq!(item.artist.as_deref(), Some("Nova Rae"));
        assert_eq!(item.album.as_deref(), Some("Night Tapes"));
        assert_eq!(item.duration, Some(std::time::Duration::from_secs(273)));
    }

    #[test]
    fn untagged_row_falls_back_to_filename() {
        let (db, _dir) = scratch_db("untagged");
        let rows = tunex_library::list_tracks(&db).expect("list works");
        let bare = rows
            .iter()
            .find(|row| row.path.ends_with("b1.flac"))
            .expect("seed present");
        let item = queue_item_from_row(bare).expect("untagged row converts");
        assert_eq!(item.title, "b1");
        assert_eq!(item.track_id, Some(bare.id));
        assert!(item.artist.is_none());
        assert!(item.album.is_none());
        assert!(item.duration.is_none());
    }

    #[test]
    fn enqueue_album_orders_disc_tracks_and_skips_missing() {
        let (db, dir) = scratch_db("album");
        let albums = tunex_library::list_albums(
            &db,
            tunex_library::AlbumSort::Title,
            tunex_library::SortDir::Asc,
        )
        .expect("albums list");
        let tapes = albums
            .iter()
            .find(|album| album.title == "Night Tapes")
            .expect("album present");
        // Vanish the second track: enqueue must skip it, not fail.
        tunex_library::set_missing(
            &db,
            tunex_library::list_tracks(&db)
                .expect("list works")
                .iter()
                .find(|row| row.path.ends_with("a2.flac"))
                .expect("seed present")
                .id,
            true,
        )
        .expect("flag works");
        let mut controller = PlaybackController::new().expect("controller builds");
        let enqueued = enqueue_album(&mut controller, &db, tapes.id).expect("enqueue works");
        assert_eq!(enqueued, 1);
        assert_eq!(controller.queue_len(), 1);
        assert_eq!(
            controller.queue_item(0).map(|item| item.title),
            Some("One".to_owned())
        );
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn enqueue_artist_collects_album_order() {
        let (db, dir) = scratch_db("artist");
        let mut controller = PlaybackController::new().expect("controller builds");
        let enqueued = enqueue_artist(&mut controller, &db, "Nova Rae").expect("enqueue works");
        assert_eq!(enqueued, 2);
        assert_eq!(
            controller.queue_item(1).map(|item| item.title),
            Some("Two".to_owned())
        );
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn enqueue_library_list_songs_matches_capped_title_order() {
        let (db, dir) = scratch_db("play-all-songs");
        let mut controller = PlaybackController::new().expect("controller builds");
        let enqueued = enqueue_library_list(&mut controller, &db, "songs", "", "title", false, 500)
            .expect("enqueue works");
        assert_eq!(enqueued, 3);
        assert_eq!(controller.queue_len(), 3);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn enqueue_library_list_albums_concatenates_album_order() {
        let (db, dir) = scratch_db("play-all-albums");
        let mut controller = PlaybackController::new().expect("controller builds");
        let enqueued =
            enqueue_library_list(&mut controller, &db, "albums", "", "title", false, 500)
                .expect("enqueue works");
        // Untagged files are not on an album, so Play all on Albums skips them.
        assert_eq!(enqueued, 2);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn library_list_rows_genres_concatenates_facet_groups() {
        let (db, dir) = scratch_db("play-all-genres");
        // The seed carries no genres, so everything lands in one group.
        let rows = library_list_rows(&db, "genres", "", "", false, 500).expect("list works");
        assert_eq!(rows.len(), 3);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }
}
