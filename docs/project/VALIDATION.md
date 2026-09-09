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
- **Follow-up:** W-001b (CMake/Corrosion/QML module/PKGBUILD)
