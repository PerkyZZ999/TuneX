# Validation evidence

### 2026-09-09 — Intake gate
- **Phase:** 0 → 1
- **Result:** pass
- **Evidence:** Workspace `the TuneX workspace`, git `master` no commits, `docs/SPEC.md` only tracked content + new `docs/project/`. Profile `product` recorded in STATE.md with rationale. No prior STATE to reconcile.
- **Waiver:** none
- **Follow-up:** none

### 2026-09-09 — Idea gate
- **Phase:** 1 → 2
- **Result:** pass
- **Evidence:** BRIEF.md has who+pain+context, primary/secondary users, observable V1 success signal (10 demo criteria from SPEC §39), 4+ non-goals, constraints. Human direction pre-approved via SPEC; formal MVP confirm still pending at Spec/Decision gates.
- **Waiver:** none (formal human agree recorded at phase 4 checkpoint)
- **Follow-up:** confirm MVP boundary in phase 3/4 question

### 2026-09-09 — Discovery gate (spikes skipped)
- **Phase:** 2 → 3
- **Result:** pass (conditional)
- **Evidence:** DISCOVERY.md inventories tech/time/legal/integrations/ops; security/privacy/a11y/data covered; top 5 risks with mitigation/accept; feasibility `go-with-conditions` tied to S1 proofs. Spikes skipped per explicit user request — no spike code written.
- **Waiver:** spike waiver by user request 2026-09-09 (owner: user; risk: R1/R2 discovered later in S1 not M0-spike; expiry: end of S1; follow-up: S1 must include cxx-qt round-trip + playbin3 + packaging proofs — PKGBUILD per D-011b change; "Flatpak" in original gate text superseded)
- **Follow-up:** S1 acceptance must demonstrate the three feasibility proofs

### 2026-09-09 — Spec gate
- **Phase:** 3 → 4
- **Result:** pass (pending human MVP confirm)
- **Evidence:** REQUIREMENTS.md has 18 testable MVP reqs (R-001–R-018 each with acceptance + validation method) + 6 NFRs + Later L-001–L-014 + non-goals consistent with BRIEF/SPEC §32/§39. Must vs later separated. Traceability to slices in ROADMAP.md.
- **Waiver:** none
- **Follow-up:** human confirms MVP boundary (question below)

### 2026-09-09 — Decision-lock gate
- **Phase:** 4 → 5
- **Result:** pass (pending human lock confirm; D-013 proposed)
- **Evidence:** DECISIONS.md has D-001–D-014; D-001–D-012 + D-014 locked with rationale/consequences/reopen/owner/evidence; D-013 (GPLv3) proposed awaiting confirm. No critical undecided blocker for architecture (license does not block S1).
- **Waiver:** none
- **Follow-up:** human confirms locks + license choice

### 2026-09-09 — Architecture gate
- **Phase:** 5 → 6 (ready, gated on human confirms above)
- **Result:** pass (conditional)
- **Evidence:** ARCHITECTURE.md matches locks (alignment table), trust/data boundaries + failure modes + test/operability addressed; ROADMAP.md has 6 vertical slices each with acceptance + req mapping; WORK_ITEMS.md seeds S1 W-001–W-010 only; full req→slice trace present.
- **Waiver:** none
- **Follow-up:** start S1 on human approval

### 2026-09-09 — Human checkpoint (MVP + locks + license + packaging change)
- **Phase:** 4/5 confirm
- **Result:** pass with change
- **Evidence:** User answers: MVP=Confirm MVP; locks=custom "Change Flatpak for PKGBUILD please !"; license=GPLv3. Applied: MVP locked, D-001–D-012+D-014 locked, D-013 flipped proposed→locked (GPLv3), D-011 superseded by D-011b (PKGBUILD/AUR primary, Flatpak deferred). Patched: SPEC §31/§38/§40, REQUIREMENTS R-016, ARCHITECTURE overview/test/ops/alignment, ROADMAP S1+S5+trace, WORK_ITEMS W-001/W-009/W-010, STATE/BRIEF/DISCOVERY install story.
- **Waiver:** none
- **Follow-up:** S1 W-001 on user go-ahead; blast radius isolated to packaging (no stack/boundary change)

### 2026-09-09 — Design docs + AGENTS.md + full-doc review
- **Phase:** 5 (pre-S1 design pass)
- **Result:** pass
- **Evidence:** `docs/DESIGN_BRIEF.md` + `docs/INFORMATION_ARCHITECTURE.md` (renamed per user) moved to `docs/`; `docs/DESIGN.md` moved to `docs/` and passes `npx @google/design.md lint` with 0 errors (1 info-level orphaned-`focus` warning kept deliberately — schema has no border/outline prop); `AGENTS.md` created at root (checks+commit rule, frontend skill trio, mockup adaptation rule, OpenRouter `openai/gpt-image-2.5-sunburst` asset rule). Review fixed: SPEC §3.7/§20/§35 stale Flatpak + palette-TBD lines; mini-player rule aligned to mockup (right panel ≥1280px, bottom bar below); 44px targets + dense-list exception; button loading/disabled + skeleton loading states; hero secondary-button + rail-label adaptations; W-003 now consumes `docs/DESIGN.md`.
- **Waiver:** none
- **Follow-up:** S1 W-001 on user go-ahead (rules wired: see entry below)

### 2026-09-09 — Coding rules wired (opencode.json + docs/rules)
- **Phase:** 5 (pre-S1 rules pass)
- **Result:** pass
- **Evidence:** `opencode.json` (`$schema` + 4 instructions, JSON-parsed OK) → `docs/rules/rust.md` (ms-rust/best-practices/reference mandatory, optimise gated to M6), `qt-qml.md` (per-task triggers; `qt6-qml-development` exact-name note; C++ skills excluded), `frontend.md` (trio + QML translations + gates), `testing-gui.md` (Kwin-MCP isolated-session workflow; no separate Computer Use tool in this env). `AGENTS.md` extended (doc map, GUI Testing section, Language Skill Triggers). Skill names verified against installed skill catalog.
- **Waiver:** none
- **Follow-up:** S1 W-001 on user go-ahead (toolchain: see entry below)

### 2026-09-09 — S1 W-001a: workspace + toolchain gates green
- **Phase:** 6 (Implement, slice S1)
- **Result:** pass
- **Evidence:** 4-crate workspace (edition 2024, `resolver="2"`, `publish=false`, workspace lints incl. pedantic) + `tunex-core::Error` (`thiserror` 2.0.20, latest via `cargo add --dry-run`) with 4 unit tests. `rust-tc quick` ✓ → `doctor` ✓ (`Rust-Toolchain: PASS`: fmt, clippy `-D warnings`, nextest, doctests, deny incl. project GPL-3.0-or-later allowance, shear, hack) → `sonar` ✓ (single Clippy JSON + single llvm-cov/nextest run + separate doctests, no `cargo-audit`; upload ANALYSIS SUCCESSFUL; Quality Gate `OK`). Fixes along the way: `publish=false` over invented repo metadata, backticked doc identifiers, resolver pin, GPL allow-listing, shear-honest dep declaration. Rust 1.95→1.98.1 via `rustup update`. Pre-commit hook installed (`scripts/install-git-hooks.sh`).
- **Waiver:** none (zero-test scaffold stage resolved with real `Error` tests instead of weakening the gate)
- **Follow-up:** W-001b (CMake/Corrosion/QML module/PKGBUILD) — see entry below

### 2026-09-09 — S1 W-001b: CMake + QML module + PKGBUILD green
- **Phase:** 6 (Implement, slice S1)
- **Result:** pass (skeleton scope; full `makepkg`/chroot deferred to S5)
- **Evidence:** `cmake -B build` warning-free (Qt 6.11.2, Corrosion v0.6.1 pinned = latest, Rust 1.98.1); `cmake --build build` green — Corrosion compiled the workspace (`tunex` 0.1.0 binary runs) + `tunex_qml` static lib with `qmlcachegen`/type-registrar output; URI-mirroring layout (`qml/TuneX/`) fixed the only configure warning. `qmllint` + `qmlformat` clean on `App.qml` (bare-window shell; real window/theme in W-003). PKGBUILD: `url` + git source set from user-provided `github.com/PerkyZZ999/TuneX`; `makepkg --printsrcinfo` OK; `namcap` 0 findings. Known gaps recorded in-file: `package()` install rules (W-002 executable), Corrosion vendoring for chroot builds (S5).
- **Waiver:** none
- **Follow-up:** W-002 (cxx-qt bridge proof) — see entry below

### 2026-09-09 — S1 W-002: cxx-qt bridge round-trip proven
- **Phase:** 6 (Implement, slice S1)
- **Result:** pass (with coverage waiver below)
- **Evidence:** `TrackListModel : QAbstractListModel` (cxx 1.0.198, cxx-qt/-lib/-build 0.10.0, Qt 6.11.2) + proof `App.qml`. 10 unit tests ✓ (`quick`/`doctor` green). KWin isolated GUI run ×3: window appears, keyboard-activated Add → `Tracks: 1` + `Signal received: row 0` + `Proof track 1  TuneX` row rendered (visually verified). `sonar` upload OK, 0 new violations / 0 duplication.
- **Waiver:** `new_coverage` ~60% < 80% gate (owner: user pending confirm; risk: low — uncovered lines are exactly the FFI pairing wrappers, which need C++ instances, and `main` boot, both GUI-verified above; rationale: no in-process Qt test harness exists yet; expiry/revisit: S4 QML-test harness, or accept permanently for bridge glue).
- **Follow-up:** W-003. Bridge learnings (binding): QML module defined ONLY in build.rs (deleted CMake `qt_add_qml_module`, QML tree moved to `crates/tunex-app/qml/` per cxx-qt-build containment); bridge code lives in the BINARY (linker gc-sections drops it from rlib-only linkage); explicit `#[link]` + direct `cxx_qt_init_*` calls in main.rs (build-script link-lib directives don't reach the bin target); `#[cxx_name]` required on every multi-word invokable (`append_track` → `appendTrack`); EIS clicks highlight without firing `clicked()` — verify GUI actions via keyboard + screenshots; copy KWin screenshots BEFORE `session_stop` (isolated home is cleaned); Qt logging is silent in the static Qt build — diagnose via behavior/probes, and keep the fail-fast null-object guard in main.rs.

### 2026-09-09 — S1 W-003: shell + theme + config green
- **Phase:** 6 (Implement, slice S1)
- **Result:** pass (coverage waiver from W-002 extends: 67.8%, same uncovered categories)
- **Evidence:** `Theme.qml` singleton (42 tokens) + shell (`App.qml` rail/topbar/history, `HomeView`, `SectionStub`, `NavItem`) + `tunex-core::config` (additive TOML schema, clamp, compat tests) + structured tracing boot. 15 unit tests ✓, `doctor` ✓, `sonar` ✓ (0 violations). KWin GUI: themed shell renders per DESIGN.md, rail selection + back/forward history proven via keyboard (evidence: `docs/project/evidence/w003-shell-home.png`, `w003-nav-library.png`). DESIGN.md body 16px after qt-ui-design review.
- **Waiver:** none new (W-002 coverage waiver still stands)
- **Follow-up:** W-004. W-003 learnings (binding): QML `on[A-Z]*` names are reserved (a `Theme.onPrimary` token killed component creation with zero diagnostics — renamed `primaryText`, mapping documented); QML singletons need `QmlFile::singleton(true)` (pragma alone is not enough); verify `list_windows` shows the window BEFORE trusting blank screenshots (rapid session cycling is flaky — always confirm presence first); never hand-edit `target/` (poisoned the cxx-qt freshness check); filesystem-load dev mode resolves singletons from the export dir, so sync it or just rebuild (qrc is the single source of truth).

### 2026-09-09 — S1 W-004: PlayerEngine green
- **Phase:** 6 (Implement, slice S1)
- **Result:** pass (coverage waiver stands: 73.4%, same uncovered categories)
- **Evidence:** `tunex-player` (`PlayerEngine` + `path_to_uri` + core `PlaybackState`/`PlayerEvent`/`Error::Player`): 14 engine tests incl. fakesink missing-file sync+async error paths, volume/mute/position, drop-join hygiene (no GStreamer criticals). 29/29 workspace tests ✓, `doctor` ✓, `sonar` ✓ (S3776 complexity 17→split into `handle_message`/`on_state_changed`/`on_duration_discovery`; 0 violations). Headless work — no UI change, no GUI run needed.
- **Waiver:** none new
- **Follow-up:** W-005 (queue pure logic done; gapless test blocked — see entry below)

### 2026-09-09 — S1 W-005 done: queue + gapless green, QG OK
- **Phase:** 6 (Implement, slice S1)
- **Result:** pass (coverage waiver CLOSED: 82.9% ≥ 80%, 0 violations, 0 duplication)
- **Evidence:** 46/46 tests ✓, `doctor` ✓, `sonar` ✓ Quality Gate `OK`. Gapless integration green after user installed `gst-plugins-good`. Root-caused on the way: identical-URI preload reconfigure loop (fixed with `queued_uri` guard), preroll-Paused transients (intent-flag filter), `Drop` disconnect-before-Null (teardown wedge), EIS-click and session-cycling test-discipline notes.
- **Waiver:** none (all prior waivers closed)
- **Follow-up:** W-006 — see entry below.

### 2026-09-09 — S1 W-006: codec matrix green
- **Phase:** 6 (Implement, slice S1)
- **Result:** pass
- **Evidence:** `scripts/generate-fixtures.sh` reproduces `tests/fixtures/` (5 s tones: wav/flac/ogg/opus/mp3/m4a, all discoverable with durations, plus deterministic corrupt file); `codec_matrix` integration tests play each to `Playing` with duration and prove corrupt input errors loudly while the engine stays usable. 53/53 tests ✓, `doctor` ✓, `sonar` ✓ QG OK (83.4%, 0 violations).
- **Waiver:** none
- **Follow-up:** W-007 — see entry below.

### 2026-09-10 — S1 W-007: SQLite harness + scan→play green
- **Phase:** 6 (Implement, slice S1)
- **Result:** pass
- **Evidence:** `tunex-library` (`db`: v1 migrations, WAL, roots/tracks/upsert; `scan`: recursive walk, 10-extension filter, `stable_key`, symlink skips, live counters) + `tunex-app` integration test scanning committed fixtures (7 rows incl. corrupt.mp3) and playing WAV to `Playing`. 61/61 tests ✓, `doctor` ✓, `sonar` ✓ QG OK (83.8%, 0 violations). GUI smoke: restructured app renders shell identically.
- **Waiver:** none
- **Follow-up:** W-008 — see entry below. W-007 learnings: bridge belongs in the LIB with the binary calling `run()` (integration-test link needs shared impls — supersedes the W-002 binary-only note); `usize` has no rusqlite `FromSql` (use i64 + convert); system SQLite headers present so no `bundled` feature needed.

### 2026-09-10 — S1 W-008: MPRIS skeleton live-verified
- **Phase:** 6 (Implement, slice S1)
- **Result:** pass
- **Evidence:** `tunex-app::mpris` (mpris-server 0.10.0 + tokio runtime on detached thread): full Root/Player interfaces over local state, 6 unit tests ✓. Live bus run (offscreen binary, real session bus): name owned, `Identity=TuneX`, `Stopped→PlayPause→Playing→PlayPause→Paused` via qdbus6, clean name release on exit. `doctor` ✓, `sonar` ✓ QG OK (83.2%, 0 violations).
- **Waiver:** none
- **Follow-up:** W-009 — see entry below. W-008 learnings: trait methods use `fdo::Result` for getters AND most methods but plain `zbus::Result` for property setters — read the vendored signatures, never guess; `Volume`/`PlaybackRate` are `f64` aliases; no `playerctl` here — `busctl`/`qdbus6` cover the smoke test; engine/queue wiring deferred to S5 by design.

### 2026-09-10 — S1 W-009: CI + DoD rehearsal green
- **Phase:** 6 (Implement, slice S1)
- **Result:** pass
- **Evidence:** `.github/workflows/ci.yml` (Arch container: system deps incl. full plugin sets, toolchain + quality tools, `just doctor`, CMake build, QML lint/format check, printsrcinfo + namcap; YAML-parsed; runs on push — first green run pending a push). `scripts/dod-demo.sh` runs all five steps locally green (gate, configure, build, lint, live MPRIS smoke with name release + stray-process cleanup).
- **Waiver:** none
- **Follow-up:** W-010 (S1 close-out) — see entry below.

### 2026-09-10 — S1 gate: slice accepted
- **Phase:** 6 (Implement slice gate, S1)
- **Result:** pass
- **Evidence:** Acceptance check per item — CMake+Corrosion builds window ✓ · cxx-qt round-trip GUI-proven ✓ · playbin3 plays all 6 fixture formats to `Playing` with durations ✓, pause/volume/mute unit-tested ✓, deterministic seek (paused poll lands 1400–1600 ms) ✓ · queue next/prev/shuffle/repeat + gapless preload tested ✓ · PKGBUILD skeleton (printsrcinfo + namcap; full build deferred S5) · MPRIS skeleton live-verified (Identity + Stopped→Playing→Paused; `playerctl` absent here so `qdbus6`/`busctl` substituted). Gates: 67/67 tests ✓, `rust-tc doctor` ✓, `dod-demo.sh` 5/5 ✓, `sonar` ✓ QG OK (84.5%, 0 violations). Traceability: R-001/R-009/R-010/R-011/R-013/R-016 + NFR-01/05 → S1 → W-001a/b–W-010 → this entry.
- **Waiver:** none
- **Follow-up:** S2 planning. R1 (cxx-qt feasibility) CLOSED, R2 (playbin3/PKGBUILD feasibility) CLOSED — no milestone loop-back.

### 2026-09-10 — S2 W-011: metadata extraction green
- **Phase:** 6 (Implement, slice S2)
- **Result:** pass
- **Evidence:** `tunex-library::metadata` (`read_metadata` + `EmbeddedArtwork`, `Error::Metadata` in core): untagged fixture proves unknowns stay unknown with duration; lofty write→read round-trip proves all fields + BMP art bytes; corrupt file errors explicitly. 70/70 tests ✓, `doctor` ✓, `sonar` ✓ QG OK (85.5%, 0 violations; RUSTSEC-2024-0436 ignore-documented in `deny.toml`).
- **Waiver:** none
- **Follow-up:** W-012. W-011 learnings: verify lofty 0.24 API against vendored sources (accessor macro names differ from memory — `disk()` not `disc()`, `get_string(ItemKey)` for album-artist/composer); hand-writable BMP beats PNG for art round-trips (no compression libs); empty-tag normalization belongs in the reader, not the DB.
