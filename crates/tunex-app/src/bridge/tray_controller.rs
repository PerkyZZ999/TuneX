//! `TrayController` `QObject`: window-close policy + tray host events.
//!
//! Thin Qt shell over [`crate::tray`]. The SNI thread never touches this
//! type; QML polls [`poll`](qobject::TrayController::poll) on the existing
//! shell timer and handles the signals on the Qt thread (D-007).

use super::models::qobject;

use core::pin::Pin;
use std::path::PathBuf;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::tray::{self, TrayEvent, TraySnapshot};

/// Window-close + tray tooltip, Qt-visible.
#[derive(Debug)]
pub struct TrayControllerRust {
    config_path: PathBuf,
}

impl Default for TrayControllerRust {
    fn default() -> Self {
        Self {
            config_path: tunex_core::config_file(),
        }
    }
}

impl TrayControllerRust {
    fn close_to_tray(&self) -> bool {
        tunex_core::load_from(&self.config_path)
            .unwrap_or_default()
            .window
            .close_to_tray
    }

    fn set_close_to_tray(&self, enabled: bool) {
        if let Err(err) = tunex_core::update(&self.config_path, |config| {
            config.window.close_to_tray = enabled;
        }) {
            tracing::warn!(
                name = "tray.close_to_tray_persist_failed",
                error = %err,
                "close-to-tray not saved"
            );
        }
    }

    fn hide_on_close(&self) -> bool {
        tray::should_hide_on_close(self.close_to_tray(), tray::is_available())
    }
}

impl qobject::TrayController {
    /// Drain host events and emit the matching QML signals.
    pub fn poll(mut self: Pin<&mut Self>) {
        let events = tray::take_events();
        for event in events {
            match event {
                TrayEvent::Popup { x, y } => self.as_mut().popup_requested(x, y),
                TrayEvent::Menu { x, y } => self.as_mut().menu_requested(x, y),
                TrayEvent::PlayPause => self.as_mut().play_pause_requested(),
            }
        }
    }

    /// Push now-playing copy into the SNI tooltip.
    pub fn set_now_playing(self: Pin<&mut Self>, title: &QString, artist: &QString, playing: bool) {
        tray::publish(TraySnapshot {
            title: String::from(title),
            artist: String::from(artist),
            playing,
        });
    }

    /// Whether a tray host accepted the icon.
    pub fn is_available(&self) -> bool {
        tray::is_available()
    }

    /// Persisted close-to-tray preference.
    pub fn close_to_tray(&self) -> bool {
        self.rust().close_to_tray()
    }

    /// Persist close-to-tray and keep the in-memory flag in sync.
    pub fn set_close_to_tray(self: Pin<&mut Self>, enabled: bool) {
        self.rust().set_close_to_tray(enabled);
    }

    /// Whether this close should hide instead of quit.
    pub fn hide_on_close(&self) -> bool {
        self.rust().hide_on_close()
    }
}

#[cfg(test)]
mod tests {
    use super::TrayControllerRust;

    fn scratch_controller(case: &str) -> (TrayControllerRust, std::path::PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("tunex-tray-ctl-{case}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("setup works");
        let config_path = dir.join("config.toml");
        (
            TrayControllerRust {
                config_path: config_path.clone(),
            },
            dir,
        )
    }

    #[test]
    fn close_to_tray_persists() {
        let (ctl, dir) = scratch_controller("persist");
        assert!(ctl.close_to_tray(), "default is on");
        ctl.set_close_to_tray(false);
        assert!(!ctl.close_to_tray());
        let loaded = tunex_core::load_from(&ctl.config_path).expect("reload works");
        assert!(!loaded.window.close_to_tray);
        // No tray host in unit tests, so close still quits.
        assert!(!ctl.hide_on_close());
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }
}
