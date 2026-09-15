//! Debounced search orchestration: keystrokes in, grouped results out.
//!
//! [`SearchCore`] mirrors the [`LibraryCore`](super::library::LibraryCore)
//! poll protocol: the UI thread calls [`submit`](SearchCore::submit) on every
//! keystroke and [`poll`](SearchCore::poll) on its timer, while a worker
//! thread owns the debounce window and the database connection — typing never
//! blocks on FTS5 (D-007, R-NFR-01). Generations make stale results
//! impossible: each submit bumps a counter, the worker drops outcomes a newer
//! query superseded, and results carrying an old generation are dropped on
//! receipt as a backstop.

use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use tunex_core::{Error, Result};
use tunex_library::{SearchResults, open_file, search_library};

/// Keystroke quiet window before a query runs (R-008: ~150 ms debounce).
///
/// Long enough to coalesce fast typing into one FTS5 query per pause, short
/// enough that results feel instant. Raising it delays every result equally;
/// lowering it multiplies database work per keystroke.
pub const SEARCH_DEBOUNCE: Duration = Duration::from_millis(150);

/// Debounced search over the library index behind a poll interface.
#[derive(Debug)]
pub struct SearchCore {
    db_path: PathBuf,
    debounce: Duration,
    submitted: u64,
    settled: u64,
    delivered: Option<SearchResults>,
    last_error: Option<String>,
    worker: Option<SearchWorker>,
}

/// Owns the debounce thread: dropping disconnects its query channel and
/// joins it, so no worker outlives the handle.
#[derive(Debug)]
struct SearchWorker {
    /// Released on drop so a blocked `recv` wakes up.
    queries: Option<std_mpsc::Sender<(u64, String)>>,
    /// Settled results, drained by [`SearchCore::poll`].
    results: std_mpsc::Receiver<(u64, Result<SearchResults, Error>)>,
    /// Joined on drop for a clean shutdown.
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for SearchWorker {
    fn drop(&mut self) {
        // Release the query sender first so a blocked `recv` wakes up.
        drop(self.queries.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl SearchCore {
    /// Search over the default index path.
    #[must_use]
    pub fn new() -> Self {
        Self::with_db_path(tunex_core::library_db_path())
    }

    /// Search over an explicit index path (tests point this at scratch dirs).
    #[must_use]
    pub fn with_db_path(db_path: PathBuf) -> Self {
        Self {
            db_path,
            debounce: SEARCH_DEBOUNCE,
            submitted: 0,
            settled: 0,
            delivered: None,
            last_error: None,
            worker: None,
        }
    }

    /// Shorten the quiet window (tests; production uses [`SEARCH_DEBOUNCE`]).
    pub fn set_debounce(&mut self, debounce: Duration) {
        self.debounce = debounce;
    }

    /// Submit raw query text. The worker spawns lazily on the first submit;
    /// every submit supersedes all older ones (stale outcomes never surface)
    /// and clears any previous failure (a new attempt owns the error line).
    pub fn submit(&mut self, query: &str) {
        self.submitted += 1;
        self.last_error = None;
        let generation = self.submitted;
        if self.worker.is_none() {
            self.spawn_worker();
        }
        let accepted = self
            .worker
            .as_ref()
            .and_then(|worker| worker.queries.as_ref())
            .is_some_and(|queries| queries.send((generation, query.to_owned())).is_ok());
        if !accepted {
            // Spawn failure is already recorded; a dead worker means the same
            // degraded empty state. Settle the generation so the UI never
            // spins forever on a query nobody will answer.
            self.settled = self.submitted;
            if self.last_error.is_none() {
                self.last_error = Some("search worker unavailable".to_owned());
            }
        }
    }

    /// Drain settled results. Generations older than the latest submit are
    /// stale (superseded before delivery) and dropped, so the UI never
    /// flashes outdated rows; failures clear the results and surface text.
    pub fn poll(&mut self) {
        loop {
            let next = self
                .worker
                .as_ref()
                .and_then(|worker| worker.results.try_recv().ok());
            let Some((generation, outcome)) = next else {
                return;
            };
            if generation < self.submitted {
                tracing::debug!(
                    name = "search.stale_dropped",
                    generation,
                    "superseded result dropped"
                );
                continue;
            }
            self.settled = generation;
            match outcome {
                Ok(found) => {
                    self.delivered = Some(found);
                    self.last_error = None;
                }
                Err(err) => {
                    tracing::warn!(name = "search.query_failed", error = %err, "results cleared");
                    self.delivered = Some(SearchResults::default());
                    self.last_error = Some(err.to_string());
                }
            }
        }
    }

    /// Take the latest settled results exactly once (views refresh on `Some`).
    pub fn take_results(&mut self) -> Option<SearchResults> {
        self.delivered.take()
    }

    /// Poll the worker, then map one settled result set (`None` while the
    /// worker runs or idles). Poll-then-take is one ordering behind this
    /// seam so no caller can take without polling first.
    pub fn poll_mapped<T>(&mut self, map: impl FnOnce(SearchResults) -> T) -> Option<T> {
        self.poll();
        self.take_results().map(map)
    }

    /// Whether a submitted query is still waiting on the worker.
    #[must_use]
    pub fn is_searching(&self) -> bool {
        self.settled < self.submitted
    }

    /// Last search failure, if any (cleared by the next submit or success).
    #[must_use]
    pub fn error_text(&self) -> Option<String> {
        self.last_error.clone()
    }

    /// Start the debounce thread. Spawn failure degrades to the empty state
    /// with surfaced text — search must never crash the app.
    fn spawn_worker(&mut self) {
        let (query_tx, query_rx) = std_mpsc::channel::<(u64, String)>();
        let (result_tx, result_rx) = std_mpsc::channel::<(u64, Result<SearchResults, Error>)>();
        let db_path = self.db_path.clone();
        let debounce = self.debounce;
        match std::thread::Builder::new()
            .name("tunex-search".to_owned())
            .spawn(move || search_loop(query_rx, result_tx, &db_path, debounce))
        {
            Ok(thread) => {
                self.worker = Some(SearchWorker {
                    queries: Some(query_tx),
                    results: result_rx,
                    thread: Some(thread),
                });
            }
            Err(err) => {
                self.settled = self.submitted;
                self.last_error = Some(
                    Error::Io {
                        path: PathBuf::from("<search thread>"),
                        source: err,
                    }
                    .to_string(),
                );
            }
        }
    }
}

impl Default for SearchCore {
    fn default() -> Self {
        Self::new()
    }
}

/// Outcome of one quiet-window wait.
enum Coalesced {
    /// Newest query after a full quiet window.
    Settled(u64, String),
    /// Handle dropped mid-window: answer the pending query, then shut down.
    Shutdown(u64, String),
}

/// Coalesce the burst behind the quiet window, keeping only the newest query.
fn coalesce_queries(
    queries: &std_mpsc::Receiver<(u64, String)>,
    mut generation: u64,
    mut query: String,
    debounce: Duration,
) -> Coalesced {
    loop {
        match queries.recv_timeout(debounce) {
            Ok((newer, newer_query)) => {
                generation = newer;
                query = newer_query;
            }
            Err(std_mpsc::RecvTimeoutError::Timeout) => {
                return Coalesced::Settled(generation, query);
            }
            Err(std_mpsc::RecvTimeoutError::Disconnected) => {
                return Coalesced::Shutdown(generation, query);
            }
        }
    }
}

/// Newest query when at least one landed while the current one ran (the
/// current outcome is stale and dropped).
fn superseding_query(queries: &std_mpsc::Receiver<(u64, String)>) -> Option<(u64, String)> {
    let mut newest = None;
    while let Ok(next) = queries.try_recv() {
        newest = Some(next);
    }
    newest
}
/// Answer queries until the handle drops: coalesce each burst behind the
/// quiet window, run the newest, and drop outcomes a newer query superseded
/// mid-flight (stale-cancel) instead of sending them.
///
/// Channels move into the worker thread, so they arrive by value even
/// though the loop itself only borrows them.
#[allow(
    clippy::needless_pass_by_value,
    reason = "owned by the search worker thread"
)]
fn search_loop(
    queries: std_mpsc::Receiver<(u64, String)>,
    results: std_mpsc::Sender<(u64, Result<SearchResults, Error>)>,
    db_path: &Path,
    debounce: Duration,
) {
    // A query superseded mid-flight is held here so it re-enters the quiet
    // window immediately instead of stalling on the blocking `recv` below.
    let mut pending: Option<(u64, String)> = None;
    loop {
        let (generation, query) = if let Some(held) = pending.take() {
            held
        } else {
            let Ok(first) = queries.recv() else {
                break;
            };
            first
        };
        let (generation, query) = match coalesce_queries(&queries, generation, query, debounce) {
            Coalesced::Settled(generation, query) => (generation, query),
            Coalesced::Shutdown(generation, query) => {
                // Final flush: answer the pending query before shutting down.
                let _ = results.send((generation, run_query(db_path, &query)));
                return;
            }
        };
        let outcome = run_query(db_path, &query);
        if let Some(held) = superseding_query(&queries) {
            tracing::debug!(
                name = "search.stale_dropped",
                generation,
                "superseded query dropped"
            );
            pending = Some(held);
            continue;
        }
        if let Ok(found) = &outcome {
            tracing::debug!(
                name = "search.executed",
                generation,
                tracks = found.tracks.len(),
                albums = found.albums.len(),
                artists = found.artists.len(),
                "query settled"
            );
        }
        if results.send((generation, outcome)).is_err() {
            break;
        }
    }
}

/// Run one query: a missing index is the empty state (never an error), an
/// unreadable one propagates for the UI to surface.
fn run_query(db_path: &Path, query: &str) -> Result<SearchResults, Error> {
    if !db_path.is_file() {
        return Ok(SearchResults::default());
    }
    let db = open_file(db_path)?;
    search_library(&db, query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use tunex_library::{NewTrack, upsert_track};

    /// Scratch index path in a unique dir (parallel-safe).
    fn scratch(case: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("tunex-search-{case}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("setup works");
        (dir.join("library.db"), dir)
    }

    /// Seed two tracks sharing an artist.
    fn seed(db_path: &Path) {
        let mut db = open_file(db_path).expect("index opens");
        for (path, title, album, genre) in [
            ("/m/midnight.flac", "Midnight", "Night Tapes", "Ambient"),
            ("/m/solaris.flac", "Solaris", "Day Tapes", "Rock"),
        ] {
            upsert_track(
                &mut db,
                &NewTrack {
                    path: path.to_owned(),
                    stable_key: path.to_owned(),
                    title: Some(title.to_owned()),
                    artist: Some("Nova Rae".to_owned()),
                    album: Some(album.to_owned()),
                    genre: Some(genre.to_owned()),
                    ..Default::default()
                },
            )
            .expect("seed works");
        }
    }

    /// Poll until the latest submit settles (every test ends idle so the
    /// joined worker never outlives its scratch dir).
    fn settle(core: &mut SearchCore) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            core.poll();
            if !core.is_searching() {
                return;
            }
            assert!(Instant::now() < deadline, "search worker never settled");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn fresh_core_is_idle() {
        let (db, _dir) = scratch("fresh");
        let mut core = SearchCore::with_db_path(db);
        assert!(!core.is_searching());
        assert!(core.take_results().is_none());
        assert!(core.error_text().is_none());
    }

    #[test]
    fn burst_coalesces_to_latest_query() {
        let (db_path, dir) = scratch("burst");
        seed(&db_path);
        let mut core = SearchCore::with_db_path(db_path);
        core.set_debounce(Duration::from_millis(30));
        core.submit("mid");
        core.submit("midn");
        core.submit("midnight");
        assert!(core.is_searching(), "submits mark the core busy");
        settle(&mut core);
        let results = core.take_results().expect("latest query settles");
        assert_eq!(results.tracks.len(), 1);
        assert_eq!(results.tracks[0].title.as_deref(), Some("Midnight"));
        assert!(core.take_results().is_none(), "results deliver once");
        assert!(!core.is_searching());
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn stale_result_never_surfaces() {
        let (db_path, dir) = scratch("stale");
        seed(&db_path);
        let mut core = SearchCore::with_db_path(db_path);
        core.set_debounce(Duration::from_millis(20));
        core.submit("mid");
        // Let the first query settle worker-side, then supersede it before
        // polling: whichever interleaving wins (worker-sent or coalesced),
        // only the newest generation may surface.
        std::thread::sleep(Duration::from_millis(100));
        core.submit("zzz-no-such-track");
        core.poll();
        assert!(
            core.take_results().is_none(),
            "superseded results never surface"
        );
        assert!(core.is_searching(), "newest query still pending");
        settle(&mut core);
        let results = core.take_results().expect("newest query settles");
        assert_eq!(results, SearchResults::default());
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn empty_query_yields_empty_results() {
        let (db_path, dir) = scratch("empty");
        seed(&db_path);
        let mut core = SearchCore::with_db_path(db_path);
        core.set_debounce(Duration::from_millis(10));
        core.submit("");
        core.submit("   ");
        settle(&mut core);
        assert_eq!(core.take_results(), Some(SearchResults::default()));
        assert!(core.error_text().is_none());
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn missing_index_yields_empty_state() {
        let (db_path, dir) = scratch("missing");
        let mut core = SearchCore::with_db_path(db_path);
        core.set_debounce(Duration::from_millis(10));
        core.submit("mid");
        settle(&mut core);
        assert_eq!(core.take_results(), Some(SearchResults::default()));
        assert!(
            core.error_text().is_none(),
            "missing index is empty state, not an error"
        );
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn corrupt_index_surfaces_error_until_next_success() {
        let (db_path, dir) = scratch("corrupt");
        std::fs::write(&db_path, b"not a database").expect("setup works");
        let mut core = SearchCore::with_db_path(db_path.clone());
        core.set_debounce(Duration::from_millis(10));
        core.submit("mid");
        settle(&mut core);
        assert_eq!(core.take_results(), Some(SearchResults::default()));
        assert!(
            core.error_text().is_some_and(|text| !text.is_empty()),
            "corrupt index surfaces text"
        );
        // A healthy index clears the failure on the next submit.
        std::fs::remove_file(&db_path).expect("setup works");
        seed(&db_path);
        core.submit("mid");
        assert!(
            core.error_text().is_none(),
            "new submit clears the stale error"
        );
        settle(&mut core);
        let results = core.take_results().expect("healthy query settles");
        assert_eq!(results.tracks.len(), 1);
        assert!(core.error_text().is_none(), "success clears the error");
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }
}
