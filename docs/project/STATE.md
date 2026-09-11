# Project state

- **Name:** TuneX
- **Artifact root:** docs/project/
- **Source spec:** docs/SPEC.md (v1.1 patched 2026-09-09)
- **Workflow profile:** product
- **Profile rationale:** Shipping Linux desktop app with native audio/DB/GPU surface. Not disposable (prototype rejected), not regulated (high-risk rejected). Full gates + decision locks + release/observe loops required.
- **Current phase:** 6 — Implement (S5 done 2026-09-10, S6 active)
- **Status:** active
- **MVP success signal:** User installs TuneX on Arch via AUR PKGBUILD, adds music dir, scans without UI freeze, browses/searches/plays with artwork, controls via app + MPRIS, manages queue + playlists, restarts without state loss, all offline.
- **Last updated:** 2026-09-11

## Authority boundaries
- Agent may decide: stack-local reversible implementation details consistent with locked decisions (crate-internal APIs, QML component names, channel types, test layout).
- Human confirmation required: production release, any network/cloud scope addition, paid services/secrets, reopening locked decisions (D-001–D-014 exc. D-011 superseded).

## Current evidence
- Latest passed gate: `rust-tc sonar` for the W-037 S5 gate (QG OK, 81.0% new coverage)
- Latest validation entry: 2026-09-11 W-043 QML best-practices lint plugin adopted (see VALIDATION.md)
- Design outputs: `docs/DESIGN_BRIEF.md`, `docs/INFORMATION_ARCHITECTURE.md`, `docs/DESIGN.md` (Google spec lint: 0 errors), `docs/mockup.png` (canonical layout), `AGENTS.md` (agent operating rules)
- Coding rules: `.cursor/rules/` (`rust.mdc`, `qt-qml.mdc`, `frontend.mdc`, `testing-gui.mdc`); prose copies in `docs/rules/`; GUI testing via Kwin-MCP
- QML gate: `scripts/qml-lint.sh` = Qt 6 `qmllint` + `qmlformat` + the local `QtQmlBestPractices` QQMLSA plugin's eight categories (discovered via sibling checkout / installed plugin dir / `TUNEX_QMLLINT_PLUGIN_PATH`; `off` skips)
- Toolchain: Rust 1.98.1 stable, `rust-tc` gate green, Sonar project `tunex`, pre-commit hook installed
- Build: CMake 4.4 + Corrosion v0.6.1 + Qt 6.11.2 configure/build green; PKGBUILD final (install rules + full plugin deps + `!lto`, local `makepkg -f` green, namcap clean; url `github.com/PerkyZZ999/TuneX`)
- S5 (W-032–W-037 done): live-engine MPRIS matrix, desktop notifications, `scripts/netoff-rehearsal.sh` ALL GREEN, Wayland/X11/HiDPI/dialogs KWin-proven; QG OK
- Bridge (W-002 done): cxx-qt 0.10 stack pinned; `TrackListModel` round-trip GUI-proven; module owned by build.rs; bridge code lives in binary; explicit init anchoring in main.rs
- Shell (W-003 done): Theme singleton + nav shell + Home/empty states GUI-proven; `tunex-core::config` + tracing live; DESIGN.md body 16px (skill compliance)
- Player (W-004 done): `PlayerEngine` playbin3 + `PlayerEvent` bus, 14 fakesink tests green, S3776 fixed
- Queue (W-005 done): pure `Queue` + preload hook + gapless integration green; Quality Gate OK (waiver closed)
- Codecs (W-006 done): 6-format fixture matrix + corrupt path green; QG OK
- Harness (W-007 done): SQLite v1 + scanner + scan→play end-to-end green; bridge unified into lib; GUI smoke OK
- MPRIS (W-008 done): bus name owned, transport round-trip live-verified; QG OK
- CI + DoD (W-009 done): Arch-container workflow + `dod-demo.sh` 5/5 green
- Metadata (W-011 done): `read_metadata` + artwork bytes, 70 tests green, QG OK
- Repo reality: S1+S2+S3+S4+S5 done; visual-contract refactor landed; S6 active — W-038 (glass hierarchy), W-039 (motion), W-040 (artwork) and W-043 (QML best-practices lint plugin) done, W-041–W-042 next. Three player/shell defects found during W-038 are fixed and validated: the end-of-track UI freeze, repeat-one never replaying, and the restored track missing from Up Next.

## Open loops
- None

## Blockers
- None (gst-plugins-good installed by user; W-005 unblocked and green).

## Next action
- W-041: finish the profile pass — scroll/frame observation and control latency in KWin, then record every number against the M6 targets (R-NFR-06, measure not gate).

## Phase checklist
- [x] 0 Intake
- [x] 1 Idea
- [x] 2 Discovery (spikes skipped per request)
- [x] 3 Spec
- [x] 4 Decision lock (records written, human confirm pending)
- [x] 5 Architecture
- [ ] 6 Implement
- [ ] 7 Release
- [ ] 8 Observe
