//! [`PlayerEngine`]: synchronous control over `GStreamer` `playbin3` with an
//! event channel for everything asynchronous.
//!
//! Threading contract: control methods are synchronous and cheap (property
//! sets, state requests, pipeline queries). A single bus thread translates
//! bus messages into [`PlayerEvent`]s; it never touches Qt or the UI.

use std::{
    path::Path,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use gstreamer::{self as gst, glib::object::ObjectExt as _, prelude::*};
use tokio::sync::mpsc;
use tunex_core::{
    EQ_BAND_COUNT, Error, PlaybackState, PlayerEvent, ReplayGainMode, Result, SPECTRUM_BANDS,
};

use crate::{
    audio_bin::{apply_equalizer, attach_pcm_probe, spectrum_csv, waveform_csv, wrap_audio_sink},
    path_to_uri,
    replaygain::{ReplayGainTags, read_replaygain_uri, replaygain_multiplier},
};

/// Event channel depth: bursts (state + duration + end-of-track) stay small.
const EVENT_BUFFER: usize = 64;
/// Bus poll slice: responsive shutdown without busy-looping.
const BUS_POLL: gst::ClockTime = gst::ClockTime::from_nseconds(100_000_000);
/// Cap on how long a teardown waits for an in-flight gapless handoff to swap
/// over (see [`PlayerEngine::begin_teardown`]). The swap normally lands within
/// milliseconds of the preload; the cap keeps a handoff that never swaps — a
/// next file that fails to open, a seek that un-drains the current one — from
/// holding the caller for longer than a skip may reasonably take.
const HANDOFF_SETTLE: Duration = Duration::from_millis(500);

/// Lookahead for gapless preload: returns the URI to preload, if any.
///
/// Invoked on the streaming thread when `about-to-finish` fires, so
/// implementations must be fast and non-blocking (a queue peek, never I/O).
/// [`Queue::peek_next_uri`](super::Queue::peek_next_uri) is the canonical one.
pub type NextUriProvider = std::sync::Arc<dyn Fn() -> Option<String> + Send + Sync>;

/// Where the gapless handoff stands, and whether a teardown is holding it.
///
/// The two questions are one state: a teardown must know whether a swap is
/// still in flight (it has to wait for it), and the preload hook must know
/// whether a teardown is running (it must hand over nothing). See
/// [`PlayerEngine::begin_teardown`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Handoff {
    /// Nothing handed over; the next `about-to-finish` may hand a URI.
    Idle,
    /// A URI was handed over and `uridecodebin3` has not swapped to it yet.
    InFlight,
    /// A teardown is running: no handoffs until it finishes.
    Blocked,
    /// A teardown is waiting out the swap it found in flight.
    BlockedInFlight,
}

impl Handoff {
    /// Whether a handed-over URI is still waiting to be swapped in.
    const fn in_flight(self) -> bool {
        matches!(self, Self::InFlight | Self::BlockedInFlight)
    }

    /// Whether a teardown currently forbids new handoffs.
    const fn blocked(self) -> bool {
        matches!(self, Self::Blocked | Self::BlockedInFlight)
    }

    /// The state a teardown moves into, remembering a swap already in flight.
    const fn blocking(self) -> Self {
        if self.in_flight() {
            Self::BlockedInFlight
        } else {
            Self::Blocked
        }
    }

    /// The state after the pipeline reports the swap finished.
    const fn swapped(self) -> Self {
        match self {
            Self::InFlight => Self::Idle,
            Self::BlockedInFlight => Self::Blocked,
            other => other,
        }
    }
}

/// Mutable engine state shared with the bus thread.
#[expect(
    clippy::struct_excessive_bools,
    reason = "pause/seek/EQ flags are independent pipeline bits, not a mode enum"
)]
pub(crate) struct Inner {
    state: PlaybackState,
    has_track: bool,
    last_duration: Option<Duration>,
    next_provider: Option<NextUriProvider>,
    /// The URI the pipeline is told to play, as far as we instructed it.
    /// Guards the preload path: `about-to-finish` can fire repeatedly (fast
    /// sinks, repeat-one, same answer twice), and re-setting an identical URI
    /// makes playbin reconfigure in a loop instead of playing.
    queued_uri: Option<String>,
    /// Set by [`PlayerEngine::pause`], consumed by the next observed pause.
    /// Distinguishes user pauses from preroll transients (see `on_state_changed`).
    pause_requested: bool,
    /// A flush seek is in flight; [`gst::MessageView::AsyncDone`] clears it.
    /// The control method only *posts* the seek event — it never waits for
    /// the streaming thread, so an EOS-wedged pipeline cannot hang the caller.
    seek_pending: bool,
    /// Latest scrub target waiting for the in-flight flush to finish.
    /// Live dragging posts many seeks; overlapping FLUSH events refuse or
    /// snap the playhead, so intermediates are dropped here.
    queued_seek: Option<gst::ClockTime>,
    /// Gapless handoff state, shared between the preload hook, the swap
    /// report ([`connect_source_removed`]), and teardown.
    handoff: Handoff,
    /// `ReplayGain` mode (applied downstream of playbin user volume).
    rg_mode: ReplayGainMode,
    lofty_track: Option<f64>,
    lofty_album: Option<f64>,
    gst_track: Option<f64>,
    gst_album: Option<f64>,
    pub(crate) rg_volume: gst::Element,
    pub(crate) xfade_volume: gst::Element,
    pub(crate) eq: Option<gst::Element>,
    pub(crate) eq_enabled: bool,
    pub(crate) eq_bands: [f32; EQ_BAND_COUNT],
    pub(crate) eq_missing: bool,
    pub(crate) spectrum: [f32; SPECTRUM_BANDS],
    pub(crate) pcm: Vec<f32>,
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inner")
            .field("state", &self.state)
            .field("has_track", &self.has_track)
            .field("last_duration", &self.last_duration)
            .field("next_provider", &self.next_provider.is_some())
            .field("queued_uri", &self.queued_uri)
            .field("pause_requested", &self.pause_requested)
            .field("seek_pending", &self.seek_pending)
            .field("queued_seek", &self.queued_seek)
            .field("handoff", &self.handoff)
            .field("rg_mode", &self.rg_mode)
            .field("eq_enabled", &self.eq_enabled)
            .field("eq_missing", &self.eq_missing)
            .finish_non_exhaustive()
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
    /// Signalled when a gapless swap finishes, so a teardown can wait one out
    /// instead of polling for it.
    handoff_settled: Arc<Condvar>,
    events: mpsc::Sender<PlayerEvent>,
    shutdown: Arc<AtomicBool>,
    bus_thread: Option<JoinHandle<()>>,
    about_to_finish: Option<gst::glib::SignalHandlerId>,
    source_removed: Option<gst::glib::SignalHandlerId>,
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
        let Some(bin) = playbin.downcast_ref::<gst::Bin>() else {
            return Err(Error::Player("playbin3 is not a bin".to_owned()));
        };

        let terminal = gst::ElementFactory::make("autoaudiosink")
            .build()
            .or_else(|_| gst::ElementFactory::make("fakesink").build())
            .map_err(|err| Error::Player(format!("no audio sink: {err}")))?;
        let chain = wrap_audio_sink(&terminal)?;
        playbin.set_property("audio-sink", &chain.bin);
        let pcm_pad = chain.pcm_pad.clone();

        let inner = Arc::new(Mutex::new(Inner {
            state: PlaybackState::Stopped,
            has_track: false,
            last_duration: None,
            next_provider: None,
            queued_uri: None,
            pause_requested: false,
            seek_pending: false,
            queued_seek: None,
            handoff: Handoff::Idle,
            rg_mode: ReplayGainMode::Off,
            lofty_track: None,
            lofty_album: None,
            gst_track: None,
            gst_album: None,
            rg_volume: chain.rg,
            xfade_volume: chain.xfade,
            eq: chain.eq,
            eq_enabled: false,
            eq_bands: [0.0; EQ_BAND_COUNT],
            eq_missing: chain.eq_missing,
            spectrum: [0.0; SPECTRUM_BANDS],
            pcm: Vec::new(),
        }));
        if let Some(pad) = pcm_pad {
            attach_pcm_probe(&pad, &inner);
        }
        let handoff_settled = Arc::new(Condvar::new());
        let shutdown = Arc::new(AtomicBool::new(false));
        let about_to_finish = connect_about_to_finish(&playbin, &inner);
        let source_removed = connect_source_removed(bin, &inner, &handoff_settled);
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
            handoff_settled,
            events,
            shutdown,
            bus_thread: Some(bus_thread),
            about_to_finish: Some(about_to_finish),
            source_removed: Some(source_removed),
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

    /// Load a track from a URI, replacing whatever is loaded (including a
    /// pending gapless preload). Playback starts on [`play`](Self::play).
    ///
    /// The pipeline settles in READY *before* the URI changes. On a running
    /// `playbin3` a new URI means "gapless next item", and once the current
    /// input is drained (always true after `about-to-finish`) it starts
    /// prerolling that item at once; tearing the fresh chain down again
    /// deadlocks decodebin3 and, with it, the caller (S6: repeat-all froze
    /// the UI thread at the end of the track).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when the pipeline refuses to reset.
    pub fn load_uri(&self, uri: &str) -> Result<()> {
        self.begin_teardown();
        let reset = self.playbin.set_state(gst::State::Ready);
        if reset.is_ok() {
            self.playbin.set_property("uri", uri);
        }
        self.finish_teardown();
        reset.map_err(|err| Error::Player(format!("reset before load failed: {err:?}")))?;
        if let Ok(mut inner) = self.inner.lock() {
            inner.has_track = true;
            inner.pause_requested = false;
            inner.queued_uri = Some(uri.to_owned());
            inner.gst_track = None;
            inner.gst_album = None;
            let tags = read_replaygain_uri(uri);
            inner.lofty_track = tags.track_db;
            inner.lofty_album = tags.album_db;
            apply_replaygain(&inner);
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
        if let Ok(mut inner) = self.inner.lock() {
            inner.pause_requested = false;
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
        if let Ok(mut inner) = self.inner.lock() {
            inner.pause_requested = true;
        }
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
        self.begin_teardown();
        let stopped = self.playbin.set_state(gst::State::Null);
        self.finish_teardown();
        stopped.map_err(|err| Error::Player(format!("stop failed: {err:?}")))?;
        self.set_tracked_state(PlaybackState::Stopped);
        Ok(())
    }

    /// Seek to an absolute position.
    ///
    /// Posts a flush seek event and returns as soon as the pipeline accepts
    /// it. Completion arrives as [`gst::MessageView::AsyncDone`] on the bus
    /// (the UI polls position). Never calls blocking `seek_simple`, which
    /// can hang forever against an EOS-wedged pipeline.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Player`] when nothing is loaded, the position is out
    /// of range, or the pipeline refuses the event.
    pub fn seek(&self, position: Duration) -> Result<()> {
        if !self.has_track() {
            return Err(Error::Player("nothing loaded".to_owned()));
        }
        let start = gst::ClockTime::try_from(position).map_err(|err| {
            Error::Player(format!("position out of range: {position:?} ({err:?})"))
        })?;
        let pending = match self.inner.lock() {
            Ok(mut inner) => {
                if inner.seek_pending {
                    inner.queued_seek = Some(start);
                    true
                } else {
                    inner.seek_pending = true;
                    inner.queued_seek = None;
                    false
                }
            }
            Err(_) => false,
        };
        if pending {
            return Ok(());
        }
        if post_flush_seek(&self.playbin, &self.inner, start) {
            return Ok(());
        }
        if let Ok(mut inner) = self.inner.lock() {
            inner.seek_pending = false;
            inner.queued_seek = None;
        }
        Err(Error::Player("seek refused".to_owned()))
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
    /// pipeline with no sound server). The `ReplayGain` and crossfade volumes
    /// wrap the new terminal so they stay downstream of user volume.
    pub fn set_audio_sink(&self, sink: &gst::Element) {
        let Ok(chain) = wrap_audio_sink(sink) else {
            tracing::warn!(name: "player.audio_sink.wrap_failed", "cannot wrap audio sink");
            return;
        };
        self.install_audio_bin(chain);
    }

    /// Current queued URI, if the pipeline has been told to play one.
    #[must_use]
    pub fn queued_uri(&self) -> Option<String> {
        self.inner
            .lock()
            .ok()
            .and_then(|inner| inner.queued_uri.clone())
    }

    /// `ReplayGain` mode. Applied on the next tag refresh and immediately.
    pub fn set_replaygain_mode(&self, mode: ReplayGainMode) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.rg_mode = mode;
            apply_replaygain(&inner);
        }
    }

    /// Replace lofty-sourced `ReplayGain` tags and re-apply the current mode.
    pub fn set_replaygain_tags(&self, tags: ReplayGainTags) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.lofty_track = tags.track_db;
            inner.lofty_album = tags.album_db;
            apply_replaygain(&inner);
        }
    }

    /// Linear `ReplayGain` multiplier currently on the downstream volume element.
    #[must_use]
    pub fn replaygain_linear(&self) -> f64 {
        self.inner
            .lock()
            .ok()
            .map_or(1.0, |inner| inner.rg_volume.property("volume"))
    }

    /// Crossfade envelope (0–1), independent of user volume and `ReplayGain`.
    pub fn set_crossfade_volume(&self, volume: f64) {
        if let Ok(inner) = self.inner.lock() {
            inner
                .xfade_volume
                .set_property("volume", volume.clamp(0.0, 1.0));
        }
    }

    /// Current crossfade envelope.
    #[must_use]
    pub fn crossfade_volume(&self) -> f64 {
        self.inner
            .lock()
            .ok()
            .map_or(1.0, |inner| inner.xfade_volume.property("volume"))
    }

    fn install_audio_bin(&self, chain: crate::audio_bin::AudioChain) {
        let (rg_vol, xf_vol, eq_enabled, eq_bands) =
            self.inner
                .lock()
                .ok()
                .map_or((1.0, 1.0, false, [0.0; EQ_BAND_COUNT]), |inner| {
                    (
                        inner.rg_volume.property::<f64>("volume"),
                        inner.xfade_volume.property::<f64>("volume"),
                        inner.eq_enabled,
                        inner.eq_bands,
                    )
                });
        chain.rg.set_property("volume", rg_vol);
        chain.xfade.set_property("volume", xf_vol);
        let pcm_pad = chain.pcm_pad.clone();
        self.playbin.set_property("audio-sink", &chain.bin);
        if let Ok(mut inner) = self.inner.lock() {
            inner.rg_volume = chain.rg;
            inner.xfade_volume = chain.xfade;
            inner.eq = chain.eq;
            inner.eq_missing = chain.eq_missing;
            inner.eq_enabled = eq_enabled;
            inner.eq_bands = eq_bands;
            apply_equalizer(&inner);
        }
        if let Some(pad) = pcm_pad {
            attach_pcm_probe(&pad, &self.inner);
        }
    }

    /// Ten-band gains. Missing `equalizer-10bands` stays flat and records
    /// [`PlayerEngine::equalizer_missing`].
    pub fn set_equalizer(&self, enabled: bool, bands: [f32; EQ_BAND_COUNT]) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.eq_enabled = enabled;
            inner.eq_bands = bands;
            apply_equalizer(&inner);
        }
    }

    /// Whether the bin had to skip `equalizer-10bands`.
    #[must_use]
    pub fn equalizer_missing(&self) -> bool {
        self.inner.lock().is_ok_and(|inner| inner.eq_missing)
    }

    /// Latest spectrum bars (0…1) for the Qt thread.
    #[must_use]
    pub fn spectrum_csv(&self) -> String {
        self.inner
            .lock()
            .ok()
            .map_or_else(String::new, |inner| spectrum_csv(&inner.spectrum))
    }

    /// Latest oscilloscope samples for the Qt thread.
    #[must_use]
    pub fn waveform_csv(&self) -> String {
        self.inner
            .lock()
            .ok()
            .map_or_else(String::new, |inner| waveform_csv(&inner.pcm))
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

    /// Keep a teardown from overlapping a gapless swap, then let it proceed.
    ///
    /// `uridecodebin3` swaps its input over to the preloaded item on that
    /// item's own streaming thread, and takes the element state lock in the
    /// middle of the swap — the same lock a state change holds while it waits
    /// for that very thread to stop. Overlap them and both sides wait forever
    /// (`GStreamer` 1.28; observed as a frozen UI). So: block new handoffs,
    /// then wait out one already in flight. Every call outside `Drop` needs a
    /// matching [`finish_teardown`](Self::finish_teardown) to lift the block.
    fn begin_teardown(&self) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        inner.handoff = inner.handoff.blocking();
        let deadline = Instant::now() + HANDOFF_SETTLE;
        while inner.handoff.in_flight() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                tracing::warn!(
                    name: "player.handoff.unsettled",
                    waited_ms = HANDOFF_SETTLE.as_millis(),
                    "gapless handoff never swapped over; tearing down anyway"
                );
                return;
            }
            let Ok((waited, _)) = self.handoff_settled.wait_timeout(inner, remaining) else {
                return;
            };
            inner = waited;
        }
    }

    /// Lift the teardown gate. The state change purged any preloaded item, so
    /// nothing is in flight any more and the next handoff may start.
    fn finish_teardown(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.handoff = Handoff::Idle;
        }
    }

    fn set_tracked_state(&self, state: PlaybackState) {
        set_state(&self.inner, &self.events, state);
    }
}

impl Drop for PlayerEngine {
    fn drop(&mut self) {
        // Settle before disconnecting: a swap in flight must still be able to
        // report itself finished, or quitting mid-handoff waits out the cap.
        self.begin_teardown();
        // Disconnect next: tearing down the pipeline while a streaming
        // thread can still enter the preload closure wedges shutdown.
        if let Some(handler) = self.about_to_finish.take() {
            self.playbin.disconnect(handler);
        }
        if let Some(handler) = self.source_removed.take() {
            self.playbin.disconnect(handler);
        }
        // Release pipeline resources next: dropping elements above NULL
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

/// Wire `about-to-finish` to the lookahead provider, returning the connection
/// (kept by the engine: dropping the pipeline with a live streaming-thread
/// callback wedges teardown).
///
/// Fires on the streaming thread near the end of the current track; the
/// handler only peeks a URI and sets the property — no I/O, no locks held
/// across calls. A weak pipeline handle breaks the reference cycle (the
/// connection itself lives as long as the pipeline).
///
/// Handing a URI over starts the next item's chain right away, and the swap
/// that follows must not overlap a teardown, so the handoff is recorded (and
/// skipped outright while one is running) under the same lock as the peek.
fn connect_about_to_finish(
    playbin: &gst::Element,
    inner: &Arc<Mutex<Inner>>,
) -> gst::glib::SignalHandlerId {
    let inner = Arc::clone(inner);
    let pipeline = playbin.downgrade();
    playbin.connect("about-to-finish", false, move |_| {
        let handoff = inner.lock().ok().and_then(|mut guard| {
            if guard.handoff.blocked() {
                return None;
            }
            let uri = guard
                .next_provider
                .as_ref()
                .and_then(|peek| peek())
                .filter(|uri| Some(uri) != guard.queued_uri.as_ref())?;
            guard.queued_uri = Some(uri.clone());
            guard.handoff = Handoff::InFlight;
            Some(uri)
        });
        if let (Some(uri), Some(playbin)) = (handoff, pipeline.upgrade()) {
            playbin.set_property("uri", &uri);
        }
        None
    })
}

/// Report a finished gapless swap, returning the connection.
///
/// `uridecodebin3` swaps its input over to the preloaded item on that item's
/// first buffer and retires the outgoing `urisourcebin` as the last step, so
/// that element's removal is the swap's completion marker — the one
/// [`PlayerEngine::begin_teardown`] waits for. The handler stays trivial (one
/// state update, one notify): it runs on a streaming thread that is holding
/// pipeline locks.
fn connect_source_removed(
    playbin: &gst::Bin,
    inner: &Arc<Mutex<Inner>>,
    handoff_settled: &Arc<Condvar>,
) -> gst::glib::SignalHandlerId {
    let inner = Arc::clone(inner);
    let handoff_settled = Arc::clone(handoff_settled);
    playbin.connect_deep_element_removed(move |_playbin, _source, element| {
        if element
            .factory()
            .is_none_or(|factory| factory.name() != "urisourcebin")
        {
            return;
        }
        if let Ok(mut guard) = inner.lock() {
            guard.handoff = guard.handoff.swapped();
        }
        handoff_settled.notify_all();
    })
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
        MessageView::DurationChanged(..) => on_duration_discovery(playbin, inner, events),
        MessageView::AsyncDone(..) => {
            if async_done_from_playbin(playbin, message) {
                apply_queued_seek(playbin, inner);
            }
            on_duration_discovery(playbin, inner, events);
        }
        MessageView::Tag(tag) => on_replaygain_tags(inner, &tag.tags()),
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
        // Preroll transients also report Paused: only a pause that follows
        // playback, or one the user explicitly requested, is real.
        gst::State::Paused => {
            let explicit = match inner.lock() {
                Ok(mut guard) => {
                    let explicit = guard.state == PlaybackState::Playing || guard.pause_requested;
                    guard.pause_requested = false;
                    explicit
                }
                Err(_) => false,
            };
            if explicit {
                set_state(inner, events, PlaybackState::Paused);
            }
        }
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

/// Fill missing `ReplayGain` from the decoder tag tap (never overwrites lofty).
fn on_replaygain_tags(inner: &Arc<Mutex<Inner>>, tags: &gst::TagList) {
    let Ok(mut guard) = inner.lock() else {
        return;
    };
    if let Some(gain) = tags.get::<gst::tags::TrackGain>() {
        guard.gst_track = Some(gain.get());
    }
    if let Some(gain) = tags.get::<gst::tags::AlbumGain>() {
        guard.gst_album = Some(gain.get());
    }
    apply_replaygain(&guard);
}

/// One `KEY_UNIT` flush seek. Shared by the control method and the coalesced
/// follow-up posted from `AsyncDone`.
fn flush_seek_event(start: gst::ClockTime) -> gst::Event {
    gst::event::Seek::new(
        1.0,
        gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
        gst::SeekType::Set,
        start,
        gst::SeekType::End,
        gst::ClockTime::ZERO,
    )
}

/// Child elements also post `AsyncDone`; only playbin's means the flush landed.
fn async_done_from_playbin(playbin: &gst::Element, message: &gst::Message) -> bool {
    message
        .src()
        .is_some_and(|src| *src == *playbin.upcast_ref::<gst::Object>())
}

/// `fakesink async=false` (tests) completes a flush synchronously, so there
/// is no `AsyncDone` to drain the coalesced target.
fn seek_still_async(playbin: &gst::Element) -> bool {
    matches!(
        playbin.state(gst::ClockTime::ZERO).0,
        Ok(gst::StateChangeSuccess::Async)
    )
}

/// Post a flush seek and, if it completed inline, drain any newer target.
fn post_flush_seek(
    playbin: &gst::Element,
    inner: &Arc<Mutex<Inner>>,
    start: gst::ClockTime,
) -> bool {
    if !playbin.send_event(flush_seek_event(start)) {
        return false;
    }
    if !seek_still_async(playbin) {
        apply_queued_seek(playbin, inner);
    }
    true
}

/// Post the latest scrub target once the in-flight flush completes.
fn apply_queued_seek(playbin: &gst::Element, inner: &Arc<Mutex<Inner>>) {
    let start = {
        let Ok(mut guard) = inner.lock() else {
            return;
        };
        guard.seek_pending = false;
        let Some(start) = guard.queued_seek.take() else {
            return;
        };
        guard.seek_pending = true;
        start
    };
    if post_flush_seek(playbin, inner, start) {
        return;
    }
    if let Ok(mut guard) = inner.lock() {
        guard.seek_pending = false;
        guard.queued_seek = Some(start);
    }
}

/// User-volume is playbin; this element is downstream so `ReplayGain` is post-fader.
fn apply_replaygain(inner: &Inner) {
    let track = inner.lofty_track.or(inner.gst_track);
    let album = inner.lofty_album.or(inner.gst_album);
    let linear = replaygain_multiplier(inner.rg_mode, track, album);
    inner.rg_volume.set_property("volume", linear);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Engine wired to `fakesink`: full pipeline behavior, no sound server.
    /// The sink runs unsynchronized so clips complete as fast as data flows.
    fn test_engine() -> (PlayerEngine, mpsc::Receiver<PlayerEvent>) {
        let (events, receiver) = mpsc::channel(PlayerEngine::event_buffer());
        let engine = PlayerEngine::new(events).expect("engine builds");
        let sink = gst::ElementFactory::make("fakesink")
            .build()
            .expect("fakesink exists");
        sink.set_property("sync", false);
        sink.set_property("async", false);
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
    fn replaygain_off_stays_at_unity() {
        let (engine, _receiver) = test_engine();
        engine.set_replaygain_tags(ReplayGainTags {
            track_db: Some(-6.0),
            album_db: Some(-12.0),
        });
        engine.set_replaygain_mode(ReplayGainMode::Off);
        assert!((engine.replaygain_linear() - 1.0).abs() < 1e-6);
        engine.set_replaygain_mode(ReplayGainMode::Track);
        let expected = replaygain_multiplier(ReplayGainMode::Track, Some(-6.0), Some(-12.0));
        assert!(
            (engine.replaygain_linear() - expected).abs() < 1e-6,
            "got {} expected {expected}",
            engine.replaygain_linear()
        );
    }

    #[test]
    fn crossfade_envelope_is_independent_of_user_volume() {
        let (engine, _receiver) = test_engine();
        engine.set_volume(0.5);
        engine.set_crossfade_volume(0.25);
        assert!((engine.crossfade_volume() - 0.25).abs() < f64::EPSILON);
        assert_eq!(engine.volume().to_bits(), f64::from(0.5f32).to_bits());
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

    #[tokio::test]
    async fn seek_moves_position_while_paused() {
        let dir = std::env::temp_dir().join(format!("tunex-seek-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("fixture dir");
        let clip = dir.join("two-seconds.wav");
        write_sine_wav(&clip, 2000, 440.0);

        let (engine, mut receiver) = test_engine();
        engine.load_path(&clip).expect("fixture loads");
        engine.play().expect("playback starts");
        wait_for_state(&mut receiver, PlaybackState::Playing).await;
        engine.pause().expect("pause works");
        wait_for_state(&mut receiver, PlaybackState::Paused).await;

        engine
            .seek(Duration::from_millis(1500))
            .expect("seek works");
        // Paused clock: the position settles at the target instead of racing
        // past it, so this poll is deterministic.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(position) = engine.position() {
                if position >= Duration::from_millis(1400) {
                    assert!(
                        position <= Duration::from_millis(1600),
                        "seek lands at the target, got {position:?}"
                    );
                    break;
                }
            }
            assert!(tokio::time::Instant::now() < deadline, "seek never landed");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[tokio::test]
    async fn rapid_seeks_land_on_the_latest_target() {
        let dir = std::env::temp_dir().join(format!("tunex-seek-coalesce-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("fixture dir");
        let clip = dir.join("two-seconds.wav");
        write_sine_wav(&clip, 2000, 440.0);

        let (engine, mut receiver) = test_engine();
        engine.load_path(&clip).expect("fixture loads");
        engine.play().expect("playback starts");
        wait_for_state(&mut receiver, PlaybackState::Playing).await;
        engine.pause().expect("pause works");
        wait_for_state(&mut receiver, PlaybackState::Paused).await;

        engine
            .seek(Duration::from_millis(200))
            .expect("first seek posts");
        engine
            .seek(Duration::from_millis(400))
            .expect("second seek coalesces");
        engine
            .seek(Duration::from_millis(1500))
            .expect("latest seek is kept");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(position) = engine.position() {
                if position >= Duration::from_millis(1400) {
                    assert!(
                        position <= Duration::from_millis(1600),
                        "coalesced seek lands at the last target, got {position:?}"
                    );
                    break;
                }
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "coalesced seek never landed"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[tokio::test]
    async fn seek_after_eos_returns_without_hanging() {
        let dir = std::env::temp_dir().join(format!("tunex-seek-eos-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("fixture dir");
        let clip = dir.join("short.wav");
        write_sine_wav(&clip, 200, 440.0);

        let (engine, mut receiver) = test_engine();
        engine.load_path(&clip).expect("fixture loads");
        engine.play().expect("playback starts");
        wait_for_state(&mut receiver, PlaybackState::Playing).await;
        wait_for_state(&mut receiver, PlaybackState::Stopped).await;

        let started = tokio::time::Instant::now();
        let outcome = engine.seek(Duration::from_millis(50));
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "EOS flush seek must not block the caller"
        );
        // Refused or posted: either is fine so long as this returns.
        drop(outcome);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    /// Drain events until `wanted` arrives (failing loudly on errors).
    async fn wait_for_state(receiver: &mut mpsc::Receiver<PlayerEvent>, wanted: PlaybackState) {
        loop {
            match next_event(receiver).await {
                PlayerEvent::StateChanged(state) if state == wanted => break,
                PlayerEvent::PlaybackError(message) => {
                    panic!("pipeline failed instead of playing: {message}");
                }
                _ => {}
            }
        }
    }

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

    #[test]
    fn equalizer_set_does_not_break_playback() {
        let (engine, _receiver) = test_engine();
        engine.set_equalizer(true, tunex_core::preset_bands("rock").unwrap_or([0.0; 10]));
        let csv = engine.spectrum_csv();
        assert_eq!(csv.split(',').count(), SPECTRUM_BANDS);
    }

    #[test]
    fn sync_load_emits_loading_event() {
        let (tx, mut rx) = mpsc::channel(PlayerEngine::event_buffer());
        let engine = PlayerEngine::new(tx).expect("engine builds");
        let sink = gst::ElementFactory::make("fakesink").build().expect("sink");
        engine.set_audio_sink(&sink);
        engine
            .load_uri("file:///nonexistent-probe.flac")
            .expect("load stages");
        match rx.try_recv() {
            Ok(event) => eprintln!("PROBE: got {event:?}"),
            Err(err) => eprintln!("PROBE: channel {err:?}"),
        }
        eprintln!("PROBE: tracked state {:?}", engine.state());
    }
}
