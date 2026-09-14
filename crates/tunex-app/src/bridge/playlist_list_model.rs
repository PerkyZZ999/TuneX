//! `PlaylistModel` rows and loaders (bridge in [`super::models`]).
//!
//! S3 W-024 playlists sidebar model: rows load from the index on `refresh`
//! (bounded, indexed queries — no scan, no decode on this path). Create,
//! rename, and add resolve through the playlist backend with explicit,
//! user-readable error text; a missing index loads zero rows (the empty
//! state, never an error surface).
//!
//! All row logic lives on the plain [`PlaylistModelRust`] struct (fully
//! unit-tested); the `impl` below only pairs Qt model notifications around
//! it (see the S1 proof model for the pairing pattern).

use super::models::qobject;

use core::pin::Pin;
use std::path::PathBuf;

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

/// Human-readable `Debug` for the generated roles enum (`#[derive]` is not
/// permitted on `#[qenum]` items, so this is written by hand).
impl std::fmt::Debug for qobject::PlaylistRoles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Variants are associated constants on the generated type; compare
        // their discriminants rather than matching by value.
        let name = match self.repr {
            repr if repr == qobject::PlaylistRoles::PlaylistId.repr => "PlaylistId",
            repr if repr == qobject::PlaylistRoles::Name.repr => "Name",
            repr if repr == qobject::PlaylistRoles::TrackCount.repr => "TrackCount",
            _ => "Unknown",
        };
        write!(f, "PlaylistRoles::{name}")
    }
}

/// Playlist row: drill-down id plus name and entry count.
type PlaylistRow = (i32, QString, i32);

/// Playlist row store.
#[derive(Debug)]
pub struct PlaylistModelRust {
    playlists: Vec<PlaylistRow>,
    index_path: PathBuf,
    last_error: Option<String>,
}

impl Default for PlaylistModelRust {
    fn default() -> Self {
        Self {
            playlists: Vec::new(),
            index_path: tunex_core::library_db_path(),
            last_error: None,
        }
    }
}

impl PlaylistModelRust {
    /// Drop all rows.
    fn drop_rows(&mut self) {
        self.playlists.clear();
    }

    /// Playlist name at `row` (empty when out of range), for keyboard select.
    fn playlist_name_at(&self, row: i32) -> QString {
        usize::try_from(row)
            .ok()
            .and_then(|index| self.playlists.get(index))
            .map_or_else(QString::default, |entry| entry.1.clone())
    }

    /// Current row count.
    fn row_count(&self) -> i32 {
        i32::try_from(self.playlists.len()).unwrap_or(i32::MAX)
    }

    /// Data for one row/role; invalid variant when out of range or the role
    /// is unknown.
    fn row_data(&self, row: usize, role: qobject::PlaylistRoles) -> QVariant {
        if let Some((playlist_id, name, track_count)) = self.playlists.get(row) {
            return match role {
                qobject::PlaylistRoles::PlaylistId => QVariant::from(playlist_id),
                qobject::PlaylistRoles::Name => QVariant::from(name),
                qobject::PlaylistRoles::TrackCount => QVariant::from(track_count),
                _ => QVariant::default(),
            };
        }
        QVariant::default()
    }

    /// Reload rows from the index; returns false with surfaced text when the
    /// index is missing or unreadable (empty state, never a crash).
    fn refresh_rows(&mut self) -> bool {
        if !self.index_path.is_file() {
            self.playlists.clear();
            self.last_error = None;
            return true;
        }
        let db = match tunex_library::open_file(&self.index_path) {
            Ok(db) => db,
            Err(err) => {
                tracing::warn!(name = "browse.playlists_failed", error = %err, "index unreadable");
                self.playlists.clear();
                self.last_error = Some("The music library cannot be read.".to_owned());
                return false;
            }
        };
        match tunex_library::list_playlists(&db) {
            Ok(playlists) => {
                self.playlists = playlists
                    .into_iter()
                    .map(|playlist| {
                        (
                            i32::try_from(playlist.id).unwrap_or(i32::MAX),
                            QString::from(&playlist.name),
                            i32::try_from(playlist.track_count).unwrap_or(i32::MAX),
                        )
                    })
                    .collect();
                self.last_error = None;
                true
            }
            Err(err) => {
                tracing::warn!(name = "browse.playlists_failed", error = %err, "index unreadable");
                self.playlists.clear();
                self.last_error = Some("The music library cannot be read.".to_owned());
                false
            }
        }
    }

    /// Open the index for mutations (creating it is pointless — playlists
    /// need a library first, so a missing index is an honest empty state).
    fn open_writable(&mut self) -> Option<rusqlite::Connection> {
        if !self.index_path.is_file() {
            self.last_error = Some("Your library is empty — add a music folder first.".to_owned());
            return None;
        }
        match tunex_library::open_file(&self.index_path) {
            Ok(db) => Some(db),
            Err(err) => {
                tracing::warn!(name = "browse.playlists_failed", error = %err, "index unreadable");
                self.last_error = Some("The music library cannot be read.".to_owned());
                None
            }
        }
    }

    /// Create a playlist; returns its id (-1 with surfaced text on failure).
    fn do_create(&mut self, name: &str) -> i32 {
        let Some(db) = self.open_writable() else {
            return -1;
        };
        match tunex_library::create_playlist(&db, name) {
            Ok(id) => {
                self.refresh_rows();
                i32::try_from(id).unwrap_or(i32::MAX)
            }
            Err(err) => {
                self.last_error = Some(friendly_error(&err));
                -1
            }
        }
    }

    /// Create an auto-named playlist ("New playlist", "New playlist 2", …)
    /// for the row-menu fast path; returns its id (-1 on failure).
    fn do_create_auto(&mut self) -> i32 {
        let Some(db) = self.open_writable() else {
            return -1;
        };
        let existing = tunex_library::list_playlists(&db)
            .map(|playlists| {
                playlists
                    .into_iter()
                    .map(|playlist| playlist.name)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut candidate = "New playlist".to_owned();
        let mut counter = 2;
        while existing.iter().any(|name| name == &candidate) {
            candidate = format!("New playlist {counter}");
            counter += 1;
        }
        match tunex_library::create_playlist(&db, &candidate) {
            Ok(id) => {
                self.refresh_rows();
                i32::try_from(id).unwrap_or(i32::MAX)
            }
            Err(err) => {
                self.last_error = Some(friendly_error(&err));
                -1
            }
        }
    }

    /// Create a smart playlist from a stored rule; returns its id (-1 on failure).
    fn do_create_smart(
        &mut self,
        name: &str,
        kind: &str,
        value: &str,
        exclude_missing: bool,
    ) -> i32 {
        let Some(mut db) = self.open_writable() else {
            return -1;
        };
        match tunex_library::create_smart_playlist(
            &mut db,
            name,
            &tunex_library::SmartRule {
                kind: kind.to_owned(),
                value: value.to_owned(),
                exclude_missing,
            },
        ) {
            Ok(id) => {
                self.refresh_rows();
                i32::try_from(id).unwrap_or(i32::MAX)
            }
            Err(err) => {
                self.last_error = Some(friendly_error(&err));
                -1
            }
        }
    }

    /// Whether this playlist is generated from a stored rule.
    fn is_smart_id(&self, id: i64) -> bool {
        if !self.index_path.is_file() {
            return false;
        }
        let Ok(db) = tunex_library::open_file(&self.index_path) else {
            return false;
        };
        tunex_library::smart_rule(&db, id).ok().flatten().is_some()
    }

    /// Rename a playlist; blank/taken/missing ids surface text.
    fn do_rename(&mut self, id: i64, name: &str) {
        let Some(db) = self.open_writable() else {
            return;
        };
        match tunex_library::rename_playlist(&db, id, name) {
            Ok(()) => {
                self.refresh_rows();
            }
            Err(err) => self.last_error = Some(friendly_error(&err)),
        }
    }

    /// Delete a playlist and its entries.
    fn do_delete(&mut self, id: i64) {
        let Some(mut db) = self.open_writable() else {
            return;
        };
        match tunex_library::delete_playlist(&mut db, id) {
            Ok(()) => {
                self.refresh_rows();
            }
            Err(err) => self.last_error = Some(friendly_error(&err)),
        }
    }

    /// Add one track to a playlist; returns 1 (0 with surfaced text when
    /// unavailable). Missing files are refused loudly — playlists keep
    /// dangling entries only when files vanish *after* adding.
    fn do_add_track(&mut self, playlist_id: i64, track_id: i64) -> i32 {
        let Some(mut db) = self.open_writable() else {
            return 0;
        };
        match tunex_library::track_by_id(&db, track_id) {
            Ok(Some(row)) if row.missing => {
                self.last_error = Some("That file is missing from disk.".to_owned());
                return 0;
            }
            Ok(None) => {
                self.last_error = Some("That track is no longer in the library.".to_owned());
                return 0;
            }
            Err(err) => {
                tracing::warn!(name = "browse.playlists_failed", error = %err, "lookup failed");
                self.last_error = Some("That track is no longer in the library.".to_owned());
                return 0;
            }
            Ok(Some(_)) => {}
        }
        match tunex_library::add_to_playlist(&mut db, playlist_id, track_id) {
            Ok(()) => {
                self.refresh_rows();
                self.last_error = None;
                1
            }
            Err(err) => {
                self.last_error = Some(friendly_error(&err));
                0
            }
        }
    }

    /// Add many library ids to one playlist.
    fn do_add_tracks(&mut self, playlist_id: i64, ids: &[i64]) -> i32 {
        let mut added = 0;
        for track_id in ids {
            added += self.do_add_track(playlist_id, *track_id);
        }
        added
    }

    /// Last failure, if any (cleared by the next success).
    fn error_message(&self) -> Option<String> {
        self.last_error.clone()
    }

    fn reload_index_path(&mut self) {
        self.index_path = tunex_core::library_db_path();
    }
}

/// Backend errors already carry user-readable messages; strip the
/// `database error: ` prefix the core Display adds.
fn friendly_error(err: &tunex_core::Error) -> String {
    let text = err.to_string();
    text.strip_prefix("database error: ")
        .unwrap_or(&text)
        .to_owned()
}

impl qobject::PlaylistModel {
    /// Reload all playlists; emits model reset.
    pub fn refresh(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().refresh_rows();
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_playlists();
            self.as_mut().end_reset_model_playlists();
        }
    }

    /// Drop all rows without touching the index (view-only reset).
    pub fn clear(mut self: Pin<&mut Self>) {
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_playlists();
            self.as_mut().rust_mut().drop_rows();
            self.as_mut().end_reset_model_playlists();
        }
    }

    /// Create a playlist; returns its id (-1 + `errorText` on failure).
    #[must_use]
    pub fn create_playlist(mut self: Pin<&mut Self>, name: &QString) -> i32 {
        let id = self.as_mut().rust_mut().do_create(&name.to_string());
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_playlists();
            self.as_mut().end_reset_model_playlists();
        };
        id
    }

    /// Create an auto-named playlist for the row-menu fast path; returns its
    /// id (-1 + `errorText` on failure).
    #[must_use]
    pub fn create_playlist_auto(mut self: Pin<&mut Self>) -> i32 {
        let id = self.as_mut().rust_mut().do_create_auto();
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_playlists();
            self.as_mut().end_reset_model_playlists();
        };
        id
    }

    /// Create a smart playlist from a stored rule; returns its id (-1 + error).
    #[must_use]
    pub fn create_smart_playlist(
        mut self: Pin<&mut Self>,
        name: &QString,
        kind: &QString,
        value: &QString,
        exclude_missing: bool,
    ) -> i32 {
        let id = self.as_mut().rust_mut().do_create_smart(
            &name.to_string(),
            &kind.to_string(),
            &value.to_string(),
            exclude_missing,
        );
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_playlists();
            self.as_mut().end_reset_model_playlists();
        };
        id
    }

    /// Whether this playlist rebuilds from a stored rule.
    #[must_use]
    pub fn is_smart(&self, id: i32) -> bool {
        self.rust().is_smart_id(i64::from(id))
    }

    /// Rename a playlist; failures surface through `errorText`.
    pub fn rename_playlist(mut self: Pin<&mut Self>, id: i32, name: &QString) {
        self.as_mut()
            .rust_mut()
            .do_rename(i64::from(id), &name.to_string());
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_playlists();
            self.as_mut().end_reset_model_playlists();
        }
    }

    /// Delete a playlist and its entries; failures surface via `errorText`.
    pub fn delete_playlist(mut self: Pin<&mut Self>, id: i32) {
        self.as_mut().rust_mut().do_delete(i64::from(id));
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_playlists();
            self.as_mut().end_reset_model_playlists();
        }
    }

    /// Add one track to a playlist; returns 1 (0 + error text when the
    /// track is missing, gone, or unplayable).
    #[must_use]
    pub fn add_track(mut self: Pin<&mut Self>, playlist_id: i32, track_id: i32) -> i32 {
        let added = self
            .as_mut()
            .rust_mut()
            .do_add_track(i64::from(playlist_id), i64::from(track_id));
        if added > 0 {
            // SAFETY: reset pair strictly paired on this single path.
            unsafe {
                self.as_mut().begin_reset_model_playlists();
                self.as_mut().end_reset_model_playlists();
            };
        }
        added
    }

    /// Add comma-separated library ids to a playlist.
    #[must_use]
    pub fn add_tracks(mut self: Pin<&mut Self>, playlist_id: i32, ids: &QString) -> i32 {
        let parsed = ids
            .to_string()
            .split(',')
            .filter_map(|part| {
                let part = part.trim();
                if part.is_empty() {
                    None
                } else {
                    part.parse::<i64>().ok()
                }
            })
            .collect::<Vec<_>>();
        let added = self
            .as_mut()
            .rust_mut()
            .do_add_tracks(i64::from(playlist_id), &parsed);
        if added > 0 {
            // SAFETY: reset pair strictly paired on this single path.
            unsafe {
                self.as_mut().begin_reset_model_playlists();
                self.as_mut().end_reset_model_playlists();
            };
        }
        added
    }

    /// Playlist name at `row` (empty when out of range).
    pub fn playlist_name_at(&self, row: i32) -> QString {
        self.rust().playlist_name_at(row)
    }

    /// Playlist id at `row` (-1 when out of range).
    pub fn playlist_id_at(&self, row: i32) -> i32 {
        usize::try_from(row)
            .ok()
            .and_then(|index| self.rust().playlists.get(index))
            .map_or(-1, |entry| entry.0)
    }

    /// Last failure, or empty when clear.
    pub fn error_text(&self) -> QString {
        self.rust()
            .error_message()
            .map(QString::from)
            .unwrap_or_default()
    }

    /// Re-read the active profile index.
    pub fn reload_index(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().reload_index_path();
    }

    /// Row count override for `QAbstractListModel`.
    pub fn row_count_playlists(&self, _parent: &QModelIndex) -> i32 {
        self.rust().row_count()
    }

    /// Role data override for `QAbstractListModel`.
    pub fn data_playlists(&self, index: &QModelIndex, role: i32) -> QVariant {
        let row = usize::try_from(index.row()).unwrap_or(usize::MAX);
        self.rust()
            .row_data(row, qobject::PlaylistRoles { repr: role })
    }

    /// Role-name table override; without it QML sees no custom roles.
    /// The Qt virtual signature requires the receiver although no instance
    /// state participates.
    pub fn role_names_playlists(&self) -> QHash<QHashPair_i32_QByteArray> {
        let _ = self;
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(
            qobject::PlaylistRoles::PlaylistId.repr,
            QByteArray::from("playlistId"),
        );
        roles.insert(qobject::PlaylistRoles::Name.repr, QByteArray::from("name"));
        roles.insert(
            qobject::PlaylistRoles::TrackCount.repr,
            QByteArray::from("trackCount"),
        );
        roles
    }
}

#[cfg(test)]
mod tests {
    use super::PlaylistModelRust;
    use super::qobject::PlaylistRoles;
    use crate::bridge::test_support::seeded_index;
    use cxx_qt_lib::{QString, QVariant};

    fn model_with_seeded_library(
        case: &str,
    ) -> (PlaylistModelRust, crate::bridge::test_support::Guard) {
        let (guard, path) = seeded_index(case);
        let model = PlaylistModelRust {
            index_path: path,
            ..Default::default()
        };
        (model, guard)
    }

    #[test]
    fn row_data_returns_every_role() {
        let mut model = PlaylistModelRust::default();
        model.playlists.push((3, QString::from("Evening"), 12));
        assert_eq!(model.row_count(), 1);
        assert_ne!(
            model.row_data(0, PlaylistRoles::PlaylistId),
            QVariant::default()
        );
        assert_ne!(model.row_data(0, PlaylistRoles::Name), QVariant::default());
        assert_ne!(
            model.row_data(0, PlaylistRoles::TrackCount),
            QVariant::default()
        );
        assert_eq!(
            model.row_data(0, PlaylistRoles { repr: i32::MAX }),
            QVariant::default()
        );
        assert_eq!(model.row_data(99, PlaylistRoles::Name), QVariant::default());
    }

    #[test]
    fn create_rename_delete_round_trip() {
        let (mut model, _guard) = model_with_seeded_library("pl-crud");
        assert!(model.refresh_rows());
        assert_eq!(model.row_count(), 0);
        let id = model.do_create("Evening");
        assert!(id > 0);
        assert_eq!(model.row_count(), 1);
        assert_eq!(model.do_create("Evening"), -1, "duplicates rejected");
        assert!(model.error_message().is_some_and(|text| !text.is_empty()));
        assert_eq!(model.do_create("  "), -1, "blank names rejected");
        model.do_rename(i64::from(id), "Night");
        assert!(model.error_message().is_none());
        assert_eq!(model.playlists[0].1, QString::from("Night"));
        model.do_delete(i64::from(id));
        assert_eq!(model.row_count(), 0);
        assert!(model.error_message().is_none());
        model.do_delete(i64::from(id));
        assert!(model.error_message().is_some(), "double delete loud");
    }

    #[test]
    fn create_smart_never_played_round_trip() {
        let (mut model, _guard) = model_with_seeded_library("pl-smart");
        let id = model.do_create_smart("Unheard", "never_played", "", true);
        assert!(id > 0);
        assert!(model.is_smart_id(i64::from(id)));
        let db = tunex_library::open_file(&model.index_path).expect("seed opens");
        let track = tunex_library::list_tracks(&db).expect("list works")[0].id;
        drop(db);
        assert_eq!(model.do_add_track(i64::from(id), track), 0);
        assert!(model.error_message().is_some_and(|text| !text.is_empty()));
    }

    #[test]
    fn auto_create_numbers_freely_and_adds_tracks() {
        let (mut model, _guard) = model_with_seeded_library("pl-auto");
        let first = model.do_create_auto();
        let second = model.do_create_auto();
        assert!(first > 0 && second > 0 && first != second);
        let names: Vec<String> = model
            .playlists
            .iter()
            .map(|entry| entry.1.to_string())
            .collect();
        assert_eq!(names, ["New playlist", "New playlist 2"]);
        let db = tunex_library::open_file(&model.index_path).expect("seed opens");
        let track = tunex_library::list_tracks(&db).expect("list works")[0].id;
        drop(db);
        assert_eq!(model.do_add_track(i64::from(first), track), 1);
        assert_eq!(model.playlists[0].2, 1, "count follows adds");
        assert_eq!(model.do_add_track(i64::from(first), 999_999), 0);
        assert!(model.error_message().is_some_and(|text| !text.is_empty()));
    }

    #[test]
    fn missing_index_reads_empty_and_blocks_writes() {
        let missing = std::env::temp_dir().join(format!("tunex-pl-noindex-{}", std::process::id()));
        let mut model = PlaylistModelRust {
            index_path: missing.join("library.db"),
            ..Default::default()
        };
        assert!(model.refresh_rows());
        assert_eq!(model.row_count(), 0);
        assert_eq!(model.do_create("Nope"), -1);
        assert!(
            model
                .error_message()
                .is_some_and(|text| text.contains("empty"))
        );
    }
}
