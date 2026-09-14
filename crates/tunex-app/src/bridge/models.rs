//! Shared CXX-Qt bridge for the S2 browse models.
//!
//! cxx-qt emits one `QAbstractListModel` downcast shim per bridge module, so
//! sibling bridges subclassing the same Qt base collide at link time
//! (duplicate-symbol on the shim). All browse models therefore share this
//! single bridge; per-model row logic, loaders, and tests stay in their own
//! `*_list_model.rs` files next to the `impl` blocks below.
//!
//! The S1 proof model (`track_list_model`) keeps its own bridge — it was
//! first, and one base declaration per bridge is exactly the supported
//! shape.

// cxx-qt 0.10 forbids any attribute but `doc`/`cxx_qt::bridge` on the bridge
// module itself, so the generated-code exception lives here at file scope:
// the macro's FFI constructor shim must box the Rust struct, and that single
// generated `Box` return is the only `unnecessary_box_returns` hit allowed.
#![allow(
    clippy::unnecessary_box_returns,
    reason = "generated FFI constructor shim"
)]
// Same exception, other direction: the macro generates the connection type
// behind every `#[qsignal]`, and it carries no `Debug`.
#![expect(
    missing_debug_implementations,
    reason = "generated signal connection type"
)]
// Inherited Qt getters (`index`) are declared here and generated as plain
// functions; `#[must_use]` is not among the attributes cxx-qt accepts on
// them, and this file is bridge declarations only.
#![expect(
    clippy::must_use_candidate,
    reason = "generated inherited-method wrappers"
)]

// Row stores live beside their loaders/tests; the bridge aliases below
// resolve through these imports (cxx-qt requires two-segment `super::T`
// paths, so the structs cannot be named across modules directly).
use super::album_list_model::AlbumListModelRust;
use super::artist_list_model::ArtistListModelRust;
use super::facet_list_model::FacetListModelRust;
use super::folder_list_model::FolderListModelRust;
use super::library_manager::LibraryManagerRust;
use super::library_track_model::LibraryTrackModelRust;
use super::playlist_list_model::PlaylistModelRust;
use super::playlist_track_model::PlaylistTrackModelRust;
use super::queue_model::QueueModelRust;
use super::track_list_model::TrackListModelRust;
use super::tray_controller::TrayControllerRust;

/// CXX-Qt bridge for the browse models; mirrors the upstream
/// `custom_base_class` pattern (`#[inherit]` model signals,
/// `#[cxx_override]` virtuals).
#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++Qt" {
        include!(<QtCore/QAbstractListModel>);
        /// Existing C++ base type; subclassed below (no Rust struct needed).
        /// Declared exactly once in this crate — see the module docs.
        #[qobject]
        type QAbstractListModel;
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/qhash.h");
        /// `QHash<i32, QByteArray>` for role names.
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;

        include!("cxx-qt-lib/qmodelindex.h");
        /// `QModelIndex` for row addressing.
        type QModelIndex = cxx_qt_lib::QModelIndex;

        include!("cxx-qt-lib/qstring.h");
        /// `QString` for name/title payloads.
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qvariant.h");
        /// `QVariant` for role data.
        type QVariant = cxx_qt_lib::QVariant;

        include!("cxx-qt-lib/qvector.h");
        /// `QVector<int>`: the changed-roles list carried by `dataChanged`.
        type QVector_i32 = cxx_qt_lib::QVector<i32>;
    }

    /// Roles exposed to QML delegates as `name` / `albumCount` / `trackCount`.
    #[qenum(ArtistListModel)]
    enum ArtistRoles {
        /// Artist name.
        Name,
        /// Attributed album count.
        AlbumCount,
        /// Attributed track count.
        TrackCount,
    }

    /// Roles exposed to QML delegates as `path` / `name` / `trackCount`.
    #[qenum(FolderListModel)]
    enum FolderRoles {
        /// Absolute directory path (drill-down key).
        Path,
        /// Last path component (row title).
        Name,
        /// Indexed tracks living directly in this folder.
        TrackCount,
    }

    /// Roles exposed to QML delegates as `name` / `trackCount` (genres and
    /// composers).
    #[qenum(FacetListModel)]
    enum FacetRoles {
        /// Genre or composer name (`Unknown` when untagged).
        Name,
        /// Indexed tracks in this group.
        TrackCount,
    }

    /// Roles exposed to QML delegates (`albumId`, `title`, `artist`, `year`,
    /// `trackCount`; `year` is 0 when unknown).
    #[qenum(AlbumListModel)]
    enum AlbumRoles {
        /// Album row id (drill-down key into the songs tab).
        AlbumId,
        /// Album title.
        Title,
        /// Attributed artist (`Unknown Artist` when untagged).
        Artist,
        /// Release year (0 when unknown).
        Year,
        /// Indexed track count.
        TrackCount,
        /// Cached cover as a `file://` URL, empty until the lazy resolver
        /// answers and for albums with no artwork (placeholder case).
        ArtUrl,
    }

    /// Roles exposed to QML delegates (`trackId`, `title`, `artist`, `album`,
    /// `trackNumber`, `durationMs`, `missing`; numbers are 0 and names
    /// `Unknown …` when untagged).
    #[qenum(LibraryTrackModel)]
    enum LibraryTrackRoles {
        /// Track title.
        Title,
        /// Track artist.
        Artist,
        /// Album title.
        Album,
        /// Track number within the disc (0 when unknown).
        TrackNumber,
        /// Duration in milliseconds (0 when unknown).
        DurationMs,
        /// File vanished from disk (kept row, badge in the delegate).
        Missing,
        /// Database row id (enqueue identity for row menus).
        TrackId,
    }

    extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        type ArtistListModel = super::ArtistListModelRust;

        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        type AlbumListModel = super::AlbumListModelRust;

        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        type LibraryTrackModel = super::LibraryTrackModelRust;

        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        type FolderListModel = super::FolderListModelRust;

        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        type FacetListModel = super::FacetListModelRust;
    }

    // Base-class model signals. QML connects to `rowsInserted` directly;
    // no Rust-side declaration is needed to *receive* it in QML.
    // Pairing contract: every `begin_reset_model` strictly pairs with
    // `end_reset_model` on a single path (see the per-model `impl`s).
    extern "RustQt" {
        /// # Safety
        ///
        /// Inherited `beginResetModel` for `ArtistListModel`.
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model_artists(self: Pin<&mut ArtistListModel>);
        /// # Safety
        ///
        /// Inherited `endResetModel` for `ArtistListModel`.
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model_artists(self: Pin<&mut ArtistListModel>);

        /// # Safety
        ///
        /// Inherited `beginResetModel` for `AlbumListModel`.
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model_albums(self: Pin<&mut AlbumListModel>);
        /// # Safety
        ///
        /// Inherited `endResetModel` for `AlbumListModel`.
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model_albums(self: Pin<&mut AlbumListModel>);

    }

    // Safe-to-call inherited members live in their own `unsafe extern`
    // block: cxx-qt requires that shape, and mixing them with the
    // begin/end pairs above would claim those are safe too.
    unsafe extern "RustQt" {
        /// Inherited `index`: the model index for one album row, needed to
        /// address a row when its artwork lands.
        #[inherit]
        #[cxx_name = "index"]
        fn index_albums(
            self: &AlbumListModel,
            row: i32,
            column: i32,
            parent: &QModelIndex,
        ) -> QModelIndex;

        /// Inherited `dataChanged`: one row's artwork replaced its
        /// placeholder. A reset would rebuild the whole grid (and lose the
        /// scroll position) for what is one role on one row.
        #[qsignal]
        #[inherit]
        #[cxx_name = "dataChanged"]
        fn data_changed_albums(
            self: Pin<&mut AlbumListModel>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QVector_i32,
        );
    }

    extern "RustQt" {
        /// # Safety
        ///
        /// Inherited `beginResetModel` for `LibraryTrackModel`.
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model_tracks(self: Pin<&mut LibraryTrackModel>);
        /// # Safety
        ///
        /// Inherited `endResetModel` for `LibraryTrackModel`.
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model_tracks(self: Pin<&mut LibraryTrackModel>);

        /// # Safety
        ///
        /// Inherited `beginResetModel` for `FolderListModel`.
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model_folders(self: Pin<&mut FolderListModel>);
        /// # Safety
        ///
        /// Inherited `endResetModel` for `FolderListModel`.
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model_folders(self: Pin<&mut FolderListModel>);

        /// # Safety
        ///
        /// Inherited `beginResetModel` for `FacetListModel`.
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model_facets(self: Pin<&mut FacetListModel>);
        /// # Safety
        ///
        /// Inherited `endResetModel` for `FacetListModel`.
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model_facets(self: Pin<&mut FacetListModel>);
    }

    extern "RustQt" {
        /// Reload all artists from the library index; emits model reset.
        /// An absent index loads zero rows (empty-library state in QML).
        #[qinvokable]
        fn refresh(self: Pin<&mut ArtistListModel>);

        /// Drop all artist rows; emits model reset so views rebuild.
        #[qinvokable]
        fn clear(self: Pin<&mut ArtistListModel>);

        /// Submit raw query text to this model's search worker (debounced,
        /// off-thread; results arrive via `pollSearch`).
        #[qinvokable]
        fn search(self: Pin<&mut ArtistListModel>, query: &QString);

        /// Drain settled search results into rows; emits model reset and
        /// returns true exactly when new rows landed.
        #[qinvokable]
        #[cxx_name = "pollSearch"]
        fn poll_search(self: Pin<&mut ArtistListModel>) -> bool;

        /// Whether a submitted query is still waiting on the worker.
        #[qinvokable]
        #[cxx_name = "isSearching"]
        fn is_searching(self: &ArtistListModel) -> bool;

        /// Last search failure, or empty when clear.
        #[qinvokable]
        #[cxx_name = "errorText"]
        fn error_text(self: &ArtistListModel) -> QString;

        /// Remember an artists-tab sort key and reload. Exposed as `setSort`.
        #[qinvokable]
        #[cxx_name = "setSort"]
        fn set_sort(self: Pin<&mut ArtistListModel>, key: &QString);

        /// Remember the artists-tab sort direction and reload. Exposed as `setSortDescending`.
        #[qinvokable]
        #[cxx_name = "setSortDescending"]
        fn set_sort_descending(self: Pin<&mut ArtistListModel>, descending: bool);

        /// Current artists-tab sort key. Exposed as `sortKey`.
        #[qinvokable]
        #[cxx_name = "sortKey"]
        fn sort_key(self: &ArtistListModel) -> QString;

        /// Display name at `row` (empty when out of range). Exposed as `nameAt`.
        #[qinvokable]
        #[cxx_name = "nameAt"]
        fn name_at(self: &ArtistListModel, row: i32) -> QString;

        /// Row count override for `QAbstractListModel`. The macro-generated
        /// glue forwards `parent`, so it must not be underscore-prefixed here
        /// (the `impl` below still ignores it explicitly).
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count_artists(self: &ArtistListModel, parent: &QModelIndex) -> i32;

        /// Role data override for `QAbstractListModel`.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "data"]
        fn data_artists(self: &ArtistListModel, index: &QModelIndex, role: i32) -> QVariant;

        /// Role-name table override; without it QML sees no custom roles.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names_artists(self: &ArtistListModel) -> QHash_i32_QByteArray;

        /// Reload all albums from the library index; emits model reset.
        /// An absent index loads zero rows (empty-library state in QML).
        #[qinvokable]
        fn refresh(self: Pin<&mut AlbumListModel>);

        /// Drop all album rows; emits model reset so views rebuild.
        #[qinvokable]
        fn clear(self: Pin<&mut AlbumListModel>);

        /// Submit raw query text to this model's search worker (debounced,
        /// off-thread; results arrive via `pollSearch`).
        #[qinvokable]
        fn search(self: Pin<&mut AlbumListModel>, query: &QString);

        /// Drain settled search results into rows; emits model reset and
        /// returns true exactly when new rows landed.
        #[qinvokable]
        #[cxx_name = "pollSearch"]
        fn poll_search(self: Pin<&mut AlbumListModel>) -> bool;

        /// Ask for one row's cover (lazy artwork, S6 W-040).
        /// Exposed to QML as `requestArt`.
        #[qinvokable]
        #[cxx_name = "requestArt"]
        fn request_art(self: Pin<&mut AlbumListModel>, row: i32);

        /// Publish covers that landed; returns true while more are coming,
        /// so the caller's pump can stop on false. Exposed as `pollArt`.
        #[qinvokable]
        #[cxx_name = "pollArt"]
        fn poll_art(self: Pin<&mut AlbumListModel>) -> bool;

        /// Whether a submitted query is still waiting on the worker.
        #[qinvokable]
        #[cxx_name = "isSearching"]
        fn is_searching(self: &AlbumListModel) -> bool;

        /// Last search failure, or empty when clear.
        #[qinvokable]
        #[cxx_name = "errorText"]
        fn error_text(self: &AlbumListModel) -> QString;

        /// Remember an albums-tab sort key and reload. Exposed as `setSort`.
        #[qinvokable]
        #[cxx_name = "setSort"]
        fn set_sort(self: Pin<&mut AlbumListModel>, key: &QString);

        /// Remember the albums-tab sort direction and reload. Exposed as `setSortDescending`.
        #[qinvokable]
        #[cxx_name = "setSortDescending"]
        fn set_sort_descending(self: Pin<&mut AlbumListModel>, descending: bool);

        /// Current albums-tab sort key. Exposed as `sortKey`.
        #[qinvokable]
        #[cxx_name = "sortKey"]
        fn sort_key(self: &AlbumListModel) -> QString;

        /// Database album id at `row` (-1 when out of range). Exposed as `albumIdAt`.
        #[qinvokable]
        #[cxx_name = "albumIdAt"]
        fn album_id_at(self: &AlbumListModel, row: i32) -> i32;

        /// Title at `row` (empty when out of range). Exposed as `titleAt`.
        #[qinvokable]
        #[cxx_name = "titleAt"]
        fn title_at(self: &AlbumListModel, row: i32) -> QString;

        /// Reload albums by one artist, optionally excluding `excludeId`
        /// (`-1` keeps every album). Exposed as `refreshForArtist`.
        #[qinvokable]
        #[cxx_name = "refreshForArtist"]
        fn refresh_for_artist(self: Pin<&mut AlbumListModel>, artist: &QString, exclude_id: i32);

        /// Reload albums from play history, newest first. Exposed as `refreshRecentlyPlayed`.
        #[qinvokable]
        #[cxx_name = "refreshRecentlyPlayed"]
        fn refresh_recently_played(self: Pin<&mut AlbumListModel>);

        /// Row count override for `QAbstractListModel` (see above on `parent`).
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count_albums(self: &AlbumListModel, parent: &QModelIndex) -> i32;

        /// Role data override for `QAbstractListModel`.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "data"]
        fn data_albums(self: &AlbumListModel, index: &QModelIndex, role: i32) -> QVariant;

        /// Role-name table override; without it QML sees no custom roles.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names_albums(self: &AlbumListModel) -> QHash_i32_QByteArray;

        /// Reload the first songs from the library index; emits model reset.
        /// Capped so the songs tab over large libraries stays bounded (the
        /// footer says so); full paging and search arrive in S3.
        #[qinvokable]
        fn refresh(self: Pin<&mut LibraryTrackModel>);

        /// Reload the tracks of one album in disc/track order; emits model
        /// reset. Exposed to QML as `refreshAlbum`.
        #[qinvokable]
        #[cxx_name = "refreshAlbum"]
        fn refresh_album(self: Pin<&mut LibraryTrackModel>, album_id: i32);

        /// Reload tracks whose parent directory is `folder`. Exposed as
        /// `refreshFolder`.
        #[qinvokable]
        #[cxx_name = "refreshFolder"]
        fn refresh_folder(self: Pin<&mut LibraryTrackModel>, folder: &QString);

        /// Reload tracks by one artist. Exposed as `refreshArtist`.
        #[qinvokable]
        #[cxx_name = "refreshArtist"]
        fn refresh_artist(self: Pin<&mut LibraryTrackModel>, artist: &QString);

        /// Reload tracks in one genre group. Exposed as `refreshGenre`.
        #[qinvokable]
        #[cxx_name = "refreshGenre"]
        fn refresh_genre(self: Pin<&mut LibraryTrackModel>, name: &QString);

        /// Reload tracks credited to one composer. Exposed as `refreshComposer`.
        #[qinvokable]
        #[cxx_name = "refreshComposer"]
        fn refresh_composer(self: Pin<&mut LibraryTrackModel>, name: &QString);

        /// Return to the capped songs tab (clears album/folder drill).
        /// Exposed as `refreshSongs`.
        #[qinvokable]
        #[cxx_name = "refreshSongs"]
        fn refresh_songs(self: Pin<&mut LibraryTrackModel>);

        /// Remember a songs-tab sort key and reload when the current browse
        /// honours it. Exposed as `setSort`.
        #[qinvokable]
        #[cxx_name = "setSort"]
        fn set_sort(self: Pin<&mut LibraryTrackModel>, key: &QString);

        /// Remember the songs-tab sort direction and reload when the current
        /// browse honours it. Exposed as `setSortDescending`.
        #[qinvokable]
        #[cxx_name = "setSortDescending"]
        fn set_sort_descending(self: Pin<&mut LibraryTrackModel>, descending: bool);

        /// Current songs-tab sort key. Exposed as `sortKey`.
        #[qinvokable]
        #[cxx_name = "sortKey"]
        fn sort_key(self: &LibraryTrackModel) -> QString;

        /// Drop all song rows; emits model reset so views rebuild.
        #[qinvokable]
        fn clear(self: Pin<&mut LibraryTrackModel>);

        /// Database row id at `row` (-1 when out of range), for row-menu
        /// and keyboard enqueue/play by position.
        /// Exposed to QML as `trackIdAt`.
        #[qinvokable]
        #[cxx_name = "trackIdAt"]
        fn track_id_at(self: &LibraryTrackModel, row: i32) -> i32;
        /// Title at `row` (empty when out of range), for type-to-select.
        /// Exposed to QML as `titleAt`.
        #[qinvokable]
        #[cxx_name = "titleAt"]
        fn title_at(self: &LibraryTrackModel, row: i32) -> QString;
        /// Whether the row at `row` can play (present and on disk).
        /// Exposed to QML as `isPlayableAt`.
        #[qinvokable]
        #[cxx_name = "isPlayableAt"]
        fn is_playable_at(self: &LibraryTrackModel, row: i32) -> bool;

        /// Submit raw query text to this model's search worker (debounced,
        /// off-thread; results arrive via `pollSearch`).
        #[qinvokable]
        fn search(self: Pin<&mut LibraryTrackModel>, query: &QString);

        /// Drain settled search results into rows; emits model reset and
        /// returns true exactly when new rows landed.
        #[qinvokable]
        #[cxx_name = "pollSearch"]
        fn poll_search(self: Pin<&mut LibraryTrackModel>) -> bool;

        /// Whether a submitted query is still waiting on the worker.
        #[qinvokable]
        #[cxx_name = "isSearching"]
        fn is_searching(self: &LibraryTrackModel) -> bool;

        /// Last search failure, or empty when clear.
        #[qinvokable]
        #[cxx_name = "errorText"]
        fn error_text(self: &LibraryTrackModel) -> QString;

        /// Row count override for `QAbstractListModel` (see above on `parent`).
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count_tracks(self: &LibraryTrackModel, parent: &QModelIndex) -> i32;

        /// Role data override for `QAbstractListModel`.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "data"]
        fn data_tracks(self: &LibraryTrackModel, index: &QModelIndex, role: i32) -> QVariant;

        /// Role-name table override; without it QML sees no custom roles.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names_tracks(self: &LibraryTrackModel) -> QHash_i32_QByteArray;

        /// Reload folders from the library index; emits model reset.
        #[qinvokable]
        fn refresh(self: Pin<&mut FolderListModel>);

        /// Drop all folder rows; emits model reset so views rebuild.
        #[qinvokable]
        fn clear(self: Pin<&mut FolderListModel>);

        /// Absolute path at `row` (empty when out of range). Exposed as `pathAt`.
        #[qinvokable]
        #[cxx_name = "pathAt"]
        fn path_at(self: &FolderListModel, row: i32) -> QString;

        /// Display name at `row` (empty when out of range). Exposed as `nameAt`.
        #[qinvokable]
        #[cxx_name = "nameAt"]
        fn name_at(self: &FolderListModel, row: i32) -> QString;

        /// Row count override for `QAbstractListModel`.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count_folders(self: &FolderListModel, parent: &QModelIndex) -> i32;

        /// Role data override for `QAbstractListModel`.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "data"]
        fn data_folders(self: &FolderListModel, index: &QModelIndex, role: i32) -> QVariant;

        /// Role-name table override; without it QML sees no custom roles.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names_folders(self: &FolderListModel) -> QHash_i32_QByteArray;

        /// Reload genre groups. Exposed as `refreshGenres`.
        #[qinvokable]
        #[cxx_name = "refreshGenres"]
        fn refresh_genres(self: Pin<&mut FacetListModel>);

        /// Reload composer groups. Exposed as `refreshComposers`.
        #[qinvokable]
        #[cxx_name = "refreshComposers"]
        fn refresh_composers(self: Pin<&mut FacetListModel>);

        /// Drop all facet rows; emits model reset so views rebuild.
        #[qinvokable]
        fn clear(self: Pin<&mut FacetListModel>);

        /// Display name at `row` (empty when out of range). Exposed as `nameAt`.
        #[qinvokable]
        #[cxx_name = "nameAt"]
        fn name_at(self: &FacetListModel, row: i32) -> QString;

        /// Row count override for `QAbstractListModel`.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count_facets(self: &FacetListModel, parent: &QModelIndex) -> i32;

        /// Role data override for `QAbstractListModel`.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "data"]
        fn data_facets(self: &FacetListModel, index: &QModelIndex, role: i32) -> QVariant;

        /// Role-name table override; without it QML sees no custom roles.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names_facets(self: &FacetListModel) -> QHash_i32_QByteArray;
    }

    /// Roles exposed to QML delegates (`title`, `artist`, `album`,
    /// `durationMs`, `isCurrent`, `trackId`; names `Unknown …` when untagged,
    /// `trackId` -1 for ad-hoc entries).
    #[qenum(QueueModel)]
    enum QueueRoles {
        /// Entry title.
        Title,
        /// Entry artist.
        Artist,
        /// Entry album.
        Album,
        /// Duration in milliseconds (0 when unknown).
        DurationMs,
        /// Currently playing entry (now-playing highlight).
        IsCurrent,
        /// Library row id (-1 for ad-hoc entries).
        TrackId,
        /// File is missing from disk (kept as a dangling Up Next row).
        Missing,
    }

    extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        type QueueModel = super::QueueModelRust;
    }

    extern "RustQt" {
        /// # Safety
        ///
        /// Inherited `beginResetModel` for `QueueModel`.
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model_queue(self: Pin<&mut QueueModel>);
        /// # Safety
        ///
        /// Inherited `endResetModel` for `QueueModel`.
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model_queue(self: Pin<&mut QueueModel>);

        /// # Safety
        ///
        /// Inherited `beginInsertRows` for `QueueModel`. Callers must pair
        /// every call with `end_insert_rows_queue` on all paths, and the rows
        /// must actually appear at `first..=last` before that call.
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        unsafe fn begin_insert_rows_queue(
            self: Pin<&mut QueueModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        /// # Safety
        ///
        /// Inherited `endInsertRows`. Must close a `begin_insert_rows_queue`
        /// pair.
        #[inherit]
        #[cxx_name = "endInsertRows"]
        unsafe fn end_insert_rows_queue(self: Pin<&mut QueueModel>);

        /// # Safety
        ///
        /// Inherited `beginRemoveRows` for `QueueModel`. Callers must pair
        /// every call with `end_remove_rows_queue` on all paths, and the rows
        /// at `first..=last` must actually be gone before that call.
        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        unsafe fn begin_remove_rows_queue(
            self: Pin<&mut QueueModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        /// # Safety
        ///
        /// Inherited `endRemoveRows`. Must close a `begin_remove_rows_queue`
        /// pair.
        #[inherit]
        #[cxx_name = "endRemoveRows"]
        unsafe fn end_remove_rows_queue(self: Pin<&mut QueueModel>);
    }

    extern "RustQt" {
        /// Drain controller events; emits model reset and returns true
        /// exactly when rows changed (length or cursor moved).
        /// Exposed to QML as `poll`.
        #[qinvokable]
        fn poll(self: Pin<&mut QueueModel>) -> bool;

        /// Start or resume playback.
        #[qinvokable]
        fn play(self: Pin<&mut QueueModel>);

        /// Pause, holding position.
        #[qinvokable]
        fn pause(self: Pin<&mut QueueModel>);

        /// Toggle play/pause from the panel transport.
        /// Exposed to QML as `playPause`.
        #[qinvokable]
        #[cxx_name = "playPause"]
        fn play_pause(self: Pin<&mut QueueModel>);

        /// Step to the next track (stopping at a bare end).
        /// Exposed to QML as `nextTrack`.
        #[qinvokable]
        #[cxx_name = "nextTrack"]
        fn next_track(self: Pin<&mut QueueModel>);

        /// Step back, honoring the restart threshold (`position_ms` past it
        /// restarts the current track instead). Exposed as `previousTrack`.
        #[qinvokable]
        #[cxx_name = "previousTrack"]
        fn previous_track(self: Pin<&mut QueueModel>, position_ms: i32);

        /// Play the entry at `index` now (Up Next direct play). Out-of-range
        /// indices are ignored. Exposed to QML as `playAt`.
        #[qinvokable]
        #[cxx_name = "playAt"]
        fn play_at(self: Pin<&mut QueueModel>, index: i32);

        /// Remove the entry at `index` (ignored when out of range).
        /// Exposed to QML as `removeAt`.
        #[qinvokable]
        #[cxx_name = "removeAt"]
        fn remove_at(self: Pin<&mut QueueModel>, index: i32);

        /// Move an entry (ignored when out of range).
        /// Exposed to QML as `moveItem`.
        #[qinvokable]
        #[cxx_name = "moveItem"]
        fn move_item(self: Pin<&mut QueueModel>, from: i32, to: i32);

        /// Move an already-queued entry so it plays next. Exposed as `playNextAt`.
        #[qinvokable]
        #[cxx_name = "playNextAt"]
        fn play_next_at(self: Pin<&mut QueueModel>, index: i32);

        /// Move an entry to the end of Up Next. Exposed as `moveToEnd`.
        #[qinvokable]
        #[cxx_name = "moveToEnd"]
        fn move_to_end(self: Pin<&mut QueueModel>, index: i32);

        /// Empty the queue (the loaded track keeps playing).
        /// Exposed to QML as `clearQueue`.
        #[qinvokable]
        #[cxx_name = "clearQueue"]
        fn clear_queue(self: Pin<&mut QueueModel>);

        /// Drop all rows without touching the controller (view-only reset).
        #[qinvokable]
        fn clear(self: Pin<&mut QueueModel>);

        /// Toggle shuffle; returns the new state.
        /// Exposed to QML as `toggleShuffle`.
        #[qinvokable]
        #[cxx_name = "toggleShuffle"]
        fn toggle_shuffle(self: Pin<&mut QueueModel>) -> bool;

        /// Whether shuffle is on. Exposed to QML as `isShuffle`.
        #[qinvokable]
        #[cxx_name = "isShuffle"]
        fn is_shuffle(self: &QueueModel) -> bool;

        /// Cycle repeat off → all → one; returns the new mode (0/1/2).
        /// Exposed to QML as `cycleRepeat`.
        #[qinvokable]
        #[cxx_name = "cycleRepeat"]
        fn cycle_repeat(self: Pin<&mut QueueModel>) -> i32;

        /// Repeat mode as 0 (off), 1 (all), 2 (one).
        /// Exposed to QML as `repeatMode`.
        #[qinvokable]
        #[cxx_name = "repeatMode"]
        fn repeat_mode(self: &QueueModel) -> i32;

        /// Playback state as 0 (stopped), 1 (loading), 2 (playing), 3 (paused).
        /// Exposed to QML as `playbackState`.
        #[qinvokable]
        #[cxx_name = "playbackState"]
        fn playback_state(self: &QueueModel) -> i32;

        /// Current pipeline position in milliseconds (0 when unknown).
        /// Exposed to QML as `positionMs`.
        #[qinvokable]
        #[cxx_name = "positionMs"]
        fn position_ms(self: &QueueModel) -> i32;

        /// Known duration in milliseconds (engine, else current row, else 0).
        /// Exposed to QML as `durationMs`.
        #[qinvokable]
        #[cxx_name = "durationMs"]
        fn duration_ms(self: &QueueModel) -> i32;

        /// Cached cover of the playing track as a `file://` URL, empty
        /// while it resolves and for tracks without one.
        /// Exposed to QML as `currentArtUrl`.
        #[qinvokable]
        #[cxx_name = "currentArtUrl"]
        fn current_art_url(self: &QueueModel) -> QString;

        /// Title of the playing row (empty when idle).
        /// Exposed to QML as `currentTitle`.
        #[qinvokable]
        #[cxx_name = "currentTitle"]
        fn current_title(self: &QueueModel) -> QString;

        /// Artist of the playing row (empty when idle).
        /// Exposed to QML as `currentArtist`.
        #[qinvokable]
        #[cxx_name = "currentArtist"]
        fn current_artist(self: &QueueModel) -> QString;

        /// Joined lyrics text. Exposed as `lyricsPlain`.
        #[qinvokable]
        #[cxx_name = "lyricsPlain"]
        fn lyrics_plain(self: &QueueModel) -> QString;

        /// Synced lyric count. Exposed as `lyricsLineCount`.
        #[qinvokable]
        #[cxx_name = "lyricsLineCount"]
        fn lyrics_line_count(self: &QueueModel) -> i32;

        /// Lyric text at `row`. Exposed as `lyricsLineAt`.
        #[qinvokable]
        #[cxx_name = "lyricsLineAt"]
        fn lyrics_line_at(self: &QueueModel, row: i32) -> QString;

        /// Lyric time at `row` (`-1` unsynced). Exposed as `lyricsTimeAt`.
        #[qinvokable]
        #[cxx_name = "lyricsTimeAt"]
        fn lyrics_time_at(self: &QueueModel, row: i32) -> i32;

        /// Active synced line. Exposed as `lyricsActiveIndex`.
        #[qinvokable]
        #[cxx_name = "lyricsActiveIndex"]
        fn lyrics_active_index(self: &QueueModel, position_ms: i32) -> i32;

        /// Whether lyrics are synced. Exposed as `lyricsSynced`.
        #[qinvokable]
        #[cxx_name = "lyricsSynced"]
        fn lyrics_synced(self: &QueueModel) -> bool;

        /// Re-read the active profile index. Exposed as `reloadIndex`.
        #[qinvokable]
        #[cxx_name = "reloadIndex"]
        fn reload_index(self: Pin<&mut QueueModel>);

        /// Output volume as 0–100. Exposed as `volumePct`.
        #[qinvokable]
        #[cxx_name = "volumePct"]
        fn volume_pct(self: &QueueModel) -> i32;

        /// Set output volume (clamped 0–100) and persist it.
        /// Exposed to QML as `setVolumePct`.
        #[qinvokable]
        #[cxx_name = "setVolumePct"]
        fn set_volume_pct(self: Pin<&mut QueueModel>, pct: i32);

        /// Whether output is muted. Exposed as `isMuted`.
        #[qinvokable]
        #[cxx_name = "isMuted"]
        fn is_muted(self: &QueueModel) -> bool;

        /// Mute or unmute (independent of the volume level) and persist.
        /// Exposed to QML as `setMuted`.
        #[qinvokable]
        #[cxx_name = "setMuted"]
        fn set_muted(self: Pin<&mut QueueModel>, muted: bool);

        /// Seek to an absolute position in milliseconds. Posts a flush seek
        /// and returns immediately (W-027). Exposed to QML as `seekMs`.
        #[qinvokable]
        #[cxx_name = "seekMs"]
        fn seek_ms(self: Pin<&mut QueueModel>, position_ms: i32);

        /// Opaque-surface preference from config. Exposed as `reduceTransparency`.
        #[qinvokable]
        #[cxx_name = "reduceTransparency"]
        fn reduce_transparency(self: &QueueModel) -> bool;

        /// Instant-motion preference from config. Exposed as `reduceMotion`.
        #[qinvokable]
        #[cxx_name = "reduceMotion"]
        fn reduce_motion(self: &QueueModel) -> bool;

        /// Persist the opaque-surface preference. Exposed as `setReduceTransparency`.
        #[qinvokable]
        #[cxx_name = "setReduceTransparency"]
        fn set_reduce_transparency(self: Pin<&mut QueueModel>, enabled: bool);

        /// Persist the instant-motion preference. Exposed as `setReduceMotion`.
        #[qinvokable]
        #[cxx_name = "setReduceMotion"]
        fn set_reduce_motion(self: Pin<&mut QueueModel>, enabled: bool);

        /// `ReplayGain` mode: 0 off, 1 track, 2 album. Exposed as `replayGainMode`.
        #[qinvokable]
        #[cxx_name = "replayGainMode"]
        fn replaygain_mode(self: &QueueModel) -> i32;

        /// Cycle `ReplayGain` off → track → album. Exposed as `cycleReplayGain`.
        #[qinvokable]
        #[cxx_name = "cycleReplayGain"]
        fn cycle_replaygain(self: Pin<&mut QueueModel>) -> i32;

        /// Crossfade length in seconds (0–12). Exposed as `crossfadeSecs`.
        #[qinvokable]
        #[cxx_name = "crossfadeSecs"]
        fn crossfade_secs(self: &QueueModel) -> i32;

        /// Set crossfade length in seconds. Exposed as `setCrossfadeSecs`.
        #[qinvokable]
        #[cxx_name = "setCrossfadeSecs"]
        fn set_crossfade_secs(self: Pin<&mut QueueModel>, secs: i32);

        /// Current output device id (empty = System). Exposed as `outputDevice`.
        #[qinvokable]
        #[cxx_name = "outputDevice"]
        fn output_device(self: &QueueModel) -> QString;

        /// Refresh the device list on a worker. Exposed as `refreshOutputs`.
        #[qinvokable]
        #[cxx_name = "refreshOutputs"]
        fn refresh_outputs(self: Pin<&mut QueueModel>);

        /// How many output devices are listed. Exposed as `outputCount`.
        #[qinvokable]
        #[cxx_name = "outputCount"]
        fn output_count(self: &QueueModel) -> i32;

        /// Device id at `index`. Exposed as `outputIdAt`.
        #[qinvokable]
        #[cxx_name = "outputIdAt"]
        fn output_id_at(self: &QueueModel, index: i32) -> QString;

        /// Device label at `index`. Exposed as `outputLabelAt`.
        #[qinvokable]
        #[cxx_name = "outputLabelAt"]
        fn output_label_at(self: &QueueModel, index: i32) -> QString;

        /// Cycle the output device. Exposed as `cycleOutput`.
        #[qinvokable]
        #[cxx_name = "cycleOutput"]
        fn cycle_output(self: Pin<&mut QueueModel>) -> i32;

        /// Cursor position (-1 when idle). Exposed as `currentIndex`.
        #[qinvokable]
        #[cxx_name = "currentIndex"]
        fn current_index(self: &QueueModel) -> i32;

        /// Last failure, or empty when clear. Exposed as `errorText`.
        #[qinvokable]
        #[cxx_name = "errorText"]
        fn error_text(self: &QueueModel) -> QString;

        /// Enqueue one library track by row id; returns 1 (0 + error text
        /// when unavailable). Exposed to QML as `enqueueTrack`.
        #[qinvokable]
        #[cxx_name = "enqueueTrack"]
        fn enqueue_track(self: Pin<&mut QueueModel>, track_id: i32) -> i32;

        /// Insert one library track to play next; returns 1 (0 + error text
        /// when unavailable). Exposed to QML as `playTrackNext`.
        #[qinvokable]
        #[cxx_name = "playTrackNext"]
        fn play_track_next(self: Pin<&mut QueueModel>, track_id: i32) -> i32;

        /// Play one library track immediately (inserted after the cursor).
        /// Failures surface through `errorText`. Exposed as `playTrackNow`.
        #[qinvokable]
        #[cxx_name = "playTrackNow"]
        fn play_track_now(self: Pin<&mut QueueModel>, track_id: i32);

        /// Enqueue one album in disc/track order; returns the number
        /// enqueued. Exposed to QML as `enqueueAlbum`.
        #[qinvokable]
        #[cxx_name = "enqueueAlbum"]
        fn enqueue_album(self: Pin<&mut QueueModel>, album_id: i32) -> i32;

        /// Enqueue every playable track in one folder (direct children),
        /// ordered by `sort_key` / `descending`. Exposed as `enqueueFolder`.
        #[qinvokable]
        #[cxx_name = "enqueueFolder"]
        fn enqueue_folder(
            self: Pin<&mut QueueModel>,
            folder: &QString,
            sort_key: &QString,
            descending: bool,
        ) -> i32;

        /// Enqueue one artist in album order; returns the number enqueued.
        /// Exposed to QML as `enqueueArtist`.
        #[qinvokable]
        #[cxx_name = "enqueueArtist"]
        fn enqueue_artist(self: Pin<&mut QueueModel>, artist: &QString) -> i32;

        /// Enqueue one playlist in entry order (dangling and missing entries
        /// skipped); returns the number enqueued.
        /// Exposed to QML as `enqueuePlaylist`.
        #[qinvokable]
        #[cxx_name = "enqueuePlaylist"]
        fn enqueue_playlist(self: Pin<&mut QueueModel>, playlist_id: i32) -> i32;

        /// Row count override for `QAbstractListModel` (see above on `parent`).
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count_queue(self: &QueueModel, parent: &QModelIndex) -> i32;

        /// Role data override for `QAbstractListModel`.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "data"]
        fn data_queue(self: &QueueModel, index: &QModelIndex, role: i32) -> QVariant;

        /// Role-name table override; without it QML sees no custom roles.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names_queue(self: &QueueModel) -> QHash_i32_QByteArray;
    }

    /// Roles exposed to QML delegates (`playlistId`, `name`,
    /// `trackCount`).
    #[qenum(PlaylistModel)]
    enum PlaylistRoles {
        /// Playlist row id (detail key).
        PlaylistId,
        /// Playlist name.
        Name,
        /// Entry count (dangling entries included).
        TrackCount,
    }

    /// Roles exposed to QML delegates (`entryId`, `trackId`, `title`,
    /// `artist`, `durationMs`, `missing`, `dangling`).
    #[qenum(PlaylistTrackModel)]
    enum PlaylistTrackRoles {
        /// Entry row id.
        EntryId,
        /// Linked track row id.
        TrackId,
        /// Entry title (`Unavailable track` when dangling).
        Title,
        /// Entry artist (`Unknown Artist` when untagged).
        Artist,
        /// Duration in milliseconds (0 when unknown).
        DurationMs,
        /// File vanished from disk (badge in the delegate).
        Missing,
        /// Track row gone (placeholder row, distinct badge).
        Dangling,
    }

    extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        type PlaylistModel = super::PlaylistModelRust;

        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        type PlaylistTrackModel = super::PlaylistTrackModelRust;
    }

    extern "RustQt" {
        /// # Safety
        ///
        /// Inherited `beginResetModel` for `PlaylistModel`.
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model_playlists(self: Pin<&mut PlaylistModel>);
        /// # Safety
        ///
        /// Inherited `endResetModel` for `PlaylistModel`.
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model_playlists(self: Pin<&mut PlaylistModel>);

        /// # Safety
        ///
        /// Inherited `beginResetModel` for `PlaylistTrackModel`.
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model_playlist_tracks(self: Pin<&mut PlaylistTrackModel>);
        /// # Safety
        ///
        /// Inherited `endResetModel` for `PlaylistTrackModel`.
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model_playlist_tracks(self: Pin<&mut PlaylistTrackModel>);
    }

    extern "RustQt" {
        /// Reload all playlists; emits model reset.
        #[qinvokable]
        fn refresh(self: Pin<&mut PlaylistModel>);

        /// Drop all rows without touching the index (view-only reset).
        #[qinvokable]
        fn clear(self: Pin<&mut PlaylistModel>);

        /// Create a playlist; returns its id (-1 + `errorText` on failure).
        /// Exposed to QML as `createPlaylist`.
        #[qinvokable]
        #[cxx_name = "createPlaylist"]
        fn create_playlist(self: Pin<&mut PlaylistModel>, name: &QString) -> i32;

        /// Create an auto-named playlist for the row-menu fast path; returns
        /// its id (-1 + `errorText` on failure). Exposed as `createPlaylistAuto`.
        #[qinvokable]
        #[cxx_name = "createPlaylistAuto"]
        fn create_playlist_auto(self: Pin<&mut PlaylistModel>) -> i32;

        /// Create a smart playlist from a stored rule. Exposed as `createSmartPlaylist`.
        #[qinvokable]
        #[cxx_name = "createSmartPlaylist"]
        fn create_smart_playlist(
            self: Pin<&mut PlaylistModel>,
            name: &QString,
            kind: &QString,
            value: &QString,
            exclude_missing: bool,
        ) -> i32;

        /// Whether this playlist rebuilds from a stored rule. Exposed as `isSmart`.
        #[qinvokable]
        #[cxx_name = "isSmart"]
        fn is_smart(self: &PlaylistModel, id: i32) -> bool;

        /// Rename a playlist; failures surface through `errorText`.
        /// Exposed to QML as `renamePlaylist`.
        #[qinvokable]
        #[cxx_name = "renamePlaylist"]
        fn rename_playlist(self: Pin<&mut PlaylistModel>, id: i32, name: &QString);

        /// Delete a playlist and its entries; failures surface via `errorText`.
        /// Exposed to QML as `deletePlaylist`.
        #[qinvokable]
        #[cxx_name = "deletePlaylist"]
        fn delete_playlist(self: Pin<&mut PlaylistModel>, id: i32);

        /// Add one track to a playlist; returns 1 (0 + error text when the
        /// track is missing, gone, or unplayable). Exposed as `addTrack`.
        #[qinvokable]
        #[cxx_name = "addTrack"]
        fn add_track(self: Pin<&mut PlaylistModel>, playlist_id: i32, track_id: i32) -> i32;

        /// Playlist id at `row` (-1 when out of range).
        /// Exposed to QML as `playlistIdAt`.
        #[qinvokable]
        #[cxx_name = "playlistIdAt"]
        fn playlist_id_at(self: &PlaylistModel, row: i32) -> i32;
        /// Playlist name at `row` (empty when out of range).
        /// Exposed to QML as `playlistNameAt`.
        #[qinvokable]
        #[cxx_name = "playlistNameAt"]
        fn playlist_name_at(self: &PlaylistModel, row: i32) -> QString;

        /// Last failure, or empty when clear. Exposed as `errorText`.
        #[qinvokable]
        #[cxx_name = "errorText"]
        fn error_text(self: &PlaylistModel) -> QString;

        /// Re-read the active profile index. Exposed as `reloadIndex`.
        #[qinvokable]
        #[cxx_name = "reloadIndex"]
        fn reload_index(self: Pin<&mut PlaylistModel>);

        /// Row count override for `QAbstractListModel` (see above on `parent`).
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count_playlists(self: &PlaylistModel, parent: &QModelIndex) -> i32;

        /// Role data override for `QAbstractListModel`.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "data"]
        fn data_playlists(self: &PlaylistModel, index: &QModelIndex, role: i32) -> QVariant;

        /// Role-name table override; without it QML sees no custom roles.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names_playlists(self: &PlaylistModel) -> QHash_i32_QByteArray;

        /// Load one playlist's entries in play order; emits model reset.
        /// Exposed to QML as `refreshPlaylist`.
        #[qinvokable]
        #[cxx_name = "refreshPlaylist"]
        fn refresh_playlist(self: Pin<&mut PlaylistTrackModel>, playlist_id: i32);

        /// Drop all rows without touching the index (view-only reset).
        #[qinvokable]
        fn clear(self: Pin<&mut PlaylistTrackModel>);

        /// Remove the entry at `index` (ignored when out of range).
        /// Exposed to QML as `removeAt`.
        #[qinvokable]
        #[cxx_name = "removeAt"]
        fn remove_at(self: Pin<&mut PlaylistTrackModel>, index: i32);

        /// Move the entry at `from` to `to` (clamped, ignored out of range).
        /// Exposed to QML as `moveItem`.
        #[qinvokable]
        #[cxx_name = "moveItem"]
        fn move_item(self: Pin<&mut PlaylistTrackModel>, from: i32, to: i32);

        /// Add one track to the open playlist; returns 1 (0 + error text
        /// when unavailable). Exposed to QML as `addTrack`.
        #[qinvokable]
        #[cxx_name = "addTrack"]
        fn add_track(self: Pin<&mut PlaylistTrackModel>, track_id: i32) -> i32;

        /// Entry id at `row` (-1 when out of range).
        /// Exposed to QML as `entryIdAt`.
        #[qinvokable]
        #[cxx_name = "entryIdAt"]
        fn entry_id_at(self: &PlaylistTrackModel, row: i32) -> i32;

        /// Track id at `row` (-1 when out of range), for play-from-detail.
        /// Exposed to QML as `trackIdAt`.
        #[qinvokable]
        #[cxx_name = "trackIdAt"]
        fn track_id_at(self: &PlaylistTrackModel, row: i32) -> i32;
        /// Whether the row at `row` can play (present, resolved, on disk).
        /// Exposed to QML as `isPlayableAt`.
        #[qinvokable]
        #[cxx_name = "isPlayableAt"]
        fn is_playable_at(self: &PlaylistTrackModel, row: i32) -> bool;

        /// Last failure, or empty when clear. Exposed as `errorText`.
        #[qinvokable]
        #[cxx_name = "errorText"]
        fn error_text(self: &PlaylistTrackModel) -> QString;

        /// Re-read the active profile index. Exposed as `reloadIndex`.
        #[qinvokable]
        #[cxx_name = "reloadIndex"]
        fn reload_index(self: Pin<&mut PlaylistTrackModel>);

        /// Row count override for `QAbstractListModel` (see above on `parent`).
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count_playlist_tracks(self: &PlaylistTrackModel, parent: &QModelIndex) -> i32;

        /// Role data override for `QAbstractListModel`.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "data"]
        fn data_playlist_tracks(
            self: &PlaylistTrackModel,
            index: &QModelIndex,
            role: i32,
        ) -> QVariant;

        /// Role-name table override; without it QML sees no custom roles.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names_playlist_tracks(self: &PlaylistTrackModel) -> QHash_i32_QByteArray;
    }

    /// Roles exposed to QML delegates as `title` / `artist` (S1 bridge
    /// proof; the library-backed songs tab lives in `LibraryTrackModel`).
    #[qenum(TrackListModel)]
    enum TrackListRoles {
        /// Track title.
        Title,
        /// Track artist (`Unknown` when untagged — never fabricated).
        Artist,
    }

    extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        type TrackListModel = super::TrackListModelRust;
    }

    extern "RustQt" {
        /// # Safety
        ///
        /// Inherited `beginInsertRows`. Callers must pair every call with
        /// `end_insert_rows_proof` on all paths, with indices derived from
        /// the row count taken immediately before.
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        unsafe fn begin_insert_rows_proof(
            self: Pin<&mut TrackListModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        /// # Safety
        ///
        /// Inherited `endInsertRows`. Must close a `begin_insert_rows_proof`
        /// pair.
        #[inherit]
        #[cxx_name = "endInsertRows"]
        unsafe fn end_insert_rows_proof(self: Pin<&mut TrackListModel>);

        /// # Safety
        ///
        /// Inherited `beginResetModel`. Callers must pair every call with
        /// `end_reset_model_proof` on all paths.
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model_proof(self: Pin<&mut TrackListModel>);
        /// # Safety
        ///
        /// Inherited `endResetModel`. Must close a `begin_reset_model_proof`
        /// pair.
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model_proof(self: Pin<&mut TrackListModel>);
    }

    extern "RustQt" {
        /// Append one row; emits `rowsInserted` so QML updates live.
        /// Exposed to QML as `appendTrack` (cxx-qt does not auto-convert
        /// snake_case; every multi-word invokable needs `cxx_name`).
        #[qinvokable]
        #[cxx_name = "appendTrack"]
        fn append_track(self: Pin<&mut TrackListModel>, title: QString, artist: QString);

        /// Drop all rows; emits model reset so views rebuild.
        #[qinvokable]
        fn clear(self: Pin<&mut TrackListModel>);

        /// Row count override for `QAbstractListModel`. The macro-generated
        /// glue forwards `parent`, so it must not be underscore-prefixed here
        /// (the `impl` below still ignores it explicitly).
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &TrackListModel, parent: &QModelIndex) -> i32;

        /// Role data override for `QAbstractListModel`.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "data"]
        fn data(self: &TrackListModel, index: &QModelIndex, role: i32) -> QVariant;

        /// Role-name table override; without it QML sees no custom roles.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &TrackListModel) -> QHash_i32_QByteArray;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type LibraryManager = super::LibraryManagerRust;
    }

    extern "RustQt" {
        /// Load folders, start the watcher, and scan.
        #[qinvokable]
        fn startup(self: Pin<&mut LibraryManager>);

        /// Drain progress/watcher channels; start pending scans when idle.
        /// The single call the UI timer makes, at any cadence.
        #[qinvokable]
        fn poll(self: Pin<&mut LibraryManager>);

        /// Request a scan now (queues when busy).
        #[qinvokable]
        fn rescan(self: Pin<&mut LibraryManager>);

        /// Whether a scan worker is currently running.
        #[qinvokable]
        #[cxx_name = "isScanning"]
        fn is_scanning(self: &LibraryManager) -> bool;

        /// One-line status for the progress surface.
        #[qinvokable]
        #[cxx_name = "statusText"]
        fn status_text(self: &LibraryManager) -> QString;

        /// Report a completed run exactly once (views refresh on `true`).
        /// Exposed to QML as `takeFinished`.
        #[qinvokable]
        #[cxx_name = "takeFinished"]
        fn take_finished(self: Pin<&mut LibraryManager>) -> bool;

        /// Configured folder count (drives the folders repeater).
        /// Exposed to QML as `folderCount`.
        #[qinvokable]
        #[cxx_name = "folderCount"]
        fn folder_count(self: &LibraryManager) -> i32;

        /// Configured folder path, or empty when out of range.
        /// Exposed to QML as `folderAt`.
        #[qinvokable]
        #[cxx_name = "folderAt"]
        fn folder_at(self: &LibraryManager, index: i32) -> QString;

        /// Add a folder (canonicalized, persisted, watched, scanned).
        /// Failures surface through `errorText`. Exposed as `addFolder`.
        #[qinvokable]
        #[cxx_name = "addFolder"]
        fn add_folder(self: Pin<&mut LibraryManager>, path: &QString);

        /// Remove a folder (unwatched, unpersisted, rows collected).
        /// Failures surface through `errorText`. Exposed as `removeFolder`.
        #[qinvokable]
        #[cxx_name = "removeFolder"]
        fn remove_folder(self: Pin<&mut LibraryManager>, path: &QString);

        /// Last folder-operation failure, or empty when clear.
        /// Exposed to QML as `errorText`.
        #[qinvokable]
        #[cxx_name = "errorText"]
        fn error_text(self: &LibraryManager) -> QString;

        /// Whether the filesystem watcher is currently active.
        /// Exposed to QML as `isWatching`.
        #[qinvokable]
        #[cxx_name = "isWatching"]
        fn is_watching(self: &LibraryManager) -> bool;

        /// Last Your Library tab. Exposed as `libraryTab`.
        #[qinvokable]
        #[cxx_name = "libraryTab"]
        fn library_tab(self: &LibraryManager) -> QString;

        /// Persist the last Your Library tab. Exposed as `setLibraryTab`.
        #[qinvokable]
        #[cxx_name = "setLibraryTab"]
        fn set_library_tab(self: Pin<&mut LibraryManager>, tab: &QString);

        /// Last songs-tab sort key. Exposed as `songsSort`.
        #[qinvokable]
        #[cxx_name = "songsSort"]
        fn songs_sort(self: &LibraryManager) -> QString;

        /// Persist the songs-tab sort key. Exposed as `setSongsSort`.
        #[qinvokable]
        #[cxx_name = "setSongsSort"]
        fn set_songs_sort(self: Pin<&mut LibraryManager>, key: &QString);

        /// Songs-tab sort descending. Exposed as `songsSortDescending`.
        #[qinvokable]
        #[cxx_name = "songsSortDescending"]
        fn songs_sort_descending(self: &LibraryManager) -> bool;

        /// Persist the songs-tab sort direction. Exposed as `setSongsSortDescending`.
        #[qinvokable]
        #[cxx_name = "setSongsSortDescending"]
        fn set_songs_sort_descending(self: Pin<&mut LibraryManager>, descending: bool);

        /// Last albums-tab sort key. Exposed as `albumsSort`.
        #[qinvokable]
        #[cxx_name = "albumsSort"]
        fn albums_sort(self: &LibraryManager) -> QString;

        /// Persist the albums-tab sort key. Exposed as `setAlbumsSort`.
        #[qinvokable]
        #[cxx_name = "setAlbumsSort"]
        fn set_albums_sort(self: Pin<&mut LibraryManager>, key: &QString);

        /// Albums-tab sort descending. Exposed as `albumsSortDescending`.
        #[qinvokable]
        #[cxx_name = "albumsSortDescending"]
        fn albums_sort_descending(self: &LibraryManager) -> bool;

        /// Persist the albums-tab sort direction. Exposed as `setAlbumsSortDescending`.
        #[qinvokable]
        #[cxx_name = "setAlbumsSortDescending"]
        fn set_albums_sort_descending(self: Pin<&mut LibraryManager>, descending: bool);

        /// Last artists-tab sort key. Exposed as `artistsSort`.
        #[qinvokable]
        #[cxx_name = "artistsSort"]
        fn artists_sort(self: &LibraryManager) -> QString;

        /// Persist the artists-tab sort key. Exposed as `setArtistsSort`.
        #[qinvokable]
        #[cxx_name = "setArtistsSort"]
        fn set_artists_sort(self: Pin<&mut LibraryManager>, key: &QString);

        /// Artists-tab sort descending. Exposed as `artistsSortDescending`.
        #[qinvokable]
        #[cxx_name = "artistsSortDescending"]
        fn artists_sort_descending(self: &LibraryManager) -> bool;

        /// Persist the artists-tab sort direction. Exposed as `setArtistsSortDescending`.
        #[qinvokable]
        #[cxx_name = "setArtistsSortDescending"]
        fn set_artists_sort_descending(self: Pin<&mut LibraryManager>, descending: bool);

        /// Open the parent directory of an indexed track. Exposed as `revealTrack`.
        #[qinvokable]
        #[cxx_name = "revealTrack"]
        fn reveal_track(self: Pin<&mut LibraryManager>, track_id: i32);

        /// Delete one index row. Exposed as `removeTrack`.
        #[qinvokable]
        #[cxx_name = "removeTrack"]
        fn remove_track(self: Pin<&mut LibraryManager>, track_id: i32);

        /// Album title by id (empty when unknown). Exposed as `albumTitle`.
        #[qinvokable]
        #[cxx_name = "albumTitle"]
        fn album_title(self: &LibraryManager, album_id: i32) -> QString;

        /// Album artist by id. Exposed as `albumArtist`.
        #[qinvokable]
        #[cxx_name = "albumArtist"]
        fn album_artist(self: &LibraryManager, album_id: i32) -> QString;

        /// Album year by id (0 when unknown). Exposed as `albumYear`.
        #[qinvokable]
        #[cxx_name = "albumYear"]
        fn album_year(self: &LibraryManager, album_id: i32) -> i32;

        /// Album track count by id. Exposed as `albumTrackCount`.
        #[qinvokable]
        #[cxx_name = "albumTrackCount"]
        fn album_track_count(self: &LibraryManager, album_id: i32) -> i32;

        /// Album duration in milliseconds. Exposed as `albumDurationMs`.
        #[qinvokable]
        #[cxx_name = "albumDurationMs"]
        fn album_duration_ms(self: &LibraryManager, album_id: i32) -> i32;

        /// Artist album count. Exposed as `artistAlbumCount`.
        #[qinvokable]
        #[cxx_name = "artistAlbumCount"]
        fn artist_album_count(self: &LibraryManager, name: &QString) -> i32;

        /// Artist track count. Exposed as `artistTrackCount`.
        #[qinvokable]
        #[cxx_name = "artistTrackCount"]
        fn artist_track_count(self: &LibraryManager, name: &QString) -> i32;

        /// Remember a search query (newest first, cap 10). Exposed as `rememberSearch`.
        #[qinvokable]
        #[cxx_name = "rememberSearch"]
        fn remember_search(self: Pin<&mut LibraryManager>, query: &QString);

        /// How many recent searches are stored. Exposed as `recentSearchCount`.
        #[qinvokable]
        #[cxx_name = "recentSearchCount"]
        fn recent_search_count(self: &LibraryManager) -> i32;

        /// Recent search at `index` (0 = newest). Exposed as `recentSearchAt`.
        #[qinvokable]
        #[cxx_name = "recentSearchAt"]
        fn recent_search_at(self: &LibraryManager, index: i32) -> QString;

        /// One tag field for the editor (`title` / `artist` / `album` / `genre` /
        /// `composer` / `year` / `track` / `disc`). Exposed as `trackValue`.
        #[qinvokable]
        #[cxx_name = "trackValue"]
        fn track_value(self: &LibraryManager, track_id: i32, key: &QString) -> QString;

        /// Write tags then re-index that path. `packed` is eight fields joined
        /// by U+001F (title, artist, album, track, disc, year, genre, composer).
        /// Exposed as `saveTags`.
        #[qinvokable]
        #[cxx_name = "saveTags"]
        fn save_tags(self: Pin<&mut LibraryManager>, track_id: i32, packed: &QString) -> i32;

        /// Active profile id (empty = default). Exposed as `activeProfile`.
        #[qinvokable]
        #[cxx_name = "activeProfile"]
        fn active_profile(self: &LibraryManager) -> QString;

        /// How many profiles (including Default). Exposed as `profileCount`.
        #[qinvokable]
        #[cxx_name = "profileCount"]
        fn profile_count(self: &LibraryManager) -> i32;

        /// Profile id at `index`. Exposed as `profileIdAt`.
        #[qinvokable]
        #[cxx_name = "profileIdAt"]
        fn profile_id_at(self: &LibraryManager, index: i32) -> QString;

        /// Profile name at `index`. Exposed as `profileNameAt`.
        #[qinvokable]
        #[cxx_name = "profileNameAt"]
        fn profile_name_at(self: &LibraryManager, index: i32) -> QString;

        /// Create a named profile. Exposed as `createProfile`.
        #[qinvokable]
        #[cxx_name = "createProfile"]
        fn create_profile(self: Pin<&mut LibraryManager>, name: &QString) -> QString;

        /// Switch the active profile and reopen its index. Exposed as `switchProfile`.
        #[qinvokable]
        #[cxx_name = "switchProfile"]
        fn switch_profile(self: Pin<&mut LibraryManager>, id: &QString);

        /// `MusicBrainz` opt-in (default off). Exposed as `musicbrainzOn`.
        #[qinvokable]
        #[cxx_name = "musicbrainzOn"]
        fn musicbrainz_on(self: &LibraryManager) -> bool;

        /// Persist the `MusicBrainz` opt-in. Exposed as `setMusicbrainzOn`.
        #[qinvokable]
        #[cxx_name = "setMusicbrainzOn"]
        fn set_musicbrainz_on(self: Pin<&mut LibraryManager>, on: bool);
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type TrayController = super::TrayControllerRust;
    }

    unsafe extern "RustQt" {
        /// Compact glass popup next to the tray icon.
        #[qsignal]
        #[cxx_name = "popupRequested"]
        fn popup_requested(self: Pin<&mut TrayController>, x: i32, y: i32);

        /// Right-click menu next to the tray icon.
        #[qsignal]
        #[cxx_name = "menuRequested"]
        fn menu_requested(self: Pin<&mut TrayController>, x: i32, y: i32);

        /// Middle-click play/pause from the tray host.
        #[qsignal]
        #[cxx_name = "playPauseRequested"]
        fn play_pause_requested(self: Pin<&mut TrayController>);
    }

    extern "RustQt" {
        /// Drain tray-host events onto QML signals.
        #[qinvokable]
        fn poll(self: Pin<&mut TrayController>);

        /// Push now-playing copy into the SNI tooltip.
        /// Exposed to QML as `setNowPlaying`.
        #[qinvokable]
        #[cxx_name = "setNowPlaying"]
        fn set_now_playing(
            self: Pin<&mut TrayController>,
            title: &QString,
            artist: &QString,
            playing: bool,
        );

        /// Whether a tray host accepted the icon.
        /// Exposed to QML as `isAvailable`.
        #[qinvokable]
        #[cxx_name = "isAvailable"]
        fn is_available(self: &TrayController) -> bool;

        /// Persisted close-to-tray preference. Exposed as `closeToTray`.
        #[qinvokable]
        #[cxx_name = "closeToTray"]
        fn close_to_tray(self: &TrayController) -> bool;

        /// Persist close-to-tray. Exposed as `setCloseToTray`.
        #[qinvokable]
        #[cxx_name = "setCloseToTray"]
        fn set_close_to_tray(self: Pin<&mut TrayController>, enabled: bool);

        /// Whether this close should hide instead of quit.
        /// Exposed to QML as `hideOnClose`.
        #[qinvokable]
        #[cxx_name = "hideOnClose"]
        fn hide_on_close(self: &TrayController) -> bool;

        /// Restored window width. Exposed as `windowWidth`.
        #[qinvokable]
        #[cxx_name = "windowWidth"]
        fn window_width(self: &TrayController) -> i32;

        /// Restored window height. Exposed as `windowHeight`.
        #[qinvokable]
        #[cxx_name = "windowHeight"]
        fn window_height(self: &TrayController) -> i32;

        /// Whether a previous session saved x/y. Exposed as `hasWindowPosition`.
        #[qinvokable]
        #[cxx_name = "hasWindowPosition"]
        fn has_window_position(self: &TrayController) -> bool;

        /// Last saved window x. Exposed as `windowX`.
        #[qinvokable]
        #[cxx_name = "windowX"]
        fn window_x(self: &TrayController) -> i32;

        /// Last saved window y. Exposed as `windowY`.
        #[qinvokable]
        #[cxx_name = "windowY"]
        fn window_y(self: &TrayController) -> i32;

        /// Persist window geometry. Exposed as `setWindowGeometry`.
        #[qinvokable]
        #[cxx_name = "setWindowGeometry"]
        fn set_window_geometry(
            self: Pin<&mut TrayController>,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
        );
    }
}
