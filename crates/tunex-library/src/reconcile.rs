//! Rescan reconciliation policy (C11): what one walked file — or one known
//! row the walk never saw — means for the index.
//!
//! The scanner walks, this module decides, and `db` executes: identity keys,
//! rename eligibility, and the missing sweep read as one policy here instead
//! of scattered across walk order, SQL helpers, and config. Every decision
//! below is pure over caller-supplied snapshots, so the full rename / move /
//! delete / rescan matrix is a table of unit cases with no filesystem beyond
//! the existence checks rename eligibility needs.
//!
//! Key formats themselves stay in `scan` (`stable_key`, `file_id`); changing
//! them goes through D-009, not through this module.

use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;
use std::path::{Path, PathBuf};

use super::db::TrackIdentity;

/// What a walked file means for its index row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Known path, clean flags, identity key still matches: count and skip.
    Unchanged,
    /// Known path but stale (edited, incomplete, or reappeared): re-upsert.
    Refresh,
    /// Unknown path whose filesystem identity matches a vanished row:
    /// retarget that row (history links survive).
    Retarget(i64),
    /// Anything else: insert a new row.
    Insert,
}

/// Cheap early-return check (stat-derived key only, no tag read): a known,
/// clean row whose identity key still matches needs no work.
#[must_use]
pub fn is_unchanged(previous: Option<&TrackIdentity>, stable_key: &str) -> bool {
    previous.is_some_and(|row| !row.missing && !row.incomplete && row.stable_key == stable_key)
}

/// Full decision for a walked file whose fresh identity is already known.
/// Callers read tags first, then execute exactly one of: re-upsert the known
/// path, atomically retarget a rename, or insert.
#[must_use]
pub fn decide<S: BuildHasher>(
    by_path: &HashMap<&str, &TrackIdentity, S>,
    by_file_id: &HashMap<&str, &TrackIdentity, S>,
    track_path: &str,
    track_file_id: Option<&str>,
) -> Decision {
    if by_path.contains_key(track_path) {
        return Decision::Refresh;
    }
    if let Some(previous) = track_file_id
        .and_then(|file_id| by_file_id.get(file_id))
        .filter(|previous| is_rename_target(previous, Path::new(track_path)))
    {
        return Decision::Retarget(previous.id);
    }
    Decision::Insert
}

/// Whether a vanished row may lend its identity to `path`: the old path must
/// really be gone (a live file with a recycled inode keeps its own row) and
/// the extension must match (deleted rows free their inodes for unrelated
/// reuse; real renames always keep the container).
#[must_use]
pub fn is_rename_target(previous: &TrackIdentity, path: &Path) -> bool {
    !Path::new(&previous.path).exists() && Path::new(&previous.path).extension() == path.extension()
}

/// Row ids the deletion pass must flag missing: known rows under `prefix`
/// that the walk never saw and that no rename retargeted. Flagging itself
/// stays with the caller (it writes); deciding stays here (it reads).
#[must_use]
pub fn missing_ids<S: BuildHasher>(
    known: &[TrackIdentity],
    seen: &HashSet<PathBuf, S>,
    retargeted: &HashSet<i64, S>,
    prefix: &str,
) -> Vec<i64> {
    known
        .iter()
        .filter(|row| {
            !retargeted.contains(&row.id)
                && !seen.contains(Path::new(&row.path))
                && row.path.starts_with(prefix)
        })
        .filter(|row| !row.missing)
        .map(|row| row.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, path: &str) -> TrackIdentity {
        TrackIdentity {
            id,
            path: path.to_owned(),
            file_id: Some(format!("9:{id}")),
            stable_key: format!("{path}:1:2"),
            missing: false,
            incomplete: false,
        }
    }

    fn maps<'a>(
        rows: &'a [&'a TrackIdentity],
    ) -> (
        HashMap<&'a str, &'a TrackIdentity>,
        HashMap<&'a str, &'a TrackIdentity>,
    ) {
        let mut by_path = HashMap::new();
        let mut by_file_id = HashMap::new();
        for row in rows {
            by_path.insert(row.path.as_str(), *row);
            if let Some(file_id) = row.file_id.as_deref() {
                by_file_id.insert(file_id, *row);
            }
        }
        (by_path, by_file_id)
    }

    #[test]
    fn unchanged_needs_clean_flags_and_matching_key() {
        let row = row(1, "/m/a.flac");
        assert!(is_unchanged(Some(&row), "/m/a.flac:1:2"));
        assert!(
            !is_unchanged(None, "/m/a.flac:1:2"),
            "unknown is never unchanged"
        );
        assert!(
            !is_unchanged(Some(&row), "/m/a.flac:9:9"),
            "edited key refreshes"
        );
        let mut missing = row.clone();
        missing.missing = true;
        assert!(!is_unchanged(Some(&missing), "/m/a.flac:1:2"));
        let mut partial = row.clone();
        partial.incomplete = true;
        assert!(!is_unchanged(Some(&partial), "/m/a.flac:1:2"));
    }

    #[test]
    fn known_path_refreshes() {
        let known = row(1, "/m/a.flac");
        let refs = [&known];
        let (by_path, by_file_id) = maps(&refs);
        assert_eq!(
            decide(&by_path, &by_file_id, "/m/a.flac", Some("9:1")),
            Decision::Refresh
        );
    }

    #[test]
    fn vanished_row_with_recycled_inode_retargets_on_extension_match() {
        // `gone.flac` is deleted (never created), so the guard passes; a new
        // file reusing its inode retargets only with a matching extension.
        let vanished = row(1, "/m/gone.flac");
        let refs = [&vanished];
        let (by_path, by_file_id) = maps(&refs);
        assert_eq!(
            decide(&by_path, &by_file_id, "/m/fresh.flac", Some("9:1")),
            Decision::Retarget(1)
        );
        assert_eq!(
            decide(&by_path, &by_file_id, "/m/fresh.ogg", Some("9:1")),
            Decision::Insert,
            "extension change means a new file, not a rename"
        );
    }

    #[test]
    fn live_file_never_lends_its_row() {
        let dir = std::env::temp_dir().join(format!("tunex-reconcile-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("setup works");
        std::fs::write(dir.join("live.flac"), [1u8; 8]).expect("setup works");
        let live = TrackIdentity {
            path: dir.join("live.flac").to_string_lossy().into_owned(),
            ..row(1, "/m/live.flac")
        };
        assert!(!is_rename_target(&live, &dir.join("live.flac")));
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn missing_sweep_skips_seen_retargeted_and_out_of_root() {
        let seen_row = row(1, "/m/seen.flac");
        let gone_row = row(2, "/m/gone.flac");
        let mut moved_row = row(3, "/m/moved.flac");
        moved_row.missing = true;
        let other_row = row(4, "/podcasts/keep.flac");
        let known = vec![seen_row, gone_row, moved_row, other_row];
        let seen: HashSet<PathBuf> = HashSet::from([PathBuf::from("/m/seen.flac")]);
        let retargeted: HashSet<i64> = HashSet::from([3]);
        let ids = missing_ids(&known, &seen, &retargeted, "/m/");
        assert_eq!(ids, vec![2]);
    }
}
