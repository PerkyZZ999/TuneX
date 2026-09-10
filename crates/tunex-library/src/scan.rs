//! Filesystem scanner: walk library roots, index supported media files.
//!
//! S1 W-007 harness scope: recursive walk, extension filter, stable-key
//! computation, index upserts with live counts. Metadata extraction, artwork,
//! deletion reconcile, and the `notify` watcher arrive in S2 — the scanner
//! only ever *adds or refreshes* rows here.
//!
//! Symlinks (files and directories) are skipped: they cannot escape roots
//! they never enter, which keeps the S1 trust story trivially safe.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tunex_core::Result;

use crate::db::{NewTrack, add_root, upsert_track};

/// Media extensions the scanner indexes (lowercase, no dots). Decodability is
/// the player's concern; the scanner only recognizes containers.
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "oga", "opus", "m4a", "m4b", "wav", "aif", "aiff",
];

/// Live counters reported while a scan runs (UI progress in S2 reads these).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScanStats {
    /// Files walked, supported or not.
    pub files_seen: u64,
    /// Rows inserted or refreshed.
    pub tracks_added: u64,
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

/// Walk `root` recursively, registering it and upserting every supported file.
/// Returns live counters; never fails on individual files (unreadable entries
/// are skipped so one bad file cannot abort a scan).
///
/// Metadata stays empty here — W-013 fills rows from [`read_metadata`](super::metadata::read_metadata).
///
/// # Errors
///
/// Returns [`Error::Database`] when root registration or an upsert fails (I/O
/// problems on individual entries are skipped, not raised).
pub fn scan_folder(db: &mut Connection, root: &Path) -> Result<ScanStats> {
    add_root(db, &root.to_string_lossy())?;
    let mut stats = ScanStats::default();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
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
                stats.files_seen += 1;
                if is_supported(&path) {
                    let path = path.to_string_lossy().into_owned();
                    upsert_track(
                        db,
                        &NewTrack {
                            stable_key: stable_key(Path::new(&path)),
                            path,
                            ..Default::default()
                        },
                    )?;
                    stats.tracks_added += 1;
                }
            }
        }
    }
    Ok(stats)
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
    use crate::db::{list_tracks, open_memory};

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
        let dir = std::env::temp_dir().join(format!("tunex-scan-{}", std::process::id()));
        let nested = dir.join("nested");
        std::fs::create_dir_all(&nested).expect("setup works");
        std::fs::write(dir.join("a.flac"), []).expect("setup works");
        std::fs::write(dir.join("b.txt"), []).expect("setup works");
        std::fs::write(nested.join("c.OGG"), []).expect("setup works");
        let mut db = open_memory().expect("db opens");
        let stats = scan_folder(&mut db, &dir).expect("scan works");
        assert_eq!(stats.files_seen, 3);
        assert_eq!(stats.tracks_added, 2);
        assert_eq!(list_tracks(&db).expect("list works").len(), 2);
        // Rescanning refreshes instead of duplicating.
        let again = scan_folder(&mut db, &dir).expect("rescan works");
        assert_eq!(again.tracks_added, 2);
        assert_eq!(list_tracks(&db).expect("list works").len(), 2);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }
}
