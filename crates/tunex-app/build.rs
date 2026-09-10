//! Build script: generates the C++ for the `#[cxx_qt::bridge]` modules and
//! defines the QML module exactly once (URI `TuneX`).
//!
//! cxx-qt-build 0.10 requires module QML files to live under this crate, so
//! the QML tree is `crates/tunex-app/qml/` (not top-level `qml/`).
//! `CMake` must NOT redeclare this module via `qt_add_qml_module` — that would
//! register `TuneX` twice. `CMake` drives the build through `Corrosion`; the
//! module, its QML files, and type registration all come from here.

use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

fn main() {
    // QML-only edits must rebuild the resources too: without these
    // directives Cargo reruns this script only when build.rs itself changes,
    // silently testing stale UI (caught live in the W-018 gate run).
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=qml/TuneX");
    println!("cargo:rerun-if-changed=src/bridge");
    CxxQtBuilder::new_qml_module(
        QmlModule::new("TuneX")
            .version(1, 0)
            .qml_files([
                "qml/TuneX/App.qml",
                "qml/TuneX/HomeView.qml",
                "qml/TuneX/SectionStub.qml",
                "qml/TuneX/NavItem.qml",
                "qml/TuneX/LibraryView.qml",
                "qml/TuneX/TrackRow.qml",
                "qml/TuneX/AlbumCard.qml",
                "qml/TuneX/ArtistCard.qml",
                "qml/TuneX/EmptyState.qml",
                "qml/TuneX/FoldersDrawer.qml",
                "qml/TuneX/SearchView.qml",
                "qml/TuneX/QueuePanel.qml",
                "qml/TuneX/MiniPlayer.qml",
                "qml/TuneX/TrackMenu.qml",
                "qml/TuneX/PlaylistsView.qml",
                "qml/TuneX/PlaylistNameDialog.qml",
            ])
            .qml_file(QmlFile::from("qml/TuneX/Theme.qml").singleton(true)),
    )
    .files(["src/bridge/models.rs"])
    .build();
}
