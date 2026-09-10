//! `LibraryManager` `QObject`: folders + scan progress for QML.
//!
//! Thin Qt shell over [`crate::library::LibraryCore`], which owns every
//! behavior and is fully unit-tested without instantiating C++ objects.
//! Workers never touch this type: QML polls [`poll`](qobject::LibraryManager::poll)
//! on a timer and renders the plain getters.

use super::models::qobject;

use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::library::LibraryCore;

/// Folder config + scan orchestration, Qt-visible.
#[derive(Debug, Default)]
pub struct LibraryManagerRust {
    core: LibraryCore,
}

impl qobject::LibraryManager {
    /// Load folders, start the watcher, and scan.
    pub fn startup(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().core.startup();
    }

    /// Drain progress/watcher channels and orchestrate pending scans.
    pub fn poll(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().core.poll();
    }

    /// Request a scan now (queues when busy).
    pub fn rescan(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().core.rescan();
    }

    /// Whether a scan worker is currently running.
    pub fn is_scanning(&self) -> bool {
        self.rust().core.is_scanning()
    }

    /// One-line status for the progress surface.
    pub fn status_text(&self) -> QString {
        QString::from(self.rust().core.status_text().as_str())
    }

    /// Report a completed run exactly once (views refresh on `true`).
    #[must_use]
    pub fn take_finished(mut self: Pin<&mut Self>) -> bool {
        self.as_mut().rust_mut().core.take_finished()
    }

    /// Configured folder count (drives the folders repeater).
    pub fn folder_count(&self) -> i32 {
        i32::try_from(self.rust().core.folders().len()).unwrap_or(i32::MAX)
    }

    /// Configured folder path, or empty when out of range.
    pub fn folder_at(&self, index: i32) -> QString {
        let folders = self.rust().core.folders();
        usize::try_from(index)
            .ok()
            .and_then(|index| folders.get(index))
            .map_or_else(QString::default, |path| {
                QString::from(path.to_string_lossy().as_ref())
            })
    }

    /// Add a folder (canonicalized, persisted, watched, scanned).
    /// Failures surface through [`error_text`](Self::error_text).
    pub fn add_folder(mut self: Pin<&mut Self>, path: &QString) {
        let path = std::path::PathBuf::from(String::from(path));
        if let Err(err) = self.as_mut().rust_mut().core.add_folder(&path) {
            tracing::warn!(name = "folders.add_failed", error = %err, "folder rejected");
        }
    }

    /// Remove a folder (unwatched, unpersisted, rows collected).
    /// Failures surface through [`error_text`](Self::error_text).
    pub fn remove_folder(mut self: Pin<&mut Self>, path: &QString) {
        let path = std::path::PathBuf::from(String::from(path));
        if let Err(err) = self.as_mut().rust_mut().core.remove_folder(&path) {
            tracing::warn!(name = "folders.remove_failed", error = %err, "folder kept");
        }
    }

    /// Last folder-operation failure, or empty when clear.
    pub fn error_text(&self) -> QString {
        self.rust()
            .core
            .error_text()
            .map_or_else(QString::default, |err| QString::from(err.as_str()))
    }
}
