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
    println!("cargo:rerun-if-changed=resources.qrc");
    println!("cargo:rerun-if-changed=../../assets/hero-charcoal.png");
    println!("cargo:rerun-if-changed=../../assets/art-placeholder.png");
    println!("cargo:rerun-if-changed=../../assets/empty-library.png");
    println!("cargo:rerun-if-changed=../../assets/empty-search.png");
    CxxQtBuilder::new_qml_module(
        QmlModule::new("TuneX")
            .version(1, 0)
            .qml_files([
                "qml/TuneX/App.qml",
                "qml/TuneX/HomeView.qml",
                "qml/TuneX/HeroCard.qml",
                "qml/TuneX/SectionStub.qml",
                "qml/TuneX/NavItem.qml",
                "qml/TuneX/Icon.qml",
                "qml/TuneX/IconButton.qml",
                "qml/TuneX/PrimaryButton.qml",
                "qml/TuneX/Chip.qml",
                "qml/TuneX/LibraryView.qml",
                "qml/TuneX/TrackRow.qml",
                "qml/TuneX/Artwork.qml",
                "qml/TuneX/AlbumCard.qml",
                "qml/TuneX/ArtistCard.qml",
                "qml/TuneX/PlaylistCard.qml",
                "qml/TuneX/EmptyState.qml",
                "qml/TuneX/SearchView.qml",
                "qml/TuneX/QueuePanel.qml",
                "qml/TuneX/MiniPlayer.qml",
                "qml/TuneX/NowPlayingView.qml",
                "qml/TuneX/TrackMenu.qml",
                "qml/TuneX/PlaylistsView.qml",
                "qml/TuneX/PlaylistNameDialog.qml",
                "qml/TuneX/PlayButton.qml",
                "qml/TuneX/PlayBadge.qml",
                "qml/TuneX/AmbientWash.qml",
                "qml/TuneX/TextLink.qml",
                "qml/TuneX/ProgressSlider.qml",
                "qml/TuneX/GlassMenu.qml",
                "qml/TuneX/GlassMenuItem.qml",
                "qml/TuneX/GlassMenuSeparator.qml",
                "qml/TuneX/GlassDialog.qml",
                "qml/TuneX/SettingsView.qml",
                "qml/TuneX/SettingsToggle.qml",
                "qml/TuneX/TrayPopup.qml",
            ])
            .qml_file(QmlFile::from("qml/TuneX/Theme.qml").singleton(true))
            .qml_file(QmlFile::from("qml/TuneX/Appearance.qml").singleton(true))
            .qml_file(QmlFile::from("qml/TuneX/GlassBackdrop.qml")),
    )
    .files(["src/bridge/models.rs"])
    .qrc("resources.qrc")
    .build();
}
