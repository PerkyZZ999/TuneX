# Project state

- **Name:** TuneX
- **Artifact root:** docs/project/
- **Source spec:** docs/SPEC.md (v1.1 patched 2026-09-09)
- **Workflow profile:** product
- **Profile rationale:** Shipping Linux desktop app with native audio/DB/GPU surface. Not disposable (prototype rejected), not regulated (high-risk rejected). Full gates + decision locks + release/observe loops required.
- **Current phase:** 6 — Implement (S4 done 2026-09-10, S5 active)
- **Status:** active
- **MVP success signal:** User installs TuneX on Arch via AUR PKGBUILD, adds music dir, scans without UI freeze, browses/searches/plays with artwork, controls via app + MPRIS, manages queue + playlists, restarts without state loss, all offline.
- **Last updated:** 2026-09-10

## Authority boundaries
- Agent may decide: stack-local reversible implementation details consistent with locked decisions (crate-internal APIs, QML component names, channel types, test layout).
- Human confirmation required: production release, any network/cloud scope addition, paid services/secrets, reopening locked decisions (D-001–D-014 exc. D-011 superseded).

## Current evidence
- Latest passed gate: `rust-tc sonar` for W-032 MPRIS engine wiring (QG OK, 81.2% new coverage)
- Latest validation entry: 2026-09-10 S5 W-032 MPRIS live engine state (see VALIDATION.md)
- Design outputs: `docs/DESIGN_BRIEF.md`, `docs/INFORMATION_ARCHITECTURE.md`, `docs/DESIGN.md` (Google spec lint: 0 errors), `docs/mockup.png` (canonical layout), `AGENTS.md` (agent operating rules)
- Coding rules: `opencode.json` → `docs/rules/` (`rust.md`, `qt-qml.md`, `frontend.md`, `testing-gui.md`); GUI testing via Kwin-MCP
- Toolchain: Rust 1.98.1 stable, `rust-tc` gate green, Sonar project `tunex`, pre-commit hook installed
- Build: CMake 4.4 + Corrosion v0.6.1 + Qt 6.11.2 configure/build green; PKGBUILD skeleton namcap-clean (url `github.com/PerkyZZ999/TuneX`)
- Bridge (W-002 done): cxx-qt 0.10 stack pinned; `TrackListModel` round-trip GUI-proven; module owned by build.rs; bridge code lives in binary; explicit init anchoring in main.rs
- Shell (W-003 done): Theme singleton + nav shell + Home/empty states GUI-proven; `tunex-core::config` + tracing live; DESIGN.md body 16px (skill compliance)
- Player (W-004 done): `PlayerEngine` playbin3 + `PlayerEvent` bus, 14 fakesink tests green, S3776 fixed
- Queue (W-005 done): pure `Queue` + preload hook + gapless integration green; Quality Gate OK (waiver closed)
- Codecs (W-006 done): 6-format fixture matrix + corrupt path green; QG OK
- Harness (W-007 done): SQLite v1 + scanner + scan→play end-to-end green; bridge unified into lib; GUI smoke OK
- MPRIS (W-008 done): bus name owned, transport round-trip live-verified; QG OK
- CI + DoD (W-009 done): Arch-container workflow + `dod-demo.sh` 5/5 green
- Metadata (W-011 done): `read_metadata` + artwork bytes, 70 tests green, QG OK
- Repo reality: S1+S2+S3+S4 done; S5 active (to be seeded from ROADMAP).

## Open loops
- None

## Blockers
- None (gst-plugins-good installed by user; W-005 unblocked and green).

## Next action
- W-037: S5 gate (network-off DoD rehearsal + playerctl matrix + Wayland/X11 + PKGBUILD).

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
