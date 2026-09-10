//! `AlbumListModel` rows and loaders (bridge in [`super::models`]).
//!
//! S2 W-016 browse model: rows load from the library index on `refresh`
//! (bounded, indexed queries — no scan, no decode, no watcher on this path).
//! A missing index file is not an error: the model stays empty and QML shows
//! the empty-library state. Artwork arrives with the W-017 background worker;
//! until then every card renders its monogram placeholder.
//!
//! All row logic lives on the plain [`AlbumListModelRust`] struct (fully
//! unit-tested); the `impl` below only pairs Qt model notifications around
//! it (see the S1 proof model for the pairing pattern).

use super::models::qobject;

use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

use crate::search::SearchCore;

/// Human-readable `Debug` for the generated roles enum (`#[derive]` is not
/// permitted on `#[qenum]` items, so this is written by hand).
impl std::fmt::Debug for qobject::AlbumRoles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Variants are associated constants on the generated type; compare
        // their discriminants rather than matching by value.
        let name = match self.repr {
            repr if repr == qobject::AlbumRoles::AlbumId.repr => "AlbumId",
            repr if repr == qobject::AlbumRoles::Title.repr => "Title",
            repr if repr == qobject::AlbumRoles::Artist.repr => "Artist",
            repr if repr == qobject::AlbumRoles::Year.repr => "Year",
            repr if repr == qobject::AlbumRoles::TrackCount.repr => "TrackCount",
            _ => "Unknown",
        };
        write!(f, "AlbumRoles::{name}")
    }
}

/// Album row store: drill-down id plus display fields.
#[derive(Debug, Default)]
pub struct AlbumListModelRust {
    albums: Vec<(i32, QString, QString, i32, i32)>,
    search: SearchCore,
}

/// One settled search result set as display rows.
type AlbumSearchRows = Vec<(i32, QString, QString, i32, i32)>;

impl AlbumListModelRust {
    /// Push one row; returns its index (saturates instead of wrapping on
    /// absurd lengths — a view count, never an allocation index).
    fn push_row(
        &mut self,
        album_id: i32,
        title: QString,
        artist: QString,
        year: i32,
        track_count: i32,
    ) -> i32 {
        let row = i32::try_from(self.albums.len()).unwrap_or(i32::MAX);
        self.albums
            .push((album_id, title, artist, year, track_count));
        row
    }

    /// Drop all rows.
    fn drop_rows(&mut self) {
        self.albums.clear();
    }

    /// Current row count.
    fn row_count(&self) -> i32 {
        i32::try_from(self.albums.len()).unwrap_or(i32::MAX)
    }

    /// Data for one row/role; invalid variant when out of range or the role
    /// is unknown.
    fn row_data(&self, row: usize, role: qobject::AlbumRoles) -> QVariant {
        if let Some((album_id, title, artist, year, track_count)) = self.albums.get(row) {
            return match role {
                qobject::AlbumRoles::AlbumId => QVariant::from(album_id),
                qobject::AlbumRoles::Title => QVariant::from(title),
                qobject::AlbumRoles::Artist => QVariant::from(artist),
                qobject::AlbumRoles::Year => QVariant::from(year),
                qobject::AlbumRoles::TrackCount => QVariant::from(track_count),
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
    fn poll_search_rows(&mut self) -> Option<AlbumSearchRows> {
        self.search.poll();
        self.search
            .take_results()
            .map(|found| found.albums.iter().map(display_album).collect())
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

/// Map one album group hit to its display row (shared by browse + search).
fn display_album(row: &tunex_library::AlbumRow) -> (i32, QString, QString, i32, i32) {
    (
        i32::try_from(row.id).unwrap_or(i32::MAX),
        QString::from(&row.title),
        QString::from(row.artist.as_deref().unwrap_or("Unknown Artist")),
        row.year
            .and_then(|year| i32::try_from(year).ok())
            .unwrap_or(0),
        i32::try_from(row.track_count).unwrap_or(i32::MAX),
    )
}

/// Load album rows from the index at `path` (empty when absent/unreadable —
/// the empty-library state, never an error surface).
fn load_albums(path: &std::path::Path) -> Vec<(i32, QString, QString, i32, i32)> {
    if !path.is_file() {
        return Vec::new();
    }
    let db = match tunex_library::open_file(path) {
        Ok(db) => db,
        Err(err) => {
            tracing::warn!(name = "browse.albums_failed", error = %err, "index unreadable");
            return Vec::new();
        }
    };
    match tunex_library::list_albums(&db) {
        Ok(rows) => rows.iter().map(display_album).collect(),
        Err(err) => {
            tracing::warn!(name = "browse.albums_failed", error = %err, "index unreadable");
            Vec::new()
        }
    }
}

impl qobject::AlbumListModel {
    /// Reload all albums from the library index; emits model reset.
    pub fn refresh(mut self: Pin<&mut Self>) {
        let rows = load_albums(&tunex_core::library_db_path());
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_albums();
            let mut rust = self.as_mut().rust_mut();
            rust.drop_rows();
            for (album_id, title, artist, year, track_count) in rows {
                rust.push_row(album_id, title, artist, year, track_count);
            }
            self.as_mut().end_reset_model_albums();
        }
    }

    /// Drop all rows; emits model reset so views rebuild.
    pub fn clear(mut self: Pin<&mut Self>) {
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_albums();
            self.as_mut().rust_mut().drop_rows();
            self.as_mut().end_reset_model_albums();
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
            self.as_mut().begin_reset_model_albums();
            let mut rust = self.as_mut().rust_mut();
            rust.drop_rows();
            for (album_id, title, artist, year, track_count) in rows {
                rust.push_row(album_id, title, artist, year, track_count);
            }
            self.as_mut().end_reset_model_albums();
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
    pub fn row_count_albums(&self, _parent: &QModelIndex) -> i32 {
        self.rust().row_count()
    }

    /// Role data override for `QAbstractListModel`.
    pub fn data_albums(&self, index: &QModelIndex, role: i32) -> QVariant {
        let row = usize::try_from(index.row()).unwrap_or(usize::MAX);
        self.rust()
            .row_data(row, qobject::AlbumRoles { repr: role })
    }

    /// Role-name table override; without it QML sees no custom roles.
    /// The Qt virtual signature requires the receiver although no instance
    /// state participates.
    pub fn role_names_albums(&self) -> QHash<QHashPair_i32_QByteArray> {
        let _ = self;
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(
            qobject::AlbumRoles::AlbumId.repr,
            QByteArray::from("albumId"),
        );
        roles.insert(qobject::AlbumRoles::Title.repr, QByteArray::from("title"));
        roles.insert(qobject::AlbumRoles::Artist.repr, QByteArray::from("artist"));
        roles.insert(qobject::AlbumRoles::Year.repr, QByteArray::from("year"));
        roles.insert(
            qobject::AlbumRoles::TrackCount.repr,
            QByteArray::from("trackCount"),
        );
        roles
    }
}

#[cfg(test)]
mod tests {
    use super::AlbumListModelRust;
    use super::qobject::AlbumRoles;
    use crate::bridge::test_support::seeded_index;
    use crate::search::SearchCore;
    use cxx_qt_lib::{QString, QVariant};
    use std::time::{Duration, Instant};

    /// Poll until one result set settles (tests end idle so the joined
    /// worker never outlives its scratch dir).
    fn settle_search(model: &mut AlbumListModelRust) -> super::AlbumSearchRows {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(rows) = model.poll_search_rows() {
                return rows;
            }
            assert!(Instant::now() < deadline, "search never settled");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn model_with_two_albums() -> AlbumListModelRust {
        let mut model = AlbumListModelRust::default();
        model.push_row(
            7,
            QString::from("Night Tapes"),
            QString::from("Nova Rae"),
            2024,
            14,
        );
        model.push_row(9, QString::from("Only"), QString::from("Solo Act"), 0, 3);
        model
    }

    #[test]
    fn push_row_returns_sequential_indices() {
        let mut model = AlbumListModelRust::default();
        assert_eq!(
            model.push_row(1, QString::from("A"), QString::from("B"), 2000, 1),
            0
        );
        assert_eq!(
            model.push_row(2, QString::from("C"), QString::from("D"), 2001, 1),
            1
        );
    }

    #[test]
    fn row_count_tracks_push_and_drop() {
        let mut model = model_with_two_albums();
        assert_eq!(model.row_count(), 2);
        model.drop_rows();
        assert_eq!(model.row_count(), 0);
    }

    #[test]
    fn row_data_returns_every_role() {
        let model = model_with_two_albums();
        assert_ne!(model.row_data(0, AlbumRoles::AlbumId), QVariant::default());
        assert_ne!(model.row_data(0, AlbumRoles::Title), QVariant::default());
        assert_ne!(model.row_data(0, AlbumRoles::Artist), QVariant::default());
        assert_ne!(model.row_data(0, AlbumRoles::Year), QVariant::default());
        assert_ne!(
            model.row_data(0, AlbumRoles::TrackCount),
            QVariant::default()
        );
    }

    #[test]
    fn row_data_for_unknown_role_yields_default() {
        let model = model_with_two_albums();
        assert_eq!(
            model.row_data(0, AlbumRoles { repr: i32::MAX }),
            QVariant::default()
        );
    }

    #[test]
    fn row_data_out_of_range_yields_default() {
        let model = model_with_two_albums();
        assert_eq!(model.row_data(99, AlbumRoles::Title), QVariant::default());
    }

    #[test]
    fn missing_index_loads_zero_rows() {
        let missing =
            std::env::temp_dir().join(format!("tunex-browse-missing-{}", std::process::id()));
        assert!(super::load_albums(&missing.join("library.db")).is_empty());
    }

    #[test]
    fn seeded_index_loads_album_rows() {
        let (_guard, path) = seeded_index("albums");
        let rows = super::load_albums(&path);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn search_settles_matching_rows() {
        let (_guard, path) = seeded_index("search-albums");
        let mut model = AlbumListModelRust {
            search: SearchCore::with_db_path(path),
            ..Default::default()
        };
        model.search.set_debounce(Duration::from_millis(10));
        model.submit_search("night");
        assert!(model.is_searching());
        let rows = settle_search(&mut model);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1, QString::from("Night Tapes"));
        assert_eq!(rows[0].4, 2, "group row carries its track count");
        assert!(!model.is_searching());
        assert!(
            model.poll_search_rows().is_none(),
            "results deliver exactly once"
        );
    }

    #[test]
    fn empty_query_searches_nothing() {
        let (_guard, path) = seeded_index("search-albums-empty");
        let mut model = AlbumListModelRust {
            search: SearchCore::with_db_path(path),
            ..Default::default()
        };
        model.search.set_debounce(Duration::from_millis(10));
        model.submit_search("");
        assert!(settle_search(&mut model).is_empty());
        assert!(model.search_error().is_none());
    }
}
