//! `AlbumListModel` rows and loaders (bridge in [`super::models`]).
//!
//! S2 W-016 browse model: rows load from the library index on `refresh`
//! (bounded, indexed queries — no scan, no decode, no watcher on this path).
//! A missing index file is not an error: the model stays empty and QML shows
//! the empty-library state. Artwork is lazy (S6 W-040): a row carries the
//! track its cover comes from, the view asks for it as the card scrolls into
//! sight, and [`ArtCore`](crate::art::ArtCore) answers from a worker thread —
//! until then (and forever, for albums without art) the card renders its
//! monogram placeholder.
//!
//! All row logic lives on the plain [`AlbumListModelRust`] struct (fully
//! unit-tested); the `impl` below only pairs Qt model notifications around
//! it (see the S1 proof model for the pairing pattern).

use super::models::qobject;

use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{
    QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant, QVector,
};

use std::path::{Path, PathBuf};

use crate::art::ArtCore;
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
            repr if repr == qobject::AlbumRoles::ArtUrl.repr => "ArtUrl",
            _ => "Unknown",
        };
        write!(f, "AlbumRoles::{name}")
    }
}

/// One album as the grid shows it, plus the track its cover comes from.
#[derive(Clone, Debug, Default)]
struct AlbumRow {
    album_id: i32,
    title: QString,
    artist: QString,
    year: i32,
    track_count: i32,
    /// Track to read artwork from; empty for an album with no indexed
    /// tracks, which simply never asks.
    art_source: PathBuf,
}

/// Album row store: display rows plus the lazy artwork resolver.
#[derive(Debug, Default)]
pub struct AlbumListModelRust {
    albums: Vec<AlbumRow>,
    art: ArtCore,
    search: SearchCore,
    sort: tunex_library::AlbumSort,
}

/// One settled search result set as display rows.
type AlbumSearchRows = Vec<AlbumRow>;

impl AlbumListModelRust {
    /// Push one row; returns its index (saturates instead of wrapping on
    /// absurd lengths — a view count, never an allocation index).
    fn push_row(&mut self, album: AlbumRow) -> i32 {
        let row = i32::try_from(self.albums.len()).unwrap_or(i32::MAX);
        self.albums.push(album);
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

    /// Database album id at `row` (-1 when out of range).
    fn album_id_at(&self, row: i32) -> i32 {
        usize::try_from(row)
            .ok()
            .and_then(|index| self.albums.get(index))
            .map_or(-1, |album| album.album_id)
    }

    /// Title at `row` (empty when out of range).
    fn title_at(&self, row: i32) -> QString {
        usize::try_from(row)
            .ok()
            .and_then(|index| self.albums.get(index))
            .map_or_else(QString::default, |album| album.title.clone())
    }

    /// Data for one row/role; invalid variant when out of range or the role
    /// is unknown.
    fn row_data(&self, row: usize, role: qobject::AlbumRoles) -> QVariant {
        if let Some(album) = self.albums.get(row) {
            return match role {
                qobject::AlbumRoles::AlbumId => QVariant::from(&album.album_id),
                qobject::AlbumRoles::Title => QVariant::from(&album.title),
                qobject::AlbumRoles::Artist => QVariant::from(&album.artist),
                qobject::AlbumRoles::Year => QVariant::from(&album.year),
                qobject::AlbumRoles::TrackCount => QVariant::from(&album.track_count),
                qobject::AlbumRoles::ArtUrl => QVariant::from(&self.art_url(row)),
                _ => QVariant::default(),
            };
        }
        QVariant::default()
    }

    /// Cached cover for a row as a QML-loadable URL, empty while unresolved
    /// or when the album has none (the card keeps its placeholder).
    fn art_url(&self, row: usize) -> QString {
        let Some(album) = self.albums.get(row) else {
            return QString::default();
        };
        let key = i64::from(album.album_id);
        self.art
            .art(key)
            .map(|art| file_url(&art.thumb256))
            .unwrap_or_default()
    }

    /// Ask for one row's cover; cheap to call on every card that appears.
    fn request_art(&mut self, row: usize) {
        let Some(album) = self.albums.get(row) else {
            return;
        };
        if album.art_source.as_os_str().is_empty() {
            return;
        }
        let key = i64::from(album.album_id);
        let source = album.art_source.clone();
        self.art.request(key, &source);
    }

    /// Drain resolved covers, returning the rows that now have one to show.
    fn poll_art(&mut self) -> Vec<usize> {
        let settled = self.art.poll();
        if settled.is_empty() {
            return Vec::new();
        }
        settled
            .iter()
            .filter_map(|album_id| {
                self.albums
                    .iter()
                    .position(|album| i64::from(album.album_id) == *album_id)
            })
            .collect()
    }

    /// Whether any cover lookup is still in flight.
    fn art_pending(&self) -> bool {
        self.art.pending()
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
fn display_album(row: &tunex_library::AlbumRow) -> AlbumRow {
    AlbumRow {
        album_id: i32::try_from(row.id).unwrap_or(i32::MAX),
        title: QString::from(&row.title),
        artist: QString::from(row.artist.as_deref().unwrap_or("Unknown Artist")),
        year: row
            .year
            .and_then(|year| i32::try_from(year).ok())
            .unwrap_or(0),
        track_count: i32::try_from(row.track_count).unwrap_or(i32::MAX),
        art_source: row
            .art_source
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_default(),
    }
}

/// Cache path as the `file://` URL QML's `Image.source` expects.
fn file_url(path: &Path) -> QString {
    QString::from(&format!("file://{}", path.display()))
}

/// Load album rows from the index at `path` (empty when absent/unreadable —
/// the empty-library state, never an error surface).
fn load_albums(path: &std::path::Path, sort: tunex_library::AlbumSort) -> Vec<AlbumRow> {
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
    match tunex_library::list_albums(&db, sort) {
        Ok(rows) => rows.iter().map(display_album).collect(),
        Err(err) => {
            tracing::warn!(name = "browse.albums_failed", error = %err, "index unreadable");
            Vec::new()
        }
    }
}

fn load_albums_for_artist(
    path: &std::path::Path,
    artist: &str,
    exclude_id: Option<i64>,
) -> Vec<AlbumRow> {
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
    match tunex_library::list_albums_for_artist(&db, artist, exclude_id) {
        Ok(rows) => rows.iter().map(display_album).collect(),
        Err(err) => {
            tracing::warn!(name = "browse.albums_failed", error = %err, "index unreadable");
            Vec::new()
        }
    }
}

fn load_recently_played(path: &std::path::Path, limit: i64) -> Vec<AlbumRow> {
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
    match tunex_library::list_recently_played_albums(&db, limit) {
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
        let sort = self.as_ref().rust().sort;
        let rows = load_albums(&tunex_core::library_db_path(), sort);
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_albums();
            let mut rust = self.as_mut().rust_mut();
            rust.drop_rows();
            for album in rows {
                rust.push_row(album);
            }
            self.as_mut().end_reset_model_albums();
        }
    }

    /// Remember an albums-tab sort key and reload. Exposed as `setSort`.
    pub fn set_sort(mut self: Pin<&mut Self>, key: &QString) {
        let sort = tunex_library::AlbumSort::from_key(&key.to_string());
        self.as_mut().rust_mut().sort = sort;
        let rows = load_albums(&tunex_core::library_db_path(), sort);
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_albums();
            let mut rust = self.as_mut().rust_mut();
            rust.drop_rows();
            for album in rows {
                rust.push_row(album);
            }
            self.as_mut().end_reset_model_albums();
        }
    }

    /// Current albums-tab sort key. Exposed as `sortKey`.
    pub fn sort_key(&self) -> QString {
        QString::from(self.rust().sort.as_key())
    }

    /// Reload albums by one artist, optionally excluding `exclude_id`.
    pub fn refresh_for_artist(mut self: Pin<&mut Self>, artist: &QString, exclude_id: i32) {
        let name = artist.to_string();
        let exclude = if exclude_id < 0 {
            None
        } else {
            Some(i64::from(exclude_id))
        };
        let rows = load_albums_for_artist(&tunex_core::library_db_path(), &name, exclude);
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_albums();
            let mut rust = self.as_mut().rust_mut();
            rust.drop_rows();
            for album in rows {
                rust.push_row(album);
            }
            self.as_mut().end_reset_model_albums();
        }
    }

    /// Reload albums from play history, newest first.
    pub fn refresh_recently_played(mut self: Pin<&mut Self>) {
        let rows = load_recently_played(&tunex_core::library_db_path(), 20);
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_albums();
            let mut rust = self.as_mut().rust_mut();
            rust.drop_rows();
            for album in rows {
                rust.push_row(album);
            }
            self.as_mut().end_reset_model_albums();
        }
    }

    /// Database album id at `row` (-1 when out of range).
    pub fn album_id_at(&self, row: i32) -> i32 {
        self.rust().album_id_at(row)
    }

    /// Title at `row` (empty when out of range).
    pub fn title_at(&self, row: i32) -> QString {
        self.rust().title_at(row)
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
            for album in rows {
                rust.push_row(album);
            }
            self.as_mut().end_reset_model_albums();
        };
        true
    }

    /// Ask for one row's cover. Views call this as a card appears; rows
    /// already answered or in flight cost nothing.
    pub fn request_art(mut self: Pin<&mut Self>, row: i32) {
        let row = usize::try_from(row).unwrap_or(usize::MAX);
        self.as_mut().rust_mut().request_art(row);
    }

    /// Publish covers that have landed and report whether more are coming.
    ///
    /// Each cover replaces one placeholder, so the rows that changed are
    /// announced with `dataChanged` — a reset would rebuild the whole grid
    /// (and drop its scroll position) for one role on one row.
    #[must_use]
    pub fn poll_art(mut self: Pin<&mut Self>) -> bool {
        let settled = self.as_mut().rust_mut().poll_art();
        let roles = QVector::<i32>::from(&[qobject::AlbumRoles::ArtUrl.repr][..]);
        for row in settled {
            let row = i32::try_from(row).unwrap_or(i32::MAX);
            let index = self.as_ref().index_albums(row, 0, &QModelIndex::default());
            self.as_mut().data_changed_albums(&index, &index, &roles);
        }
        self.rust().art_pending()
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
        roles.insert(qobject::AlbumRoles::ArtUrl.repr, QByteArray::from("artUrl"));
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
    use super::qobject::AlbumRoles;
    use super::{AlbumListModelRust, AlbumRow};
    use crate::bridge::test_support::seeded_index;
    use crate::search::SearchCore;
    use cxx_qt_lib::{QString, QVariant};
    use std::path::PathBuf;
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

    /// Display row with a named album and an artwork source path.
    fn album(album_id: i32, title: &str, artist: &str, year: i32, source: &str) -> AlbumRow {
        AlbumRow {
            album_id,
            title: QString::from(title),
            artist: QString::from(artist),
            year,
            track_count: 3,
            art_source: PathBuf::from(source),
        }
    }

    fn model_with_two_albums() -> AlbumListModelRust {
        let mut model = AlbumListModelRust::default();
        model.push_row(album(7, "Night Tapes", "Nova Rae", 2024, "/music/a1.flac"));
        model.push_row(album(9, "Only", "Solo Act", 0, "/music/b1.flac"));
        model
    }

    #[test]
    fn push_row_returns_sequential_indices() {
        let mut model = AlbumListModelRust::default();
        assert_eq!(model.push_row(album(1, "A", "B", 2000, "/music/a.flac")), 0);
        assert_eq!(model.push_row(album(2, "C", "D", 2001, "/music/c.flac")), 1);
    }

    #[test]
    fn art_url_is_empty_until_a_cover_resolves() {
        // The card shows its monogram placeholder while the resolver works,
        // and keeps it for albums that never had a cover.
        let model = model_with_two_albums();
        assert_eq!(model.art_url(0), QString::default());
    }

    #[test]
    fn an_album_without_indexed_tracks_never_asks_for_art() {
        let mut model = AlbumListModelRust::default();
        model.push_row(album(1, "Empty", "Nobody", 0, ""));
        model.request_art(0);
        assert!(!model.art_pending(), "no source, no request");
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
        assert!(
            super::load_albums(&missing.join("library.db"), tunex_library::AlbumSort::Title)
                .is_empty()
        );
    }

    #[test]
    fn seeded_index_loads_album_rows() {
        let (_guard, path) = seeded_index("albums");
        let rows = super::load_albums(&path, tunex_library::AlbumSort::Title);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].title, QString::from("Night Tapes"));
        let by_artist = super::load_albums(&path, tunex_library::AlbumSort::Artist);
        assert_eq!(by_artist[0].artist, QString::from("Nova Rae"));
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
        assert_eq!(rows[0].title, QString::from("Night Tapes"));
        assert_eq!(rows[0].track_count, 2, "group row carries its track count");
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
