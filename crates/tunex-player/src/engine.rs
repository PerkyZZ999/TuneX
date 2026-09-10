//! [`PlayerEngine`]: synchronous control over `GStreamer` `playbin3` with an
//! event channel for everything asynchronous.
//!
//! Threading contract: control methods are synchronous and cheap (property
//! sets, state requests, pipeline queries). A single bus thread translates
//! bus messages into [`PlayerEvent`]s; it never touches Qt or the UI.

use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

use gstreamer::{self as gst, glib::object::ObjectExt as _, prelude::*};
use tokio::sync::mpsc;
use tunex_core::{Error, PlaybackState, PlayerEvent, Result};

use crate::path_to_uri;

/// Event channel depth: bursts (state + duration + end-of-track) stay small.
const EVENT_BUFFER: usize = 64;
/// Bus poll slice: responsive shutdown without busy-looping.
const BUS_POLL: gst::ClockTime = gst::ClockTime::from_nseconds(100_000_000);

/// Lookahead for gapless preload: returns the URI to preload, if any.
///
/// Invoked on the streaming thread when `about-to-finish` fires, so
/// implementations must be fast and non-blocking (a queue peek, never I/O).
/// [`Queue::peek_next_uri`](super::Queue::peek_next_uri) is the canonical one.
pub type NextUriProvider = std::sync::Arc<dyn Fn() -> Option<String> + Send + Sync>;

/// Mutable engine state shared with the bus thread.
struct Inner {
    state: PlaybackState,
    has_track: bool,
    last_duration: Option<Duration>,
    next_provider: Option<NextUriProvider>,
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inner")
            .field("state", &self.state)
            .field("has_track", &self.has_track)
            .field("last_duration", &self.last_duration)
            .field("next_provider", &self.next_provider.is_some())
            .finish()
    }
}

/// Stable application-level abstraction over `GStreamer` `playbin3`.
///
/// Gapless handoff (`about-to-finish` preloading) is owned by the queue
/// (W-005), which drives this engine; the engine itself stays a single-track
/// state machine.
#[derive(Debug)]
pub struct PlayerEngine {
    playbin: gst::Element,
    inner: Arc<Mutex<Inner>>,
    events: mpsc::Sender<PlayerEvent>,
    shutdown: Arc<AtomicBool>,
    bus_thread: Option<JoinHandle<()>>,
}

impl PlayerEngine {
    /// Build the pipeline and start the bus thread. `events` receives state
    /// changes, errors, end-of-track, and duration discovery.
    ///
    /// [`EVENT_BUFFER`] bounds bursts; a lagging receiver drops nothing —
    /// sends from workers are best-effort only when the app is gone.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when `GStreamer` cannot initialize or
    /// `playbin3` is unavailable.
    pub fn new(events: mpsc::Sender<PlayerEvent>) -> Result<Self> {
        gst::init().map_err(|err| Error::Player(format!("gstreamer init failed: {err}")))?;
        let playbin = gst::ElementFactory::make("playbin3")
            .build()
            .map_err(|err| Error::Player(format!("playbin3 unavailable: {err}")))?;
        let Some(bus) = playbin.bus() else {
            return Err(Error::Player("playbin3 exposes no bus".to_owned()));
        };

        let inner = Arc::new(Mutex::new(Inner {
            state: PlaybackState::Stopped,
            has_track: false,
            last_duration: None,
            next_provider: None,
        }));
        let shutdown = Arc::new(AtomicBool::new(false));
        connect_about_to_finish(&playbin, &inner);
        let bus_thread = {
            let playbin = playbin.clone();
            let inner = Arc::clone(&inner);
            let events = events.clone();
            let shutdown = Arc::clone(&shutdown);
            std::thread::Builder::new()
                .name("tunex-bus".to_owned())
                .spawn(move || bus_loop(&playbin, &bus, &inner, &events, &shutdown))
                .map_err(|err| Error::Player(format!("cannot start bus thread: {err}")))?
        };

        Ok(Self {
            playbin,
            inner,
            events,
            shutdown,
            bus_thread: Some(bus_thread),
        })
    }

    /// Channel capacity for [`PlayerEngine::new`] callers.
    #[must_use]
    pub const fn event_buffer() -> usize {
        EVENT_BUFFER
    }

    /// Load a track from a filesystem path (percent-encoded internally).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when the path cannot become a URI or the
    /// pipeline refuses to preroll.
    pub fn load_path(&self, path: &Path) -> Result<()> {
        let uri = path_to_uri(path)?;
        self.load_uri(&uri)
    }

    /// Load a track from a URI. The pipeline prerolls; playback starts on
    /// [`play`](Self::play).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when the pipeline refuses to preroll.
    pub fn load_uri(&self, uri: &str) -> Result<()> {
        self.playbin.set_property("uri", uri);
        self.playbin
            .set_state(gst::State::Ready)
            .map_err(|err| Error::Player(format!("preroll failed: {err:?}")))?;
        if let Ok(mut inner) = self.inner.lock() {
            inner.has_track = true;
        }
        self.set_tracked_state(PlaybackState::Loading);
        Ok(())
    }

    /// Start or resume playback.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when nothing is loaded or the pipeline
    /// refuses the transition.
    pub fn play(&self) -> Result<()> {
        if !self.has_track() {
            return Err(Error::Player("nothing loaded".to_owned()));
        }
        self.playbin
            .set_state(gst::State::Playing)
            .map_err(|err| Error::Player(format!("play failed: {err:?}")))?;
        Ok(())
    }

    /// Pause, holding position.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when the pipeline refuses the transition.
    pub fn pause(&self) -> Result<()> {
        self.playbin
            .set_state(gst::State::Paused)
            .map_err(|err| Error::Player(format!("pause failed: {err:?}")))?;
        Ok(())
    }

    /// Stop and release the pipeline; the loaded track (if any) stays loaded.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when the pipeline refuses the transition.
    pub fn stop(&self) -> Result<()> {
        self.playbin
            .set_state(gst::State::Null)
            .map_err(|err| Error::Player(format!("stop failed: {err:?}")))?;
        self.set_tracked_state(PlaybackState::Stopped);
        Ok(())
    }

    /// Seek to an absolute position.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when nothing is loaded, the position is out
    /// of range, or the seek fails.
    pub fn seek(&self, position: Duration) -> Result<()> {
        if !self.has_track() {
            return Err(Error::Player("nothing loaded".to_owned()));
        }
        let position = gst::ClockTime::try_from(position).map_err(|err| {
            Error::Player(format!("position out of range: {position:?} ({err:?})"))
        })?;
        self.playbin
            .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT, position)
            .map_err(|err| Error::Player(format!("seek failed: {err}")))?;
        Ok(())
    }

    /// Set output volume, clamped to `0.0` (mute) through `1.0` (full).
    pub fn set_volume(&self, volume: f32) {
        self.playbin
            .set_property("volume", f64::from(volume.clamp(0.0, 1.0)));
    }

    /// Current output volume.
    #[must_use]
    pub fn volume(&self) -> f64 {
        self.playbin.property("volume")
    }

    /// Mute or unmute (independent of the volume level).
    pub fn set_muted(&self, muted: bool) {
        self.playbin.set_property("mute", muted);
    }

    /// Whether output is muted.
    #[must_use]
    pub fn muted(&self) -> bool {
        self.playbin.property("mute")
    }

    /// Override the audio sink (test seam: point at `fakesink` to run the
    /// pipeline with no sound server).
    pub fn set_audio_sink(&self, sink: &gst::Element) {
        self.playbin.set_property("audio-sink", sink);
    }

    /// Install (or clear) the gapless lookahead consulted when
    /// `about-to-finish` fires. The queue registers its peek here; without a
    /// provider the pipeline simply ends tracks normally.
    pub fn set_next_provider(&self, provider: Option<NextUriProvider>) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.next_provider = provider;
        }
    }

    /// Current playback position, when the pipeline can report one.
    #[must_use]
    pub fn position(&self) -> Option<Duration> {
        self.playbin
            .query_position::<gst::ClockTime>()
            .map(Duration::from)
    }

    /// Known track duration, when the stream has exposed one.
    #[must_use]
    pub fn duration(&self) -> Option<Duration> {
        self.playbin
            .query_duration::<gst::ClockTime>()
            .map(Duration::from)
    }

    /// Last tracked application state (updated by the bus thread).
    ///
    /// A poisoned lock (previous panic elsewhere) fails safe to `Stopped`
    /// rather than propagating the poison into playback decisions.
    #[must_use]
    pub fn state(&self) -> PlaybackState {
        match self.inner.lock() {
            Ok(inner) => inner.state,
            Err(_) => PlaybackState::default(),
        }
    }

    fn has_track(&self) -> bool {
        match self.inner.lock() {
            Ok(inner) => inner.has_track,
            Err(_) => false,
        }
    }

    fn set_tracked_state(&self, state: PlaybackState) {
        set_state(&self.inner, &self.events, state);
    }
}

impl Drop for PlayerEngine {
    fn drop(&mut self) {
        // Release pipeline resources first: dropping elements above NULL
        // state logs GStreamer criticals. Like all GStreamer apps, a wedged
        // driver can stall here; healthy pipelines release in milliseconds.
        let _ = self.playbin.set_state(gst::State::Null);
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(thread) = self.bus_thread.take() {
            let _ = thread.join();
        }
    }
}

/// Best-effort event delivery: `try_send` never blocks, so engine methods stay
/// callable from async contexts. A full channel means the app stopped
/// consuming (traced); a closed one means it is gone (silent).
fn emit(events: &mpsc::Sender<PlayerEvent>, event: PlayerEvent) {
    if let Err(mpsc::error::TrySendError::Full(event)) = events.try_send(event) {
        tracing::trace!(
            name: "player.bus.dropped",
            event = ?event,
            "event channel full, dropping signal"
        );
    }
}

/// Wire `about-to-finish` to the lookahead provider.
///
/// Fires on the streaming thread near the end of the current track; the
/// handler only peeks a URI and sets the property — no I/O, no locks held
/// across calls. A weak pipeline handle breaks the reference cycle (the
/// connection itself lives as long as the pipeline).
fn connect_about_to_finish(playbin: &gst::Element, inner: &Arc<Mutex<Inner>>) {
    let inner = Arc::clone(inner);
    let pipeline = playbin.downgrade();
    playbin.connect("about-to-finish", false, move |_| {
        let uri = inner
            .lock()
            .ok()
            .and_then(|guard| guard.next_provider.as_ref().and_then(|peek| peek()));
        if let (Some(uri), Some(playbin)) = (uri, pipeline.upgrade()) {
            playbin.set_property("uri", uri);
        }
        None
    });
}

/// Record a state transition, emitting exactly one event per change.
fn set_state(inner: &Arc<Mutex<Inner>>, events: &mpsc::Sender<PlayerEvent>, state: PlaybackState) {
    let changed = match inner.lock() {
        Ok(mut guard) => {
            let changed = guard.state != state;
            guard.state = state;
            changed
        }
        // Poisoned by a previous panic elsewhere: record nothing, emit
        // nothing; the next successful lock heals the view.
        Err(_) => false,
    };
    if changed {
        emit(events, PlayerEvent::StateChanged(state));
    }
}

/// Bus pump: translate pipeline messages into [`PlayerEvent`]s.
fn bus_loop(
    playbin: &gst::Element,
    bus: &gst::Bus,
    inner: &Arc<Mutex<Inner>>,
    events: &mpsc::Sender<PlayerEvent>,
    shutdown: &Arc<AtomicBool>,
) {
    while !shutdown.load(Ordering::Relaxed) {
        let Some(message) = bus.timed_pop(BUS_POLL) else {
            continue;
        };
        handle_message(playbin, inner, events, &message);
    }
}

/// Route one bus message. Kept flat so `bus_loop` stays under the complexity
/// budget; per-kind logic lives in the `on_*` helpers.
fn handle_message(
    playbin: &gst::Element,
    inner: &Arc<Mutex<Inner>>,
    events: &mpsc::Sender<PlayerEvent>,
    message: &gst::Message,
) {
    use gst::MessageView;

    match message.view() {
        MessageView::Eos(..) => {
            set_state(inner, events, PlaybackState::Stopped);
            emit(events, PlayerEvent::EndOfTrack);
        }
        MessageView::Error(err) => {
            tracing::warn!(
                name: "player.bus.error",
                error = %err.error(),
                debug = ?err.debug(),
                "pipeline error"
            );
            set_state(inner, events, PlaybackState::Stopped);
            emit(events, PlayerEvent::PlaybackError(err.error().to_string()));
        }
        MessageView::StateChanged(..) => on_state_changed(playbin, inner, events),
        MessageView::DurationChanged(..) | MessageView::AsyncDone(..) => {
            on_duration_discovery(playbin, inner, events);
        }
        _ => {}
    }
}

/// Re-query pipeline truth on any state chatter.
///
/// Child elements post state changes too, so the message source is ignored by
/// design; `current_state` plus change-deduping keeps the tracked state exact.
fn on_state_changed(
    playbin: &gst::Element,
    inner: &Arc<Mutex<Inner>>,
    events: &mpsc::Sender<PlayerEvent>,
) {
    match playbin.current_state() {
        gst::State::Playing => set_state(inner, events, PlaybackState::Playing),
        gst::State::Paused => set_state(inner, events, PlaybackState::Paused),
        // Ready means prerolled with content (our loads go through it);
        // a fresh pipeline rests in Null, which is genuinely stopped.
        gst::State::Ready => set_state(inner, events, PlaybackState::Loading),
        gst::State::Null => set_state(inner, events, PlaybackState::Stopped),
        // Transitional marker, never a settled state: observe only.
        gst::State::VoidPending => {}
    }
}

/// Emit duration discovery exactly once per value.
fn on_duration_discovery(
    playbin: &gst::Element,
    inner: &Arc<Mutex<Inner>>,
    events: &mpsc::Sender<PlayerEvent>,
) {
    let Some(duration) = playbin
        .query_duration::<gst::ClockTime>()
        .map(Duration::from)
    else {
        return;
    };
    let changed = match inner.lock() {
        Ok(mut guard) => {
            let changed = guard.last_duration != Some(duration);
            guard.last_duration = Some(duration);
            changed
        }
        Err(_) => false,
    };
    if changed {
        emit(events, PlayerEvent::DurationChanged(duration));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Engine wired to `fakesink`: full pipeline behavior, no sound server.
    fn test_engine() -> (PlayerEngine, mpsc::Receiver<PlayerEvent>) {
        let (events, receiver) = mpsc::channel(PlayerEngine::event_buffer());
        let engine = PlayerEngine::new(events).expect("engine builds");
        let sink = gst::ElementFactory::make("fakesink")
            .build()
            .expect("fakesink exists");
        engine.set_audio_sink(&sink);
        (engine, receiver)
    }

    /// Drain one event or fail the test after a grace period.
    async fn next_event(receiver: &mut mpsc::Receiver<PlayerEvent>) -> PlayerEvent {
        tokio::time::timeout(Duration::from_secs(5), receiver.recv())
            .await
            .expect("event arrives in time")
            .expect("sender lives as long as the engine")
    }

    /// Write a tiny mono WAV (8 kHz, 16-bit sine) for pipeline tests. Real
    /// container, synthetic content — decodable by base plugins anywhere.
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
            // Fixture audio: amplitude keeps every sample inside i16 range by
            // construction, but the lint cannot see value bounds.
            #[allow(
                clippy::cast_possible_truncation,
                reason = "0.4-amplitude sine always fits i16"
            )]
            let sample = (phase.sin() * 0.4 * 32767.0) as i16;
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        std::fs::write(path, bytes).expect("fixture writes");
    }

    #[test]
    fn initial_state_is_stopped() {
        let (engine, _receiver) = test_engine();
        assert_eq!(engine.state(), PlaybackState::Stopped);
    }

    #[test]
    fn play_without_track_is_an_explicit_error() {
        let (engine, _receiver) = test_engine();
        let err = engine.play().expect_err("playing nothing must fail loudly");
        assert!(matches!(err, Error::Player(_)));
    }

    #[test]
    fn seek_without_track_is_an_explicit_error() {
        let (engine, _receiver) = test_engine();
        let err = engine
            .seek(Duration::from_secs(10))
            .expect_err("seeking nothing must fail loudly");
        assert!(matches!(err, Error::Player(_)));
    }

    #[test]
    fn volume_round_trips_bit_exact() {
        let (engine, _receiver) = test_engine();
        engine.set_volume(0.42);
        assert_eq!(engine.volume().to_bits(), f64::from(0.42f32).to_bits());
    }

    #[test]
    fn volume_clamps_both_ends() {
        let (engine, _receiver) = test_engine();
        engine.set_volume(7.5);
        assert_eq!(engine.volume().to_bits(), 1.0f64.to_bits());
        engine.set_volume(-1.0);
        assert_eq!(engine.volume().to_bits(), 0.0f64.to_bits());
    }

    #[test]
    fn mute_round_trips() {
        let (engine, _receiver) = test_engine();
        assert!(!engine.muted());
        engine.set_muted(true);
        assert!(engine.muted());
    }

    #[test]
    fn position_is_none_while_idle() {
        let (engine, _receiver) = test_engine();
        assert_eq!(engine.position(), None);
    }

    #[test]
    fn load_missing_file_reports_loading_then_errors() {
        let (engine, _receiver) = test_engine();
        engine
            .load_path(&PathBuf::from("/nonexistent-tunex-probe/flac"))
            .expect("load only stages the URI");
        assert_eq!(engine.state(), PlaybackState::Loading);
    }

    /// Drive a missing file to its error, on whichever path `GStreamer` takes:
    /// a synchronous transition refusal or an asynchronous error event.
    async fn expect_missing_file_error(
        engine: &PlayerEngine,
        receiver: &mut mpsc::Receiver<PlayerEvent>,
    ) {
        if let Err(err) = engine.play() {
            assert!(matches!(err, Error::Player(_)));
        } else {
            let event = next_event(receiver).await;
            assert!(
                matches!(event, PlayerEvent::PlaybackError(_)),
                "missing file must surface as an error event, got {event:?}"
            );
        }
    }

    #[tokio::test]
    async fn missing_file_playback_emits_error_event() {
        let (engine, mut receiver) = test_engine();
        engine
            .load_path(&PathBuf::from("/nonexistent-tunex-probe.flac"))
            .expect("load only stages the URI");
        expect_missing_file_error(&engine, &mut receiver).await;
    }

    #[tokio::test]
    async fn stop_after_error_leaves_stopped_state() {
        let (engine, mut receiver) = test_engine();
        engine
            .load_path(&PathBuf::from("/nonexistent-tunex-probe.flac"))
            .expect("load only stages the URI");
        expect_missing_file_error(&engine, &mut receiver).await;
        engine.stop().expect("stop works");
        assert_eq!(engine.state(), PlaybackState::Stopped);
    }
    // Blocked on this machine until gst-plugins-good (wavparse) is installed;
    // tracked as WORK_ITEMS W-005 blocker. Run explicitly once unblocked:
    // cargo nextest run -p tunex-player about_to_finish
    #[ignore = "needs gst-plugins-good (wavparse): sudo pacman -S --needed gst-plugins-good"]
    #[tokio::test]
    async fn about_to_finish_preloads_next_uri_gaplessly() {
        let dir = std::env::temp_dir().join(format!("tunex-gapless-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("fixture dir");
        let first = dir.join("first.wav");
        let second = dir.join("second.wav");
        write_sine_wav(&first, 400, 440.0);
        write_sine_wav(&second, 400, 660.0);

        let second_uri = crate::path_to_uri(&second).expect("fixture path converts");
        let preloaded = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&preloaded);
        let provider: NextUriProvider = Arc::new(move || {
            flag.store(true, Ordering::Relaxed);
            Some(second_uri.clone())
        });

        let (engine, mut receiver) = test_engine();
        engine.set_next_provider(Some(provider));
        engine.load_path(&first).expect("fixture loads");
        engine.play().expect("playback starts");

        // Drain until the final boundary. Gapless means the state sequence is
        // exactly Loading → Playing → Stopped: no stop between the preloaded
        // handoff, and the provider must have been consulted on the way.
        let mut states = Vec::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        loop {
            assert!(
                tokio::time::Instant::now() < deadline,
                "first end-of-track never arrived"
            );
            let event = next_event(&mut receiver).await;
            match event {
                PlayerEvent::StateChanged(state) => states.push(state),
                PlayerEvent::EndOfTrack => break,
                PlayerEvent::PlaybackError(message) => {
                    panic!("pipeline failed instead of playing: {message}");
                }
                PlayerEvent::DurationChanged(_) => {}
            }
        }
        assert!(preloaded.load(Ordering::Relaxed), "provider was consulted");
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
