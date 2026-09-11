//! Lazy artwork resolution off the UI thread.
//!
//! [`ArtCore`] mirrors the [`SearchCore`](super::search::SearchCore) poll
//! protocol: a view asks for the art behind a row as that row comes into
//! sight, a worker thread resolves it through the library cache (embedded
//! bytes first, then folder art — D-010), and the UI thread only enqueues
//! and drains. Decoding on the UI thread is forbidden (R-005, R-NFR-01), so
//! nothing here touches an image; the answer is a path into the cache.
//!
//! Requests carry the caller's own token (an album or track id) and are
//! deduplicated, so asking again while one is in flight — or after it has
//! answered — costs nothing. A track with no art answers just as definitely
//! as one with art: the row then shows its generated placeholder forever
//! instead of asking on every scroll pass.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;

use tunex_library::{CachedArt, default_cache_dir, read_metadata, resolve_track_art};

/// Lazy artwork lookups behind a poll interface.
#[derive(Debug)]
pub struct ArtCore {
    cache_dir: PathBuf,
    /// Answered tokens; `None` means "resolved to no art" (placeholder).
    resolved: HashMap<i64, Option<CachedArt>>,
    /// Tokens with a request in flight.
    asked: HashSet<i64>,
    worker: Option<ArtWorker>,
}

/// Owns the resolver thread: dropping releases its request channel and joins
/// it, so no worker outlives the handle.
#[derive(Debug)]
struct ArtWorker {
    /// Released on drop so a blocked `recv` wakes up.
    requests: Option<std_mpsc::Sender<(i64, PathBuf)>>,
    /// Resolved artwork, drained by [`ArtCore::poll`].
    answers: std_mpsc::Receiver<(i64, Option<CachedArt>)>,
    /// Joined on drop for a clean shutdown.
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for ArtWorker {
    fn drop(&mut self) {
        // Release the request sender first so a blocked `recv` wakes up.
        drop(self.requests.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Default for ArtCore {
    fn default() -> Self {
        Self::new()
    }
}

impl ArtCore {
    /// Resolver over the default XDG artwork cache.
    #[must_use]
    pub fn new() -> Self {
        Self::with_cache_dir(default_cache_dir())
    }

    /// Resolver over an explicit cache root (tests point at a scratch dir).
    #[must_use]
    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            resolved: HashMap::new(),
            asked: HashSet::new(),
            worker: None,
        }
    }

    /// Ask for the art behind `token`, reading it from `source`.
    ///
    /// Cheap to call on every row that scrolls past: tokens already answered
    /// or already in flight are ignored, and the worker is started on the
    /// first real request.
    pub fn request(&mut self, token: i64, source: &Path) {
        if self.resolved.contains_key(&token) || !self.asked.insert(token) {
            return;
        }
        let worker = self
            .worker
            .get_or_insert_with(|| spawn(self.cache_dir.clone()));
        let Some(requests) = worker.requests.as_ref() else {
            return;
        };
        if requests.send((token, source.to_path_buf())).is_err() {
            // The worker died; drop it so the next request starts a new one.
            self.worker = None;
            self.asked.remove(&token);
        }
    }

    /// Drain finished lookups, returning the tokens whose art just landed.
    ///
    /// Callers turn those into row updates; the vec is empty on a quiet poll,
    /// which is the common case.
    pub fn poll(&mut self) -> Vec<i64> {
        let Some(worker) = self.worker.as_ref() else {
            return Vec::new();
        };
        let mut settled = Vec::new();
        while let Ok((token, art)) = worker.answers.try_recv() {
            self.asked.remove(&token);
            self.resolved.insert(token, art);
            settled.push(token);
        }
        settled
    }

    /// Cached artwork for `token`, once resolved and once it had any.
    #[must_use]
    pub fn art(&self, token: i64) -> Option<&CachedArt> {
        self.resolved.get(&token)?.as_ref()
    }

    /// Whether any lookup is still in flight, so a caller's poll pump knows
    /// when it can stop.
    #[must_use]
    pub fn pending(&self) -> bool {
        !self.asked.is_empty()
    }
}

/// Start the resolver thread over `cache_dir`.
fn spawn(cache_dir: PathBuf) -> ArtWorker {
    let (request_tx, request_rx) = std_mpsc::channel::<(i64, PathBuf)>();
    let (answer_tx, answer_rx) = std_mpsc::channel::<(i64, Option<CachedArt>)>();
    let thread = std::thread::Builder::new()
        .name("tunex-art".to_owned())
        .spawn(move || resolve_loop(&cache_dir, &request_rx, &answer_tx))
        .ok();
    ArtWorker {
        requests: Some(request_tx),
        answers: answer_rx,
        thread,
    }
}

/// Resolve requests until the sender is dropped.
fn resolve_loop(
    cache_dir: &Path,
    requests: &std_mpsc::Receiver<(i64, PathBuf)>,
    answers: &std_mpsc::Sender<(i64, Option<CachedArt>)>,
) {
    while let Ok((token, source)) = requests.recv() {
        let art = resolve_one(cache_dir, &source);
        if answers.send((token, art)).is_err() {
            return;
        }
    }
}

/// Resolve one track's artwork, or `None` for the placeholder case.
///
/// Tag reading and decoding both happen here, on the worker. Failures are
/// routine (untagged files, unreadable art) and resolve to `None`; only the
/// tracing trail distinguishes them.
fn resolve_one(cache_dir: &Path, source: &Path) -> Option<CachedArt> {
    let embedded = read_metadata(source)
        .inspect_err(|error| {
            tracing::debug!(
                name = "art.metadata_failed",
                path = %source.display(),
                %error,
                "artwork source unreadable"
            );
        })
        .ok()
        // The reader keeps every embedded picture; the pipeline takes the
        // first, which is the front cover it prefers when tagging.
        .and_then(|metadata| metadata.artwork.into_iter().next());
    match resolve_track_art(cache_dir, source, embedded.as_ref()) {
        Ok(art) => art,
        Err(error) => {
            tracing::debug!(
                name = "art.resolve_failed",
                path = %source.display(),
                %error,
                "artwork cache unavailable"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Scratch cache root, unique per test (parallel-safe).
    fn scratch(case: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tunex-artcore-{case}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch cache");
        dir
    }

    /// Fixture track with folder art beside it; returns the track path.
    fn track_with_folder_art(dir: &Path) -> PathBuf {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        let track = dir.join("song.flac");
        std::fs::copy(source.join("sine.flac"), &track).expect("fixture copies");
        let art = image::RgbImage::from_fn(96, 96, |x, y| {
            image::Rgb([
                u8::try_from(x % 256).unwrap_or(0),
                40,
                u8::try_from(y % 256).unwrap_or(0),
            ])
        });
        art.save(dir.join("cover.png")).expect("cover writes");
        track
    }

    /// Poll until `core` answers `token` or the deadline passes.
    fn settle(core: &mut ArtCore, token: i64) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while core.poll().is_empty() {
            assert!(Instant::now() < deadline, "artwork never resolved");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(core.resolved.contains_key(&token), "token answered");
    }

    #[test]
    fn folder_art_resolves_into_the_cache() {
        let dir = scratch("folder-art");
        let track = track_with_folder_art(&dir);
        let mut core = ArtCore::with_cache_dir(dir.join("cache"));
        core.request(7, &track);
        settle(&mut core, 7);
        let art = core.art(7).expect("folder art resolves");
        assert!(art.thumb256.is_file(), "thumbnail landed in the cache");
    }

    #[test]
    fn a_track_without_art_answers_once_and_stays_answered() {
        // The definite "no art" answer is what stops a scrolling grid from
        // asking again on every pass.
        let dir = scratch("no-art");
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        let track = dir.join("bare.wav");
        std::fs::copy(source.join("sine.wav"), &track).expect("fixture copies");
        let mut core = ArtCore::with_cache_dir(dir.join("cache"));
        core.request(1, &track);
        settle(&mut core, 1);
        assert!(core.art(1).is_none(), "no art resolves to the placeholder");
        core.request(1, &track);
        assert!(
            core.poll().is_empty(),
            "an answered token is never re-asked"
        );
    }

    #[test]
    fn an_unreadable_source_resolves_to_the_placeholder() {
        let dir = scratch("unreadable");
        let mut core = ArtCore::with_cache_dir(dir.join("cache"));
        core.request(3, &dir.join("gone.flac"));
        settle(&mut core, 3);
        assert!(core.art(3).is_none(), "missing files never error out");
    }
}
