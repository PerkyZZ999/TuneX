//! `PlaylistTrackModel` rows and loaders (bridge in [`super::models`]).
//!
//! S3 W-024 playlist detail model: the entries of one playlist in play order,
//! with resolved display data plus dangling/missing flags the delegate turns
//! into badges. Position-based remove/move keep the detail list editable;
//! adds flow through [`PlaylistModel`](super::playlist_list_model) (which
//! owns the list counts) and land here on the next refresh.

use super::models::qobject;

use core::pin::Pin;
use std::path::PathBuf;

use cxx_qt::CxxQtType;
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

/// Human-readable `Debug` for the generated roles enum (`#[derive]` is not
/// permitted on `#[qenum]` items, so this is written by hand).
impl std::fmt::Debug for qobject::PlaylistTrackRoles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Variants are associated constants on the generated type; compare
        // their discriminants rather than matching by value.
        let name = match self.repr {
            repr if repr == qobject::PlaylistTrackRoles::EntryId.repr => "EntryId",
            repr if repr == qobject::PlaylistTrackRoles::TrackId.repr => "TrackId",
            repr if repr == qobject::PlaylistTrackRoles::Title.repr => "Title",
            repr if repr == qobject::PlaylistTrackRoles::Artist.repr => "Artist",
            repr if repr == qobject::PlaylistTrackRoles::DurationMs.repr => "DurationMs",
            repr if repr == qobject::PlaylistTrackRoles::Missing.repr => "Missing",
            repr if repr == qobject::PlaylistTrackRoles::Dangling.repr => "Dangling",
            _ => "Unknown",
        };
        write!(f, "PlaylistTrackRoles::{name}")
    }
}

/// Entry row: entry/track identity plus display data and state flags.
type PlaylistTrackRow = (i32, i32, QString, QString, i32, bool, bool);

/// Detail row store for one playlist.
#[derive(Debug)]
pub struct PlaylistTrackModelRust {
    entries: Vec<PlaylistTrackRow>,
    index_path: PathBuf,
    playlist_id: Option<i64>,
    last_error: Option<String>,
}

impl Default for PlaylistTrackModelRust {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            index_path: tunex_core::library_db_path(),
            playlist_id: None,
            last_error: None,
        }
    }
}

impl PlaylistTrackModelRust {
    /// Drop all rows.
    fn drop_rows(&mut self) {
        self.entries.clear();
    }

    /// Current row count.
    fn row_count(&self) -> i32 {
        i32::try_from(self.entries.len()).unwrap_or(i32::MAX)
    }

    /// Data for one row/role; invalid variant when out of range or the role
    /// is unknown.
    fn row_data(&self, row: usize, role: qobject::PlaylistTrackRoles) -> QVariant {
        if let Some((entry_id, track_id, title, artist, duration_ms, missing, dangling)) =
            self.entries.get(row)
        {
            return match role {
                qobject::PlaylistTrackRoles::EntryId => QVariant::from(entry_id),
                qobject::PlaylistTrackRoles::TrackId => QVariant::from(track_id),
                qobject::PlaylistTrackRoles::Title => QVariant::from(title),
                qobject::PlaylistTrackRoles::Artist => QVariant::from(artist),
                qobject::PlaylistTrackRoles::DurationMs => QVariant::from(duration_ms),
                qobject::PlaylistTrackRoles::Missing => QVariant::from(missing),
                qobject::PlaylistTrackRoles::Dangling => QVariant::from(dangling),
                _ => QVariant::default(),
            };
        }
        QVariant::default()
    }

    /// Load one playlist's entries in play order; dangling entries resolve
    /// to placeholder rows (never dropped). A missing index or playlist
    /// loads zero rows with surfaced text.
    fn refresh_rows(&mut self, playlist_id: i64) -> bool {
        self.playlist_id = Some(playlist_id);
        if !self.index_path.is_file() {
            self.entries.clear();
            self.last_error = Some("Your library is empty — add a music folder first.".to_owned());
            return false;
        }
        let mut db = match tunex_library::open_file(&self.index_path) {
            Ok(db) => db,
            Err(err) => {
                tracing::warn!(name = "browse.playlist_failed", error = %err, "index unreadable");
                self.entries.clear();
                self.last_error = Some("The music library cannot be read.".to_owned());
                return false;
            }
        };
        if let Err(err) = tunex_library::evaluate_smart_playlist(&mut db, playlist_id) {
            tracing::debug!(
                name = "browse.smart_eval_failed",
                error = %err,
                "smart playlist not rebuilt"
            );
        }
        match tunex_library::list_entries(&db, playlist_id) {
            Ok(entries) => {
                self.entries = entries.iter().map(display_entry).collect();
                self.last_error = None;
                true
            }
            Err(err) => {
                tracing::warn!(name = "browse.playlist_failed", error = %err, "entries unreadable");
                self.entries.clear();
                self.last_error = Some("That playlist is no longer in the library.".to_owned());
                false
            }
        }
    }

    /// Remove the entry at `position` and refresh (ignored out of range).
    fn do_remove_at(&mut self, position: usize) {
        let Some(playlist_id) = self.playlist_id else {
            return;
        };
        let Some(entry_id) = self.entry_id_at(position) else {
            return;
        };
        let Ok(mut db) = tunex_library::open_file(&self.index_path) else {
            self.last_error = Some("The music library cannot be read.".to_owned());
            return;
        };
        match tunex_library::remove_from_playlist(&mut db, playlist_id, entry_id) {
            Ok(()) => {
                self.refresh_rows(playlist_id);
            }
            Err(err) => self.last_error = Some(friendly_error(&err)),
        }
    }

    /// Move the entry at `from` to `to` and refresh (clamped, ignored out
    /// of range).
    fn do_move(&mut self, from: usize, to: usize) {
        let Some(playlist_id) = self.playlist_id else {
            return;
        };
        let Some(entry_id) = self.entry_id_at(from) else {
            return;
        };
        let Ok(mut db) = tunex_library::open_file(&self.index_path) else {
            self.last_error = Some("The music library cannot be read.".to_owned());
            return;
        };
        match tunex_library::move_entry(&mut db, playlist_id, entry_id, to) {
            Ok(()) => {
                self.refresh_rows(playlist_id);
            }
            Err(err) => self.last_error = Some(friendly_error(&err)),
        }
    }

    /// Entry id at a display position, if any.
    fn entry_id_at(&self, position: usize) -> Option<i64> {
        self.entries.get(position).map(|entry| i64::from(entry.0))
    }

    /// Add one track to the open playlist; returns 1 (0 + surfaced text
    /// when unavailable). Missing files are refused loudly.
    fn do_add_track(&mut self, track_id: i64) -> i32 {
        let Some(playlist_id) = self.playlist_id else {
            self.last_error = Some("Open a playlist first.".to_owned());
            return 0;
        };
        let Ok(db) = tunex_library::open_file(&self.index_path) else {
            self.last_error = Some("The music library cannot be read.".to_owned());
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
                tracing::warn!(name = "browse.playlist_failed", error = %err, "lookup failed");
                self.last_error = Some("That track is no longer in the library.".to_owned());
                return 0;
            }
            Ok(Some(_)) => {}
        }
        let Ok(mut db) = tunex_library::open_file(&self.index_path) else {
            self.last_error = Some("The music library cannot be read.".to_owned());
            return 0;
        };
        match tunex_library::add_to_playlist(&mut db, playlist_id, track_id) {
            Ok(()) => {
                self.refresh_rows(playlist_id);
                self.last_error = None;
                1
            }
            Err(err) => {
                self.last_error = Some(friendly_error(&err));
                0
            }
        }
    }

    /// Last failure, if any (cleared by the next success).
    fn error_message(&self) -> Option<String> {
        self.last_error.clone()
    }

    fn reload_index_path(&mut self) {
        self.index_path = tunex_core::library_db_path();
    }

    /// Whether the row at `row` can play (present, resolved, and on disk).
    fn is_playable_at(&self, row: i32) -> bool {
        usize::try_from(row)
            .ok()
            .and_then(|index| self.entries.get(index))
            .is_some_and(|entry| !entry.5 && !entry.6)
    }
}

/// Map one entry to its display row: resolved tracks show tags, dangling
/// entries show an honest placeholder the delegate badges.
fn display_entry(entry: &tunex_library::PlaylistEntry) -> PlaylistTrackRow {
    let entry_id = i32::try_from(entry.id).unwrap_or(i32::MAX);
    let track_id = i32::try_from(entry.track_id).unwrap_or(i32::MAX);
    match &entry.track {
        Some(row) => (
            entry_id,
            track_id,
            QString::from(row.title.as_deref().unwrap_or("Unknown Title")),
            QString::from(row.artist.as_deref().unwrap_or("Unknown Artist")),
            row.duration_ms
                .and_then(|duration| i32::try_from(duration).ok())
                .unwrap_or(0),
            row.missing,
            false,
        ),
        None => (
            entry_id,
            track_id,
            QString::from("Unavailable track"),
            QString::from("Removed from library"),
            0,
            false,
            true,
        ),
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

impl qobject::PlaylistTrackModel {
    /// Load one playlist's entries in play order; emits model reset.
    pub fn refresh_playlist(mut self: Pin<&mut Self>, playlist_id: i32) {
        self.as_mut()
            .rust_mut()
            .refresh_rows(i64::from(playlist_id));
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_playlist_tracks();
            self.as_mut().end_reset_model_playlist_tracks();
        }
    }

    /// Drop all rows without touching the index (view-only reset).
    pub fn clear(mut self: Pin<&mut Self>) {
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_playlist_tracks();
            self.as_mut().rust_mut().drop_rows();
            self.as_mut().end_reset_model_playlist_tracks();
        }
    }

    /// Remove the entry at `index` (ignored when out of range).
    pub fn remove_at(mut self: Pin<&mut Self>, index: i32) {
        if let Ok(at) = usize::try_from(index) {
            self.as_mut().rust_mut().do_remove_at(at);
        }
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_playlist_tracks();
            self.as_mut().end_reset_model_playlist_tracks();
        }
    }

    /// Move the entry at `from` to `to` (clamped, ignored out of range).
    pub fn move_item(mut self: Pin<&mut Self>, from: i32, to: i32) {
        if let (Ok(from), Ok(to)) = (usize::try_from(from), usize::try_from(to)) {
            self.as_mut().rust_mut().do_move(from, to);
        }
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_playlist_tracks();
            self.as_mut().end_reset_model_playlist_tracks();
        }
    }

    /// Add one track to the open playlist; returns 1 (0 + error text when
    /// unavailable).
    #[must_use]
    pub fn add_track(mut self: Pin<&mut Self>, track_id: i32) -> i32 {
        let added = self.as_mut().rust_mut().do_add_track(i64::from(track_id));
        if added > 0 {
            // SAFETY: reset pair strictly paired on this single path.
            unsafe {
                self.as_mut().begin_reset_model_playlist_tracks();
                self.as_mut().end_reset_model_playlist_tracks();
            };
        }
        added
    }

    /// Entry id at `row` (-1 when out of range).
    pub fn entry_id_at(&self, row: i32) -> i32 {
        usize::try_from(row)
            .ok()
            .and_then(|index| self.rust().entries.get(index))
            .map_or(-1, |entry| entry.0)
    }

    /// Whether the row at `row` can play (present, resolved, on disk).
    pub fn is_playable_at(&self, row: i32) -> bool {
        self.rust().is_playable_at(row)
    }

    /// Track id at `row` (-1 when out of range), for play-from-detail.
    pub fn track_id_at(&self, row: i32) -> i32 {
        usize::try_from(row)
            .ok()
            .and_then(|index| self.rust().entries.get(index))
            .map_or(-1, |entry| entry.1)
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
    pub fn row_count_playlist_tracks(&self, _parent: &QModelIndex) -> i32 {
        self.rust().row_count()
    }

    /// Role data override for `QAbstractListModel`.
    pub fn data_playlist_tracks(&self, index: &QModelIndex, role: i32) -> QVariant {
        let row = usize::try_from(index.row()).unwrap_or(usize::MAX);
        self.rust()
            .row_data(row, qobject::PlaylistTrackRoles { repr: role })
    }

    /// Role-name table override; without it QML sees no custom roles.
    /// The Qt virtual signature requires the receiver although no instance
    /// state participates.
    pub fn role_names_playlist_tracks(&self) -> QHash<QHashPair_i32_QByteArray> {
        let _ = self;
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(
            qobject::PlaylistTrackRoles::EntryId.repr,
            QByteArray::from("entryId"),
        );
        roles.insert(
            qobject::PlaylistTrackRoles::TrackId.repr,
            QByteArray::from("trackId"),
        );
        roles.insert(
            qobject::PlaylistTrackRoles::Title.repr,
            QByteArray::from("title"),
        );
        roles.insert(
            qobject::PlaylistTrackRoles::Artist.repr,
            QByteArray::from("artist"),
        );
        roles.insert(
            qobject::PlaylistTrackRoles::DurationMs.repr,
            QByteArray::from("durationMs"),
        );
        roles.insert(
            qobject::PlaylistTrackRoles::Missing.repr,
            QByteArray::from("missing"),
        );
        roles.insert(
            qobject::PlaylistTrackRoles::Dangling.repr,
            QByteArray::from("dangling"),
        );
        roles
    }
}

#[cfg(test)]
mod tests {
    use super::PlaylistTrackModelRust;
    use super::qobject::PlaylistTrackRoles;
    use crate::bridge::test_support::seeded_index;
    use cxx_qt_lib::{QString, QVariant};

    fn model_with_seeded_library(
        case: &str,
    ) -> (PlaylistTrackModelRust, crate::bridge::test_support::Guard) {
        let (guard, path) = seeded_index(case);
        let model = PlaylistTrackModelRust {
            index_path: path,
            ..Default::default()
        };
        (model, guard)
    }

    #[test]
    fn row_data_returns_every_role() {
        let mut model = PlaylistTrackModelRust::default();
        model.entries.push((
            11,
            7,
            QString::from("Midnight"),
            QString::from("Nova Rae"),
            273_000,
            false,
            false,
        ));
        assert_eq!(model.row_count(), 1);
        assert_ne!(
            model.row_data(0, PlaylistTrackRoles::EntryId),
            QVariant::default()
        );
        assert_ne!(
            model.row_data(0, PlaylistTrackRoles::TrackId),
            QVariant::default()
        );
        assert_ne!(
            model.row_data(0, PlaylistTrackRoles::Title),
            QVariant::default()
        );
        assert_ne!(
            model.row_data(0, PlaylistTrackRoles::Artist),
            QVariant::default()
        );
        assert_ne!(
            model.row_data(0, PlaylistTrackRoles::DurationMs),
            QVariant::default()
        );
        assert_eq!(
            model.row_data(0, PlaylistTrackRoles { repr: i32::MAX }),
            QVariant::default()
        );
        assert_eq!(
            model.row_data(99, PlaylistTrackRoles::Title),
            QVariant::default()
        );
    }

    #[test]
    fn refresh_remove_and_move_round_trip() {
        let (mut model, _guard) = model_with_seeded_library("ptracks-ops");
        let db = tunex_library::open_file(&model.index_path).expect("seed opens");
        let id = tunex_library::create_playlist(&db, "Mix").expect("create works");
        let tracks = tunex_library::list_tracks(&db).expect("list works");
        drop(db);
        let mut db = tunex_library::open_file(&model.index_path).expect("reopen works");
        for row in &tracks {
            tunex_library::add_to_playlist(&mut db, id, row.id).expect("add works");
        }
        drop(db);
        assert!(model.refresh_rows(id));
        assert_eq!(model.row_count(), 3);
        model.do_remove_at(0);
        assert_eq!(model.row_count(), 2);
        model.do_remove_at(99);
        assert_eq!(model.row_count(), 2, "out-of-range remove ignored");
        model.do_move(1, 0);
        assert_eq!(model.entries[0].2, QString::from("Solo"));
        assert!(model.error_message().is_none());
    }

    #[test]
    fn dangling_entries_render_placeholders() {
        let (mut model, _guard) = model_with_seeded_library("ptracks-dangle");
        let db = tunex_library::open_file(&model.index_path).expect("seed opens");
        let id = tunex_library::create_playlist(&db, "Fragile").expect("create works");
        let track = tunex_library::list_tracks(&db).expect("list works")[0].id;
        drop(db);
        let mut db = tunex_library::open_file(&model.index_path).expect("reopen works");
        tunex_library::add_to_playlist(&mut db, id, track).expect("add works");
        db.execute("DELETE FROM tracks WHERE id = ?1", [track])
            .expect("delete works");
        drop(db);
        assert!(model.refresh_rows(id));
        assert_eq!(model.row_count(), 1);
        assert!(model.entries[0].6, "dangling flag set");
        assert_eq!(model.entries[0].2, QString::from("Unavailable track"));
    }

    #[test]
    fn playable_guard_rejects_dangling() {
        let (mut model, _guard) = model_with_seeded_library("ptracks-playable");
        let db = tunex_library::open_file(&model.index_path).expect("seed opens");
        let id = tunex_library::create_playlist(&db, "Mix").expect("create works");
        let track = tunex_library::list_tracks(&db).expect("list works")[0].id;
        drop(db);
        let mut db = tunex_library::open_file(&model.index_path).expect("reopen works");
        tunex_library::add_to_playlist(&mut db, id, track).expect("add works");
        drop(db);
        assert!(model.refresh_rows(id));
        assert!(model.is_playable_at(0));
        assert!(!model.is_playable_at(99));
        let db = tunex_library::open_file(&model.index_path).expect("reopen works");
        db.execute("DELETE FROM tracks WHERE id = ?1", [track])
            .expect("delete works");
        drop(db);
        assert!(model.refresh_rows(id));
        assert!(!model.is_playable_at(0), "dangling rows never play");
    }

    #[test]
    fn missing_index_loads_zero_rows() {
        let missing =
            std::env::temp_dir().join(format!("tunex-ptracks-noindex-{}", std::process::id()));
        let mut model = PlaylistTrackModelRust {
            index_path: missing.join("library.db"),
            ..Default::default()
        };
        assert!(!model.refresh_rows(1));
        assert_eq!(model.row_count(), 0);
        assert!(model.error_message().is_some_and(|text| !text.is_empty()));
    }
}
