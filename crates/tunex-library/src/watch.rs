//! Filesystem watcher: debounced change batches for library roots.
//!
//! The watcher never scans, reads tags, or touches the database — `notify`
//! callbacks only enqueue raw events, and a dedicated debounce thread
//! coalesces bursts (save storms, bulk copies) into path batches sent over a
//! bounded channel. Rescans run in [`scan_folder`](super::scan::scan_folder)
//! on the consumer side (D-007: notify callbacks enqueue only).
//!
//! Only creations, data modifications, removals, and renames are reported;
//! access-time noise is ignored. Watched roots are recursive.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};
use tokio::sync::mpsc;
use tunex_core::{Error, Result};

/// Quiet window coalescing one burst of filesystem activity into a batch.
///
/// Short enough to feel live, long enough to swallow save storms.
pub const DEBOUNCE_WINDOW: Duration = Duration::from_millis(500);

/// Owns the `notify` watcher and its debounce thread.
///
/// Dropping stops both: the raw-event sender is released so the thread's
/// `recv` fails, and the thread is joined — no detached threads outlive the
/// handle.
pub struct LibraryWatcher {
    /// Kept alive: dropping the watcher unwatches every root.
    _inner: RecommendedWatcher,
    /// Released on drop so the debounce thread exits.
    stop: Option<std_mpsc::Sender<()>>,
    /// Joined on drop for a clean shutdown.
    thread: Option<std::thread::JoinHandle<()>>,
}

impl std::fmt::Debug for LibraryWatcher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LibraryWatcher")
            .finish_non_exhaustive()
    }
}

impl Drop for LibraryWatcher {
    fn drop(&mut self) {
        // Release the stop sender first so a blocked `recv` wakes up.
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Watch `roots` recursively, sending coalesced path batches to `updates`.
///
/// `debounce` overrides [`DEBOUNCE_WINDOW`] (tests use a short window; the
/// app uses the default). Batches carry every changed path once, sorted, so
/// consumers rescan deterministically. The channel is bounded: a receiver
/// that stops draining back-pressures the debounce thread instead of
/// buffering unboundedly.
///
/// # Errors
///
/// Returns [`Error::Io`] when a root cannot be watched.
pub fn watch_roots(
    roots: &[PathBuf],
    debounce: Duration,
    updates: mpsc::Sender<Vec<PathBuf>>,
) -> Result<LibraryWatcher> {
    let (raw_sender, raw_receiver) = std_mpsc::channel::<notify::Result<Event>>();
    let mut inner = RecommendedWatcher::new(
        move |event| {
            // Enqueue only: no filtering, no I/O, no database — the debounce
            // thread below owns all decisions (D-007).
            let _ = raw_sender.send(event);
        },
        notify::Config::default(),
    )
    .map_err(|err| watch_error(roots.first(), &err))?;
    for root in roots {
        inner
            .watch(root, RecursiveMode::Recursive)
            .map_err(|err| watch_error(Some(root), &err))?;
    }
    let (stop_sender, stop_receiver) = std_mpsc::channel::<()>();
    let thread = std::thread::Builder::new()
        .name("tunex-watch".to_owned())
        .spawn(move || debounce_loop(raw_receiver, stop_receiver, debounce, updates))
        .map_err(|err| Error::Io {
            path: PathBuf::from("<watcher thread>"),
            source: err,
        })?;
    tracing::info!(
        name = "watch.started",
        roots = roots.len(),
        debounce_ms = debounce.as_millis(),
        "filesystem watcher active"
    );
    Ok(LibraryWatcher {
        _inner: inner,
        stop: Some(stop_sender),
        thread: Some(thread),
    })
}

/// Whether a `notify` event kind concerns the library index.
fn is_relevant(kind: EventKind) -> bool {
    use notify::event::ModifyKind::{Data, Name};
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(Data(_) | Name(_)) | EventKind::Remove(_)
    )
}

/// Collect raw events into quiet-window batches until the stop signal.
///
/// Channels move into the debounce thread, so they arrive by value even
/// though the loop itself only borrows them.
#[allow(
    clippy::needless_pass_by_value,
    reason = "owned by the debounce thread"
)]
fn debounce_loop(
    raw: std_mpsc::Receiver<notify::Result<Event>>,
    stop: std_mpsc::Receiver<()>,
    debounce: Duration,
    updates: mpsc::Sender<Vec<PathBuf>>,
) {
    let mut pending: HashSet<PathBuf> = HashSet::new();
    let mut quiet_since: Option<Instant> = None;
    loop {
        // Wait while idle; poll briefly while a batch is open so the quiet
        // window is honored without sleeping past it.
        let timeout = quiet_since.map_or(Duration::from_millis(50), |since| {
            debounce.saturating_sub(since.elapsed())
        });
        match raw.recv_timeout(timeout) {
            Ok(Ok(event)) => {
                if is_relevant(event.kind) {
                    let before = pending.len();
                    pending.extend(event.paths);
                    if pending.len() != before {
                        quiet_since = Some(Instant::now());
                    }
                }
            }
            Ok(Err(err)) => {
                tracing::debug!(name = "watch.raw_error", error = %err, "ignoring watcher error");
            }
            Err(std_mpsc::RecvTimeoutError::Timeout) => {
                if quiet_since.is_some_and(|since| since.elapsed() >= debounce) {
                    flush(&mut pending, &updates);
                    quiet_since = None;
                }
            }
            Err(std_mpsc::RecvTimeoutError::Disconnected) => break,
        }
        // `Empty` means keep watching; `Ok` (explicit stop) and
        // `Disconnected` (handle dropped) both mean shut down.
        if !matches!(stop.try_recv(), Err(std_mpsc::TryRecvError::Empty)) {
            break;
        }
    }
    // Final flush: never swallow changes observed before shutdown.
    flush(&mut pending, &updates);
}

/// Send one sorted, deduplicated batch (no-op when empty).
fn flush(pending: &mut HashSet<PathBuf>, updates: &mpsc::Sender<Vec<PathBuf>>) {
    if pending.is_empty() {
        return;
    }
    let mut batch: Vec<PathBuf> = pending.drain().collect();
    batch.sort();
    tracing::debug!(name = "watch.batch", paths = batch.len(), "change batch");
    let _ = updates.blocking_send(batch);
}

/// Map a `notify` failure onto the coarse crate error vocabulary.
fn watch_error(root: Option<&PathBuf>, err: &notify::Error) -> Error {
    Error::Io {
        path: root.cloned().unwrap_or_else(|| PathBuf::from("<watcher>")),
        source: std::io::Error::other(err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Watch `dir` with a short window and collect batches until `wanted`
    /// paths arrive or the deadline passes.
    async fn collect_until(
        dir: &Path,
        debounce: Duration,
        wanted: &[PathBuf],
        act: impl FnOnce(),
    ) -> Vec<PathBuf> {
        let roots = [dir.into()];
        let (updates, mut receiver) = mpsc::channel(16);
        let _watcher = watch_roots(&roots, debounce, updates).expect("watch starts");
        act();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut seen: HashSet<PathBuf> = HashSet::new();
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let Some(batch) = tokio::time::timeout(remaining, receiver.recv())
                .await
                .expect("watcher lives through the test")
            else {
                break;
            };
            seen.extend(batch);
            if wanted.iter().all(|path| seen.contains(path)) {
                break;
            }
        }
        let mut seen: Vec<PathBuf> = seen.into_iter().collect();
        seen.sort();
        seen
    }

    #[tokio::test]
    async fn created_file_arrives_in_a_batch() {
        let dir = std::env::temp_dir().join(format!("tunex-watch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("setup works");
        let target = dir.join("new.flac");
        let seen = collect_until(
            &dir,
            Duration::from_millis(50),
            std::slice::from_ref(&target),
            || {
                std::fs::write(&target, [0u8; 8]).expect("setup works");
            },
        )
        .await;
        assert!(
            seen.contains(&target),
            "created file must be reported, saw {seen:?}"
        );
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[tokio::test]
    async fn burst_coalesces_into_one_batch_per_window() {
        let dir = std::env::temp_dir().join(format!("tunex-watch-burst-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("setup works");
        let targets: Vec<PathBuf> = (0..5)
            .map(|index| dir.join(format!("track{index}.flac")))
            .collect();
        let (updates, mut receiver) = mpsc::channel(16);
        let roots = [dir.clone()];
        let _watcher =
            watch_roots(&roots, Duration::from_millis(200), updates).expect("watch starts");
        for target in &targets {
            std::fs::write(target, [0u8; 8]).expect("setup works");
        }
        // One quiet window: at most two batches carry all five files.
        let mut batches = 0_u32;
        let mut seen: HashSet<PathBuf> = HashSet::new();
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline && !targets.iter().all(|path| seen.contains(path)) {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if let Ok(Some(batch)) = tokio::time::timeout(remaining, receiver.recv()).await {
                batches += 1;
                seen.extend(batch);
            }
        }
        assert!(
            targets.iter().all(|path| seen.contains(path)),
            "every burst file must arrive, saw {seen:?}"
        );
        assert!(
            batches <= 2,
            "one burst should coalesce, took {batches} batches"
        );
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[tokio::test]
    async fn watching_missing_root_errors_loudly() {
        let missing = std::env::temp_dir().join(format!("tunex-watch-gone-{}", std::process::id()));
        let (updates, _receiver) = mpsc::channel(16);
        let err = watch_roots(
            std::slice::from_ref(&missing),
            Duration::from_millis(50),
            updates,
        )
        .expect_err("missing root must fail");
        assert!(matches!(err, Error::Io { .. }));
    }
}
