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
//! track instead of replaying itself forever, while a track that simply ended
//! follows the repeat mode. Skips surface as
//! [`PlayerEvent::PlaybackError`] through [`poll`](PlaybackController::poll),
//! alongside the engine's own events.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use tokio::sync::mpsc;
use tunex_core::{Error, PlaybackState, PlayerEvent, RepeatMode, ReplayGainMode, Result};

use super::{
    Advance, NextUriProvider, PlayerEngine, Queue, QueueItem, Rewind, output::sink_for_device,
    read_replaygain_uri,
};

/// Give playbin time to leave STOPPED before treating restore as failed.
const RESTORE_SEEK_DEADLINE: Duration = Duration::from_secs(5);
/// Minimum gap between restore seek retries (poll is ~16–50 ms).
const RESTORE_SEEK_RETRY: Duration = Duration::from_millis(80);
/// Position close enough to the saved restore target counts as landed.
const RESTORE_SEEK_SLACK: Duration = Duration::from_millis(100);

/// Volume envelope on a single decoder (not a second pipeline).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fade {
    Idle,
    Out { started: Instant },
    In { started: Instant },
}

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
    /// Restart position to re-seek after preroll; `None` once it lands.
    pending_restore: Option<Duration>,
    restore_started: Option<Instant>,
    last_restore_seek: Option<Instant>,
    crossfade: Duration,
    fade: Fade,
    fade_index: Option<usize>,
    rg_uri: Option<String>,
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
            pending_restore: None,
            restore_started: None,
            last_restore_seek: None,
            crossfade: Duration::ZERO,
            fade: Fade::Idle,
            fade_index: None,
            rg_uri: None,
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

    /// Restore one track paused at `position` (restart resume). Missing
    /// files error explicitly so the caller can skip without blocking.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when the item is missing or cannot load.
    pub fn restore_paused(&mut self, item: QueueItem, position: Duration) -> Result<()> {
        if local_path_missing(&item.uri) {
            return Err(Error::Player(format!("file is missing: {}", item.title)));
        }
        let at = lock_queue(&self.queue).push_back(item);
        lock_queue(&self.queue).jump(at);
        let Some(current) = lock_queue(&self.queue).current().cloned() else {
            return Err(Error::Player("queue lost the restored item".to_owned()));
        };
        self.engine.load_uri(&current.uri)?;
        self.engine.pause()?;
        if position.is_zero() {
            self.pending_restore = None;
        } else {
            self.pending_restore = Some(position);
            self.restore_started = Some(Instant::now());
            self.last_restore_seek = None;
            let _ = self.engine.seek(position);
        }
        Ok(())
    }

    /// Restore a full queue paused at `cursor`/`position`. Missing files stay
    /// in the list as dangling rows and never auto-play.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when the current file exists but cannot load.
    pub fn restore_queue(
        &mut self,
        items: Vec<QueueItem>,
        cursor: usize,
        position: Duration,
    ) -> Result<()> {
        {
            let mut queue = lock_queue(&self.queue);
            queue.clear();
            for item in items {
                queue.push_back(item);
            }
            if !queue.is_empty() {
                let at = cursor.min(queue.len().saturating_sub(1));
                queue.jump(at);
            }
        }
        let Some(current) = self.current_item() else {
            return Ok(());
        };
        if local_path_missing(&current.uri) {
            return Ok(());
        }
        self.engine.load_uri(&current.uri)?;
        self.engine.pause()?;
        if position.is_zero() {
            self.pending_restore = None;
        } else {
            self.pending_restore = Some(position);
            self.restore_started = Some(Instant::now());
            self.last_restore_seek = None;
            let _ = self.engine.seek(position);
        }
        Ok(())
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

    /// Move an already-queued entry so it plays next.
    pub fn play_next_at(&mut self, index: usize) {
        lock_queue(&self.queue).play_next_at(index);
    }

    /// Snapshot of every queued item in order.
    #[must_use]
    pub fn queue_items(&self) -> Vec<QueueItem> {
        lock_queue(&self.queue).items()
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
        self.pending_restore = None;
        self.engine.seek(position)
    }

    /// Whether a restart seek is still catching up after preroll.
    #[must_use]
    pub fn restore_pending(&self) -> bool {
        self.pending_restore.is_some()
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

    /// `ReplayGain` mode (off / track / album).
    pub fn set_replaygain_mode(&mut self, mode: ReplayGainMode) {
        self.engine.set_replaygain_mode(mode);
        if let Some(item) = self.current_item() {
            self.engine
                .set_replaygain_tags(read_replaygain_uri(&item.uri));
        }
    }

    /// Linear `ReplayGain` multiplier currently applied.
    #[must_use]
    pub fn replaygain_linear(&self) -> f64 {
        self.engine.replaygain_linear()
    }

    /// Crossfade length. `0` is today's gapless cut. Visual reduce-motion does
    /// not disable this envelope.
    pub fn set_crossfade(&mut self, duration: Duration) {
        self.crossfade = duration.min(Duration::from_secs(12));
        if self.crossfade.is_zero() {
            self.fade = Fade::Idle;
            self.engine.set_crossfade_volume(1.0);
        }
    }

    /// Current crossfade setting.
    #[must_use]
    pub fn crossfade(&self) -> Duration {
        self.crossfade
    }

    /// Current crossfade envelope (tests).
    #[must_use]
    pub fn crossfade_volume(&self) -> f64 {
        self.engine.crossfade_volume()
    }

    /// Switch the `PipeWire`/`GStreamer` sink. The queue is left intact.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when the device cannot be opened or the
    /// current track cannot be reloaded onto the new sink.
    pub fn set_output_device(&mut self, id: &str) -> Result<()> {
        let terminal = sink_for_device(id)?;
        let uri = self.engine.queued_uri();
        let position = self.engine.position();
        let state = self.engine.state();
        if uri.is_some() {
            let _ = self.engine.pause();
        }
        self.engine.set_audio_sink(&terminal);
        let Some(uri) = uri else {
            return Ok(());
        };
        self.engine.load_uri(&uri)?;
        if let Some(position) = position {
            let _ = self.engine.seek(position);
        }
        match state {
            PlaybackState::Playing => self.engine.play(),
            PlaybackState::Paused | PlaybackState::Loading => self.engine.pause(),
            PlaybackState::Stopped => Ok(()),
        }
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
            // A track that ended normally advances by the repeat mode, so
            // repeat-one replays it; one that *failed* advances error-aware,
            // stepping past it instead of retrying it forever.
            let after_error = match &event {
                PlayerEvent::EndOfTrack => false,
                PlayerEvent::PlaybackError(_) => true,
                PlayerEvent::StateChanged(_) | PlayerEvent::DurationChanged(_) => {
                    out.push(event);
                    continue;
                }
            };
            out.push(event);
            // Skip failures already queue as pending error events;
            // only an engine-stop failure needs reporting here.
            if let Err(err) = self.advance(after_error) {
                tracing::warn!(
                    name = "player.advance_failed",
                    error = %err,
                    "end-of-track advance failed"
                );
            }
        }
        self.retry_restore_seek();
        self.refresh_replaygain();
        self.tick_crossfade();
        out
    }

    fn refresh_replaygain(&mut self) {
        let uri = self.engine.queued_uri();
        if uri == self.rg_uri {
            return;
        }
        self.rg_uri.clone_from(&uri);
        let Some(uri) = uri else {
            return;
        };
        self.engine.set_replaygain_tags(read_replaygain_uri(&uri));
    }

    fn tick_crossfade(&mut self) {
        if self.crossfade.is_zero() {
            if !matches!(self.fade, Fade::Idle) {
                self.fade = Fade::Idle;
                self.engine.set_crossfade_volume(1.0);
            }
            self.fade_index = self.current_index();
            return;
        }
        let index = self.current_index();
        if index != self.fade_index {
            if self.fade_index.is_some() && index.is_some() {
                self.fade = Fade::In {
                    started: Instant::now(),
                };
                self.engine.set_crossfade_volume(0.0);
            }
            self.fade_index = index;
        }
        if self.engine.state() == PlaybackState::Playing
            && matches!(self.fade, Fade::Idle)
            && let (Some(position), Some(duration)) =
                (self.engine.position(), self.engine.duration())
        {
            let remaining = duration.saturating_sub(position);
            if remaining > Duration::ZERO && remaining <= self.crossfade {
                self.fade = Fade::Out {
                    started: Instant::now(),
                };
            }
        }
        match self.fade {
            Fade::Idle => {}
            Fade::Out { started } => {
                let t = fade_progress(started.elapsed(), self.crossfade);
                self.engine.set_crossfade_volume(1.0 - t);
            }
            Fade::In { started } => {
                let t = fade_progress(started.elapsed(), self.crossfade);
                self.engine.set_crossfade_volume(t);
                if t >= 1.0 {
                    self.engine.set_crossfade_volume(1.0);
                    self.fade = Fade::Idle;
                }
            }
        }
    }

    /// Re-issue the restore seek once the pipeline has prerolled. The first
    /// seek at load time races PAUSED; poll retries until it lands or 5 s.
    fn retry_restore_seek(&mut self) {
        let Some(target) = self.pending_restore else {
            return;
        };
        if self
            .restore_started
            .is_some_and(|started| started.elapsed() > RESTORE_SEEK_DEADLINE)
        {
            self.pending_restore = None;
            return;
        }
        if self
            .last_restore_seek
            .is_some_and(|at| at.elapsed() < RESTORE_SEEK_RETRY)
        {
            return;
        }
        if self.engine.state() == PlaybackState::Stopped {
            return;
        }
        let current = self.engine.position().unwrap_or_default();
        if current + RESTORE_SEEK_SLACK >= target {
            self.pending_restore = None;
            return;
        }
        let _ = self.engine.seek(target);
        self.last_restore_seek = Some(Instant::now());
    }

    /// Load a URI and start it.
    fn load_and_play(&mut self, uri: &str) -> Result<()> {
        self.pending_restore = None;
        self.engine.load_uri(uri)?;
        self.engine.set_replaygain_tags(read_replaygain_uri(uri));
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

fn fade_progress(elapsed: Duration, total: Duration) -> f64 {
    if total.is_zero() {
        return 1.0;
    }
    (elapsed.as_secs_f64() / total.as_secs_f64()).clamp(0.0, 1.0)
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

    /// Shared codec fixture (`tests/fixtures/`, W-006) as a queue item.
    fn fixture_item(name: &str) -> QueueItem {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(name);
        let path = std::fs::canonicalize(&path).expect("fixture exists");
        let uri = crate::path_to_uri(&path).expect("fixture path converts");
        QueueItem::new(&uri, name)
    }

    /// Run `session` on its own thread and fail — instead of hanging the
    /// suite — when it does not finish in time. A wedged pipeline blocks the
    /// polling thread forever, so the deadline must live outside it. A
    /// session that fails on its own re-raises its original panic.
    fn within_deadline(timeout: Duration, what: &str, session: impl FnOnce() + Send + 'static) {
        let (done, finished) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            session();
            let _ = done.send(());
        });
        if let Err(std::sync::mpsc::RecvTimeoutError::Timeout) = finished.recv_timeout(timeout) {
            panic!("{what} did not finish within {timeout:?}: playback wedged the polling thread");
        }
        if let Err(failure) = worker.join() {
            std::panic::resume_unwind(failure);
        }
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

    #[test]
    fn repeat_all_single_track_loops_without_wedging_poll() {
        // Every lap reloads the track after about-to-finish has drained its
        // input: the window where setting the URI before stopping deadlocked
        // decodebin3 inside `poll` (S6: the UI froze at the end of a
        // repeat-all track). Ogg Opus like real libraries; a free-running
        // sink makes laps take milliseconds, and a hundred of them wedged the
        // old order in every measured run.
        within_deadline(Duration::from_secs(30), "a hundred repeat-all laps", || {
            let mut controller = test_controller(false);
            controller.enqueue(fixture_item("sine.opus"));
            controller.set_repeat(RepeatMode::All);
            controller.play().expect("playback starts");
            let mut laps = 0_u32;
            let deadline = std::time::Instant::now() + Duration::from_secs(25);
            while laps < 100 {
                for event in controller.poll() {
                    match event {
                        PlayerEvent::EndOfTrack => laps += 1,
                        PlayerEvent::PlaybackError(message) => {
                            panic!("healthy track errored: {message}");
                        }
                        PlayerEvent::StateChanged(_) | PlayerEvent::DurationChanged(_) => {}
                    }
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "repeat-all stopped looping after {laps} lap(s)"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            assert_eq!(controller.current_index(), Some(0), "cursor stays put");
        });
    }

    #[test]
    fn next_track_during_gapless_preload_never_wedges() {
        // about-to-finish fires once the tail of the track fits the decoder
        // queue (~1.6 s before the end for these fixtures) and the provider
        // hands the next URI over right then; `uridecodebin3` swaps its input
        // across a few milliseconds later, on the new item's own streaming
        // thread. Seeking into the tail buys one handoff per lap, and
        // skipping the instant it lands aims the reload straight at that
        // swap — the overlap that deadlocks GStreamer 1.28, which wedged
        // `poll` in most measured runs without the teardown gate.
        const SKIPS: u32 = 24;

        within_deadline(Duration::from_secs(60), "skips into fresh preloads", || {
            let mut controller = test_controller(true);
            controller.enqueue(fixture_item("sine.opus"));
            controller.enqueue(fixture_item("sine.ogg"));
            controller.set_repeat(RepeatMode::All);
            controller.play().expect("playback starts");
            let mut preloaded = 0_u32;
            for _ in 0..SKIPS {
                poll_until(&mut controller, Duration::from_secs(10), |controller| {
                    controller.state() == PlaybackState::Playing
                });
                // The provider moves the cursor as it hands the URI over, so
                // the cursor leaving the playing track marks the handoff.
                let playing = controller.current_index();
                controller
                    .seek(Duration::from_millis(4300))
                    .expect("seek into the tail");
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                while controller.current_index() == playing {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "tail seek never triggered the preload"
                    );
                    let _ = controller.poll();
                }
                preloaded += 1;
                controller.next_track().expect("skip works");
            }
            assert_eq!(preloaded, SKIPS, "every skip landed on a fresh preload");
            poll_until(&mut controller, Duration::from_secs(10), |controller| {
                controller.state() == PlaybackState::Playing
            });
        });
    }

    #[test]
    fn repeat_one_replays_the_track_at_its_natural_end() {
        // A second queued track makes the difference visible: stepping onto
        // it is repeat-off behaviour, staying put is repeat-one. Two ends,
        // because the first replay is what the mode is for.
        within_deadline(Duration::from_secs(30), "two repeat-one ends", || {
            let mut controller = test_controller(false);
            controller.enqueue(fixture_item("sine.opus"));
            controller.enqueue(fixture_item("sine.ogg"));
            controller.set_repeat(RepeatMode::One);
            controller.play().expect("playback starts");
            let mut ends = 0_u32;
            let deadline = std::time::Instant::now() + Duration::from_secs(25);
            while ends < 2 {
                for event in controller.poll() {
                    match event {
                        PlayerEvent::EndOfTrack => ends += 1,
                        PlayerEvent::PlaybackError(message) => {
                            panic!("healthy track errored: {message}");
                        }
                        PlayerEvent::StateChanged(_) | PlayerEvent::DurationChanged(_) => {}
                    }
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "repeat-one stopped after {ends} end(s)"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            assert_eq!(
                controller.current_index(),
                Some(0),
                "repeat-one replays the same track instead of stepping forward"
            );
        });
    }

    #[test]
    fn restore_paused_loads_missing_file_as_error() {
        let mut controller = test_controller(false);
        let err = controller
            .restore_paused(
                QueueItem::new("file:///nonexistent-tunex-restore.flac", "Gone"),
                Duration::from_millis(800),
            )
            .expect_err("missing file is explicit");
        assert!(
            err.to_string().contains("missing"),
            "unexpected restore error: {err}"
        );
        assert_eq!(controller.queue_len(), 0);
    }

    #[test]
    fn restore_paused_holds_track_without_playing() {
        let (dir, items) = two_clip_album("restore", 2000);
        let mut controller = test_controller(true);
        controller
            .restore_paused(items[0].clone(), Duration::from_millis(1500))
            .expect("restore loads");
        assert_eq!(controller.queue_len(), 1);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let _ = controller.poll();
            let paused = controller.state() == PlaybackState::Paused
                || controller.state() == PlaybackState::Loading;
            let position = controller.position().unwrap_or_default();
            if paused && position >= Duration::from_millis(1400) {
                break;
            }
            assert!(
                controller.state() != PlaybackState::Playing,
                "restore must not start audible playback"
            );
            assert!(
                std::time::Instant::now() < deadline,
                "restored position never settled"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn output_device_change_keeps_the_queue() {
        let mut controller = test_controller(false);
        controller.enqueue(fixture_item("sine.wav"));
        controller.enqueue(fixture_item("sine.flac"));
        controller.play().expect("play starts");
        let _ = controller.poll();
        assert_eq!(controller.queue_len(), 2);
        controller
            .set_output_device("")
            .expect("system sink reloads");
        assert_eq!(controller.queue_len(), 2);
        assert!(controller.current_index().is_some());
    }

    #[test]
    fn replaygain_missing_tags_stay_at_unity() {
        let mut controller = test_controller(false);
        controller.set_replaygain_mode(ReplayGainMode::Track);
        controller.enqueue(fixture_item("sine.wav"));
        controller.play().expect("play starts");
        let _ = controller.poll();
        assert!(
            (controller.replaygain_linear() - 1.0).abs() < f64::EPSILON,
            "untagged fixture is unity, got {}",
            controller.replaygain_linear()
        );
    }

    #[test]
    fn crossfade_zero_keeps_unity_envelope() {
        let mut controller = test_controller(false);
        controller.set_crossfade(Duration::ZERO);
        controller.enqueue(fixture_item("sine.wav"));
        controller.play().expect("play starts");
        let _ = controller.poll();
        assert!((controller.crossfade_volume() - 1.0).abs() < f64::EPSILON);
    }
}
