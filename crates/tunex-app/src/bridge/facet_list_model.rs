//! `FacetListModel` rows and loaders (bridge in [`super::models`]).
//!
//! Genre and composer browse: name plus track count. Untagged rows group
//! as `Unknown` in SQL; this model only displays what the index returns.

use super::models::qobject;

use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

/// Human-readable `Debug` for the generated roles enum.
impl std::fmt::Debug for qobject::FacetRoles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self.repr {
            repr if repr == qobject::FacetRoles::Name.repr => "Name",
            repr if repr == qobject::FacetRoles::TrackCount.repr => "TrackCount",
            _ => "Unknown",
        };
        write!(f, "FacetRoles::{name}")
    }
}

/// Which facet list is showing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FacetKind {
    #[default]
    Genres,
    Composers,
}

/// Genre/composer row store.
#[derive(Debug, Default)]
pub struct FacetListModelRust {
    rows: Vec<(QString, i32)>,
    kind: FacetKind,
}

impl FacetListModelRust {
    fn push_row(&mut self, name: QString, track_count: i32) -> i32 {
        let row = i32::try_from(self.rows.len()).unwrap_or(i32::MAX);
        self.rows.push((name, track_count));
        row
    }

    fn drop_rows(&mut self) {
        self.rows.clear();
    }

    fn row_count(&self) -> i32 {
        i32::try_from(self.rows.len()).unwrap_or(i32::MAX)
    }

    fn name_at(&self, row: i32) -> QString {
        usize::try_from(row)
            .ok()
            .and_then(|index| self.rows.get(index))
            .map(|entry| entry.0.clone())
            .unwrap_or_default()
    }

    fn row_data(&self, row: usize, role: qobject::FacetRoles) -> QVariant {
        if let Some((name, track_count)) = self.rows.get(row) {
            return match role {
                qobject::FacetRoles::Name => QVariant::from(name),
                qobject::FacetRoles::TrackCount => QVariant::from(track_count),
                _ => QVariant::default(),
            };
        }
        QVariant::default()
    }
}

fn display_facet(row: &tunex_library::FacetRow) -> (QString, i32) {
    (
        QString::from(&row.name),
        i32::try_from(row.track_count).unwrap_or(i32::MAX),
    )
}

fn load_facets(path: &std::path::Path, kind: FacetKind) -> Vec<(QString, i32)> {
    if !path.is_file() {
        return Vec::new();
    }
    let db = match tunex_library::open_file(path) {
        Ok(db) => db,
        Err(err) => {
            tracing::warn!(name = "browse.facets_failed", error = %err, "index unreadable");
            return Vec::new();
        }
    };
    let rows = match kind {
        FacetKind::Genres => tunex_library::list_genres(&db),
        FacetKind::Composers => tunex_library::list_composers(&db),
    };
    match rows {
        Ok(rows) => rows.iter().map(display_facet).collect(),
        Err(err) => {
            tracing::warn!(name = "browse.facets_failed", error = %err, "index unreadable");
            Vec::new()
        }
    }
}

impl qobject::FacetListModel {
    /// Reload genre groups. Exposed as `refreshGenres`.
    pub fn refresh_genres(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().kind = FacetKind::Genres;
        self.as_mut().reload();
    }

    /// Reload composer groups. Exposed as `refreshComposers`.
    pub fn refresh_composers(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().kind = FacetKind::Composers;
        self.as_mut().reload();
    }

    fn reload(mut self: Pin<&mut Self>) {
        let kind = self.as_ref().rust().kind;
        let rows = load_facets(&tunex_core::library_db_path(), kind);
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_facets();
            let mut rust = self.as_mut().rust_mut();
            rust.drop_rows();
            for (name, track_count) in rows {
                rust.push_row(name, track_count);
            }
            self.as_mut().end_reset_model_facets();
        }
    }

    /// Drop all rows; emits model reset so views rebuild.
    pub fn clear(mut self: Pin<&mut Self>) {
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_facets();
            self.as_mut().rust_mut().drop_rows();
            self.as_mut().end_reset_model_facets();
        }
    }

    /// Display name at `row` (empty when out of range). Exposed as `nameAt`.
    pub fn name_at(&self, row: i32) -> QString {
        self.rust().name_at(row)
    }

    /// Row count override for `QAbstractListModel`.
    pub fn row_count_facets(&self, _parent: &QModelIndex) -> i32 {
        self.rust().row_count()
    }

    /// Role data override for `QAbstractListModel`.
    pub fn data_facets(&self, index: &QModelIndex, role: i32) -> QVariant {
        let row = usize::try_from(index.row()).unwrap_or(usize::MAX);
        self.rust()
            .row_data(row, qobject::FacetRoles { repr: role })
    }

    /// Role-name table override; without it QML sees no custom roles.
    pub fn role_names_facets(&self) -> QHash<QHashPair_i32_QByteArray> {
        let _ = self;
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(qobject::FacetRoles::Name.repr, QByteArray::from("name"));
        roles.insert(
            qobject::FacetRoles::TrackCount.repr,
            QByteArray::from("trackCount"),
        );
        roles
    }
}

#[cfg(test)]
mod tests {
    use super::FacetListModelRust;
    use super::qobject::FacetRoles;
    use cxx_qt_lib::{QString, QVariant};

    fn model_with_two() -> FacetListModelRust {
        let mut model = FacetListModelRust::default();
        model.push_row(QString::from("Ambient"), 4);
        model.push_row(QString::from("Unknown"), 2);
        model
    }

    #[test]
    fn row_data_returns_name_and_count() {
        let model = model_with_two();
        assert_eq!(model.row_count(), 2);
        assert_eq!(model.name_at(0), QString::from("Ambient"));
        assert_ne!(
            model.row_data(0, FacetRoles::TrackCount),
            QVariant::default()
        );
        assert_eq!(model.name_at(99), QString::default());
    }
}
