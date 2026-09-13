//! `ArtistListModel` rows and loaders (bridge in [`super::models`]).
//!
//! S2 W-016 browse model: rows load from the library index on `refresh`
//! (bounded, indexed queries — no scan, no decode, no watcher on this path).
//! A missing index file is not an error: the model stays empty and QML shows
//! the empty-library state.
//!
//! All row logic lives on the plain [`ArtistListModelRust`] struct (fully
//! unit-tested); the `impl` below only pairs Qt model notifications around
//! it (see the S1 proof model for the pairing pattern).

use super::models::qobject;

use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

use crate::search::SearchCore;

/// Human-readable `Debug` for the generated roles enum (`#[derive]` is not
/// permitted on `#[qenum]` items, so this is written by hand).
impl std::fmt::Debug for qobject::ArtistRoles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Variants are associated constants on the generated type; compare
        // their discriminants rather than matching by value.
        let name = match self.repr {
            repr if repr == qobject::ArtistRoles::Name.repr => "Name",
            repr if repr == qobject::ArtistRoles::AlbumCount.repr => "AlbumCount",
            repr if repr == qobject::ArtistRoles::TrackCount.repr => "TrackCount",
            _ => "Unknown",
        };
        write!(f, "ArtistRoles::{name}")
    }
}

/// Artist row store: display name plus collection counts.
#[derive(Debug, Default)]
pub struct ArtistListModelRust {
    artists: Vec<(QString, i32, i32)>,
    search: SearchCore,
    sort: tunex_library::ArtistSort,
}

impl ArtistListModelRust {
    /// Push one row; returns its index (saturates instead of wrapping on
    /// absurd lengths — a view count, never an allocation index).
    fn push_row(&mut self, name: QString, album_count: i32, track_count: i32) -> i32 {
        let row = i32::try_from(self.artists.len()).unwrap_or(i32::MAX);
        self.artists.push((name, album_count, track_count));
        row
    }

    /// Drop all rows.
    fn drop_rows(&mut self) {
        self.artists.clear();
    }

    /// Current row count.
    fn row_count(&self) -> i32 {
        i32::try_from(self.artists.len()).unwrap_or(i32::MAX)
    }

    /// Data for one row/role; invalid variant when out of range or the role
    /// is unknown.
    fn row_data(&self, row: usize, role: qobject::ArtistRoles) -> QVariant {
        if let Some((name, album_count, track_count)) = self.artists.get(row) {
            return match role {
                qobject::ArtistRoles::Name => QVariant::from(name),
                qobject::ArtistRoles::AlbumCount => QVariant::from(album_count),
                qobject::ArtistRoles::TrackCount => QVariant::from(track_count),
                _ => QVariant::default(),
            };
        }
        QVariant::default()
    }

    /// Submit raw query text to the debounced search worker.
    fn submit_search(&mut self, query: &str) {
        self.search.submit(query);
    }

    /// Drain one settled result set into row payloads (`None` while the
    /// worker runs or idles — the caller refreshes views on `Some`).
    fn poll_search_rows(&mut self) -> Option<Vec<(QString, i32, i32)>> {
        self.search.poll();
        self.search
            .take_results()
            .map(|found| found.artists.iter().map(display_artist).collect())
    }

    /// Whether a submitted query is still waiting on the worker.
    fn is_searching(&self) -> bool {
        self.search.is_searching()
    }

    /// Last search failure, if any.
    fn search_error(&self) -> Option<String> {
        self.search.error_text()
    }
}

/// Map one artist group hit to its display row (shared by browse + search).
fn display_artist(row: &tunex_library::ArtistRow) -> (QString, i32, i32) {
    (
        QString::from(&row.name),
        i32::try_from(row.album_count).unwrap_or(i32::MAX),
        i32::try_from(row.track_count).unwrap_or(i32::MAX),
    )
}

/// Load artist rows from the index at `path` (empty when absent/unreadable —
/// the empty-library state, never an error surface).
fn load_artists(
    path: &std::path::Path,
    sort: tunex_library::ArtistSort,
) -> Vec<(QString, i32, i32)> {
    if !path.is_file() {
        return Vec::new();
    }
    let db = match tunex_library::open_file(path) {
        Ok(db) => db,
        Err(err) => {
            tracing::warn!(name = "browse.artists_failed", error = %err, "index unreadable");
            return Vec::new();
        }
    };
    match tunex_library::list_artists(&db, sort) {
        Ok(rows) => rows.iter().map(display_artist).collect(),
        Err(err) => {
            tracing::warn!(name = "browse.artists_failed", error = %err, "index unreadable");
            Vec::new()
        }
    }
}

impl qobject::ArtistListModel {
    /// Reload all artists from the library index; emits model reset.
    pub fn refresh(mut self: Pin<&mut Self>) {
        let sort = self.as_ref().rust().sort;
        let rows = load_artists(&tunex_core::library_db_path(), sort);
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_artists();
            let mut rust = self.as_mut().rust_mut();
            rust.drop_rows();
            for (name, album_count, track_count) in rows {
                rust.push_row(name, album_count, track_count);
            }
            self.as_mut().end_reset_model_artists();
        }
    }

    /// Remember an artists-tab sort key and reload. Exposed as `setSort`.
    pub fn set_sort(mut self: Pin<&mut Self>, key: &QString) {
        let sort = tunex_library::ArtistSort::from_key(&key.to_string());
        self.as_mut().rust_mut().sort = sort;
        let rows = load_artists(&tunex_core::library_db_path(), sort);
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_artists();
            let mut rust = self.as_mut().rust_mut();
            rust.drop_rows();
            for (name, album_count, track_count) in rows {
                rust.push_row(name, album_count, track_count);
            }
            self.as_mut().end_reset_model_artists();
        }
    }

    /// Current artists-tab sort key. Exposed as `sortKey`.
    pub fn sort_key(&self) -> QString {
        QString::from(self.rust().sort.as_key())
    }

    /// Drop all rows; emits model reset so views rebuild.
    pub fn clear(mut self: Pin<&mut Self>) {
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_artists();
            self.as_mut().rust_mut().drop_rows();
            self.as_mut().end_reset_model_artists();
        }
    }

    /// Submit raw query text to this model's search worker.
    pub fn search(mut self: Pin<&mut Self>, query: &QString) {
        self.as_mut().rust_mut().submit_search(&query.to_string());
    }

    /// Drain settled search results into rows; emits model reset when new
    /// rows land, and reports whether anything did.
    #[must_use]
    pub fn poll_search(mut self: Pin<&mut Self>) -> bool {
        let found = self.as_mut().rust_mut().poll_search_rows();
        let Some(rows) = found else {
            return false;
        };
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_artists();
            let mut rust = self.as_mut().rust_mut();
            rust.drop_rows();
            for (name, album_count, track_count) in rows {
                rust.push_row(name, album_count, track_count);
            }
            self.as_mut().end_reset_model_artists();
        };
        true
    }

    /// Whether a submitted query is still waiting on the worker.
    pub fn is_searching(&self) -> bool {
        self.rust().is_searching()
    }

    /// Last search failure, or empty when clear.
    pub fn error_text(&self) -> QString {
        self.rust()
            .search_error()
            .map(QString::from)
            .unwrap_or_default()
    }

    /// Row count override for `QAbstractListModel`.
    pub fn row_count_artists(&self, _parent: &QModelIndex) -> i32 {
        self.rust().row_count()
    }

    /// Role data override for `QAbstractListModel`.
    pub fn data_artists(&self, index: &QModelIndex, role: i32) -> QVariant {
        let row = usize::try_from(index.row()).unwrap_or(usize::MAX);
        self.rust()
            .row_data(row, qobject::ArtistRoles { repr: role })
    }

    /// Role-name table override; without it QML sees no custom roles.
    /// The Qt virtual signature requires the receiver although no instance
    /// state participates.
    pub fn role_names_artists(&self) -> QHash<QHashPair_i32_QByteArray> {
        let _ = self;
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(qobject::ArtistRoles::Name.repr, QByteArray::from("name"));
        roles.insert(
            qobject::ArtistRoles::AlbumCount.repr,
            QByteArray::from("albumCount"),
        );
        roles.insert(
            qobject::ArtistRoles::TrackCount.repr,
            QByteArray::from("trackCount"),
        );
        roles
    }
}

#[cfg(test)]
mod tests {
    use super::ArtistListModelRust;
    use super::qobject::ArtistRoles;
    use crate::bridge::test_support::seeded_index;
    use crate::search::SearchCore;
    use cxx_qt_lib::{QString, QVariant};
    use std::time::{Duration, Instant};

    /// Poll until one result set settles (tests end idle so the joined
    /// worker never outlives its scratch dir).
    fn settle_search(model: &mut ArtistListModelRust) -> Vec<(QString, i32, i32)> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(rows) = model.poll_search_rows() {
                return rows;
            }
            assert!(Instant::now() < deadline, "search never settled");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn model_with_two_artists() -> ArtistListModelRust {
        let mut model = ArtistListModelRust::default();
        model.push_row(QString::from("Nova Rae"), 2, 14);
        model.push_row(QString::from("Solo Act"), 1, 3);
        model
    }

    #[test]
    fn push_row_returns_sequential_indices() {
        let mut model = ArtistListModelRust::default();
        assert_eq!(model.push_row(QString::from("A"), 1, 1), 0);
        assert_eq!(model.push_row(QString::from("B"), 1, 1), 1);
    }

    #[test]
    fn row_count_tracks_push_and_drop() {
        let mut model = model_with_two_artists();
        assert_eq!(model.row_count(), 2);
        model.drop_rows();
        assert_eq!(model.row_count(), 0);
    }

    #[test]
    fn row_data_returns_name_and_counts() {
        let model = model_with_two_artists();
        assert_ne!(model.row_data(1, ArtistRoles::Name), QVariant::default());
        assert_ne!(
            model.row_data(1, ArtistRoles::AlbumCount),
            QVariant::default()
        );
        assert_ne!(
            model.row_data(1, ArtistRoles::TrackCount),
            QVariant::default()
        );
    }

    #[test]
    fn row_data_for_unknown_role_yields_default() {
        let model = model_with_two_artists();
        assert_eq!(
            model.row_data(0, ArtistRoles { repr: i32::MAX }),
            QVariant::default()
        );
    }

    #[test]
    fn row_data_out_of_range_yields_default() {
        let model = model_with_two_artists();
        assert_eq!(model.row_data(99, ArtistRoles::Name), QVariant::default());
    }

    #[test]
    fn missing_index_loads_zero_rows() {
        let missing =
            std::env::temp_dir().join(format!("tunex-browse-missing-{}", std::process::id()));
        assert!(
            super::load_artists(&missing.join("library.db"), tunex_library::ArtistSort::Name)
                .is_empty()
        );
    }

    #[test]
    fn seeded_index_loads_artist_rows() {
        let (_guard, path) = seeded_index("artists");
        let rows = super::load_artists(&path, tunex_library::ArtistSort::Name);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, QString::from("Nova Rae"));
        let by_songs = super::load_artists(&path, tunex_library::ArtistSort::Songs);
        assert_eq!(by_songs[0].2, 2);
    }

    #[test]
    fn search_settles_matching_rows() {
        let (_guard, path) = seeded_index("search-artists");
        let mut model = ArtistListModelRust {
            search: SearchCore::with_db_path(path),
            ..Default::default()
        };
        model.search.set_debounce(Duration::from_millis(10));
        model.submit_search("nova");
        assert!(model.is_searching());
        let rows = settle_search(&mut model);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, QString::from("Nova Rae"));
        assert!(!model.is_searching());
        assert!(
            model.poll_search_rows().is_none(),
            "results deliver exactly once"
        );
    }

    #[test]
    fn empty_query_searches_nothing() {
        let (_guard, path) = seeded_index("search-artists-empty");
        let mut model = ArtistListModelRust {
            search: SearchCore::with_db_path(path),
            ..Default::default()
        };
        model.search.set_debounce(Duration::from_millis(10));
        model.submit_search("");
        assert!(settle_search(&mut model).is_empty());
        assert!(model.search_error().is_none());
    }
}
