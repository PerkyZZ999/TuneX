//! MPRIS skeleton (W-008): D-Bus presence plus a transport surface.
//!
//! Exposes `org.mpris.MediaPlayer2.tunex` with identity, playback status, and
//! transport methods over a locally owned state machine. Engine/queue wiring
//! and live metadata land in S5; this slice proves bus presence and control
//! round-trips (`busctl`/`qdbus6`).

use std::sync::{Arc, Mutex};

use mpris_server::{
    LoopStatus, Metadata, PlaybackRate, PlaybackStatus as MprisStatus, PlayerInterface,
    RootInterface, Server, Time, TrackId, Volume,
    zbus::{Result as ZbusResult, fdo},
};
use tunex_core::PlaybackState;

/// Local transport state until the engine owns playback (S5).
#[derive(Debug, Default)]
struct Transport {
    status: PlaybackState,
    volume: f64,
    position_micros: i64,
}

/// MPRIS player implementation: publishes identity and transport over D-Bus.
#[derive(Debug, Default)]
pub struct MprisPlayer {
    transport: Arc<Mutex<Transport>>,
}

impl MprisPlayer {
    /// Player with stopped transport.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Current transport status (for tests and later UI sync).
    #[must_use]
    pub fn status(&self) -> PlaybackState {
        match self.transport.lock() {
            Ok(transport) => transport.status,
            Err(_) => PlaybackState::default(),
        }
    }

    fn set_status(&self, status: PlaybackState) {
        if let Ok(mut transport) = self.transport.lock() {
            transport.status = status;
        }
        tracing::debug!(
            name: "app.mpris.status",
            status = ?status,
            "transport state changed"
        );
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
        Ok(vec!["audio/mpeg".to_owned(), "audio/flac".to_owned()])
    }
}

// Trait signatures mandate `async`; one-liner impls have nothing to await.
#[allow(
    clippy::unused_async_trait_impl,
    reason = "mpris-server requires async trait methods"
)]
impl PlayerInterface for MprisPlayer {
    async fn next(&self) -> fdo::Result<()> {
        tracing::info!(name: "app.mpris.next", "no queue wired yet (S5)");
        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        tracing::info!(name: "app.mpris.previous", "no queue wired yet (S5)");
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        tracing::info!(name: "app.mpris.pause", "transport pause");
        self.set_status(PlaybackState::Paused);
        Ok(())
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        let next = match self.status() {
            PlaybackState::Playing => PlaybackState::Paused,
            _ => PlaybackState::Playing,
        };
        tracing::info!(name: "app.mpris.play_pause", "transport toggled");
        self.set_status(next);
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        tracing::info!(name: "app.mpris.stop", "transport stop");
        self.set_status(PlaybackState::Stopped);
        Ok(())
    }

    async fn play(&self) -> fdo::Result<()> {
        tracing::info!(name: "app.mpris.play", "transport play");
        self.set_status(PlaybackState::Playing);
        Ok(())
    }

    async fn seek(&self, _offset: Time) -> fdo::Result<()> {
        Ok(())
    }

    async fn set_position(&self, _track_id: TrackId, _position: Time) -> fdo::Result<()> {
        Ok(())
    }

    async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
        Ok(())
    }

    async fn playback_status(&self) -> fdo::Result<MprisStatus> {
        Ok(match self.status() {
            PlaybackState::Stopped => MprisStatus::Stopped,
            PlaybackState::Loading | PlaybackState::Playing => MprisStatus::Playing,
            PlaybackState::Paused => MprisStatus::Paused,
        })
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Ok(LoopStatus::None)
    }

    async fn set_loop_status(&self, _loop_status: LoopStatus) -> ZbusResult<()> {
        Ok(())
    }

    async fn rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn set_rate(&self, _rate: PlaybackRate) -> ZbusResult<()> {
        Ok(())
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_shuffle(&self, _shuffle: bool) -> ZbusResult<()> {
        Ok(())
    }

    async fn metadata(&self) -> fdo::Result<Metadata> {
        Ok(Metadata::new())
    }

    async fn volume(&self) -> fdo::Result<Volume> {
        match self.transport.lock() {
            Ok(transport) => Ok(transport.volume),
            Err(_) => Ok(1.0),
        }
    }

    async fn set_volume(&self, volume: Volume) -> ZbusResult<()> {
        if let Ok(mut transport) = self.transport.lock() {
            transport.volume = volume.clamp(0.0, 1.0);
        }
        Ok(())
    }

    async fn position(&self) -> fdo::Result<Time> {
        match self.transport.lock() {
            Ok(transport) => Ok(Time::from_micros(transport.position_micros)),
            Err(_) => Ok(Time::ZERO),
        }
    }

    async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}

/// Serve MPRIS on a background thread for the process lifetime.
///
/// The thread (and its single-threaded async runtime) is intentionally
/// detached: the OS reclaims it on exit. Graceful shutdown and engine/queue
/// wiring land in S5; Qt keeps owning the main thread throughout.
pub fn spawn() {
    if std::thread::Builder::new()
        .name("tunex-mpris".to_owned())
        .spawn(|| {
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
            runtime.block_on(async {
                match Server::new("tunex", MprisPlayer::new()).await {
                    Ok(_server) => {
                        tracing::info!(
                            name: "app.mpris.serving",
                            "MPRIS serving org.mpris.MediaPlayer2.tunex"
                        );
                        std::future::pending::<()>().await;
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
        })
        .is_err()
    {
        tracing::error!(name: "app.mpris.spawn", "MPRIS thread failed to spawn");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn play_pause_toggles_stopped_playing_paused() {
        let player = MprisPlayer::new();
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
    async fn transport_methods_set_state_directly() {
        let player = MprisPlayer::new();
        player.play().await.expect("play works");
        assert_eq!(player.status(), PlaybackState::Playing);
        player.pause().await.expect("pause works");
        assert_eq!(player.status(), PlaybackState::Paused);
        player.stop().await.expect("stop works");
        assert_eq!(player.status(), PlaybackState::Stopped);
    }

    #[tokio::test]
    async fn identity_is_tunex() {
        let player = MprisPlayer::new();
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

    #[tokio::test]
    async fn transport_surface_reports_skeleton_defaults() {
        let player = MprisPlayer::new();
        player.next().await.expect("next accepts");
        player.previous().await.expect("previous accepts");
        player
            .seek(Time::from_millis(1000))
            .await
            .expect("seek accepts");
        player
            .set_position(TrackId::NO_TRACK, Time::ZERO)
            .await
            .expect("position accepts");
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
            player.rate().await.expect("rate reads").to_bits(),
            1.0f64.to_bits()
        );
        player.set_rate(1.0).await.expect("rate sets");
        assert!(!player.shuffle().await.expect("shuffle reads"));
        player.set_shuffle(true).await.expect("shuffle sets");
        assert!(player.metadata().await.is_ok());
        assert_eq!(player.position().await.expect("position reads"), Time::ZERO);
        assert_eq!(
            player.minimum_rate().await.expect("rate reads").to_bits(),
            1.0f64.to_bits()
        );
        assert_eq!(
            player.maximum_rate().await.expect("rate reads").to_bits(),
            1.0f64.to_bits()
        );
        assert!(player.can_play().await.expect("flag reads"));
        assert!(player.can_pause().await.expect("flag reads"));
        assert!(player.can_control().await.expect("flag reads"));
        assert!(!player.can_go_next().await.expect("flag reads"));
        assert!(!player.can_seek().await.expect("flag reads"));
    }

    #[tokio::test]
    async fn volume_clamps() {
        let player = MprisPlayer::new();
        player.set_volume(7.5).await.expect("volume sets");
        assert_eq!(
            player.volume().await.expect("volume reads").to_bits(),
            1.0f64.to_bits()
        );
    }
}
