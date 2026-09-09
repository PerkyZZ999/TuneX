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
- Latest passed gate: 5 Architecture (human-confirmed 2026-09-09)
- Latest validation entry: 2026-09-09 design docs (brief + IA + DESIGN.md lint 0 errors + AGENTS.md)
- Design outputs: `docs/DESIGN_BRIEF.md`, `docs/INFORMATION_ARCHITECTURE.md`, `docs/DESIGN.md` (Google spec lint: 0 errors), `docs/mockup.png` (canonical layout), `AGENTS.md` (agent operating rules)
- Repo reality: docs-only foundation on `master` (no code yet).

## Open loops
- None

## Blockers
- None. MVP + locks + GPLv3 confirmed; packaging switched to PKGBUILD/AUR per user.

## Next action
- Start S1 W-001 (workspace + CMake/Corrosion + PKGBUILD skeleton) on user go-ahead.

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
