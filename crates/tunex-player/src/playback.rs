//! Queue + engine orchestration: the single owner of "what plays now".
//!
//! [`PlaybackController`] pairs the pure [`Queue`] with the [`PlayerEngine`]
//! behind a synchronous, UI-thread-friendly API: every method is cheap
//! (locks, property sets, bounded channel drains) and every blocking wait
//! stays inside worker threads. The gapless preload provider is installed at
//! construction and advances the queue cursor itself, so the cursor follows
//! silent `about-to-finish` handoffs without any bus event per track.
//!
//! Failure discipline: unloadable tracks are skipped forward (cycle-guarded),
//! never retried in place — a broken file under repeat-one steps to the next
//! track instead of replaying itself forever. Skips surface as
//! [`PlayerEvent::PlaybackError`] through [`poll`](PlaybackController::poll),
//! alongside the engine's own events.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::sync::mpsc;
use tunex_core::{Error, PlaybackState, PlayerEvent, RepeatMode, Result};

use super::{Advance, NextUriProvider, PlayerEngine, Queue, QueueItem, Rewind};

/// Lock the queue, recovering from a poisoned mutex.
///
/// Poison means a holder panicked mid-update; the entries are intact but the
/// cursor may be stale, and wedging playback over it would be worse.
fn lock_queue(mutex: &Mutex<Queue>) -> std::sync::MutexGuard<'_, Queue> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Whether a `file://` URI points at a vanished local file. A cheap stat
/// that keeps missing files off the pipeline entirely: loading one blocks
/// ~10 s in playbin timeouts, which must never stall the UI thread.
/// Non-file URIs and undecodable paths return false (attempt the load;
/// failures surface as error events either way).
fn local_path_missing(uri: &str) -> bool {
    let Ok((path, _hostname)) = gstreamer::glib::filename_from_uri(uri) else {
        return false;
    };
    !path.exists()
}

/// Lock an event holder, recovering from a poisoned mutex (same rationale:
/// skip bookkeeping must never wedge playback).
fn lock_events<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Queue + engine orchestration behind a synchronous API.
///
/// Mutating methods take `&mut self`; the queue itself additionally lives
/// behind a mutex shared with the streaming-thread preload provider. Readers
/// stay `&self`, so Qt-side status polling never needs exclusive access.
pub struct PlaybackController {
    engine: PlayerEngine,
    queue: Arc<Mutex<Queue>>,
    events: Mutex<mpsc::Receiver<PlayerEvent>>,
    /// Skip failures queued for the next [`poll`](Self::poll).
    pending: Mutex<Vec<PlayerEvent>>,
}

impl std::fmt::Debug for PlaybackController {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PlaybackController")
            .field("engine", &self.engine)
            .field("queue", &self.queue)
            .finish_non_exhaustive()
    }
}

impl PlaybackController {
    /// Build the engine, install the cursor-following preload provider, and
    /// take ownership of the event channel (consumed via [`poll`](Self::poll)).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when `GStreamer` cannot initialize or
    /// `playbin3` is unavailable.
    pub fn new() -> Result<Self> {
        let (events_tx, events_rx) = mpsc::channel(PlayerEngine::event_buffer());
        let engine = PlayerEngine::new(events_tx)?;
        let queue = Arc::new(Mutex::new(Queue::new()));
        engine.set_next_provider(Some(Self::provider(Arc::clone(&queue))));
        Ok(Self {
            engine,
            queue,
            events: Mutex::new(events_rx),
            pending: Mutex::new(Vec::new()),
        })
    }

    /// Gapless lookahead that advances the cursor as it peeks: peek previews
    /// exactly what `next` takes, so consuming the pick lands the cursor on
    /// the preloaded track. Nothing is consumed at the true end (peek is
    /// `None`), leaving the cursor for the end-of-track advance.
    fn provider(queue: Arc<Mutex<Queue>>) -> NextUriProvider {
        Arc::new(move || {
            let mut guard = lock_queue(&queue);
            let uri = guard.peek_next_uri()?;
            guard.next();
            Some(uri)
        })
    }

    /// Override the audio sink (test seam: point at `fakesink` to run the
    /// pipeline with no sound server).
    pub fn set_audio_sink(&mut self, sink: &gstreamer::Element) {
        self.engine.set_audio_sink(sink);
    }

    /// Append to the end of the queue; returns the item index.
    pub fn enqueue(&mut self, item: QueueItem) -> usize {
        lock_queue(&self.queue).push_back(item)
    }

    /// Insert directly after the cursor so it plays next; returns the index.
    pub fn enqueue_next(&mut self, item: QueueItem) -> usize {
        lock_queue(&self.queue).push_next(item)
    }

    /// Play an item immediately: inserted after the cursor (front when idle)
    /// and started. A load failure errors explicitly instead of silently
    /// playing something else.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when the item is missing, cannot load, or
    /// the pipeline refuses the transition.
    pub fn play_now(&mut self, item: QueueItem) -> Result<()> {
        if local_path_missing(&item.uri) {
            return Err(Error::Player(format!("file is missing: {}", item.title)));
        }
        let at = lock_queue(&self.queue).push_next(item);
        lock_queue(&self.queue).jump(at);
        let Some(current) = lock_queue(&self.queue).current().cloned() else {
            return Err(Error::Player("queue lost the play-now item".to_owned()));
        };
        self.load_and_play(&current.uri)
    }

    /// Play the entry at `index` now (Up Next direct play). Out-of-range
    /// indices are ignored; missing files error explicitly without moving
    /// the cursor.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when the entry is missing or cannot load.
    pub fn play_at(&mut self, index: usize) -> Result<()> {
        let Some(item) = lock_queue(&self.queue).get(index).cloned() else {
            return Ok(());
        };
        if local_path_missing(&item.uri) {
            return Err(Error::Player(format!("file is missing: {}", item.title)));
        }
        lock_queue(&self.queue).jump(index);
        self.load_and_play(&item.uri)
    }

    /// Remove an entry, returning it.
    pub fn remove_at(&mut self, index: usize) -> Option<QueueItem> {
        lock_queue(&self.queue).remove(index)
    }

    /// Move an entry, keeping the cursor on the same track.
    pub fn move_item(&mut self, from: usize, to: usize) {
        lock_queue(&self.queue).move_item(from, to);
    }

    /// Empty the queue (the loaded track keeps playing; the next advance
    /// stops at the emptied end).
    pub fn clear_queue(&mut self) {
        lock_queue(&self.queue).clear();
    }

    /// Number of queued items.
    #[must_use]
    pub fn queue_len(&self) -> usize {
        lock_queue(&self.queue).len()
    }

    /// Queued entry by position, for Up Next rows.
    #[must_use]
    pub fn queue_item(&self, index: usize) -> Option<QueueItem> {
        lock_queue(&self.queue).get(index).cloned()
    }

    /// Cursor position, for Up Next highlight and handoff tracking.
    #[must_use]
    pub fn current_index(&self) -> Option<usize> {
        lock_queue(&self.queue).current_index()
    }

    /// Currently playing item, if the cursor points at one.
    #[must_use]
    pub fn current_item(&self) -> Option<QueueItem> {
        lock_queue(&self.queue).current().cloned()
    }

    /// Enable or disable shuffle.
    pub fn set_shuffle(&mut self, shuffle: bool) {
        lock_queue(&self.queue).set_shuffle(shuffle);
    }

    /// Whether shuffle is on.
    #[must_use]
    pub fn is_shuffle(&self) -> bool {
        lock_queue(&self.queue).shuffle()
    }

    /// Set the repeat mode.
    pub fn set_repeat(&mut self, repeat: RepeatMode) {
        lock_queue(&self.queue).set_repeat(repeat);
    }

    /// Current repeat mode.
    #[must_use]
    pub fn repeat_mode(&self) -> RepeatMode {
        lock_queue(&self.queue).repeat()
    }

    /// Start or resume playback. An idle cursor starts the queue; an empty
    /// queue errors explicitly.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when the queue is empty or the pipeline
    /// refuses the transition. Unloadable heads are skipped (surfacing as
    /// error events on the next [`poll`](Self::poll)), never fatal.
    pub fn play(&mut self) -> Result<()> {
        if lock_queue(&self.queue).is_empty() {
            return Err(Error::Player("queue is empty".to_owned()));
        }
        if lock_queue(&self.queue).current().is_none() {
            return self.advance(false);
        }
        self.engine.play()
    }

    /// Pause, holding position.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when the pipeline refuses the transition.
    pub fn pause(&mut self) -> Result<()> {
        self.engine.pause()
    }

    /// Stop and release the pipeline.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when the pipeline refuses the transition.
    pub fn stop(&mut self) -> Result<()> {
        self.engine.stop()
    }

    /// Step to the next track (wrapping per the repeat mode, stopping at a
    /// bare end). Unloadable resolutions are skipped, never fatal.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when the pipeline refuses the transition.
    pub fn next_track(&mut self) -> Result<()> {
        self.advance(false)
    }

    /// Step back, honoring the queue's restart threshold: past it the caller
    /// restarts the current track; otherwise the previous track loads.
    /// Missing tracks on the way back are skipped (surfacing as error
    /// events), never blocking.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when the seek or load fails.
    pub fn previous_track(&mut self, position: Duration) -> Result<()> {
        let mut position = position;
        loop {
            // Bind the outcome first: the queue guard must drop before the
            // arms below borrow `self` mutably.
            let rewind = lock_queue(&self.queue).previous(position);
            match rewind {
                Rewind::Restart => return self.engine.seek(Duration::ZERO),
                Rewind::Item(_) => {
                    let Some(current) = lock_queue(&self.queue).current().cloned() else {
                        return Ok(());
                    };
                    if local_path_missing(&current.uri) {
                        self.push_pending(PlayerEvent::PlaybackError(format!(
                            "file is missing: {}",
                            current.title
                        )));
                        // Keep walking back from the top (the threshold
                        // applied to the original position only).
                        position = Duration::ZERO;
                        continue;
                    }
                    return self.load_and_play(&current.uri);
                }
                Rewind::None => return Ok(()),
            }
        }
    }

    /// Seek to an absolute position.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when nothing is loaded, the position is out
    /// of range, or the seek fails.
    pub fn seek(&mut self, position: Duration) -> Result<()> {
        self.engine.seek(position)
    }

    /// Set output volume, clamped to `0.0` (mute) through `1.0` (full).
    pub fn set_volume(&mut self, volume: f32) {
        self.engine.set_volume(volume);
    }

    /// Current output volume.
    #[must_use]
    pub fn volume(&self) -> f64 {
        self.engine.volume()
    }

    /// Mute or unmute (independent of the volume level).
    pub fn set_muted(&mut self, muted: bool) {
        self.engine.set_muted(muted);
    }

    /// Whether output is muted.
    #[must_use]
    pub fn muted(&self) -> bool {
        self.engine.muted()
    }

    /// Current playback position, when the pipeline can report one.
    #[must_use]
    pub fn position(&self) -> Option<Duration> {
        self.engine.position()
    }

    /// Known track duration, when the stream has exposed one.
    #[must_use]
    pub fn duration(&self) -> Option<Duration> {
        self.engine.duration()
    }

    /// Last tracked application state (updated by the bus thread).
    #[must_use]
    pub fn state(&self) -> PlaybackState {
        self.engine.state()
    }

    /// Drain engine events plus queued skip failures. End-of-track and
    /// pipeline errors advance internally (broken tracks are skipped, the
    /// bare end stops), so the returned vec is the complete UI feed: state
    /// changes, errors, duration discovery, and end-of-track markers.
    pub fn poll(&mut self) -> Vec<PlayerEvent> {
        let mut out = std::mem::take(&mut *lock_events(&self.pending));
        // Drain under the lock, then release before advancing: advance takes
        // `&mut self`, which cannot coexist with the held guard.
        let drained: Vec<PlayerEvent> = {
            let mut events = lock_events(&self.events);
            let mut drained = Vec::new();
            while let Ok(event) = events.try_recv() {
                drained.push(event);
            }
            drained
        };
        for event in drained {
            match &event {
                PlayerEvent::EndOfTrack | PlayerEvent::PlaybackError(_) => {
                    out.push(event);
                    // Skip failures already queue as pending error events;
                    // only an engine-stop failure needs reporting here.
                    if let Err(err) = self.advance(true) {
                        tracing::warn!(
                            name = "player.advance_failed",
                            error = %err,
                            "end-of-track advance failed"
                        );
                    }
                }
                _ => out.push(event),
            }
        }
        out
    }

    /// Load a URI and start it.
    fn load_and_play(&mut self, uri: &str) -> Result<()> {
        self.engine.load_uri(uri)?;
        self.engine.play()
    }

    /// Record a skip failure for the next [`poll`](Self::poll).
    fn push_pending(&mut self, event: PlayerEvent) {
        lock_events(&self.pending).push(event);
    }

    /// Advance once; unloadable resolutions are skipped forward until
    /// something plays or the queue ends. `after_error` takes the first pick
    /// error-aware too (a track that just errored must move on even under
    /// repeat-one). The visited set bounds the walk: shuffle and repeat-all
    /// cycle back over tried entries instead of looping forever.
    fn advance(&mut self, after_error: bool) -> Result<()> {
        let mut seen = HashSet::new();
        let mut pick = if after_error {
            lock_queue(&self.queue).advance_past_error()
        } else {
            lock_queue(&self.queue).next()
        };
        loop {
            match pick {
                Advance::Item(index) => match self.play_pick(&mut seen, index) {
                    PickOutcome::Playing => return Ok(()),
                    PickOutcome::Skipped(next) => pick = next,
                    PickOutcome::Stopped(result) => return result,
                },
                Advance::End => return self.engine.stop(),
            }
        }
    }

    /// Attempt one resolved pick: play it, skip it forward on failure, or
    /// stop when every remaining entry has been tried.
    fn play_pick(&mut self, seen: &mut HashSet<usize>, index: usize) -> PickOutcome {
        let queue_len = lock_queue(&self.queue).len();
        if !seen.insert(index) && seen.len() >= queue_len {
            tracing::warn!(
                name = "player.skip_exhausted",
                tried = seen.len(),
                "every queued track failed to load, stopping"
            );
            return PickOutcome::Stopped(self.engine.stop());
        }
        let Some(item) = lock_queue(&self.queue).current().cloned() else {
            return PickOutcome::Stopped(self.engine.stop());
        };
        let outcome = if local_path_missing(&item.uri) {
            Err(Error::Player(format!("file is missing: {}", item.title)))
        } else {
            self.load_and_play(&item.uri)
        };
        match outcome {
            Ok(()) => PickOutcome::Playing,
            Err(err) => {
                tracing::warn!(
                    name = "player.skip_unloadable",
                    error = %err,
                    "skipping unloadable track"
                );
                self.push_pending(PlayerEvent::PlaybackError(err.to_string()));
                PickOutcome::Skipped(lock_queue(&self.queue).advance_past_error())
            }
        }
    }
}

/// Outcome of attempting one resolved queue pick.
enum PickOutcome {
    /// Now playing.
    Playing,
    /// Unloadable; continue with this follow-up pick.
    Skipped(Advance),
    /// Nothing playable remains; engine stopped with this result.
    Stopped(Result<()>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use gstreamer::glib::object::ObjectExt as _;
    use std::path::PathBuf;

    /// Controller wired to `fakesink`: full pipeline behavior, no sound
    /// server. `sync` selects real-time (`true`, deterministic cursor timing)
    /// or free-running (`false`, fast state/error tests) consumption.
    fn test_controller(sync: bool) -> PlaybackController {
        let mut controller = PlaybackController::new().expect("controller builds");
        let sink = gstreamer::ElementFactory::make("fakesink")
            .build()
            .expect("fakesink exists");
        sink.set_property("sync", sync);
        sink.set_property("async", false);
        controller.set_audio_sink(&sink);
        controller
    }

    /// Write a tiny mono WAV (8 kHz, 16-bit sine) for pipeline tests.
    fn write_sine_wav(path: &std::path::Path, millis: u64, hertz: f32) {
        const RATE: u32 = 8000;
        let frames = u32::try_from(u64::from(RATE) * millis / 1000).expect("test clips are short");
        let mut bytes = Vec::with_capacity(44 + frames as usize * 2);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + frames * 2).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&RATE.to_le_bytes());
        bytes.extend_from_slice(&(RATE * 2).to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(frames * 2).to_le_bytes());
        for frame in 0..frames {
            let time = f64::from(frame) / f64::from(RATE);
            let phase = time * f64::from(hertz) * f64::from(std::f32::consts::TAU);
            #[allow(
                clippy::cast_possible_truncation,
                reason = "0.4-amplitude sine always fits i16"
            )]
            let sample = (phase.sin() * 0.4 * 32767.0) as i16;
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        std::fs::write(path, bytes).expect("fixture writes");
    }

    /// Scratch dir with two sine clips of `millis` each; returns the dir plus
    /// their queue items. One second keeps real-time tests fast while leaving
    /// wide margins between handoffs and assertions; longer clips make
    /// end-of-stream unreachable where a test must never observe it.
    fn two_clip_album(case: &str, millis: u64) -> (PathBuf, Vec<QueueItem>) {
        let dir =
            std::env::temp_dir().join(format!("tunex-playback-{case}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("fixture dir");
        let items = [("a.wav", 440.0, 0), ("b.wav", 660.0, 1)]
            .into_iter()
            .map(|(name, hertz, track_id)| {
                let path = dir.join(name);
                write_sine_wav(&path, millis, hertz);
                let uri = crate::path_to_uri(&path).expect("fixture path converts");
                QueueItem::new(&uri, name).with_track_id(track_id)
            })
            .collect();
        (dir, items)
    }

    /// Poll until `condition` holds or the deadline passes, collecting every
    /// drained event on the way.
    fn poll_until(
        controller: &mut PlaybackController,
        timeout: Duration,
        condition: impl Fn(&PlaybackController) -> bool,
    ) -> Vec<PlayerEvent> {
        let mut drained = Vec::new();
        let deadline = std::time::Instant::now() + timeout;
        while !condition(controller) {
            assert!(
                std::time::Instant::now() < deadline,
                "playback condition never held"
            );
            drained.extend(controller.poll());
            std::thread::sleep(Duration::from_millis(5));
        }
        drained.extend(controller.poll());
        drained
    }

    #[test]
    fn play_on_empty_queue_errors() {
        let mut controller = test_controller(false);
        let err = controller
            .play()
            .expect_err("playing nothing must fail loudly");
        assert!(matches!(err, Error::Player(_)));
    }

    #[test]
    fn play_at_validates_before_loading() {
        let mut controller = PlaybackController::new().expect("controller builds");
        controller.enqueue(QueueItem::new(
            "file:///nonexistent-tunex-probe.flac",
            "Gone",
        ));
        let err = controller
            .play_at(0)
            .expect_err("missing entry must fail loudly");
        assert!(matches!(err, Error::Player(_)));
        assert_eq!(
            controller.current_index(),
            None,
            "failed direct play leaves the cursor"
        );
        controller.play_at(99).expect("out-of-range ignored");
    }

    #[test]
    fn play_now_starts_playback_immediately() {
        let (dir, items) = two_clip_album("now", 1000);
        let mut controller = test_controller(true);
        controller.enqueue(items[1].clone());
        controller
            .play_now(items[0].clone())
            .expect("play-now works");
        assert_eq!(controller.current_index(), Some(0));
        assert_eq!(controller.queue_len(), 2);
        poll_until(&mut controller, Duration::from_secs(10), |controller| {
            controller.state() == PlaybackState::Playing
        });
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn next_and_previous_reload_and_restart() {
        // Single track under repeat-all: the cursor cannot leave index 0 no
        // matter when the preload provider fires, so every assertion below
        // is timing-proof. Distinct-index movement is covered exhaustively
        // by the pure queue tests; this proves the controller wiring. Ten
        // seconds of audio keeps end-of-stream unreachable: a blocking seek
        // against an EOS-wedged pipeline hangs instead of failing.
        let (dir, items) = two_clip_album("cursor", 10000);
        let mut controller = test_controller(true);
        controller.enqueue(items[0].clone());
        controller.set_repeat(RepeatMode::All);
        controller.play().expect("playback starts");
        poll_until(&mut controller, Duration::from_secs(10), |controller| {
            controller.state() == PlaybackState::Playing
        });
        controller.next_track().expect("next works");
        assert_eq!(controller.current_index(), Some(0));
        controller
            .previous_track(Duration::ZERO)
            .expect("previous works");
        assert_eq!(controller.current_index(), Some(0));
        // Past the restart threshold: seeks to zero instead of stepping.
        // Settle Paused first — seeking mid-transition fails the seek.
        controller.pause().expect("pause works");
        poll_until(&mut controller, Duration::from_secs(10), |controller| {
            controller.state() == PlaybackState::Paused
        });
        controller
            .previous_track(Duration::from_secs(30))
            .expect("restart works");
        assert_eq!(controller.current_index(), Some(0));
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn queue_ops_expose_items_for_up_next() {
        let mut controller = PlaybackController::new().expect("controller builds");
        let first = QueueItem::new("file:///a.flac", "A").with_track_id(1);
        let second = QueueItem::new("file:///b.flac", "B").with_track_id(2);
        assert_eq!(controller.enqueue(first), 0);
        assert_eq!(controller.enqueue_next(second.clone()), 0);
        assert_eq!(controller.queue_len(), 2);
        assert_eq!(controller.queue_item(0), Some(second));
        assert_eq!(controller.current_index(), None);
        controller.move_item(0, 1);
        assert_eq!(
            controller.queue_item(1).and_then(|item| item.track_id),
            Some(2)
        );
        assert!(controller.remove_at(0).is_some());
        assert_eq!(controller.queue_len(), 1);
        controller.clear_queue();
        assert_eq!(controller.queue_len(), 0);
    }

    #[test]
    fn missing_file_skips_to_next_with_error_event() {
        let (dir, items) = two_clip_album("skip", 1000);
        // Real-time sink: the surviving track must stay Playing long enough
        // to observe (free-running clips finish between polls).
        let mut controller = test_controller(true);
        controller.enqueue(QueueItem::new(
            "file:///nonexistent-tunex-probe.flac",
            "Gone",
        ));
        controller.enqueue(items[0].clone());
        controller.play().expect("play accepts the queue");
        let drained = poll_until(&mut controller, Duration::from_secs(15), |controller| {
            controller.state() == PlaybackState::Playing
        });
        assert!(
            drained
                .iter()
                .any(|event| matches!(event, PlayerEvent::PlaybackError(_))),
            "skipped track surfaces an error, got {drained:?}"
        );
        assert_eq!(
            controller.current_index(),
            Some(1),
            "playback moved past the missing file"
        );
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn gapless_album_handoff_keeps_playing_without_stop() {
        let (dir, items) = two_clip_album("gapless", 1000);
        let mut controller = test_controller(true);
        for item in &items {
            controller.enqueue(item.clone());
        }
        controller.play().expect("album starts");
        // Sample the cursor every poll: it must visit the second track while
        // the state sequence stays exactly Loading → Playing → Stopped (any
        // intermediate stop breaks gapless playback).
        let mut states = Vec::new();
        let mut saw_second = false;
        let mut ends = 0_u32;
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        loop {
            saw_second |= controller.current_index() == Some(1);
            for event in controller.poll() {
                match event {
                    PlayerEvent::StateChanged(state) => states.push(state),
                    PlayerEvent::EndOfTrack => ends += 1,
                    PlayerEvent::PlaybackError(message) => {
                        panic!("healthy album errored: {message}");
                    }
                    PlayerEvent::DurationChanged(_) => {}
                }
            }
            if ends == 1 {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "true end never arrived"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(saw_second, "cursor followed the silent preload handoff");
        assert_eq!(
            states,
            vec![
                PlaybackState::Loading,
                PlaybackState::Playing,
                PlaybackState::Stopped
            ],
            "gapless handoff without intermediate stop"
        );
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }
}
