# Work items (current slice: S1 — Shell + real playback)

> First slice only per workflow. S2–S6 seeded when S1 completes.

## Current slice
S1 — Shell + real playback (M0+M1). Goal: prove R1/R2 feasibility (cxx-qt + playbin3 + PKGBUILD) with real files.

## Queue
- [ ] W-001 — Scaffold 4-crate Cargo workspace + CMake top-level + Corrosion + qt_add_qml_module + PKGBUILD skeleton (R-001, R-016)
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
