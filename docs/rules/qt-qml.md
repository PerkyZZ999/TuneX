# Qt / QML rules — TuneX

> Cursor applies this as `.cursor/rules/qt-qml.mdc` (globs: `**/*.qml`, CMake, `crates/tunex-app/build.rs`). Keep both files in sync.

> Name note: the skill is `qt6-qml-development` (not `qt-qml-development`). Use the exact names below.

## Load triggers (load only what the task needs)

| Task | Skill |
|------|-------|
| Writing, fixing, refactoring, optimizing QML | `qt-qml` (always for `.qml` work) |
| CMake targets, `qt_add_qml_module`, resources, build wiring | `qt-project` |
| Qt 6 APIs, QML type system, `qmllint`/`qmlformat`, C++/QML integration concepts (maps to our `cxx-qt` bridge patterns) | `qt6-qml-development` |
| Implementing screens against `docs/mockup.png` / `docs/DESIGN.md`, or auditing UX | `qt-ui-design` |
| Reviewing QML before commit (bindings, layout, delegates, perf) | `qt-qml-review` |
| Writing QML component tests (`TestCase`, `SignalSpy`) | `qt-qml-test` |
| Running QML tests (`qmltestrunner`/CTest) | `qt-qml-test-run` |
| Documenting a QML component/module to Markdown | `qt-qml-docs` |
| Jank/dropped frames investigation (M6, or profile-backed hotspot hunt) | `qt-qml-profiler` |

Do **not** load `qt-cpp-review` / `qt-cpp-docs` for app code: TuneX has no C++ application code — Qt interop is Rust via `cxx-qt` (see `.cursor/rules/rust.mdc`).

## Hard QML rules (D-014)

- QML is presentation only: no filesystem, SQLite, GStreamer, or process APIs. Data arrives via `cxx-qt` `QAbstractListModel`s; actions go out via bridged slots/signals.
- Theme tokens centralized (Theme singleton consuming `docs/DESIGN.md` frontmatter). Never ad-hoc hexes, radii, or durations.
- Component names match `docs/DESIGN_BRIEF.md` inventory (`TrackRow`, `AlbumCard`, `MiniPlayer`, …).
- Visual contract per view: `docs/mockup.png` layout + `docs/DESIGN_BRIEF.md` adaptation map; `docs/DESIGN.md` wins any tie on out-of-scope patterns.
- Progressive disclosure per `docs/INFORMATION_ARCHITECTURE.md` map; empty/loading/error/success states on every async surface.

## Checks (binding, see `AGENTS.md`)

`scripts/qml-lint.sh` clean (Qt 6 `qmllint` + `qmlformat`; the Arch PATH binaries are Qt 5) · CMake build passes · `qt-qml-review` before committing QML · `DESIGN.md` lint (0 errors) when tokens touched.

The lint script also loads the local `QtQmlBestPractices` QQMLSA plugin and gates on its eight `Plugin.BestPractices.*` categories: unversioned imports, concrete property types over `var`, no mutable state in view delegates, layout children sized with `Layout.*`, declarative bindings over `Component.onCompleted` assignments, no customization under a native Controls style, interaction signals over `onValueChanged`, and no redundant `QtQuick.Window` import. It is discovered (sibling checkout, installed plugin dir, or `TUNEX_QMLLINT_PLUGIN_PATH`), so a machine without it still lints and reports which categories went unchecked.
