//! MPRIS over live engine state (S5 W-032).
//!
//! The server owns no playback. A shared [`MprisSnapshot`] published by the
//! Qt-side `QueueModel` poll carries engine truth (status, position, volume,
//! metadata, capability flags), and inbound [`MprisCommand`]s drain back
//! through the same poll onto the `PlaybackController` (D-007: the MPRIS
//! thread never touches Qt or the engine). The server watch loop emits
//! `PropertiesChanged` when non-position state moves and `Seeked` after
//! explicit seeks. Art URLs stay `None`: queue rows are still monogram
//! placeholders until the lazy art pipeline lands (W-021 deferral stands).

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use mpris_server::{
    LoopStatus, Metadata, PlaybackRate, PlaybackStatus as MprisStatus, PlayerInterface, Property,
    RootInterface, Server, Signal, Time, TrackId, Volume,
    zbus::{Result as ZbusResult, fdo},
};
use tokio::sync::mpsc;
use tunex_core::{PlaybackState, RepeatMode};

/// Object-path prefix for published track ids.
pub const TRACK_PATH_PREFIX: &str = "/org/mpris/MediaPlayer2/track/";

/// Depth of the inbound MPRIS→app command channel. The Qt poll drains it
/// every 300 ms, so 32 slots never legitimately fill; overflow drops the
/// oldest client poke with a warning rather than wedging D-Bus.
const COMMAND_CHANNEL_DEPTH: usize = 32;

/// Cadence of the server watch loop (change detection only; clients poll
/// `Position` themselves per the MPRIS spec).
const WATCH_INTERVAL: Duration = Duration::from_millis(250);

/// The currently exposed track, if any.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MprisTrack {
    /// Full object path (`TRACK_PATH_PREFIX` + stable suffix).
    pub id_path: String,
    /// Playback URI (`file://…`).
    pub uri: String,
    /// Display title.
    pub title: String,
    /// Display artist, when tagged.
    pub artist: Option<String>,
    /// Display album, when tagged.
    pub album: Option<String>,
    /// Known length in microseconds.
    pub length_us: Option<i64>,
}

/// Capability flags shared with the D-Bus thread (one bool per MPRIS
/// `Can*` property — grouped so the snapshot stays a plain data shape).
#[allow(
    clippy::struct_excessive_bools,
    reason = "mirrors the five MPRIS Can* properties 1:1"
)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MprisCapabilities {
    /// Whether stepping forward can do anything.
    pub next: bool,
    /// Whether stepping back can do anything.
    pub previous: bool,
    /// Whether an engine exists to start.
    pub play: bool,
    /// Whether pause is meaningful right now.
    pub pause: bool,
    /// Whether seeking is meaningful right now.
    pub seek: bool,
}

/// Engine truth shared with the D-Bus thread.
#[derive(Clone, Debug, PartialEq)]
pub struct MprisSnapshot {
    /// Controller state.
    pub status: PlaybackState,
    /// Engine position in microseconds.
    pub position_us: i64,
    /// Output volume `0.0`–`1.0` (never zeroed by mute: MPRIS has no mute,
    /// so the property stays honest).
    pub volume: f64,
    /// Queue shuffle flag.
    pub shuffle: bool,
    /// Queue repeat mode.
    pub repeat: RepeatMode,
    /// Current track, when one is loaded.
    pub track: Option<MprisTrack>,
    /// Capability flags.
    pub capabilities: MprisCapabilities,
}

impl Default for MprisSnapshot {
    fn default() -> Self {
        Self {
            status: PlaybackState::Stopped,
            position_us: 0,
            volume: 1.0,
            shuffle: false,
            repeat: RepeatMode::Off,
            track: None,
            capabilities: MprisCapabilities::default(),
        }
    }
}

impl MprisSnapshot {
    /// Everything clients must re-read on change, minus the free-running
    /// position (clients poll that themselves).
    fn state_signature(&self) -> MprisSnapshotSignature {
        let mut signature = self.clone();
        signature.position_us = 0;
        MprisSnapshotSignature(signature)
    }
}

/// Newtype so the watch loop can compare published state cheaply.
#[derive(Clone, Debug, PartialEq)]
struct MprisSnapshotSignature(MprisSnapshot);

/// Commands from D-Bus clients, executed on the Qt thread by the
/// `QueueModel` poll. Method handlers return `Ok` at enqueue time: execution
/// itself is asynchronous by design (D-007).
#[derive(Clone, Debug, PartialEq)]
pub enum MprisCommand {
    /// Start playback (no-op with an empty queue — the snapshot stays
    /// `Stopped` rather than lying).
    Play,
    /// Hold position.
    Pause,
    /// Toggle, mirroring the panel transport.
    PlayPause,
    /// Stop the pipeline.
    Stop,
    /// Step forward.
    Next,
    /// Step back (or restart past the threshold — same rule as the panel).
    Previous,
    /// Relative seek in microseconds (clamped at zero).
    Seek {
        /// Signed offset in microseconds.
        offset_us: i64,
    },
    /// Absolute seek in microseconds; ignored unless `track` matches the
    /// currently published id (MPRIS §2.2).
    SetPosition {
        /// Object path the client saw.
        track: String,
        /// Target position in microseconds.
        position_us: i64,
    },
    /// Output volume `0.0`–`1.0` (clamped).
    SetVolume(f64),
    /// Queue shuffle flag.
    SetShuffle(bool),
    /// Queue repeat mode.
    SetLoop(LoopStatus),
}

/// Shareable server side: snapshot plus the inbound command sender.
#[derive(Clone, Debug)]
pub struct MprisHub {
    inner: Arc<Mutex<HubInner>>,
    tx: mpsc::Sender<MprisCommand>,
}

#[derive(Debug)]
struct HubInner {
    snapshot: MprisSnapshot,
    last_emitted: MprisSnapshotSignature,
    emit_pending: bool,
    seeked_pending: Option<i64>,
}

impl MprisHub {
    /// Hub with an explicit inbox (tests and the global install share this).
    #[must_use]
    pub fn with_channel(depth: usize) -> (Self, mpsc::Receiver<MprisCommand>) {
        let snapshot = MprisSnapshot::default();
        let (tx, rx) = mpsc::channel(depth);
        let hub = Self {
            inner: Arc::new(Mutex::new(HubInner {
                last_emitted: snapshot.state_signature(),
                snapshot,
                emit_pending: false,
                seeked_pending: None,
            })),
            tx,
        };
        (hub, rx)
    }

    /// Current snapshot (poison-forgiving: a panicked publisher must not
    /// wedge every D-Bus property read).
    #[must_use]
    pub fn snapshot(&self) -> MprisSnapshot {
        self.inner
            .lock()
            .map(|inner| inner.snapshot.clone())
            .unwrap_or_default()
    }

    /// Publish fresh engine truth; marks `PropertiesChanged` pending when
    /// non-position state moved.
    pub fn publish(&self, snapshot: MprisSnapshot) {
        if let Ok(mut inner) = self.inner.lock() {
            let signature = snapshot.state_signature();
            if signature != inner.last_emitted {
                inner.last_emitted = signature;
                inner.emit_pending = true;
            }
            inner.snapshot = snapshot;
        }
    }

    /// Enqueue a client command (dropped with a warning when the Qt side
    /// stopped draining — never blocks D-Bus).
    fn send(&self, command: MprisCommand) {
        if let Err(err) = self.tx.try_send(command) {
            tracing::warn!(name: "app.mpris.command_dropped", error = %err, "Qt side not draining");
        }
    }

    /// Tweak the published snapshot optimistically so property reads answer
    /// immediately; the next Qt poll publishes ground truth.
    fn update(&self, update: impl FnOnce(&mut MprisSnapshot)) {
        if let Ok(mut inner) = self.inner.lock() {
            update(&mut inner.snapshot);
            let signature = inner.snapshot.state_signature();
            if signature != inner.last_emitted {
                inner.last_emitted = signature;
                inner.emit_pending = true;
            }
        }
    }

    /// Take a pending `Seeked` position for emission, if any.
    fn take_seeked(&self) -> Option<i64> {
        self.inner
            .lock()
            .ok()
            .and_then(|mut inner| inner.seeked_pending.take())
    }

    /// Take a pending `PropertiesChanged` flag.
    fn take_emit_pending(&self) -> bool {
        self.inner.lock().is_ok_and(|mut inner| {
            let pending = inner.emit_pending;
            inner.emit_pending = false;
            pending
        })
    }

    /// Record an explicit seek for `Seeked` emission.
    fn note_seeked(&self, position_us: i64) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.seeked_pending = Some(position_us);
        }
    }
}

/// Process-wide install: the Qt-created `QueueModel` cannot receive the hub
/// through QML, so `spawn` publishes it here and the model picks it up on
/// its first poll. Tests bypass this with explicit [`MprisHub`] pairs.
static INSTALLED: OnceLock<InstalledHub> = OnceLock::new();

struct InstalledHub {
    hub: MprisHub,
    inbox: Mutex<Option<mpsc::Receiver<MprisCommand>>>,
}

/// Publish engine truth to the installed hub (no-op before `spawn`).
pub fn publish(snapshot: MprisSnapshot) {
    if let Some(installed) = INSTALLED.get() {
        installed.hub.publish(snapshot);
    }
}

/// Take the inbound command inbox (first `QueueModel` wins; later models
/// share one queue by design — App.qml owns exactly one).
#[must_use]
pub fn take_inbox() -> Option<mpsc::Receiver<MprisCommand>> {
    INSTALLED
        .get()
        .and_then(|installed| installed.inbox.lock().ok()?.take())
}

/// Map application state onto the MPRIS vocabulary. `Loading` has no MPRIS
/// counterpart; it reports `Playing` (the pipeline is already committed).
#[must_use]
pub fn map_status(status: PlaybackState) -> MprisStatus {
    match status {
        PlaybackState::Stopped => MprisStatus::Stopped,
        PlaybackState::Loading | PlaybackState::Playing => MprisStatus::Playing,
        PlaybackState::Paused => MprisStatus::Paused,
    }
}

/// Map queue repeat onto the MPRIS loop vocabulary.
#[must_use]
pub fn map_loop(repeat: RepeatMode) -> LoopStatus {
    match repeat {
        RepeatMode::Off => LoopStatus::None,
        RepeatMode::All => LoopStatus::Playlist,
        RepeatMode::One => LoopStatus::Track,
    }
}

/// Map the MPRIS loop vocabulary back onto queue repeat.
#[must_use]
pub fn unmap_loop(loop_status: LoopStatus) -> RepeatMode {
    match loop_status {
        LoopStatus::None => RepeatMode::Off,
        LoopStatus::Playlist => RepeatMode::All,
        LoopStatus::Track => RepeatMode::One,
    }
}

/// Build client metadata from the published track. Empty (no `mpris:trackid`)
/// when nothing is loaded — the spec's explicit "no track" shape.
#[must_use]
pub fn metadata_for(track: Option<&MprisTrack>) -> Metadata {
    let Some(track) = track else {
        return Metadata::new();
    };
    let track_id = TrackId::try_from(track.id_path.as_str()).unwrap_or(TrackId::NO_TRACK);
    let mut builder = Metadata::builder()
        .trackid(track_id)
        .title(track.title.clone())
        .url(track.uri.clone());
    if let Some(artist) = &track.artist {
        builder = builder.artist([artist.clone()]);
    }
    if let Some(album) = &track.album {
        builder = builder.album(album.clone());
    }
    if let Some(length_us) = track.length_us {
        builder = builder.length(Time::from_micros(length_us));
    }
    builder.build()
}

/// MPRIS player implementation: publishes shared engine truth and enqueues
/// client commands for the Qt thread.
#[derive(Clone, Debug)]
pub struct MprisPlayer {
    hub: MprisHub,
}

impl MprisPlayer {
    /// Player over an explicit hub (tests and `spawn` share this).
    #[must_use]
    pub fn with_hub(hub: MprisHub) -> Self {
        Self { hub }
    }

    /// Player with stopped transport (skeleton path; engine truth arrives
    /// via [`MprisHub::publish`] once the app runs).
    #[must_use]
    pub fn new() -> Self {
        let (hub, _rx) = MprisHub::with_channel(COMMAND_CHANNEL_DEPTH);
        Self { hub }
    }

    /// Current transport status (for tests and later UI sync).
    #[must_use]
    pub fn status(&self) -> PlaybackState {
        self.hub.snapshot().status
    }

    /// Drainable commands (tests assert the Qt side would see these).
    #[must_use]
    pub fn hub(&self) -> &MprisHub {
        &self.hub
    }
}

impl Default for MprisPlayer {
    fn default() -> Self {
        Self::new()
    }
}

// Trait signatures mandate `async`; one-liner impls have nothing to await.
#[allow(
    clippy::unused_async_trait_impl,
    reason = "mpris-server requires async trait methods"
)]
impl RootInterface for MprisPlayer {
    async fn identity(&self) -> fdo::Result<String> {
        Ok("TuneX".to_owned())
    }

    async fn raise(&self) -> fdo::Result<()> {
        Ok(())
    }

    async fn quit(&self) -> fdo::Result<()> {
        Ok(())
    }

    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _fullscreen: bool) -> ZbusResult<()> {
        Ok(())
    }

    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok("tunex".to_owned())
    }

    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["file".to_owned()])
    }

    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![
            "audio/mpeg".to_owned(),
            "audio/flac".to_owned(),
            "audio/ogg".to_owned(),
            "audio/mp4".to_owned(),
            "audio/opus".to_owned(),
            "audio/x-wav".to_owned(),
        ])
    }
}

// Trait signatures mandate `async`; one-liner impls have nothing to await.
#[allow(
    clippy::unused_async_trait_impl,
    reason = "mpris-server requires async trait methods"
)]
impl PlayerInterface for MprisPlayer {
    async fn next(&self) -> fdo::Result<()> {
        self.hub.send(MprisCommand::Next);
        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        self.hub.send(MprisCommand::Previous);
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        self.hub.send(MprisCommand::Pause);
        self.hub.update(|snapshot| {
            if snapshot.status == PlaybackState::Playing {
                snapshot.status = PlaybackState::Paused;
            }
        });
        Ok(())
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        self.hub.send(MprisCommand::PlayPause);
        self.hub.update(|snapshot| {
            snapshot.status = match snapshot.status {
                PlaybackState::Playing => PlaybackState::Paused,
                _ => PlaybackState::Playing,
            };
        });
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        self.hub.send(MprisCommand::Stop);
        self.hub
            .update(|snapshot| snapshot.status = PlaybackState::Stopped);
        Ok(())
    }

    async fn play(&self) -> fdo::Result<()> {
        self.hub.send(MprisCommand::Play);
        self.hub.update(|snapshot| {
            if snapshot.status == PlaybackState::Stopped {
                snapshot.status = PlaybackState::Playing;
            }
        });
        Ok(())
    }

    async fn seek(&self, offset: Time) -> fdo::Result<()> {
        let offset_us = offset.as_micros();
        let target = (self.hub.snapshot().position_us + offset_us).max(0);
        self.hub.send(MprisCommand::Seek { offset_us });
        self.hub.update(|snapshot| snapshot.position_us = target);
        self.hub.note_seeked(target);
        Ok(())
    }

    async fn set_position(&self, track_id: TrackId, position: Time) -> fdo::Result<()> {
        let current = self.hub.snapshot();
        let matches = current
            .track
            .as_ref()
            .is_some_and(|track| track.id_path == track_id.as_str());
        if !matches {
            tracing::debug!(name: "app.mpris.stale_seek", "SetPosition for unknown track ignored");
            return Ok(());
        }
        let position_us = position.as_micros().max(0);
        self.hub.send(MprisCommand::SetPosition {
            track: track_id.as_str().to_owned(),
            position_us,
        });
        self.hub
            .update(|snapshot| snapshot.position_us = position_us);
        self.hub.note_seeked(position_us);
        Ok(())
    }

    async fn open_uri(&self, uri: String) -> fdo::Result<()> {
        tracing::info!(name: "app.mpris.open_uri", uri = %uri, "queueing URIs over MPRIS lands after playlists (S5 later)");
        Ok(())
    }

    async fn playback_status(&self) -> fdo::Result<MprisStatus> {
        Ok(map_status(self.status()))
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Ok(map_loop(self.hub.snapshot().repeat))
    }

    async fn set_loop_status(&self, loop_status: LoopStatus) -> ZbusResult<()> {
        self.hub.send(MprisCommand::SetLoop(loop_status));
        self.hub.update(|snapshot| {
            snapshot.repeat = unmap_loop(loop_status);
        });
        Ok(())
    }

    async fn rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn set_rate(&self, _rate: PlaybackRate) -> ZbusResult<()> {
        Ok(())
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(self.hub.snapshot().shuffle)
    }

    async fn set_shuffle(&self, shuffle: bool) -> ZbusResult<()> {
        self.hub.send(MprisCommand::SetShuffle(shuffle));
        self.hub.update(|snapshot| snapshot.shuffle = shuffle);
        Ok(())
    }

    async fn metadata(&self) -> fdo::Result<Metadata> {
        Ok(metadata_for(self.hub.snapshot().track.as_ref()))
    }

    async fn volume(&self) -> fdo::Result<Volume> {
        Ok(self.hub.snapshot().volume)
    }

    async fn set_volume(&self, volume: Volume) -> ZbusResult<()> {
        let volume = volume.clamp(0.0, 1.0);
        self.hub.send(MprisCommand::SetVolume(volume));
        self.hub.update(|snapshot| snapshot.volume = volume);
        Ok(())
    }

    async fn position(&self) -> fdo::Result<Time> {
        Ok(Time::from_micros(self.hub.snapshot().position_us))
    }

    async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(self.hub.snapshot().capabilities.next)
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(self.hub.snapshot().capabilities.previous)
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(self.hub.snapshot().capabilities.play)
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(self.hub.snapshot().capabilities.pause)
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(self.hub.snapshot().capabilities.seek)
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}

/// Snapshot the properties clients must re-read on change.
fn changed_properties(snapshot: &MprisSnapshot) -> Vec<Property> {
    vec![
        Property::PlaybackStatus(map_status(snapshot.status)),
        Property::LoopStatus(map_loop(snapshot.repeat)),
        Property::Shuffle(snapshot.shuffle),
        Property::Metadata(metadata_for(snapshot.track.as_ref())),
        Property::Volume(snapshot.volume),
        Property::CanGoNext(snapshot.capabilities.next),
        Property::CanGoPrevious(snapshot.capabilities.previous),
        Property::CanPlay(snapshot.capabilities.play),
        Property::CanPause(snapshot.capabilities.pause),
        Property::CanSeek(snapshot.capabilities.seek),
    ]
}
/// Serve MPRIS on a background thread for the process lifetime.
///
/// Installs the process hub (the `QueueModel` poll publishes engine truth
/// into it and drains inbound commands) and emits `PropertiesChanged` /
/// `Seeked` as shared state moves. The thread (and its single-threaded
/// async runtime) is intentionally detached: the OS reclaims it on exit.
/// Qt keeps owning the main thread throughout.
pub fn spawn() {
    let (hub, inbox) = MprisHub::with_channel(COMMAND_CHANNEL_DEPTH);
    let _ = INSTALLED.set(InstalledHub {
        hub: hub.clone(),
        inbox: Mutex::new(Some(inbox)),
    });
    if std::thread::Builder::new()
        .name("tunex-mpris".to_owned())
        .spawn(move || run_server_thread(hub))
        .is_err()
    {
        tracing::error!(name: "app.mpris.spawn", "MPRIS thread failed to spawn");
    }
}

/// Body of the detached MPRIS thread: single-threaded runtime plus serve.
fn run_server_thread(hub: MprisHub) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let runtime = match runtime {
        Ok(runtime) => runtime,
        Err(err) => {
            tracing::error!(
                name: "app.mpris.runtime",
                error = %err,
                "MPRIS runtime failed to start"
            );
            return;
        }
    };
    runtime.block_on(async move {
        match Server::new("tunex", MprisPlayer::with_hub(hub.clone())).await {
            Ok(server) => {
                tracing::info!(
                    name: "app.mpris.serving",
                    "MPRIS serving org.mpris.MediaPlayer2.tunex"
                );
                serve_forever(&server, &hub).await;
            }
            Err(err) => {
                tracing::error!(
                    name: "app.mpris.serve",
                    error = %err,
                    "MPRIS server failed to start"
                );
            }
        }
    });
}

/// Watch shared state forever, emitting `Seeked` and `PropertiesChanged`
/// as it moves.
async fn serve_forever(server: &Server<MprisPlayer>, hub: &MprisHub) {
    loop {
        tokio::time::sleep(WATCH_INTERVAL).await;
        if let Some(position) = hub.take_seeked() {
            emit_seeked(server, position).await;
        }
        if hub.take_emit_pending() {
            emit_properties(server, hub).await;
        }
    }
}

/// Emit one `Seeked` signal (failures are debug noise: clients re-poll).
async fn emit_seeked(server: &Server<MprisPlayer>, position: i64) {
    if let Err(err) = server
        .emit(Signal::Seeked {
            position: Time::from_micros(position),
        })
        .await
    {
        tracing::debug!(
            name: "app.mpris.seeked_emit",
            error = %err,
            "Seeked emission failed"
        );
    }
}

/// Emit the current `PropertiesChanged` set (failures are debug noise).
async fn emit_properties(server: &Server<MprisPlayer>, hub: &MprisHub) {
    if let Err(err) = server
        .properties_changed(changed_properties(&hub.snapshot()))
        .await
    {
        tracing::debug!(
            name: "app.mpris.props_emit",
            error = %err,
            "PropertiesChanged emission failed"
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// Hub pair that never touches the process-global install.
    fn test_hub() -> (MprisHub, mpsc::Receiver<MprisCommand>) {
        MprisHub::with_channel(8)
    }

    fn sample_track() -> MprisTrack {
        MprisTrack {
            id_path: format!("{TRACK_PATH_PREFIX}7"),
            uri: "file:///music/still-here.flac".to_owned(),
            title: "Still Here".to_owned(),
            artist: Some("Quests".to_owned()),
            album: Some("Night Tapes".to_owned()),
            length_us: Some(5_000_000),
        }
    }

    #[test]
    fn status_mapping_covers_every_variant() {
        assert_eq!(map_status(PlaybackState::Stopped), MprisStatus::Stopped);
        assert_eq!(map_status(PlaybackState::Playing), MprisStatus::Playing);
        assert_eq!(map_status(PlaybackState::Paused), MprisStatus::Paused);
        // Loading has no MPRIS counterpart: the pipeline is committed.
        assert_eq!(map_status(PlaybackState::Loading), MprisStatus::Playing);
    }

    #[test]
    fn loop_mapping_round_trips() {
        assert_eq!(map_loop(RepeatMode::Off), LoopStatus::None);
        assert_eq!(map_loop(RepeatMode::All), LoopStatus::Playlist);
        assert_eq!(map_loop(RepeatMode::One), LoopStatus::Track);
        assert_eq!(unmap_loop(LoopStatus::None), RepeatMode::Off);
        assert_eq!(unmap_loop(LoopStatus::Playlist), RepeatMode::All);
        assert_eq!(unmap_loop(LoopStatus::Track), RepeatMode::One);
    }

    #[test]
    fn metadata_without_track_is_empty() {
        let metadata = metadata_for(None);
        assert!(metadata.get_value("mpris:trackid").is_none());
    }

    #[test]
    fn metadata_carries_track_fields() {
        let track = sample_track();
        let metadata = metadata_for(Some(&track));
        assert_eq!(
            metadata.trackid(),
            TrackId::try_from(format!("{TRACK_PATH_PREFIX}7").as_str()).ok()
        );
        assert_eq!(metadata.title(), Some("Still Here"));
        assert_eq!(
            metadata.url().as_deref(),
            Some("file:///music/still-here.flac")
        );
        assert_eq!(metadata.length(), Some(Time::from_micros(5_000_000)));
        assert_eq!(metadata.album(), Some("Night Tapes"));
    }

    #[test]
    fn publish_marks_emit_only_on_state_change() {
        let (hub, _rx) = test_hub();
        assert!(!hub.take_emit_pending());
        // Position-only movement never pages clients.
        hub.publish(MprisSnapshot {
            position_us: 1_500_000,
            ..MprisSnapshot::default()
        });
        assert!(!hub.take_emit_pending());
        assert_eq!(hub.snapshot().position_us, 1_500_000);
        // Status movement does.
        hub.publish(MprisSnapshot {
            status: PlaybackState::Playing,
            capabilities: MprisCapabilities {
                play: true,
                pause: true,
                ..MprisCapabilities::default()
            },
            ..MprisSnapshot::default()
        });
        assert!(hub.take_emit_pending());
        assert!(!hub.take_emit_pending());
    }

    #[tokio::test]
    async fn play_pause_toggles_stopped_playing_paused() {
        let (hub, _rx) = test_hub();
        let player = MprisPlayer::with_hub(hub);
        assert_eq!(player.status(), PlaybackState::Stopped);
        player.play_pause().await.expect("toggle works");
        assert_eq!(player.status(), PlaybackState::Playing);
        assert_eq!(
            player.playback_status().await.expect("status reads"),
            MprisStatus::Playing
        );
        player.play_pause().await.expect("toggle works");
        assert_eq!(player.status(), PlaybackState::Paused);
    }

    #[tokio::test]
    async fn transport_methods_enqueue_commands() {
        let (hub, mut rx) = test_hub();
        let player = MprisPlayer::with_hub(hub);
        player.play().await.expect("play works");
        player.pause().await.expect("pause works");
        player.stop().await.expect("stop works");
        player.next().await.expect("next works");
        player.previous().await.expect("previous works");
        let mut commands = Vec::new();
        while let Ok(command) = rx.try_recv() {
            commands.push(command);
        }
        assert_eq!(
            commands,
            vec![
                MprisCommand::Play,
                MprisCommand::Pause,
                MprisCommand::Stop,
                MprisCommand::Next,
                MprisCommand::Previous,
            ]
        );
        assert_eq!(player.status(), PlaybackState::Stopped);
    }

    #[tokio::test]
    async fn transport_surface_reports_skeleton_defaults() {
        let (hub, _rx) = test_hub();
        let player = MprisPlayer::with_hub(hub);
        player
            .open_uri("file:///music/a.flac".to_owned())
            .await
            .expect("open accepts");
        assert_eq!(
            player.loop_status().await.expect("loop reads"),
            LoopStatus::None
        );
        player
            .set_loop_status(LoopStatus::Playlist)
            .await
            .expect("loop sets");
        assert_eq!(
            player.loop_status().await.expect("loop reads"),
            LoopStatus::Playlist
        );
        assert_eq!(
            player.rate().await.expect("rate reads").to_bits(),
            1.0f64.to_bits()
        );
        player.set_rate(1.0).await.expect("rate sets");
        assert!(!player.shuffle().await.expect("shuffle reads"));
        player.set_shuffle(true).await.expect("shuffle sets");
        assert!(player.shuffle().await.expect("shuffle reads"));
        assert!(
            metadata_for(player.hub.snapshot().track.as_ref())
                .get_value("mpris:trackid")
                .is_none()
        );
        assert_eq!(player.position().await.expect("position reads"), Time::ZERO);
        assert_eq!(
            player.minimum_rate().await.expect("rate reads").to_bits(),
            1.0f64.to_bits()
        );
        assert_eq!(
            player.maximum_rate().await.expect("rate reads").to_bits(),
            1.0f64.to_bits()
        );
        assert!(player.can_control().await.expect("flag reads"));
        // Nothing published yet: capabilities stay denied, never fabricated.
        assert!(!player.can_play().await.expect("flag reads"));
        assert!(!player.can_go_next().await.expect("flag reads"));
        assert!(!player.can_seek().await.expect("flag reads"));
    }

    #[tokio::test]
    async fn volume_clamps() {
        let (hub, mut rx) = test_hub();
        let player = MprisPlayer::with_hub(hub);
        player.set_volume(7.5).await.expect("volume sets");
        assert_eq!(
            player.volume().await.expect("volume reads").to_bits(),
            1.0f64.to_bits()
        );
        assert_eq!(
            rx.try_recv().expect("command queued"),
            MprisCommand::SetVolume(1.0)
        );
    }

    #[tokio::test]
    async fn seek_clamps_at_zero_and_notes_seeked() {
        let (hub, mut rx) = test_hub();
        let player = MprisPlayer::with_hub(hub.clone());
        player
            .seek(Time::from_millis(-5_000))
            .await
            .expect("seek works");
        assert_eq!(player.position().await.expect("reads"), Time::ZERO);
        assert_eq!(
            rx.try_recv().expect("command queued"),
            MprisCommand::Seek {
                offset_us: -5_000_000
            }
        );
        assert_eq!(hub.take_seeked(), Some(0));
    }

    #[tokio::test]
    async fn set_position_ignores_unknown_tracks() {
        let (hub, mut rx) = test_hub();
        let player = MprisPlayer::with_hub(hub.clone());
        // No track published: every SetPosition is stale.
        player
            .set_position(TrackId::NO_TRACK, Time::from_millis(1_000))
            .await
            .expect("stale seek is Ok");
        assert!(rx.try_recv().is_err());
        assert!(hub.take_seeked().is_none());
        // A published track accepts its own id only.
        hub.publish(MprisSnapshot {
            track: Some(sample_track()),
            ..MprisSnapshot::default()
        });
        let track_id = TrackId::try_from(format!("{TRACK_PATH_PREFIX}7").as_str())
            .expect("sample path is valid");
        player
            .set_position(track_id, Time::from_millis(1_000))
            .await
            .expect("matching seek works");
        assert_eq!(
            rx.try_recv().expect("command queued"),
            MprisCommand::SetPosition {
                track: format!("{TRACK_PATH_PREFIX}7"),
                position_us: 1_000_000,
            }
        );
        assert_eq!(hub.take_seeked(), Some(1_000_000));
    }

    #[tokio::test]
    async fn identity_is_tunex() {
        let (hub, _rx) = test_hub();
        let player = MprisPlayer::with_hub(hub);
        assert_eq!(player.identity().await.expect("identity reads"), "TuneX");
        assert_eq!(player.desktop_entry().await.expect("entry reads"), "tunex");
        assert!(!player.can_quit().await.expect("flag reads"));
        assert!(!player.can_raise().await.expect("flag reads"));
        assert!(!player.has_track_list().await.expect("flag reads"));
        assert_eq!(
            player.supported_uri_schemes().await.expect("schemes read"),
            vec!["file".to_owned()]
        );
    }
}
