//! `FolderListModel` rows and loaders (bridge in [`super::models`]).
//!
//! Folders *browse*: unique parent directories of indexed tracks. This is
//! not library-root add/remove (that stays on [`LibraryManager`] / Settings).

use super::models::qobject;

use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

debug_roles!(qobject::FolderRoles, FolderRoles, [Path, Name, TrackCount]);

/// Folder row store: path (drill key), display name, track count.
#[derive(Debug, Default)]
pub struct FolderListModelRust {
    folders: Vec<(QString, QString, i32)>,
}

impl FolderListModelRust {
    /// Push one row; returns its index.
    fn push_row(&mut self, path: QString, name: QString, track_count: i32) -> i32 {
        let row = i32::try_from(self.folders.len()).unwrap_or(i32::MAX);
        self.folders.push((path, name, track_count));
        row
    }

    /// Drop all rows.
    fn drop_rows(&mut self) {
        self.folders.clear();
    }

    /// Current row count.
    fn row_count(&self) -> i32 {
        i32::try_from(self.folders.len()).unwrap_or(i32::MAX)
    }

    /// Absolute path at `row` (empty when out of range).
    fn path_at(&self, row: i32) -> QString {
        usize::try_from(row)
            .ok()
            .and_then(|index| self.folders.get(index))
            .map(|entry| entry.0.clone())
            .unwrap_or_default()
    }

    /// Display name at `row` (empty when out of range).
    fn name_at(&self, row: i32) -> QString {
        usize::try_from(row)
            .ok()
            .and_then(|index| self.folders.get(index))
            .map(|entry| entry.1.clone())
            .unwrap_or_default()
    }

    /// Data for one row/role; invalid variant when out of range or unknown.
    fn row_data(&self, row: usize, role: qobject::FolderRoles) -> QVariant {
        if let Some((path, name, track_count)) = self.folders.get(row) {
            return match role {
                qobject::FolderRoles::Path => QVariant::from(path),
                qobject::FolderRoles::Name => QVariant::from(name),
                qobject::FolderRoles::TrackCount => QVariant::from(track_count),
                _ => QVariant::default(),
            };
        }
        QVariant::default()
    }
}

/// Map one folder group to its display row.
fn display_folder(row: &tunex_library::FolderRow) -> (QString, QString, i32) {
    (
        QString::from(&row.path),
        QString::from(&row.name),
        i32::try_from(row.track_count).unwrap_or(i32::MAX),
    )
}

/// Load folder rows from the index at `path` (empty when absent/unreadable).
fn load_folders(path: &std::path::Path) -> Vec<(QString, QString, i32)> {
    if !path.is_file() {
        return Vec::new();
    }
    let db = match tunex_library::open_file(path) {
        Ok(db) => db,
        Err(err) => {
            tracing::warn!(name = "browse.folders_failed", error = %err, "index unreadable");
            return Vec::new();
        }
    };
    match tunex_library::list_track_folders(&db) {
        Ok(rows) => rows.iter().map(display_folder).collect(),
        Err(err) => {
            tracing::warn!(name = "browse.folders_failed", error = %err, "index unreadable");
            Vec::new()
        }
    }
}

impl qobject::FolderListModel {
    /// Reload folders from the library index; emits model reset.
    pub fn refresh(mut self: Pin<&mut Self>) {
        let rows = load_folders(&tunex_core::library_db_path());
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_folders();
            let mut rust = self.as_mut().rust_mut();
            rust.drop_rows();
            for (path, name, track_count) in rows {
                rust.push_row(path, name, track_count);
            }
            self.as_mut().end_reset_model_folders();
        }
    }

    /// Drop all rows; emits model reset so views rebuild.
    pub fn clear(mut self: Pin<&mut Self>) {
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_folders();
            self.as_mut().rust_mut().drop_rows();
            self.as_mut().end_reset_model_folders();
        }
    }

    /// Absolute path at `row` (empty when out of range). Exposed as `pathAt`.
    pub fn path_at(&self, row: i32) -> QString {
        self.rust().path_at(row)
    }

    /// Display name at `row` (empty when out of range). Exposed as `nameAt`.
    pub fn name_at(&self, row: i32) -> QString {
        self.rust().name_at(row)
    }

    /// Row count override for `QAbstractListModel`.
    pub fn row_count_folders(&self, _parent: &QModelIndex) -> i32 {
        self.rust().row_count()
    }

    /// Role data override for `QAbstractListModel`.
    pub fn data_folders(&self, index: &QModelIndex, role: i32) -> QVariant {
        let row = usize::try_from(index.row()).unwrap_or(usize::MAX);
        self.rust()
            .row_data(row, qobject::FolderRoles { repr: role })
    }

    /// Role-name table override; without it QML sees no custom roles.
    pub fn role_names_folders(&self) -> QHash<QHashPair_i32_QByteArray> {
        let _ = self;
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(qobject::FolderRoles::Path.repr, QByteArray::from("path"));
        roles.insert(qobject::FolderRoles::Name.repr, QByteArray::from("name"));
        roles.insert(
            qobject::FolderRoles::TrackCount.repr,
            QByteArray::from("trackCount"),
        );
        roles
    }
}

#[cfg(test)]
mod tests {
    use super::FolderListModelRust;
    use super::qobject::FolderRoles;
    use crate::bridge::test_support::seeded_index;
    use cxx_qt_lib::{QString, QVariant};

    fn model_with_two_folders() -> FolderListModelRust {
        let mut model = FolderListModelRust::default();
        model.push_row(
            QString::from("/music/Night Tapes"),
            QString::from("Night Tapes"),
            2,
        );
        model.push_row(QString::from("/music/Only"), QString::from("Only"), 1);
        model
    }

    #[test]
    fn push_row_returns_sequential_indices() {
        let mut model = FolderListModelRust::default();
        assert_eq!(
            model.push_row(QString::from("/a"), QString::from("a"), 1),
            0
        );
        assert_eq!(
            model.push_row(QString::from("/b"), QString::from("b"), 1),
            1
        );
    }

    #[test]
    fn row_count_tracks_push_and_drop() {
        let mut model = model_with_two_folders();
        assert_eq!(model.row_count(), 2);
        model.drop_rows();
        assert_eq!(model.row_count(), 0);
    }

    #[test]
    fn row_data_returns_path_name_and_count() {
        let model = model_with_two_folders();
        assert_ne!(model.row_data(0, FolderRoles::Path), QVariant::default());
        assert_ne!(model.row_data(0, FolderRoles::Name), QVariant::default());
        assert_ne!(
            model.row_data(0, FolderRoles::TrackCount),
            QVariant::default()
        );
        assert_eq!(model.path_at(0), QString::from("/music/Night Tapes"));
        assert_eq!(model.name_at(0), QString::from("Night Tapes"));
        assert_eq!(model.path_at(99), QString::default());
        assert_eq!(model.name_at(99), QString::default());
    }

    #[test]
    fn row_data_for_unknown_role_yields_default() {
        let model = model_with_two_folders();
        assert_eq!(
            model.row_data(0, FolderRoles { repr: i32::MAX }),
            QVariant::default()
        );
    }

    #[test]
    fn missing_index_loads_zero_rows() {
        let missing =
            std::env::temp_dir().join(format!("tunex-browse-missing-{}", std::process::id()));
        assert!(super::load_folders(&missing.join("library.db")).is_empty());
    }

    #[test]
    fn seeded_index_loads_one_parent_folder() {
        let (_guard, path) = seeded_index("folders");
        let rows = super::load_folders(&path);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1, QString::from("music"));
        assert_eq!(rows[0].2, 3);
    }
}
