# Project state

- **Name:** TuneX
- **Artifact root:** docs/project/
- **Source spec:** docs/SPEC.md (v1.1 patched 2026-09-09)
- **Workflow profile:** product
- **Profile rationale:** Shipping Linux desktop app with native audio/DB/GPU surface. Not disposable (prototype rejected), not regulated (high-risk rejected). Full gates + decision locks + release/observe loops required.
- **Current phase:** 5 — Architecture & delivery plan (0–5 complete, human-confirmed 2026-09-09; S1 ready)
- **Status:** active
- **MVP success signal:** User installs TuneX on Arch via AUR PKGBUILD, adds music dir, scans without UI freeze, browses/searches/plays with artwork, controls via app + MPRIS, manages queue + playlists, restarts without state loss, all offline.
- **Last updated:** 2026-09-09

## Authority boundaries
- Agent may decide: stack-local reversible implementation details consistent with locked decisions (crate-internal APIs, QML component names, channel types, test layout).
- Human confirmation required: production release, any network/cloud scope addition, paid services/secrets, reopening locked decisions (D-001–D-014 exc. D-011 superseded).

## Current evidence
- Latest passed gate: `rust-tc sonar` (QG OK) for W-001a
- Latest validation entry: 2026-09-09 toolchain gates (quick/doctor/sonar)
- Design outputs: `docs/DESIGN_BRIEF.md`, `docs/INFORMATION_ARCHITECTURE.md`, `docs/DESIGN.md` (Google spec lint: 0 errors), `docs/mockup.png` (canonical layout), `AGENTS.md` (agent operating rules)
- Coding rules: `opencode.json` → `docs/rules/` (`rust.md`, `qt-qml.md`, `frontend.md`, `testing-gui.md`); GUI testing via Kwin-MCP
- Toolchain: Rust 1.98.1 stable, `rust-tc` gate green, Sonar project `tunex`, pre-commit hook installed
- Build: CMake 4.4 + Corrosion v0.6.1 + Qt 6.11.2 configure/build green; PKGBUILD skeleton namcap-clean (url `github.com/PerkyZZ999/TuneX`)
- Bridge (W-002 done): cxx-qt 0.10 stack pinned; `TrackListModel` round-trip GUI-proven; module owned by build.rs; bridge code lives in binary; explicit init anchoring in main.rs
- Shell (W-003 done): Theme singleton + nav shell + Home/empty states GUI-proven; `tunex-core::config` + tracing live; DESIGN.md body 16px (skill compliance)
- Repo reality: S1 W-001a/b, W-002, W-003 done; W-004 (PlayerEngine + queue) next.

## Open loops
- None

## Blockers
- None. MVP + locks + GPLv3 confirmed; packaging switched to PKGBUILD/AUR per user.

## Next action
- W-004: PlayerEngine over GStreamer `playbin3` + in-memory queue (uses `gstreamer` crate; needs GStreamer dev libs + `gst-plugin-*` at build/run).

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
