# Work items (current slice: S1 — Shell + real playback)

> First slice only per workflow. S2–S6 seeded when S1 completes.

## Current slice
S1 — Shell + real playback (M0+M1). Goal: prove R1/R2 feasibility (cxx-qt + playbin3 + PKGBUILD) with real files.

## Queue
- [x] W-001a — 4-crate Cargo workspace (workspace deps/lints, `publish=false`, core `Error` + tests) + Rust-Toolchain install (justfile, deny incl. GPL-3.0-or-later, nextest, rust-toolchain stable, Sonar `tunex` project + token) + pre-commit hook (`rust-tc doctor` gate). Gates: `quick` ✓ `doctor` ✓ `sonar` ✓ (QG OK). (R-001 part)
- [x] W-001b — CMake top-level (Qt 6.8+, Corrosion v0.6.1 pinned, `qt_add_qml_module` URI `TuneX` + `App.qml` shell) + PKGBUILD skeleton (url + git source `github.com/PerkyZZ999/TuneX`, namcap clean). Verified: configure warning-free, full build green (Corrosion Rust + QML module), `tunex` binary runs, `qmllint`/`qmlformat` clean, `printsrcinfo` OK. Full `makepkg` + chroot deferred to S5 (needs executable + Corrosion vendoring). (R-001, R-016)
- [x] W-002 — cxx-qt bridge proof (pinned: cxx 1.0.198, cxx-qt/-lib/-build 0.10.0): `TrackListModel : QAbstractListModel` (Title/Artist roles, rowCount/data/roleNames overrides, append/clear invokables) + Rust `main` booting Qt + proof `App.qml`. Verified: 10 unit tests ✓, KWin GUI round-trip 3× (button → Rust append → rowsInserted → row rendered, screenshot-observed), `doctor` ✓, `sonar` ✓ (0 violations; coverage waiver below). (R-001)
- [x] W-003 — app window + nav skeleton + theme tokens + tracing + XDG config: `Theme.qml` singleton (42 DESIGN.md tokens), shell (`App.qml` + rail/topbar/history, `HomeView`, `SectionStub`, `NavItem`), `tunex-core::config` (TOML schema + clamp + round-trip/compat tests), structured `tracing` boot. Verified: 15 unit tests ✓, KWin GUI (themed shell renders, rail nav + history proven, evidence shots), `doctor` ✓, `sonar` ✓ (0 violations; coverage waiver stands). (R-001, R-015)
- [ ] W-002 — cxx-qt bridge proof: 1 Rust QObject + QAbstractListModel + signal round-trip to QML, pinned versions (D-003)
- [ ] W-003 — App window + nav skeleton + theme tokens (consumes `docs/DESIGN.md` + `docs/mockup.png` layout) + tracing + XDG config load/save (R-001, R-015)
- [ ] W-004 — PlayerEngine over playbin3: load/play/pause/stop/seek/volume + bus thread → AppEvent (R-009, R-011)
- [ ] W-005 — In-memory queue + about-to-finish preload + next/prev/shuffle/repeat (R-010)
- [ ] W-006 — Codec matrix fixtures MP3/FLAC/OGG/M4A/Opus/WAV + corrupt-file error path (R-009, R-NFR-05)
- [ ] W-007 — Minimal SQLite open + minimal scan harness (list files → play one) to make S1 end-to-end real (R-006 part)
- [ ] W-008 — MPRIS skeleton (play/pause/next/prev/status) + playerctl smoke (R-013 part)
- [ ] W-009 — CI: native build + qmllint/qmlformat + cargo test + PKGBUILD/`makepkg` check; DoD demo script draft (R-016)
- [ ] W-010 — S1 gate: record codec + PKGBUILD + playerctl evidence in VALIDATION.md, close or loop R1/R2

## Done (recent)
- None yet
