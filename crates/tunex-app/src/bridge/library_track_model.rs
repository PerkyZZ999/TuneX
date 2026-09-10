//! `LibraryTrackModel` rows and loaders (bridge in [`super::models`]).
//!
//! S2 W-016 browse model: the songs tab (`refresh`, capped) and the album
//! drill-down (`refresh_album`) both load from the library index through
//! bounded, indexed queries — no scan, no decode, no watcher on this path.
//! A missing index file is not an error: the model stays empty and QML shows
//! the empty-library state.
//!
//! All row logic lives on the plain [`LibraryTrackModelRust`] struct (fully
//! unit-tested); the `impl` below only pairs Qt model notifications around
//! it (see the S1 proof model for the pairing pattern).

use super::models::qobject;

use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

use crate::search::SearchCore;

/// Human-readable `Debug` for the generated roles enum (`#[derive]` is not
/// permitted on `#[qenum]` items, so this is written by hand).
impl std::fmt::Debug for qobject::LibraryTrackRoles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Variants are associated constants on the generated type; compare
        // their discriminants rather than matching by value.
        let name = match self.repr {
            repr if repr == qobject::LibraryTrackRoles::Title.repr => "Title",
            repr if repr == qobject::LibraryTrackRoles::Artist.repr => "Artist",
            repr if repr == qobject::LibraryTrackRoles::Album.repr => "Album",
            repr if repr == qobject::LibraryTrackRoles::TrackNumber.repr => "TrackNumber",
            repr if repr == qobject::LibraryTrackRoles::DurationMs.repr => "DurationMs",
            repr if repr == qobject::LibraryTrackRoles::Missing.repr => "Missing",
            repr if repr == qobject::LibraryTrackRoles::TrackId.repr => "TrackId",
            _ => "Unknown",
        };
        write!(f, "LibraryTrackRoles::{name}")
    }
}

/// Songs-tab cap: the view stays virtualized and bounded; S3 adds paging.
pub const SONGS_CAP: u32 = 500;

/// Song row store: display strings plus numeric roles.
#[derive(Debug, Default)]
pub struct LibraryTrackModelRust {
    tracks: Vec<(i32, QString, QString, QString, i32, i32, bool)>,
    search: SearchCore,
}

/// One settled search result set as display rows.
type TrackSearchRows = Vec<(i32, QString, QString, QString, i32, i32, bool)>;

impl LibraryTrackModelRust {
    /// Drop all song rows; emits model reset so views rebuild.
    fn drop_rows(&mut self) {
        self.tracks.clear();
    }

    /// Database row id at `row` (-1 when out of range), for row-menu and
    /// keyboard enqueue/play by position.
    fn track_id_at(&self, row: i32) -> i32 {
        usize::try_from(row)
            .ok()
            .and_then(|index| self.tracks.get(index))
            .map_or(-1, |entry| entry.0)
    }

    /// Current row count.
    fn row_count(&self) -> i32 {
        i32::try_from(self.tracks.len()).unwrap_or(i32::MAX)
    }

    /// Replace every row (single reset around the caller).
    fn replace_rows(&mut self, rows: Vec<(i32, QString, QString, QString, i32, i32, bool)>) {
        self.tracks = rows;
    }

    /// Submit raw query text to the debounced search worker.
    fn submit_search(&mut self, query: &str) {
        self.search.submit(query);
    }

    /// Drain one settled result set into row payloads (`None` while the
    /// worker runs or idles — the caller refreshes views on `Some`).
    fn poll_search_rows(&mut self) -> Option<TrackSearchRows> {
        self.search.poll();
        self.search
            .take_results()
            .map(|found| found.tracks.iter().map(display_row).collect())
    }

    /// Whether a submitted query is still waiting on the worker.
    fn is_searching(&self) -> bool {
        self.search.is_searching()
    }

    /// Last search failure, if any.
    fn search_error(&self) -> Option<String> {
        self.search.error_text()
    }

    /// Data for one row/role; invalid variant when out of range or the role
    /// is unknown.
    fn row_data(&self, row: usize, role: qobject::LibraryTrackRoles) -> QVariant {
        if let Some((track_id, title, artist, album, track_number, duration_ms, missing)) =
            self.tracks.get(row)
        {
            return match role {
                qobject::LibraryTrackRoles::Title => QVariant::from(title),
                qobject::LibraryTrackRoles::Artist => QVariant::from(artist),
                qobject::LibraryTrackRoles::Album => QVariant::from(album),
                qobject::LibraryTrackRoles::TrackNumber => QVariant::from(track_number),
                qobject::LibraryTrackRoles::DurationMs => QVariant::from(duration_ms),
                qobject::LibraryTrackRoles::Missing => QVariant::from(missing),
                qobject::LibraryTrackRoles::TrackId => QVariant::from(track_id),
                _ => QVariant::default(),
            };
        }
        QVariant::default()
    }
}

/// Display mapping shared by every loader: unknowns stay visible as such,
/// numbers saturate into `i32`.
fn display_row(row: &tunex_library::TrackRow) -> (i32, QString, QString, QString, i32, i32, bool) {
    (
        i32::try_from(row.id).unwrap_or(i32::MAX),
        QString::from(row.title.as_deref().unwrap_or("Unknown Title")),
        QString::from(row.artist.as_deref().unwrap_or("Unknown Artist")),
        QString::from(row.album.as_deref().unwrap_or("Unknown Album")),
        row.track_number
            .and_then(|number| i32::try_from(number).ok())
            .unwrap_or(0),
        row.duration_ms
            .and_then(|duration| i32::try_from(duration).ok())
            .unwrap_or(0),
        row.missing,
    )
}

/// Load song rows from the index at `path`: `None` album means the capped
/// songs tab, `Some(id)` means that album in disc/track order. An absent or
/// unreadable index loads zero rows (the empty-library state in QML).
fn load_tracks(
    path: &std::path::Path,
    album: Option<i64>,
) -> Vec<(i32, QString, QString, QString, i32, i32, bool)> {
    if !path.is_file() {
        return Vec::new();
    }
    let db = match tunex_library::open_file(path) {
        Ok(db) => db,
        Err(err) => {
            tracing::warn!(name = "browse.tracks_failed", error = %err, "index unreadable");
            return Vec::new();
        }
    };
    let rows = match album {
        Some(album_id) => tunex_library::list_tracks_in_album(&db, album_id),
        None => tunex_library::list_tracks_capped(&db, SONGS_CAP),
    };
    match rows {
        Ok(rows) => rows.iter().map(display_row).collect(),
        Err(err) => {
            tracing::warn!(name = "browse.tracks_failed", error = %err, "index unreadable");
            Vec::new()
        }
    }
}

impl qobject::LibraryTrackModel {
    /// Reload the capped songs tab; emits model reset.
    pub fn refresh(mut self: Pin<&mut Self>) {
        let rows = load_tracks(&tunex_core::library_db_path(), None);
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_tracks();
            self.as_mut().rust_mut().replace_rows(rows);
            self.as_mut().end_reset_model_tracks();
        }
    }

    /// Reload one album's tracks in disc/track order; emits model reset.
    pub fn refresh_album(mut self: Pin<&mut Self>, album_id: i32) {
        let rows = load_tracks(&tunex_core::library_db_path(), Some(i64::from(album_id)));
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_tracks();
            self.as_mut().rust_mut().replace_rows(rows);
            self.as_mut().end_reset_model_tracks();
        }
    }

    /// Drop all rows; emits model reset so views rebuild.
    pub fn clear(mut self: Pin<&mut Self>) {
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model_tracks();
            self.as_mut().rust_mut().drop_rows();
            self.as_mut().end_reset_model_tracks();
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
            self.as_mut().begin_reset_model_tracks();
            self.as_mut().rust_mut().replace_rows(rows);
            self.as_mut().end_reset_model_tracks();
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

    /// Database row id at `row` (-1 when out of range).
    pub fn track_id_at(&self, row: i32) -> i32 {
        self.rust().track_id_at(row)
    }

    /// Row count override for `QAbstractListModel`.
    pub fn row_count_tracks(&self, _parent: &QModelIndex) -> i32 {
        self.rust().row_count()
    }

    /// Role data override for `QAbstractListModel`.
    pub fn data_tracks(&self, index: &QModelIndex, role: i32) -> QVariant {
        let row = usize::try_from(index.row()).unwrap_or(usize::MAX);
        self.rust()
            .row_data(row, qobject::LibraryTrackRoles { repr: role })
    }

    /// Role-name table override; without it QML sees no custom roles.
    /// The Qt virtual signature requires the receiver although no instance
    /// state participates.
    pub fn role_names_tracks(&self) -> QHash<QHashPair_i32_QByteArray> {
        let _ = self;
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(
            qobject::LibraryTrackRoles::Title.repr,
            QByteArray::from("title"),
        );
        roles.insert(
            qobject::LibraryTrackRoles::Artist.repr,
            QByteArray::from("artist"),
        );
        roles.insert(
            qobject::LibraryTrackRoles::Album.repr,
            QByteArray::from("album"),
        );
        roles.insert(
            qobject::LibraryTrackRoles::TrackNumber.repr,
            QByteArray::from("trackNumber"),
        );
        roles.insert(
            qobject::LibraryTrackRoles::DurationMs.repr,
            QByteArray::from("durationMs"),
        );
        roles.insert(
            qobject::LibraryTrackRoles::Missing.repr,
            QByteArray::from("missing"),
        );
        roles.insert(
            qobject::LibraryTrackRoles::TrackId.repr,
            QByteArray::from("trackId"),
        );
        roles
    }
}

#[cfg(test)]
mod tests {
    use super::LibraryTrackModelRust;
    use super::qobject::LibraryTrackRoles;
    use crate::bridge::test_support::seeded_index;
    use crate::search::SearchCore;
    use cxx_qt_lib::{QString, QVariant};
    use std::time::{Duration, Instant};

    /// Poll until one result set settles (tests end idle so the joined
    /// worker never outlives its scratch dir).
    fn settle_search(model: &mut LibraryTrackModelRust) -> super::TrackSearchRows {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(rows) = model.poll_search_rows() {
                return rows;
            }
            assert!(Instant::now() < deadline, "search never settled");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn model_with_two_songs() -> LibraryTrackModelRust {
        let mut model = LibraryTrackModelRust::default();
        model.replace_rows(vec![
            (
                7,
                QString::from("Midnight"),
                QString::from("Nova Rae"),
                QString::from("Night Tapes"),
                3,
                273_000,
                false,
            ),
            (
                9,
                QString::from("Solace"),
                QString::from("Nove"),
                QString::from("Night Tapes"),
                4,
                195_000,
                true,
            ),
        ]);
        model
    }

    #[test]
    fn replace_rows_sets_sequential_content() {
        let mut model = LibraryTrackModelRust::default();
        model.replace_rows(vec![(
            1,
            QString::from("A"),
            QString::from("B"),
            QString::from("C"),
            1,
            1000,
            false,
        )]);
        assert_eq!(model.row_count(), 1);
        model.replace_rows(vec![]);
        assert_eq!(model.row_count(), 0);
    }

    #[test]
    fn row_count_tracks_push_and_drop() {
        let mut model = model_with_two_songs();
        assert_eq!(model.row_count(), 2);
        model.drop_rows();
        assert_eq!(model.row_count(), 0);
    }

    #[test]
    fn row_data_returns_every_role() {
        let model = model_with_two_songs();
        assert_ne!(
            model.row_data(0, LibraryTrackRoles::Title),
            QVariant::default()
        );
        assert_ne!(
            model.row_data(0, LibraryTrackRoles::Artist),
            QVariant::default()
        );
        assert_ne!(
            model.row_data(0, LibraryTrackRoles::Album),
            QVariant::default()
        );
        assert_ne!(
            model.row_data(0, LibraryTrackRoles::TrackNumber),
            QVariant::default()
        );
        assert_ne!(
            model.row_data(0, LibraryTrackRoles::DurationMs),
            QVariant::default()
        );
        assert_ne!(
            model.row_data(1, LibraryTrackRoles::Missing),
            QVariant::default()
        );
        assert_ne!(
            model.row_data(0, LibraryTrackRoles::TrackId),
            QVariant::default()
        );
    }

    #[test]
    fn row_data_for_unknown_role_yields_default() {
        let model = model_with_two_songs();
        assert_eq!(
            model.row_data(0, LibraryTrackRoles { repr: i32::MAX }),
            QVariant::default()
        );
    }

    #[test]
    fn row_data_out_of_range_yields_default() {
        let model = model_with_two_songs();
        assert_eq!(
            model.row_data(99, LibraryTrackRoles::Title),
            QVariant::default()
        );
    }

    #[test]
    fn track_id_at_resolves_rows_or_negative() {
        let model = model_with_two_songs();
        assert_eq!(model.track_id_at(0), 7);
        assert_eq!(model.track_id_at(1), 9);
        assert_eq!(model.track_id_at(99), -1);
        assert_eq!(model.track_id_at(-1), -1);
    }

    #[test]
    fn missing_index_loads_zero_rows() {
        let missing =
            std::env::temp_dir().join(format!("tunex-browse-missing-{}", std::process::id()));
        assert!(super::load_tracks(&missing.join("library.db"), None).is_empty());
    }

    #[test]
    fn seeded_index_loads_song_rows() {
        let (_guard, path) = seeded_index("tracks");
        let rows = super::load_tracks(&path, None);
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn search_settles_matching_rows() {
        let (_guard, path) = seeded_index("search-tracks");
        let mut model = LibraryTrackModelRust {
            search: SearchCore::with_db_path(path),
            ..Default::default()
        };
        model.search.set_debounce(Duration::from_millis(10));
        model.submit_search("nova");
        assert!(model.is_searching());
        let rows = settle_search(&mut model);
        assert_eq!(rows.len(), 2);
        let mut titles: Vec<String> = rows.iter().map(|row| row.1.to_string()).collect();
        titles.sort();
        assert_eq!(titles, ["One", "Two"]);
        assert!(!model.is_searching());
        assert!(
            model.poll_search_rows().is_none(),
            "results deliver exactly once"
        );
    }

    #[test]
    fn empty_query_searches_nothing() {
        let (_guard, path) = seeded_index("search-tracks-empty");
        let mut model = LibraryTrackModelRust {
            search: SearchCore::with_db_path(path),
            ..Default::default()
        };
        model.search.set_debounce(Duration::from_millis(10));
        model.submit_search("");
        assert!(settle_search(&mut model).is_empty());
        assert!(model.search_error().is_none());
    }

    #[test]
    fn display_row_keeps_unknowns_visible() {
        let row = tunex_library::TrackRow {
            id: 1,
            path: "/music/x.flac".to_owned(),
            title: None,
            stable_key: "k".to_owned(),
            artist: None,
            album: None,
            genre: None,
            composer: None,
            year: None,
            track_number: None,
            disc_number: None,
            duration_ms: None,
            missing: true,
        };
        let (track_id, title, artist, album, number, duration, missing) = super::display_row(&row);
        assert_eq!(track_id, 1);
        assert_eq!(title, QString::from("Unknown Title"));
        assert_eq!(artist, QString::from("Unknown Artist"));
        assert_eq!(album, QString::from("Unknown Album"));
        assert_eq!((number, duration, missing), (0, 0, true));
    }
}
