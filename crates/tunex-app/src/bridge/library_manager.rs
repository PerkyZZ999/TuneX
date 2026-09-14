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

    /// One tag field for the editor.
    pub fn track_value(&self, track_id: i32, key: &QString) -> QString {
        QString::from(
            self.rust()
                .core
                .track_value(i64::from(track_id), &key.to_string())
                .as_str(),
        )
    }

    /// Write tags then re-index that path. Returns 1 on success (0 + `errorText`).
    pub fn save_tags(self: Pin<&mut Self>, track_id: i32, packed: &QString) -> i32 {
        let packed = packed.to_string();
        let mut fields = packed.split('\u{1f}');
        let edit = tunex_library::TagEdit {
            title: nonempty_str(fields.next().unwrap_or("")),
            artist: nonempty_str(fields.next().unwrap_or("")),
            album: nonempty_str(fields.next().unwrap_or("")),
            track_number: parse_u32_str(fields.next().unwrap_or("")),
            disc_number: parse_u32_str(fields.next().unwrap_or("")),
            year: parse_u32_str(fields.next().unwrap_or("")),
            genre: nonempty_str(fields.next().unwrap_or("")),
            composer: nonempty_str(fields.next().unwrap_or("")),
        };
        match self.rust().core.save_track_tags(i64::from(track_id), edit) {
            Ok(()) => 1,
            Err(err) => {
                tracing::warn!(name = "library.tags_failed", error = %err, "tags not written");
                0
            }
        }
    }

    /// Active profile id (empty = default).
    pub fn active_profile(&self) -> QString {
        QString::from(self.rust().core.active_profile().as_str())
    }

    /// How many profiles including Default.
    pub fn profile_count(&self) -> i32 {
        i32::try_from(self.rust().core.profiles().len()).unwrap_or(i32::MAX)
    }

    /// Profile id at `index`.
    pub fn profile_id_at(&self, index: i32) -> QString {
        let rows = self.rust().core.profiles();
        usize::try_from(index)
            .ok()
            .and_then(|at| rows.get(at))
            .map(|(id, _)| QString::from(id.as_str()))
            .unwrap_or_default()
    }

    /// Profile name at `index`.
    pub fn profile_name_at(&self, index: i32) -> QString {
        let rows = self.rust().core.profiles();
        usize::try_from(index)
            .ok()
            .and_then(|at| rows.get(at))
            .map(|(_, name)| QString::from(name.as_str()))
            .unwrap_or_default()
    }

    /// Create a named profile.
    pub fn create_profile(self: Pin<&mut Self>, name: &QString) -> QString {
        match self.rust().core.create_profile(&name.to_string()) {
            Ok(id) => QString::from(id.as_str()),
            Err(err) => {
                tracing::warn!(name = "library.profile_create_failed", error = %err, "profile not created");
                QString::default()
            }
        }
    }

    /// Switch the active profile and reopen its index.
    pub fn switch_profile(mut self: Pin<&mut Self>, id: &QString) {
        if let Err(err) = self
            .as_mut()
            .rust_mut()
            .core
            .switch_profile(&id.to_string())
        {
            tracing::warn!(name = "library.profile_switch_failed", error = %err, "profile kept");
        }
    }

    /// `MusicBrainz` opt-in (default off).
    pub fn musicbrainz_on(&self) -> bool {
        self.rust().core.musicbrainz_on()
    }

    /// Persist the `MusicBrainz` opt-in.
    pub fn set_musicbrainz_on(self: Pin<&mut Self>, on: bool) {
        if let Err(err) = self.rust().core.set_musicbrainz(on) {
            tracing::warn!(name = "library.musicbrainz_persist_failed", error = %err, "setting not saved");
        }
    }
}

fn nonempty_str(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn parse_u32_str(value: &str) -> Option<u32> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse().ok().filter(|number| *number > 0)
}
