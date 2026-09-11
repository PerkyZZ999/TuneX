//! Filesystem scanner: walk library roots, index supported media files with
//! extracted metadata, and reconcile renames and deletions across rescans.
//!
//! One scan is a single pass: every supported file refreshes its row (tags
//! included), a file whose identity (`dev:ino`) matches a known row under a
//! new path retargets that row instead of duplicating it, and known rows
//! whose files vanished are flagged missing — never deleted — so playlists
//! and history keep pointing at them (SPEC §11.1, D-009).
//!
//! Metadata failure is routine, never fatal: unreadable tags index by path
//! with unknowns and count toward `metadata_failed`, so one bad file cannot
//! abort a run and valid audio stays playable.
//!
//! Symlinks (files and directories) are skipped: they cannot escape roots
//! they never enter, which keeps the trust story trivially safe.
//!
//! Heavy lifting stays synchronous on the calling thread; [`scan_folder_live`]
//! moves it onto a blocking thread and streams [`ScanProgress`] snapshots for
//! UI progress bars. Watch callbacks (W-014) only ever enqueue paths —
//! rescans run here.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tokio::sync::mpsc;
use tunex_core::{Error, Result};

use crate::db::{
    NewTrack, TrackIdentity, add_root, rename_track, set_missing, track_identities, upsert_track,
};
use crate::metadata::read_metadata;

/// Media extensions the scanner indexes (lowercase, no dots). Decodability is
/// the player's concern; the scanner only recognizes containers.
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "oga", "opus", "m4a", "m4b", "wav", "aif", "aiff",
];

/// Live counters reported while a scan runs (UI progress reads these).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScanStats {
    /// Files walked, supported or not.
    pub files_seen: u64,
    /// Rows inserted or refreshed.
    pub tracks_added: u64,
    /// Of those, rows a rescan recognized as untouched and left alone
    /// (counted in `tracks_added` as well — they are indexed tracks).
    pub tracks_unchanged: u64,
    /// Supported files indexed without tags (unreadable metadata).
    pub metadata_failed: u64,
    /// Known rows retargeted at a new path (no duplication on rename).
    pub renamed: u64,
    /// Known rows newly flagged missing because their file vanished.
    pub missing_marked: u64,
}

/// One progress snapshot from a running [`scan_folder_live`] scan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanProgress {
    /// Root being scanned.
    pub root: PathBuf,
    /// Counters so far.
    pub stats: ScanStats,
    /// True on the final snapshot, when the counters are complete.
    pub finished: bool,
}

/// Whether a path names a supported media file (case-insensitive extension).
#[must_use]
pub fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| SUPPORTED_EXTENSIONS.contains(&extension.to_lowercase().as_str()))
}

/// Stable identity for one file: canonical path plus mtime plus size, so
/// edits invalidate the key while renames keep history linkable (S2).
#[must_use]
pub fn stable_key(path: &Path) -> String {
    let (mtime, size) = std::fs::metadata(path)
        .ok()
        .and_then(|metadata| {
            metadata
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|age| (age.as_secs(), metadata.len()))
        })
        .unwrap_or((0, 0));
    // Canonicalization resolves symlinks for identity; if it fails (vanishing
    // file raced us), fall back to the walked path rather than dropping it.
    let identity = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    format!("{}:{mtime}:{size}", identity.display())
}

/// Filesystem identity for rename matching: `dev:ino` on Unix (stable
/// across renames on one filesystem), `None` elsewhere — where rescans fall
/// back to path matching and cross-root moves count as missing + new.
#[cfg(unix)]
#[must_use]
pub fn file_id(path: &Path) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path)
        .ok()
        .map(|metadata| format!("{}:{}", metadata.dev(), metadata.ino()))
}

/// Filesystem identity for rename matching (non-Unix: unsupported).
#[cfg(not(unix))]
#[must_use]
pub fn file_id(_path: &Path) -> Option<String> {
    None
}

/// Build the index row for one file: identity always, tags when readable.
///
/// A metadata failure still returns a usable path-only row (unknowns stay
/// unknown per R-004); the `false` flag tells the caller to count it.
fn track_from_file(path: &Path) -> (NewTrack, bool) {
    let mut track = NewTrack {
        path: path.to_string_lossy().into_owned(),
        stable_key: stable_key(path),
        file_id: file_id(path),
        ..Default::default()
    };
    match read_metadata(path) {
        Ok(metadata) => {
            track.title = metadata.title;
            track.artist = metadata.artist;
            track.album = metadata.album;
            track.album_artist = metadata.album_artist;
            track.composer = metadata.composer;
            track.genre = metadata.genre;
            track.year = metadata.year.map(i64::from);
            track.track_number = metadata.track_number.map(i64::from);
            track.disc_number = metadata.disc_number.map(i64::from);
            track.duration_ms = metadata
                .duration
                .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX));
            (track, false)
        }
        Err(err) => {
            tracing::debug!(
                name = "scan.metadata_failed",
                path = %track.path,
                error = %err,
                "indexing without tags"
            );
            (track, true)
        }
    }
}

/// Path prefix (with trailing separator) scoping rows to one root, so a
/// rescan of `/music` never flags `/podcasts` rows missing.
fn root_prefix(root: &Path) -> String {
    let mut prefix = root.to_string_lossy().into_owned();
    if !prefix.ends_with(std::path::MAIN_SEPARATOR) {
        prefix.push(std::path::MAIN_SEPARATOR);
    }
    prefix
}

/// Walk `root` recursively, registering it and upserting every supported file.
///
/// Every file refreshes its row with freshly read tags; known rows matched
/// by filesystem identity retarget instead of duplicating; known rows under
/// `root` whose files vanished are flagged missing. Returns live counters;
/// never fails on individual files.
///
/// # Errors
///
/// Returns [`Error::Database`] when root registration, an upsert, or the
/// reconcile pass fails (I/O problems on individual entries are skipped,
/// not raised).
pub fn scan_folder(db: &mut Connection, root: &Path) -> Result<ScanStats> {
    scan_folder_with_callback(db, root, |_| {})
}

/// [`scan_folder`] with progress snapshots every 50 files plus the completed
/// counters at the end, for progress bars over large libraries.
///
/// # Errors
///
/// Same as [`scan_folder`].
pub fn scan_folder_with_callback(
    db: &mut Connection,
    root: &Path,
    mut on_progress: impl FnMut(&ScanStats),
) -> Result<ScanStats> {
    add_root(db, &root.to_string_lossy())?;
    let known = track_identities(db)?;
    let mut by_path: HashMap<&str, &TrackIdentity> = HashMap::new();
    let mut by_file_id: HashMap<&str, &TrackIdentity> = HashMap::new();
    for row in &known {
        by_path.insert(row.path.as_str(), row);
        if let Some(file_id) = row.file_id.as_deref() {
            by_file_id.insert(file_id, row);
        }
    }
    let mut run = ScanRun::default();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        scan_one_dir(
            db,
            &dir,
            &mut stack,
            &by_path,
            &by_file_id,
            &mut run,
            &mut on_progress,
        )?;
    }
    run.stats.missing_marked = flag_vanished_missing(db, &known, &run, &root_prefix(root))?;
    on_progress(&run.stats);
    let stats = run.stats;
    tracing::info!(
        name = "scan.finished",
        root = %root.display(),
        files_seen = stats.files_seen,
        tracks_added = stats.tracks_added,
        metadata_failed = stats.metadata_failed,
        renamed = stats.renamed,
        missing_marked = stats.missing_marked,
        "scan complete"
    );
    Ok(stats)
}

/// Mutable per-run walk state, threaded through the scan helpers so each
/// stays small enough to read (and to satisfy the complexity gate).
#[derive(Debug, Default)]
struct ScanRun {
    /// Counters reported to progress listeners.
    stats: ScanStats,
    /// Walked files, for the deletion pass below.
    seen: HashSet<PathBuf>,
    /// Rows retargeted by renames (exempt from the deletion pass).
    retargeted: HashSet<i64>,
    /// Files since the last progress snapshot.
    since_report: u32,
}

/// Walk one directory: subdirectories rejoin the stack, supported files
/// index, everything else only counts. Unreadable directories are skipped.
fn scan_one_dir(
    db: &mut Connection,
    dir: &Path,
    stack: &mut Vec<PathBuf>,
    by_path: &HashMap<&str, &TrackIdentity>,
    by_file_id: &HashMap<&str, &TrackIdentity>,
    run: &mut ScanRun,
    on_progress: &mut impl FnMut(&ScanStats),
) -> Result<()> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Symlinks never enter the index (see module docs).
        if entry.file_type().is_ok_and(|kind| kind.is_symlink()) {
            continue;
        }
        if path.is_dir() {
            stack.push(path);
        } else {
            run.stats.files_seen += 1;
            if is_supported(&path) {
                index_file(
                    db,
                    &path,
                    by_path,
                    by_file_id,
                    &mut run.retargeted,
                    &mut run.stats,
                )?;
                run.seen.insert(path);
                run.since_report += 1;
                if run.since_report >= 50 {
                    run.since_report = 0;
                    on_progress(&run.stats);
                }
            }
        }
    }
    Ok(())
}

/// Deletion pass: known rows under this root that the walk never saw and
/// that no rename retargeted are flagged missing, never dropped — playlists
/// and history keep pointing at them. Returns newly flagged rows for the
/// progress counters.
fn flag_vanished_missing(
    db: &Connection,
    known: &[TrackIdentity],
    run: &ScanRun,
    prefix: &str,
) -> Result<u64> {
    let mut marked = 0_u64;
    for row in known {
        if run.retargeted.contains(&row.id)
            || run.seen.contains(Path::new(&row.path))
            || !row.path.starts_with(prefix)
        {
            continue;
        }
        if !row.missing {
            set_missing(db, row.id, true)?;
            marked += 1;
        }
    }
    Ok(marked)
}

/// Scan on a blocking worker thread, streaming [`ScanProgress`] snapshots.
///
/// The worker opens its own database connection from `db_path` (file index),
/// so the Qt thread and callers never touch SQLite during a run. Snapshots
/// arrive every 50 files plus one final `finished` snapshot; the channel is
/// bounded, so a receiver that stops draining back-pressures the scan rather
/// than buffering unboundedly.
///
/// # Errors
///
/// Returns [`Error::Database`] when the index cannot be opened, the scan
/// fails, or the worker thread panics. Individual files never fail the run
/// (see [`scan_folder`]).
pub async fn scan_folder_live(
    db_path: PathBuf,
    root: PathBuf,
    updates: mpsc::Sender<ScanProgress>,
) -> Result<ScanStats> {
    tokio::task::spawn_blocking(move || {
        let mut db = crate::db::open_file(&db_path)?;
        let stats = scan_folder_with_callback(&mut db, &root, |snapshot| {
            let _ = updates.blocking_send(ScanProgress {
                root: root.clone(),
                stats: *snapshot,
                finished: false,
            });
        })?;
        let _ = updates.blocking_send(ScanProgress {
            root: root.clone(),
            stats,
            finished: true,
        });
        Ok(stats)
    })
    .await
    .map_err(|err| Error::Database(format!("scan worker panicked: {err}")))?
}

/// Index one supported file: refresh the known path, retarget a rename, or
/// insert a new row.
///
/// A rename only retargets when the previously indexed path is actually
/// gone — a reused inode pointing at two live files indexes as new instead
/// of stealing the row.
///
/// A file already indexed under this path whose identity key still matches
/// returns early: the key covers mtime and size, so its tags cannot have
/// changed, and reading them is by far the most expensive thing here. Every
/// start rescans the roots, so on a settled library this is the common case
/// — rescanning an unchanged 530-track library went from 97 ms to 7 ms (S6
/// W-041). The work skipped is file I/O, so the gap widens when the page
/// cache is cold; indexing the same library from empty costs 607 ms. Rows
/// flagged missing still take the full path, because only the upsert clears
/// that flag.
fn index_file(
    db: &mut Connection,
    path: &Path,
    by_path: &HashMap<&str, &TrackIdentity>,
    by_file_id: &HashMap<&str, &TrackIdentity>,
    retargeted: &mut HashSet<i64>,
    stats: &mut ScanStats,
) -> Result<()> {
    if let Some(previous) = by_path.get(path.to_string_lossy().as_ref())
        && !previous.missing
        && previous.stable_key == stable_key(path)
    {
        stats.tracks_added += 1;
        stats.tracks_unchanged += 1;
        return Ok(());
    }
    let (track, metadata_failed) = track_from_file(path);
    if metadata_failed {
        stats.metadata_failed += 1;
    }
    if by_path.contains_key(track.path.as_str()) {
        upsert_track(db, &track)?;
    } else if let Some(previous) = track
        .file_id
        .as_deref()
        .and_then(|file_id| by_file_id.get(file_id))
        .filter(|previous| !Path::new(&previous.path).exists())
    {
        rename_track(db, previous.id, &track.path, &track.stable_key)?;
        upsert_track(db, &track)?;
        retargeted.insert(previous.id);
        stats.renamed += 1;
    } else {
        upsert_track(db, &track)?;
    }
    stats.tracks_added += 1;
    Ok(())
}

/// Collect media files without touching the database (fixture probes, tests).
#[must_use]
pub fn collect_media_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().is_ok_and(|kind| kind.is_symlink()) {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else if is_supported(&path) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{list_tracks, open_file, open_memory};
    use lofty::{file::AudioFile as _, file::TaggedFileExt as _, tag::Accessor as _};

    /// Unique scratch dir per test (nextest runs tests in parallel).
    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("tunex-{name}-{}", std::process::id()))
    }

    /// Copy of the committed corrupt fixture: tag-unreadable, still indexed.
    fn corrupt_source() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/corrupt.mp3")
    }

    #[test]
    fn extension_match_is_case_insensitive() {
        assert!(is_supported(Path::new("a.MP3")));
        assert!(is_supported(Path::new("a.Flac")));
        assert!(is_supported(Path::new("a.ogg")));
        assert!(!is_supported(Path::new("a.txt")));
        assert!(!is_supported(Path::new("no-extension")));
    }

    #[test]
    fn stable_key_changes_with_content() {
        let dir = std::env::temp_dir().join(format!("tunex-key-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("setup works");
        let file = dir.join("a.wav");
        std::fs::write(&file, [0u8; 16]).expect("setup works");
        let before = stable_key(&file);
        std::fs::write(&file, [0u8; 32]).expect("setup works");
        let after = stable_key(&file);
        assert_ne!(before, after, "size change invalidates the key");
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn scan_indexes_supported_files_only() {
        let dir = scratch("scan");
        let nested = dir.join("nested");
        std::fs::create_dir_all(&nested).expect("setup works");
        std::fs::write(dir.join("a.flac"), []).expect("setup works");
        std::fs::write(dir.join("b.txt"), []).expect("setup works");
        std::fs::write(nested.join("c.OGG"), []).expect("setup works");
        let mut db = open_memory().expect("db opens");
        let stats = scan_folder(&mut db, &dir).expect("scan works");
        assert_eq!(stats.files_seen, 3);
        assert_eq!(stats.tracks_added, 2);
        // Empty headers are tag-unreadable but still indexed by path.
        assert_eq!(stats.metadata_failed, 2);
        assert_eq!(list_tracks(&db).expect("list works").len(), 2);
        // Rescanning refreshes instead of duplicating.
        let again = scan_folder(&mut db, &dir).expect("rescan works");
        assert_eq!(again.tracks_added, 2);
        assert_eq!(list_tracks(&db).expect("list works").len(), 2);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn corrupt_file_indexes_by_path_and_counts_failure() {
        let dir = scratch("corrupt");
        std::fs::create_dir_all(&dir).expect("setup works");
        std::fs::copy(corrupt_source(), dir.join("corrupt.mp3")).expect("setup works");
        let mut db = open_memory().expect("db opens");
        let stats = scan_folder(&mut db, &dir).expect("scan works");
        assert_eq!(stats.tracks_added, 1);
        assert_eq!(stats.metadata_failed, 1);
        let tracks = list_tracks(&db).expect("list works");
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, None, "unknown stays unknown");
        assert!(!tracks[0].missing);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn rescan_leaves_untouched_files_unread() {
        let dir = scratch("unchanged");
        std::fs::create_dir_all(&dir).expect("setup works");
        std::fs::copy(corrupt_source(), dir.join("same.mp3")).expect("setup works");
        let mut db = open_memory().expect("db opens");
        let first = scan_folder(&mut db, &dir).expect("first scan works");
        assert_eq!(first.tracks_unchanged, 0, "a first index reads every file");
        let second = scan_folder(&mut db, &dir).expect("rescan works");
        assert_eq!(second.tracks_unchanged, 1);
        assert_eq!(
            second.tracks_added, 1,
            "a skipped file is still an indexed track"
        );
        assert_eq!(
            second.metadata_failed, 0,
            "the skip means the tags were never read again"
        );
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn rescan_reindexes_a_file_whose_contents_changed() {
        let dir = scratch("edited");
        std::fs::create_dir_all(&dir).expect("setup works");
        let file = dir.join("edited.flac");
        std::fs::write(&file, [3u8; 64]).expect("setup works");
        let mut db = open_memory().expect("db opens");
        scan_folder(&mut db, &dir).expect("first scan works");
        std::fs::write(&file, [3u8; 128]).expect("setup works");
        let stats = scan_folder(&mut db, &dir).expect("rescan works");
        assert_eq!(stats.tracks_unchanged, 0, "a changed key forces a re-read");
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn rename_retargets_row_without_duplicating() {
        let dir = scratch("rename");
        std::fs::create_dir_all(&dir).expect("setup works");
        std::fs::write(dir.join("before.flac"), [1u8; 64]).expect("setup works");
        let mut db = open_memory().expect("db opens");
        scan_folder(&mut db, &dir).expect("first scan works");
        let id = list_tracks(&db).expect("list works")[0].id;
        std::fs::rename(dir.join("before.flac"), dir.join("after.flac")).expect("setup works");
        let stats = scan_folder(&mut db, &dir).expect("rescan works");
        assert_eq!(stats.renamed, 1);
        let tracks = list_tracks(&db).expect("list works");
        assert_eq!(tracks.len(), 1, "rename must not duplicate");
        assert_eq!(tracks[0].id, id, "row identity (history links) survives");
        assert!(tracks[0].path.ends_with("after.flac"));
        assert!(!tracks[0].missing);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn deleted_file_flags_missing_and_return_clears_it() {
        let dir = scratch("missing");
        std::fs::create_dir_all(&dir).expect("setup works");
        std::fs::write(dir.join("gone.flac"), [2u8; 64]).expect("setup works");
        let mut db = open_memory().expect("db opens");
        scan_folder(&mut db, &dir).expect("first scan works");
        std::fs::remove_file(dir.join("gone.flac")).expect("setup works");
        let stats = scan_folder(&mut db, &dir).expect("rescan works");
        assert_eq!(stats.missing_marked, 1);
        let tracks = list_tracks(&db).expect("list works");
        assert_eq!(tracks.len(), 1, "missing rows are kept, never dropped");
        assert!(tracks[0].missing);
        // The file returns: rescan clears the flag on the same row.
        std::fs::write(dir.join("gone.flac"), [2u8; 64]).expect("setup works");
        scan_folder(&mut db, &dir).expect("third scan works");
        let tracks = list_tracks(&db).expect("list works");
        assert_eq!(tracks.len(), 1);
        assert!(!tracks[0].missing);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn tagged_file_scans_with_full_metadata() {
        let dir = scratch("tagged");
        std::fs::create_dir_all(&dir).expect("setup works");
        std::fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/sine.flac"),
            dir.join("sine.flac"),
        )
        .expect("setup works");
        // Tag the copy in place (same write path the metadata test proves).
        let tagged = dir.join("sine.flac");
        let mut file = lofty::probe::Probe::open(&tagged)
            .and_then(lofty::probe::Probe::read)
            .expect("setup reads");
        let mut tag = lofty::tag::Tag::new(lofty::tag::TagType::VorbisComments);
        tag.set_artist("Nova Rae".to_owned());
        tag.set_title("Midnight".to_owned());
        tag.set_album("Night Tapes".to_owned());
        file.insert_tag(tag);
        file.save_to_path(&tagged, lofty::config::WriteOptions::default())
            .expect("setup writes");

        let mut db = open_memory().expect("db opens");
        let stats = scan_folder(&mut db, &dir).expect("scan works");
        assert_eq!(stats.metadata_failed, 0);
        let tracks = list_tracks(&db).expect("list works");
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title.as_deref(), Some("Midnight"));
        assert_eq!(tracks[0].artist.as_deref(), Some("Nova Rae"));
        assert_eq!(tracks[0].album.as_deref(), Some("Night Tapes"));
        assert!(tracks[0].duration_ms.unwrap_or(0) > 0);
        assert_eq!(
            crate::db::search_track_ids(&db, "midnight")
                .expect("search works")
                .len(),
            1
        );
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[tokio::test]
    async fn live_scan_streams_progress_to_finished() {
        let dir = scratch("live");
        std::fs::create_dir_all(&dir).expect("setup works");
        std::fs::write(dir.join("a.flac"), []).expect("setup works");
        std::fs::write(dir.join("b.txt"), []).expect("setup works");
        let db_path = dir.join("library.db");
        let (updates, mut receiver) = mpsc::channel(16);
        let stats = scan_folder_live(db_path.clone(), dir.clone(), updates)
            .await
            .expect("live scan works");
        assert_eq!(stats.tracks_added, 1);
        let mut saw_finished = false;
        while let Some(update) = receiver.recv().await {
            assert_eq!(update.root, dir);
            if update.finished {
                assert_eq!(update.stats, stats);
                saw_finished = true;
                break;
            }
        }
        assert!(saw_finished, "live scan ends with a finished snapshot");
        // The worker's own connection indexed the row into the file db.
        let db = open_file(&db_path).expect("file reopens");
        assert_eq!(list_tracks(&db).expect("list works").len(), 1);
        drop(db);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }
}
