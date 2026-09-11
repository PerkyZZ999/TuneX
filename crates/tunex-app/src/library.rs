//! Library orchestration: folder config, scan worker, watcher batches.
//!
//! [`LibraryCore`] owns the whole background half of the folders UI: the
//! configured roots (persisted in `config.toml`), one scan worker thread at a
//! time, and the debounced filesystem watcher. The Qt side (see
//! `bridge::library_manager`) polls [`LibraryCore::poll`] on a timer and
//! renders plain getters — worker threads never touch a `QObject`, and the
//! UI thread never scans, decodes, or queries (D-007, R-NFR-01).
//!
//! Lifecycle: `startup` loads folders, starts the watcher, and scans;
//! watcher batches and explicit `rescan` calls funnel through one pending
//! flag, so bursts coalesce into a single follow-up run instead of piling
//! workers. Progress snapshots stream over a bounded channel; when the
//! sender drops, the run is over and [`LibraryCore::take_finished`] reports
//! it exactly once so views refresh a single time.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc as std_mpsc};
use std::time::Duration;

use tokio::sync::mpsc as tokio_mpsc;
use tunex_core::{Error, Result, TunexConfig, load_from, save_to};
use tunex_library::{
    DEBOUNCE_WINDOW, LibraryWatcher, ScanStats, open_file, remove_library_root,
    scan_folder_with_callback, watch_roots,
};

/// Mutable orchestration state, shared with the scan worker.
#[derive(Debug, Default)]
struct CoreState {
    /// A scan worker is currently running.
    scanning: bool,
    /// A rescan was requested while busy (or by the watcher): run again after.
    pending_rescan: bool,
    /// A run completed since the last [`LibraryCore::take_finished`].
    finished: bool,
    /// Counters of the current (or most recent) run.
    last_stats: ScanStats,
    /// Configured roots (mirrors `config.toml`).
    folders: Vec<PathBuf>,
    /// Last folder-operation failure, surfaced to the UI until cleared.
    last_error: Option<String>,
}

/// Lock the orchestration state, recovering from a poisoned mutex.
///
/// Poison means a worker panicked mid-update; the counters may be stale but
/// the shape is intact, and wedging the whole UI over it would be worse.
fn lock_state(mutex: &Mutex<CoreState>) -> std::sync::MutexGuard<'_, CoreState> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Folder config, scan worker, and watcher batches behind a poll interface.
pub struct LibraryCore {
    config_path: PathBuf,
    db_path: PathBuf,
    debounce: Duration,
    state: Arc<Mutex<CoreState>>,
    scan_rx: Option<std_mpsc::Receiver<ScanStats>>,
    watch_rx: Option<tokio_mpsc::Receiver<Vec<PathBuf>>>,
    watcher: Option<LibraryWatcher>,
}

impl std::fmt::Debug for LibraryCore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LibraryCore")
            .field("config_path", &self.config_path)
            .field("db_path", &self.db_path)
            .finish_non_exhaustive()
    }
}

impl LibraryCore {
    /// Orchestration over the default config and index paths.
    #[must_use]
    pub fn new() -> Self {
        Self::with_paths(tunex_core::config_file(), tunex_core::library_db_path())
    }

    /// Orchestration over explicit paths (tests point these at scratch dirs).
    #[must_use]
    pub fn with_paths(config_path: PathBuf, db_path: PathBuf) -> Self {
        Self {
            config_path,
            db_path,
            debounce: DEBOUNCE_WINDOW,
            state: Arc::new(Mutex::new(CoreState::default())),
            scan_rx: None,
            watch_rx: None,
            watcher: None,
        }
    }
}

impl Default for LibraryCore {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryCore {
    /// Shorten the watcher quiet window (tests; production uses [`DEBOUNCE_WINDOW`]).
    pub fn set_debounce(&mut self, debounce: Duration) {
        self.debounce = debounce;
    }

    /// Load folders, start the watcher, and scan. Missing config starts
    /// empty (first run); an unreadable index surfaces on first scan.
    pub fn startup(&mut self) {
        let config = match load_from(&self.config_path) {
            Ok(config) => config,
            Err(err) => {
                tracing::warn!(name = "folders.config_fallback", error = %err, "starting empty");
                TunexConfig::default()
            }
        };
        lock_state(&self.state).folders = config.library_roots;
        self.restart_watcher();
        self.rescan();
    }

    /// Drain finished progress snapshots and watcher batches; start a pending
    /// scan when idle. The single call the UI timer makes, at any cadence.
    pub fn poll(&mut self) {
        // Watcher batches (non-blocking): any change requests a rescan.
        if let Some(watch_rx) = &mut self.watch_rx {
            let mut batches = 0_u32;
            while watch_rx.try_recv().is_ok() {
                batches += 1;
            }
            if batches > 0 {
                tracing::debug!(name = "folders.watch_batch", batches, "rescan requested");
                lock_state(&self.state).pending_rescan = true;
            }
        }
        // Scan progress: keep the latest snapshot; a dropped sender ends the run.
        if self.scan_rx.is_some() {
            let mut disconnected = false;
            while let Some(scan_rx) = &self.scan_rx {
                match scan_rx.try_recv() {
                    Ok(stats) => {
                        lock_state(&self.state).last_stats = stats;
                    }
                    Err(std_mpsc::TryRecvError::Empty) => break,
                    Err(std_mpsc::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
            if disconnected {
                self.scan_rx = None;
                let mut state = lock_state(&self.state);
                state.scanning = false;
                state.finished = true;
                tracing::info!(
                    name = "folders.scan_finished",
                    files_seen = state.last_stats.files_seen,
                    tracks_added = state.last_stats.tracks_added,
                    "scan run complete"
                );
            }
        }
        // One worker at a time: bursts coalesce into a single follow-up run.
        let pending = lock_state(&self.state).pending_rescan;
        let scanning = lock_state(&self.state).scanning;
        if pending && !scanning {
            lock_state(&self.state).pending_rescan = false;
            self.rescan();
        }
    }

    /// Request a scan: starts immediately when idle, else queues one run.
    ///
    /// # Panics
    ///
    /// Panics when the OS refuses the worker thread (resource exhaustion) —
    /// a broken host, not a recoverable scan failure.
    pub fn rescan(&mut self) {
        if lock_state(&self.state).scanning {
            lock_state(&self.state).pending_rescan = true;
            return;
        }
        let roots = lock_state(&self.state).folders.clone();
        if roots.is_empty() {
            return;
        }
        let (scan_tx, scan_rx) = std_mpsc::channel();
        self.scan_rx = Some(scan_rx);
        lock_state(&self.state).scanning = true;
        lock_state(&self.state).finished = false;
        let db_path = self.db_path.clone();
        let state = Arc::clone(&self.state);
        let thread_roots = roots.clone();
        // Detached by design: the run always ends with a final snapshot (or a
        // dropped sender on panic), which `poll` translates into `finished`.
        // Process teardown mid-run is safe — every upsert commits atomically.
        std::thread::Builder::new()
            .name("tunex-scan".to_owned())
            .spawn(move || {
                scan_all_roots(&db_path, &thread_roots, &scan_tx, &state);
            })
            .expect("scan thread spawns");
        tracing::info!(
            name = "folders.scan_started",
            roots = roots.len(),
            "scan run started"
        );
    }

    /// Whether a scan worker is currently running.
    #[must_use]
    pub fn is_scanning(&self) -> bool {
        lock_state(&self.state).scanning
    }

    /// One-line status for the progress surface.
    #[must_use]
    pub fn status_text(&self) -> String {
        let state = lock_state(&self.state);
        if state.scanning {
            format!(
                "Scanning… {} files · {} tracks",
                state.last_stats.files_seen, state.last_stats.tracks_added
            )
        } else if state.last_stats.files_seen > 0 {
            format!(
                "Up to date · {} tracks from {} files",
                state.last_stats.tracks_added, state.last_stats.files_seen
            )
        } else {
            "Idle".to_owned()
        }
    }

    /// Report a completed run exactly once (views refresh on `true`).
    pub fn take_finished(&mut self) -> bool {
        let mut state = lock_state(&self.state);
        std::mem::replace(&mut state.finished, false)
    }

    /// Configured roots, in config order.
    #[must_use]
    pub fn folders(&self) -> Vec<PathBuf> {
        lock_state(&self.state).folders.clone()
    }

    /// Last folder-operation failure, if any (cleared by the next success).
    #[must_use]
    pub fn error_text(&self) -> Option<String> {
        lock_state(&self.state).last_error.clone()
    }

    /// Add a folder: canonicalized, deduplicated, persisted, watched, scanned.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the path is missing, not a directory, or
    /// the config cannot be persisted.
    pub fn add_folder(&mut self, path: &Path) -> Result<()> {
        let canonical = match canonical_folder(path) {
            Ok(canonical) => canonical,
            Err(err) => {
                lock_state(&self.state).last_error = Some(err.to_string());
                return Err(err);
            }
        };
        let mut state = lock_state(&self.state);
        if state.folders.contains(&canonical) {
            return Ok(());
        }
        state.folders.push(canonical);
        state.last_error = None;
        drop(state);
        if let Err(err) = self.persist_folders() {
            lock_state(&self.state).last_error = Some(err.to_string());
            return Err(err);
        }
        self.restart_watcher();
        self.rescan();
        Ok(())
    }

    /// Remove a folder: unwatched, unpersisted, rows garbage-collected.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the config cannot be persisted and
    /// [`Error::Database`] when the index cleanup fails.
    pub fn remove_folder(&mut self, path: &Path) -> Result<()> {
        // Match the canonical form when the folder still exists, else the
        // literal form (removing a plugged-out drive must still work).
        let key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let mut state = lock_state(&self.state);
        let Some(position) = state.folders.iter().position(|root| root == &key) else {
            return Ok(());
        };
        let removed = state.folders.remove(position);
        state.last_error = None;
        drop(state);
        if self.db_path.is_file() {
            let db = open_file(&self.db_path)?;
            let collected = remove_library_root(&db, &removed.to_string_lossy())?;
            tracing::info!(
                name = "folders.root_removed",
                root = %removed.display(),
                tracks_collected = collected,
                "folder removed with its rows"
            );
        }
        if let Err(err) = self.persist_folders() {
            lock_state(&self.state).last_error = Some(err.to_string());
            return Err(err);
        }
        self.restart_watcher();
        Ok(())
    }

    /// Persist the in-memory folders over the config file.
    fn persist_folders(&self) -> Result<()> {
        let mut config = load_from(&self.config_path).unwrap_or_default();
        config
            .library_roots
            .clone_from(&lock_state(&self.state).folders);
        save_to(&self.config_path, &config)
    }

    /// Whether the filesystem watcher is currently active.
    #[must_use]
    pub fn is_watching(&self) -> bool {
        self.watcher.is_some()
    }

    /// (Re)start the watcher over the current folders. Failures degrade to
    /// no watching (manual rescans still work) with a warning, never a crash.
    fn restart_watcher(&mut self) {
        // Dropping the old watcher joins its thread before the new one starts.
        self.watcher = None;
        self.watch_rx = None;
        let folders = lock_state(&self.state).folders.clone();
        if folders.is_empty() {
            return;
        }
        let (watch_tx, watch_rx) = tokio_mpsc::channel(32);
        match watch_roots(&folders, self.debounce, watch_tx) {
            Ok(watcher) => {
                self.watch_rx = Some(watch_rx);
                self.watcher = Some(watcher);
            }
            Err(err) => {
                tracing::warn!(name = "folders.watch_failed", error = %err, "continuing unwatched");
            }
        }
    }
}

/// Validate one folder pick: must exist and be a directory; stored canonical
/// so symlinked picks cannot escape the roots they name (R-002, R-NFR-03).
fn canonical_folder(path: &Path) -> Result<PathBuf> {
    if !path.is_dir() {
        return Err(Error::Io {
            path: path.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::NotADirectory, "not a music directory"),
        });
    }
    std::fs::canonicalize(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Scan every root on the worker thread, streaming snapshots; the final
/// snapshot always sends (even on open failure) so `poll` can finish the run.
fn scan_all_roots(
    db_path: &Path,
    roots: &[PathBuf],
    scan_tx: &std_mpsc::Sender<ScanStats>,
    state: &Mutex<CoreState>,
) {
    let mut db = match open_file(db_path) {
        Ok(db) => db,
        Err(err) => {
            tracing::warn!(name = "folders.index_open_failed", error = %err, "scan aborted");
            let _ = scan_tx.send(ScanStats::default());
            return;
        }
    };
    let mut total = ScanStats::default();
    for root in roots {
        match scan_folder_with_callback(&mut db, root, |snapshot| {
            let combined = ScanStats {
                files_seen: total.files_seen + snapshot.files_seen,
                tracks_added: total.tracks_added + snapshot.tracks_added,
                tracks_unchanged: total.tracks_unchanged + snapshot.tracks_unchanged,
                metadata_failed: total.metadata_failed + snapshot.metadata_failed,
                renamed: total.renamed + snapshot.renamed,
                missing_marked: total.missing_marked + snapshot.missing_marked,
            };
            lock_state(state).last_stats = combined;
            let _ = scan_tx.send(combined);
        }) {
            Ok(run_stats) => {
                total.files_seen += run_stats.files_seen;
                total.tracks_added += run_stats.tracks_added;
                total.tracks_unchanged += run_stats.tracks_unchanged;
                total.metadata_failed += run_stats.metadata_failed;
                total.renamed += run_stats.renamed;
                total.missing_marked += run_stats.missing_marked;
            }
            Err(err) => {
                tracing::warn!(
                    name = "folders.root_scan_failed",
                    root = %root.display(),
                    error = %err,
                    "continuing with remaining roots"
                );
            }
        }
    }
    lock_state(state).last_stats = total;
    let _ = scan_tx.send(total);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// Scratch config + db paths in a unique dir (parallel-safe).
    fn scratch(case: &str) -> (PathBuf, PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("tunex-folders-{case}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("setup works");
        (dir.join("config.toml"), dir.join("library.db"), dir)
    }

    /// Poll until no worker runs and nothing is queued (every test ends idle
    /// so detached workers never outlive their scratch dir).
    fn run_until_idle(core: &mut LibraryCore) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            core.poll();
            let idle = {
                let state = lock_state(&core.state);
                !state.scanning && !state.pending_rescan
            };
            if idle {
                return;
            }
            assert!(Instant::now() < deadline, "scan worker never went idle");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Seed one scannable file inside `dir/music`.
    fn seed_music(dir: &Path) -> PathBuf {
        let music = dir.join("music");
        std::fs::create_dir_all(&music).expect("setup works");
        std::fs::write(music.join("a.flac"), []).expect("setup works");
        music
    }

    #[test]
    fn fresh_core_is_idle_with_no_folders() {
        let (config, db, _dir) = scratch("fresh");
        let core = LibraryCore::with_paths(config, db);
        assert!(!core.is_scanning());
        assert!(!core.is_watching());
        assert!(core.folders().is_empty());
        assert_eq!(core.status_text(), "Idle");
    }

    #[test]
    fn add_folder_persists_scans_and_reports() {
        let (config, db, dir) = scratch("add");
        let music = seed_music(&dir);
        let mut core = LibraryCore::with_paths(config.clone(), db.clone());
        core.startup();
        core.add_folder(&music).expect("add works");
        assert!(core.is_watching(), "watcher follows folders");
        assert_eq!(
            core.folders(),
            vec![music.canonicalize().expect("setup works")]
        );
        // Persisted to config.toml.
        let saved = std::fs::read_to_string(&config).expect("config written");
        assert!(saved.contains("music"), "roots persist, got: {saved}");
        run_until_idle(&mut core);
        assert!(core.take_finished(), "completed run reported once");
        assert!(!core.take_finished(), "finish flag consumed");
        assert!(core.status_text().contains("tracks"), "status names tracks");
        let reopened = open_file(&db).expect("db reopens");
        assert_eq!(
            tunex_library::list_tracks(&reopened)
                .expect("list works")
                .len(),
            1
        );
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn restart_restores_folders_and_rows() {
        let (config, db, dir) = scratch("restart");
        let music = seed_music(&dir);
        let mut core = LibraryCore::with_paths(config.clone(), db.clone());
        core.startup();
        core.add_folder(&music).expect("add works");
        run_until_idle(&mut core);
        drop(core);
        // A fresh core over the same paths restores everything.
        let mut revived = LibraryCore::with_paths(config, db.clone());
        revived.startup();
        assert_eq!(revived.folders().len(), 1, "folders survive restart");
        let reopened = open_file(&db).expect("db reopens");
        assert_eq!(
            tunex_library::list_tracks(&reopened)
                .expect("list works")
                .len(),
            1,
            "rows survive restart"
        );
        run_until_idle(&mut revived);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn remove_folder_unpersists_and_collects_rows() {
        let (config, db, dir) = scratch("remove");
        let music = seed_music(&dir);
        let mut core = LibraryCore::with_paths(config.clone(), db.clone());
        core.startup();
        core.add_folder(&music).expect("add works");
        run_until_idle(&mut core);
        core.remove_folder(&music).expect("remove works");
        assert!(core.folders().is_empty());
        let saved = std::fs::read_to_string(&config).expect("config written");
        assert!(!saved.contains("music"), "roots unpersist, got: {saved}");
        let reopened = open_file(&db).expect("db reopens");
        assert!(
            tunex_library::list_tracks(&reopened)
                .expect("list works")
                .is_empty(),
            "removed roots garbage-collect"
        );
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn non_directory_pick_errors_loudly() {
        let (config, db, dir) = scratch("bad-pick");
        let file = dir.join("not-a-dir.flac");
        std::fs::write(&file, []).expect("setup works");
        let mut core = LibraryCore::with_paths(config, db);
        let err = core.add_folder(&file).expect_err("files are not folders");
        assert!(matches!(err, Error::Io { .. }));
        assert!(core.folders().is_empty());
        assert!(core.error_text().is_some_and(|text| !text.is_empty()));
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn watcher_batch_triggers_a_follow_up_scan() {
        let (config, db, dir) = scratch("watch");
        let music = seed_music(&dir);
        let mut core = LibraryCore::with_paths(config, db.clone());
        core.set_debounce(Duration::from_millis(50));
        core.startup();
        core.add_folder(&music).expect("add works");
        run_until_idle(&mut core);
        // A new file lands: the watcher batch must wake a second run.
        std::fs::write(music.join("b.flac"), []).expect("setup works");
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            core.poll();
            if core.is_scanning() {
                break;
            }
            assert!(Instant::now() < deadline, "watcher never woke a rescan");
            std::thread::sleep(Duration::from_millis(20));
        }
        run_until_idle(&mut core);
        let reopened = open_file(&db).expect("db reopens");
        assert_eq!(
            tunex_library::list_tracks(&reopened)
                .expect("list works")
                .len(),
            2,
            "watch-triggered rescan indexes the new file"
        );
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }
}
