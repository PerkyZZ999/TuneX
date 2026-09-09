cmake_minimum_required(VERSION 3.27)

project(TuneX
    VERSION 0.1.0
    LANGUAGES CXX
)

# Qt 6 requires C++17 or newer; set before find_package so a too-old
# compiler fails here with a clear error instead of deep in Qt macros.
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Explicit minimum per project policy; 6.8 opts into the QML module
# defaults this project relies on (QTP0001/QTP0005 via REQUIRES below).
find_package(Qt6 6.8 REQUIRED COMPONENTS Core Gui Qml Quick)

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

add_subdirectory(qml)
