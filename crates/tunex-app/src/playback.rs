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
use tunex_library::{TrackRow, display_title_artist, list_tracks_for_artist, list_tracks_in_album};
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
}
