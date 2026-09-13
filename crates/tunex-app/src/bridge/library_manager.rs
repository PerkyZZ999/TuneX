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

    /// Whether the filesystem watcher is currently active.
    pub fn is_watching(&self) -> bool {
        self.rust().core.is_watching()
    }

    /// Last Your Library tab (`songs` / `albums` / `artists` / `folders`).
    pub fn library_tab(&self) -> QString {
        QString::from(self.rust().core.view_prefs().library_tab.as_str())
    }

    /// Persist the last Your Library tab.
    pub fn set_library_tab(self: Pin<&mut Self>, tab: &QString) {
        let tab = String::from(tab);
        if let Err(err) = self
            .rust()
            .core
            .set_view_prefs(|view| view.library_tab = tab)
        {
            tracing::warn!(name = "library.tab_persist_failed", error = %err, "tab not saved");
        }
    }

    /// Last songs-tab sort key.
    pub fn songs_sort(&self) -> QString {
        QString::from(self.rust().core.view_prefs().songs_sort.as_str())
    }

    /// Persist the songs-tab sort key.
    pub fn set_songs_sort(self: Pin<&mut Self>, key: &QString) {
        let key = String::from(key);
        if let Err(err) = self
            .rust()
            .core
            .set_view_prefs(|view| view.songs_sort = key)
        {
            tracing::warn!(name = "library.sort_persist_failed", error = %err, "sort not saved");
        }
    }

    /// Last albums-tab sort key.
    pub fn albums_sort(&self) -> QString {
        QString::from(self.rust().core.view_prefs().albums_sort.as_str())
    }

    /// Persist the albums-tab sort key.
    pub fn set_albums_sort(self: Pin<&mut Self>, key: &QString) {
        let key = String::from(key);
        if let Err(err) = self
            .rust()
            .core
            .set_view_prefs(|view| view.albums_sort = key)
        {
            tracing::warn!(name = "library.sort_persist_failed", error = %err, "sort not saved");
        }
    }

    /// Last artists-tab sort key.
    pub fn artists_sort(&self) -> QString {
        QString::from(self.rust().core.view_prefs().artists_sort.as_str())
    }

    /// Persist the artists-tab sort key.
    pub fn set_artists_sort(self: Pin<&mut Self>, key: &QString) {
        let key = String::from(key);
        if let Err(err) = self
            .rust()
            .core
            .set_view_prefs(|view| view.artists_sort = key)
        {
            tracing::warn!(name = "library.sort_persist_failed", error = %err, "sort not saved");
        }
    }

    /// Open the parent directory of an indexed track in the file manager.
    pub fn reveal_track(self: Pin<&mut Self>, track_id: i32) {
        if let Err(err) = self.rust().core.reveal_track(i64::from(track_id)) {
            tracing::warn!(name = "library.reveal_failed", error = %err, "folder not opened");
        }
    }

    /// Delete one index row. Playlist links stay dangling.
    pub fn remove_track(self: Pin<&mut Self>, track_id: i32) {
        if let Err(err) = self.rust().core.remove_track(i64::from(track_id)) {
            tracing::warn!(name = "library.remove_track_failed", error = %err, "row kept");
        }
    }

    /// Album title by id.
    pub fn album_title(&self, album_id: i32) -> QString {
        self.rust()
            .core
            .album(i64::from(album_id))
            .map(|row| QString::from(row.title.as_str()))
            .unwrap_or_default()
    }

    /// Album artist by id.
    pub fn album_artist(&self, album_id: i32) -> QString {
        self.rust()
            .core
            .album(i64::from(album_id))
            .and_then(|row| row.artist)
            .map_or_else(
                || QString::from("Unknown Artist"),
                |name| QString::from(name.as_str()),
            )
    }

    /// Album year by id (0 when unknown).
    pub fn album_year(&self, album_id: i32) -> i32 {
        self.rust()
            .core
            .album(i64::from(album_id))
            .and_then(|row| row.year)
            .and_then(|year| i32::try_from(year).ok())
            .unwrap_or(0)
    }

    /// Album track count by id.
    pub fn album_track_count(&self, album_id: i32) -> i32 {
        self.rust()
            .core
            .album(i64::from(album_id))
            .map_or(0, |row| i32::try_from(row.track_count).unwrap_or(i32::MAX))
    }

    /// Album duration in milliseconds.
    pub fn album_duration_ms(&self, album_id: i32) -> i32 {
        i32::try_from(self.rust().core.album_duration_ms(i64::from(album_id))).unwrap_or(0)
    }

    /// Artist album count.
    pub fn artist_album_count(&self, name: &QString) -> i32 {
        self.rust()
            .core
            .artist(&String::from(name))
            .map_or(0, |row| i32::try_from(row.album_count).unwrap_or(i32::MAX))
    }

    /// Artist track count.
    pub fn artist_track_count(&self, name: &QString) -> i32 {
        self.rust()
            .core
            .artist(&String::from(name))
            .map_or(0, |row| i32::try_from(row.track_count).unwrap_or(i32::MAX))
    }

    /// Remember a search query, newest first, cap 10, case-insensitive dedupe.
    pub fn remember_search(self: Pin<&mut Self>, query: &QString) {
        let trimmed = String::from(query).trim().to_owned();
        if trimmed.len() < 2 {
            return;
        }
        if let Err(err) = self.rust().core.set_view_prefs(|view| {
            view.recent_searches
                .retain(|item| !item.eq_ignore_ascii_case(&trimmed));
            view.recent_searches.insert(0, trimmed);
            view.recent_searches.truncate(10);
        }) {
            tracing::warn!(name = "library.search_remember_failed", error = %err, "query not saved");
        }
    }

    /// How many recent searches are stored.
    pub fn recent_search_count(&self) -> i32 {
        i32::try_from(self.rust().core.view_prefs().recent_searches.len()).unwrap_or(i32::MAX)
    }

    /// Recent search at `index` (0 = newest).
    pub fn recent_search_at(&self, index: i32) -> QString {
        usize::try_from(index)
            .ok()
            .and_then(|index| {
                self.rust()
                    .core
                    .view_prefs()
                    .recent_searches
                    .get(index)
                    .cloned()
            })
            .map(|query| QString::from(query.as_str()))
            .unwrap_or_default()
    }
}
