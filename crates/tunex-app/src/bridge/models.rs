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

// Row stores live beside their loaders/tests; the bridge aliases below
// resolve through these imports (cxx-qt requires two-segment `super::T`
// paths, so the structs cannot be named across modules directly).
use super::album_list_model::AlbumListModelRust;
use super::artist_list_model::ArtistListModelRust;
use super::library_manager::LibraryManagerRust;
use super::library_track_model::LibraryTrackModelRust;
use super::track_list_model::TrackListModelRust;

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
    }

    /// Roles exposed to QML delegates (`title`, `artist`, `album`,
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

        /// Whether a submitted query is still waiting on the worker.
        #[qinvokable]
        #[cxx_name = "isSearching"]
        fn is_searching(self: &AlbumListModel) -> bool;

        /// Last search failure, or empty when clear.
        #[qinvokable]
        #[cxx_name = "errorText"]
        fn error_text(self: &AlbumListModel) -> QString;

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

        /// Drop all song rows; emits model reset so views rebuild.
        #[qinvokable]
        fn clear(self: Pin<&mut LibraryTrackModel>);

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
    }
}
