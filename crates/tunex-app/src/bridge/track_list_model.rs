//! `TrackListModel`: `QAbstractListModel` subclass bridging track rows into QML.
//!
//! S1 W-002 bridge proof: an in-memory row store with `append_track` / `clear`
//! invokables, `rowCount` / `data` / `roleNames` overrides, and the base-class
//! `rowsInserted` signal carrying Rust → QML notifications. The library-backed
//! store (S2/S3) replaces the `Vec` without touching this interface.
//!
//! Design note: all row logic lives on the plain [`TrackListModelRust`] struct
//! so unit tests exercise behavior without instantiating C++ objects. The
//! `impl` blocks on the generated type are thin `begin/end` pairing wrappers,
//! verified through the running UI instead.

// cxx-qt 0.10 forbids any attribute but `doc`/`cxx_qt::bridge` on the bridge
// module itself, so the generated-code exception lives here at file scope:
// the macro's FFI constructor shim must box the Rust struct, and that single
// generated `Box` return is the only `unnecessary_box_returns` hit allowed.
#![allow(
    clippy::unnecessary_box_returns,
    reason = "generated FFI constructor shim"
)]

/// CXX-Qt bridge for [`TrackListModelRust`]; mirrors the upstream
/// `custom_base_class` pattern (`#[inherit]` model signals,
/// `#[cxx_override]` virtuals).
#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++Qt" {
        include!(<QtCore/QAbstractListModel>);
        /// Existing C++ base type; subclassed below (no Rust struct needed).
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
        /// `QString` for title/artist payloads.
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qvariant.h");
        /// `QVariant` for role data.
        type QVariant = cxx_qt_lib::QVariant;
    }

    /// Roles exposed to QML delegates as `title` / `artist`.
    #[qenum(TrackListModel)]
    enum Roles {
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

    // Base-class model signals. QML connects to `rowsInserted` directly;
    // no Rust-side declaration is needed to *receive* it in QML.
    extern "RustQt" {
        /// # Safety
        ///
        /// Inherited `beginInsertRows`. Callers must pair every call with
        /// `end_insert_rows` on all paths, with indices derived from the row
        /// count taken immediately before.
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        unsafe fn begin_insert_rows(
            self: Pin<&mut TrackListModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        /// # Safety
        ///
        /// Inherited `endInsertRows`. Must close a `begin_insert_rows` pair.
        #[inherit]
        #[cxx_name = "endInsertRows"]
        unsafe fn end_insert_rows(self: Pin<&mut TrackListModel>);

        /// # Safety
        ///
        /// Inherited `beginResetModel`. Callers must pair every call with
        /// `end_reset_model` on all paths.
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model(self: Pin<&mut TrackListModel>);
        /// # Safety
        ///
        /// Inherited `endResetModel`. Must close a `begin_reset_model` pair.
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model(self: Pin<&mut TrackListModel>);
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
        fn data(self: &TrackListModel, index: &QModelIndex, role: i32) -> QVariant;

        /// Role-name table override; without it QML sees no custom roles.
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &TrackListModel) -> QHash_i32_QByteArray;
    }
}

use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QModelIndex, QString, QVariant};

/// Human-readable `Debug` for the generated roles enum (`#[derive]` is not
/// permitted on `#[qenum]` items, so this is written by hand).
impl std::fmt::Debug for qobject::Roles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Variants are associated constants on the generated type; compare
        // their discriminants rather than matching by value.
        let name = match self.repr {
            repr if repr == qobject::Roles::Title.repr => "Title",
            repr if repr == qobject::Roles::Artist.repr => "Artist",
            _ => "Unknown",
        };
        write!(f, "Roles::{name}")
    }
}

/// In-memory row store. Replaced by the library-backed store in S2/S3.
///
/// All behavior lives here (plain Rust, fully unit-tested); the generated
/// type's `impl` below only pairs Qt model notifications around it.
#[derive(Debug, Default)]
pub struct TrackListModelRust {
    tracks: Vec<(QString, QString)>,
}

impl TrackListModelRust {
    /// Push one row; returns its index (saturates instead of wrapping on
    /// absurd lengths — a view count, never an allocation index).
    fn push_row(&mut self, title: QString, artist: QString) -> i32 {
        let row = i32::try_from(self.tracks.len()).unwrap_or(i32::MAX);
        self.tracks.push((title, artist));
        row
    }

    /// Drop all rows.
    fn drop_rows(&mut self) {
        self.tracks.clear();
    }

    /// Current row count.
    fn row_count(&self) -> i32 {
        i32::try_from(self.tracks.len()).unwrap_or(i32::MAX)
    }

    /// Data for one row/role; invalid variant when out of range or the role
    /// is unknown.
    fn row_data(&self, row: usize, role: qobject::Roles) -> QVariant {
        if let Some((title, artist)) = self.tracks.get(row) {
            return match role {
                qobject::Roles::Title => QVariant::from(title),
                qobject::Roles::Artist => QVariant::from(artist),
                _ => QVariant::default(),
            };
        }
        QVariant::default()
    }
}

impl qobject::TrackListModel {
    /// Append one row; emits `rowsInserted` so QML updates live.
    pub fn append_track(mut self: Pin<&mut Self>, title: QString, artist: QString) {
        let row = self.as_mut().rust_mut().row_count();
        // SAFETY: `begin_insert_rows`/`end_insert_rows` strictly pair on this
        // single path, and the range covers exactly the pushed row.
        unsafe {
            self.as_mut()
                .begin_insert_rows(&QModelIndex::default(), row, row);
            let pushed = self.as_mut().rust_mut().push_row(title, artist);
            debug_assert_eq!(pushed, row);
            self.as_mut().end_insert_rows();
        }
    }

    /// Drop all rows; emits model reset so views rebuild.
    pub fn clear(mut self: Pin<&mut Self>) {
        // SAFETY: reset pair strictly paired on this single path.
        unsafe {
            self.as_mut().begin_reset_model();
            self.as_mut().rust_mut().drop_rows();
            self.as_mut().end_reset_model();
        }
    }

    /// Row count override for `QAbstractListModel`.
    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.rust().row_count()
    }

    /// Role data override for `QAbstractListModel`.
    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let row = usize::try_from(index.row()).unwrap_or(usize::MAX);
        self.rust().row_data(row, qobject::Roles { repr: role })
    }

    /// Role-name table override; without it QML sees no custom roles.
    /// The Qt virtual signature requires the receiver although no instance
    /// state participates.
    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let _ = self;
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(qobject::Roles::Title.repr, QByteArray::from("title"));
        roles.insert(qobject::Roles::Artist.repr, QByteArray::from("artist"));
        roles
    }
}

#[cfg(test)]
mod tests {
    use super::TrackListModelRust;
    use super::qobject::Roles;
    use cxx_qt_lib::{QString, QVariant};

    fn model_with_two_tracks() -> TrackListModelRust {
        let mut model = TrackListModelRust::default();
        model.push_row(QString::from("Midnight"), QString::from("Nova Rae"));
        model.push_row(QString::from("Solace"), QString::from("Nove"));
        model
    }

    #[test]
    fn push_row_returns_sequential_indices() {
        let mut model = TrackListModelRust::default();
        assert_eq!(model.push_row(QString::from("A"), QString::from("B")), 0);
        assert_eq!(model.push_row(QString::from("C"), QString::from("D")), 1);
    }

    #[test]
    fn row_count_tracks_push_and_drop() {
        let mut model = model_with_two_tracks();
        assert_eq!(model.row_count(), 2);
        model.drop_rows();
        assert_eq!(model.row_count(), 0);
    }

    #[test]
    fn row_data_returns_title_and_artist() {
        let model = model_with_two_tracks();
        assert_ne!(model.row_data(1, Roles::Title), QVariant::default());
        assert_ne!(model.row_data(1, Roles::Artist), QVariant::default());
    }

    #[test]
    fn row_data_for_unknown_role_yields_default() {
        let model = model_with_two_tracks();
        assert_eq!(
            model.row_data(0, Roles { repr: i32::MAX }),
            QVariant::default()
        );
    }

    #[test]
    fn row_data_out_of_range_yields_default() {
        let model = model_with_two_tracks();
        assert_eq!(model.row_data(99, Roles::Title), QVariant::default());
    }

    #[test]
    fn roles_debug_names_known_and_unknown_variants() {
        assert_eq!(
            format!(
                "{:?}",
                Roles {
                    repr: Roles::Title.repr
                }
            ),
            "Roles::Title"
        );
        assert_eq!(
            format!(
                "{:?}",
                Roles {
                    repr: Roles::Artist.repr
                }
            ),
            "Roles::Artist"
        );
        assert_eq!(format!("{:?}", Roles { repr: i32::MAX }), "Roles::Unknown");
    }
}
