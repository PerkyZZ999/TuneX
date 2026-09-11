cmake_minimum_required(VERSION 3.27)

project(TuneX
    VERSION 0.1.0
    LANGUAGES CXX
)

# Qt 6 requires C++17 or newer; set before find_package so a too-old
# compiler fails here with a clear error instead of deep in Qt macros.
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Explicit minimum per project policy: 6.9 for QtQuick.Effects
# RectangularShadow (DESIGN.md shadow/glow scale, S6 W-038); 6.8 policies
# still opt into the QML module defaults (QTP0001/QTP0005 via REQUIRES).
find_package(Qt6 6.9 REQUIRED COMPONENTS Core Gui Qml Quick QuickControls2 QuickEffects)

qt_standard_project_setup(REQUIRES 6.8)

# Rust workspace. Corrosion drives Cargo at build time; the pinned tag is
# bumped deliberately with a full clean build (see AGENTS.md freshness).
include(FetchContent)
FetchContent_Declare(
    Corrosion
    GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
    GIT_TAG v0.6.1 # Latest 2026-09-09.
)
FetchContent_MakeAvailable(Corrosion)

corrosion_import_crate(MANIFEST_PATH Cargo.toml)

# No qt_add_qml_module here: the QML module (URI TuneX) is defined exactly
# once in crates/tunex-app/build.rs (CxxQtBuilder QmlModule) and compiled into
# the Rust binary. Redeclaring it in CMake would register TuneX twice.
# QML sources live in qml/ and are linted via qmllint/qmlformat directly.
