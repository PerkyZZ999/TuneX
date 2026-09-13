# Project state

- **Name:** TuneX
- **Artifact root:** docs/project/
- **Source spec:** docs/SPEC.md (v1.1 patched 2026-09-09)
- **Workflow profile:** product
- **Profile rationale:** Shipping Linux desktop app with native audio/DB/GPU surface. Not disposable (prototype rejected), not regulated (high-risk rejected). Full gates + decision locks + release/observe loops required.
- **Current phase:** 6 — Implement (S7 done; S8 session/player UX done 2026-09-13)
- **Status:** active
- **MVP success signal:** User installs TuneX on Arch via AUR PKGBUILD, adds music dir, scans without UI freeze, browses/searches/plays with artwork, controls via app + MPRIS, manages queue + playlists, restarts without state loss, all offline.
- **Last updated:** 2026-09-13

## Authority boundaries
- Agent may decide: stack-local reversible implementation details consistent with locked decisions (crate-internal APIs, QML component names, channel types, test layout).
- Human confirmation required: production release, any network/cloud scope addition, paid services/secrets, reopening locked decisions (D-001–D-014 exc. D-011 superseded).

## Current evidence
- Latest passed gate: `rust-tc sonar` for the W-042 S6 gate (QG OK, 81.9% coverage, 0 violations)
- Latest validation entry: 2026-09-13 W-048–W-051 session memory and player UX — see VALIDATION.md
- M6 numbers (R-NFR-06, measured not estimated): cold start 850 ms windowed at 50k (<1.5 s) · search p95 6.8–9.2 ms at 50k (<50 ms) · scroll p50 16 ms with 0.36 ms app-side work per frame (60 fps) · transport control ≤8 ms (<50 ms) · scan 52,400 files/min (>500) · no app-side frame cost during scans
- Visual identity: charcoal-black neutrals (`#0B0B0E` up, hue-free) with royal blue `#2B5CE6` as the only brand family; chrome is tinted-translucent over an artwork-derived ambient wash, real blur reserved for overlays
- Design outputs: `docs/DESIGN_BRIEF.md`, `docs/INFORMATION_ARCHITECTURE.md`, `docs/DESIGN.md` (Google spec lint: 0 errors), `docs/mockup.png` (canonical layout), `AGENTS.md` (agent operating rules)
- Coding rules: `.cursor/rules/` (`rust.mdc`, `qt-qml.mdc`, `frontend.mdc`, `testing-gui.mdc`); prose copies in `docs/rules/`; GUI testing via Kwin-MCP
- QML gate: `scripts/qml-lint.sh` = Qt 6 `qmllint` + `qmlformat` + the local `QtQmlBestPractices` QQMLSA plugin's eight categories (discovered via sibling checkout / installed plugin dir / `TUNEX_QMLLINT_PLUGIN_PATH`; `off` skips)
- Toolchain: Rust 1.98.1 stable, `rust-tc` gate green, Sonar project `tunex`, pre-commit hook installed
- Build: CMake 4.4 + Corrosion v0.6.1 + Qt 6.11.2 configure/build green; PKGBUILD final (install rules + full plugin deps + `!lto`, local `makepkg -f` green, namcap clean; url `github.com/PerkyZZ999/TuneX`)
- S5 (W-032–W-037 done): live-engine MPRIS matrix, desktop notifications, `scripts/netoff-rehearsal.sh` ALL GREEN, Wayland/X11/HiDPI/dialogs KWin-proven; QG OK
- Bridge (W-002 done): cxx-qt 0.10 stack pinned; `TrackListModel` round-trip GUI-proven; module owned by build.rs; bridge code lives in binary; explicit init anchoring in main.rs
- Shell (W-003 done): Theme singleton + nav shell + Home/empty states GUI-proven; `tunex-core::config` + tracing live; DESIGN.md body 13px (W-047 density pass; caption 12px floor)
- Player (W-004 done): `PlayerEngine` playbin3 + `PlayerEvent` bus, 14 fakesink tests green, S3776 fixed
- Queue (W-005 done): pure `Queue` + preload hook + gapless integration green; Quality Gate OK (waiver closed)
- Codecs (W-006 done): 6-format fixture matrix + corrupt path green; QG OK
- Harness (W-007 done): SQLite v1 + scanner + scan→play end-to-end green; bridge unified into lib; GUI smoke OK
- MPRIS (W-008 done): bus name owned, transport round-trip live-verified; QG OK
- CI + DoD (W-009 done): Arch-container workflow + `dod-demo.sh` 5/5 green
- Metadata (W-011 done): `read_metadata` + artwork bytes, 70 tests green, QG OK
- Repo reality: S1–S7 done. S8 (W-048–W-051) persists window geometry and library view prefs, scrubs from MiniPlayer/Up Next, completes the keyboard map, and adds missing-file reveal/remove. Visual identity is charcoal + royal blue with tinted-translucent chrome over an artwork wash; glass stays hierarchical; glow appears exactly twice. M6 numbers remain aspirational and were met. Maintainer follow-ups before AUR publish are unchanged from S5.

## Open loops
- S7 W-046 KWin/Plasma tray hover + close-to-tray visual proof (this environment has no StatusNotifierWatcher / isolated Plasma session)
- Library UX (sort / Folders browse / click-to-play) rebased onto W-046; scanned-folder add/remove lives in Settings → Library. Charles tests once this PR is conflict-free.

## Blockers
- None (gst-plugins-good installed by user; W-005 unblocked and green).

## Next action
- Continue post-V1 expansion at S9 (album/artist landing pages, genre/composer browse, search recents). Phase 7 Release still waits on human confirmation for AUR publish.

## Phase checklist
- [x] 0 Intake
- [x] 1 Idea
- [x] 2 Discovery (spikes skipped per request)
- [x] 3 Spec
- [x] 4 Decision lock (records written, human confirm pending)
- [x] 5 Architecture
- [x] 6 Implement
- [ ] 7 Release
- [ ] 8 Observe
