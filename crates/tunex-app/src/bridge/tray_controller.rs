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

    fn window_width(&self) -> i32 {
        i32::try_from(
            tunex_core::load_from(&self.config_path)
                .unwrap_or_default()
                .window
                .restored_width(),
        )
        .unwrap_or(i32::MAX)
    }

    fn window_height(&self) -> i32 {
        i32::try_from(
            tunex_core::load_from(&self.config_path)
                .unwrap_or_default()
                .window
                .restored_height(),
        )
        .unwrap_or(i32::MAX)
    }

    fn has_window_position(&self) -> bool {
        let window = tunex_core::load_from(&self.config_path)
            .unwrap_or_default()
            .window;
        window.x.is_some() && window.y.is_some()
    }

    fn window_x(&self) -> i32 {
        tunex_core::load_from(&self.config_path)
            .unwrap_or_default()
            .window
            .x
            .unwrap_or(0)
    }

    fn window_y(&self) -> i32 {
        tunex_core::load_from(&self.config_path)
            .unwrap_or_default()
            .window
            .y
            .unwrap_or(0)
    }

    fn set_window_geometry(&self, x: i32, y: i32, width: i32, height: i32) {
        if let Err(err) = tunex_core::update(&self.config_path, |config| {
            config.window.x = Some(x);
            config.window.y = Some(y);
            config.window.width = u32::try_from(width.max(0)).unwrap_or(0);
            config.window.height = u32::try_from(height.max(0)).unwrap_or(0);
        }) {
            tracing::warn!(
                name = "tray.window_geometry_persist_failed",
                error = %err,
                "window geometry not saved"
            );
        }
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

    /// Restored window width (default or last saved, clamped).
    pub fn window_width(&self) -> i32 {
        self.rust().window_width()
    }

    /// Restored window height (default or last saved, clamped).
    pub fn window_height(&self) -> i32 {
        self.rust().window_height()
    }

    /// Whether a previous session saved an x/y position.
    pub fn has_window_position(&self) -> bool {
        self.rust().has_window_position()
    }

    /// Last saved window x (0 when unset).
    pub fn window_x(&self) -> i32 {
        self.rust().window_x()
    }

    /// Last saved window y (0 when unset).
    pub fn window_y(&self) -> i32 {
        self.rust().window_y()
    }

    /// Persist window geometry (QML debounces).
    pub fn set_window_geometry(self: Pin<&mut Self>, x: i32, y: i32, width: i32, height: i32) {
        self.rust().set_window_geometry(x, y, width, height);
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

    #[test]
    fn window_geometry_persists() {
        let (ctl, dir) = scratch_controller("geom");
        assert_eq!(ctl.window_width(), 1280);
        assert_eq!(ctl.window_height(), 800);
        assert!(!ctl.has_window_position());
        ctl.set_window_geometry(48, 64, 1100, 760);
        assert!(ctl.has_window_position());
        assert_eq!(ctl.window_x(), 48);
        assert_eq!(ctl.window_y(), 64);
        assert_eq!(ctl.window_width(), 1100);
        assert_eq!(ctl.window_height(), 760);
        let loaded = tunex_core::load_from(&ctl.config_path).expect("reload works");
        assert_eq!(loaded.window.width, 1100);
        assert_eq!(loaded.window.x, Some(48));
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }
}
