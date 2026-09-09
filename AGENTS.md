# AGENTS.md — TuneX

> README for agents. Human-facing product context lives in `docs/SPEC.md`. This file is normative for how work gets done in this repo. Follow it before asking the user for anything it already answers.

## Project Overview

TuneX is a local-first, offline-mandatory Linux music player: a fast Rust engine (library index + GStreamer playback + SQLite/FTS5) wrapped in a GPU-accelerated Qt Quick dark-glass UI. No accounts, no streaming, no network dependency for core flows.

| Area | Decision (locked) |
|------|-------------------|
| Language | Rust (all domain logic) |
| UI | Qt 6 + Qt Quick/QML, SceneGraph/RHI (Vulkan preferred) |
| Rust↔QML bridge | `cxx-qt` (pinned; only `tunex-app` touches Qt) |
| Build | CMake top-level + Corrosion + Cargo |
| Audio | GStreamer `playbin3` + `about-to-finish` gapless; PipeWire default output |
| DB/Search | SQLite WAL + `rusqlite_migration` + FTS5 external-content/BM25 |
| Threading | `tokio` workers + `AppEvent` bus → Qt thread; UI thread never blocks |
| Crates | `tunex-core` / `tunex-library` / `tunex-player` / `tunex-app` (4 to start) |
| Packaging (V1) | Arch PKGBUILD/AUR (Flatpak deferred post-V1) |
| License | GPLv3 app + LGPL Qt dynamic linking |
| Perf policy | Aspirational targets; functional first, optimize in M6 |

Strict boundary: **QML = presentation only, Rust = logic.** QML never touches FS/SQLite/GStreamer directly — everything goes through `cxx-qt` objects and `QAbstractListModel`s fed by `AppEvent`s.

Doc map: `docs/SPEC.md` (source spec) · `docs/project/` (`STATE`, `BRIEF`, `REQUIREMENTS`, `DECISIONS`, `ARCHITECTURE`, `ROADMAP`, `WORK_ITEMS`, `VALIDATION`, `DISCOVERY`) · `docs/DESIGN_BRIEF.md` · `docs/INFORMATION_ARCHITECTURE.md` · `docs/DESIGN.md` (normative visual tokens) · `docs/mockup.png` (canonical layout reference).

Coding rules live in `opencode.json` → `docs/rules/` (`rust.md`, `qt-qml.md`, `frontend.md`, `testing-gui.md`) and are auto-loaded as instructions. The sections below restate the binding parts; rule files hold the detailed load-triggers.

## Operating Rules (binding)

1. **Work slice by slice.** Current queue is `docs/project/WORK_ITEMS.md` (S1 first). Do exactly one slice, keep requirement → slice → work item → evidence links current.
2. **Checks + commit after every task/phase/slice.** `rust-tc` is the public gate — never invoke `just` directly. "All checks" for the touched area, all green, then one commit per logical change:
   - Rust: `rust-tc doctor` (fmt + clippy + nextest + doctests + deny + shear + hack); fast loop: `rust-tc quick`. Sonar upload when requested: `rust-tc sonar` (never nests `doctor`).
   - QML: `qmllint` on touched files · `qmlformat --check` (or repo-configured verify) · CMake build passes
   - Design tokens touched: `npx @google/design.md lint docs/DESIGN.md` — 0 errors required
   - Packaging touched: `makepkg` / clean-chroot build check
   - Never commit with failing checks. Never commit secrets. Conventional Commits (`feat|fix|docs|refactor|test|chore(scope): …`).
   - A pre-commit hook enforces this: install with `scripts/install-git-hooks.sh` (runs `rust-tc doctor` + staged QML/DESIGN lints on every commit).
3. **Never silently change a locked decision** (`docs/project/DECISIONS.md` D-001–D-014). If a lock blocks you, run the change-impact loop: update the decision record + every downstream doc (SPEC, REQUIREMENTS, ARCHITECTURE, ROADMAP, design docs) in the same change, and say so in the commit message.
4. **Keep docs in sync.** Slices that alter behavior update `REQUIREMENTS.md` acceptance, `VALIDATION.md` evidence, and `STATE.md` (current slice, blockers, next action) in the same commit as the code.
5. **Perf numbers are aspirational until M6** — but keeping work off the UI thread is a correctness rule from day one.

## Dependency Freshness (binding)

Always build on the latest mutually-compatible versions — never pin old releases out of habit:

- **Rust:** `stable` channel only (`rust-toolchain.toml` tracks it); run `rustup update` when starting significant work. `rust-version` in `Cargo.toml` is a *floor* (currently 1.85, the edition-2024 minimum), not a pin — bump it only when new language features are actually used.
- **Crates:** introduce dependencies with `cargo add` (resolves latest), declare them in `[workspace.dependencies]` with caret requirements, and run `cargo update` before releases so `Cargo.lock` (committed) tracks latest semver-compatible. Upgrading a major version is a deliberate, tested change — full `rust-tc doctor` after.
- **System/CI deps:** PKGBUILD `depends`/`makedepends` follow current Arch repos (no frozen snapshots); refresh the exact set whenever the native or clean-chroot build drifts.
- **Stale check:** if a dependency is >1 minor version behind latest at release time, either update it or record why in the commit message / `DECISIONS.md`.

## Setup Commands (Arch Linux)

```bash
# System deps (pacman names indicative; pin exact set in PKGBUILD when it lands)
sudo pacman -S --needed qt6-base qt6-declarative qt6-shadertools cmake rustup \
  gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-plugins-ugly \
  gst-libav sqlite pipewire pkgconf

rustup default stable

# Configure + build (once CMakeLists lands in S1 W-001)
cmake -B build -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build build -j"$(nproc)"

# Rust-only fast loop
cargo test
cargo clippy --all-targets -- -D warnings
```

## Development Workflow

- Start from `docs/project/WORK_ITEMS.md`. Before editing, inspect the relevant crates/views and state the slice's concrete done condition.
- `tunex-core` must keep zero deps on Qt/GStreamer/SQLite/`notify`. `tunex-library`/`tunex-player` talk only via `tunex-core` types + channels. Only `tunex-app` depends on `cxx-qt`.
- QML modules via `qt_add_qml_module` with explicit URIs; theme tokens centralized (S1 W-003 consumes `docs/DESIGN.md`).
- Logging: `tracing` (structured). Scan progress, GStreamer bus, and search latency at debug.
- MPRIS smoke: `playerctl -p tunex play-pause / next / previous / metadata` while playing.

## Testing Instructions

- `cargo test` — unit/component: queue/shuffle/repeat machines, `stable_key` reconcile, FTS5 ranking, playlist ops, migration v1→v2.
- Integration fixtures (S1 W-006): MP3/FLAC/OGG/M4A/Opus/WAV + one corrupt file → explicit error, app survives.
- Manual DoD rehearsal per slice: install → add folder → scan-while-browsing → search → play/seek/queue/playlist → `playerctl` → restart → full run with networking disabled.
- 12k-track fixture for jank observation (numbers not gating pre-M6).
- Add or update tests for every behavior change, even if unasked.

## GUI Testing (binding for visual/DoD verification)

- **Kwin-MCP is the GUI automation path** (there is no separate built-in Computer Use tool here). Load the `kwin-mcp` skill and follow `docs/rules/testing-gui.md`: isolated `session_start` by default, fixture libraries with `isolate_home`, semantic inspection before acting, keyboard-first flows + `playerctl` cross-checks, screenshot evidence into `docs/project/VALIDATION.md`, always `session_stop`.
- GUI runs prove integration (slices' DoD, glass/grids/dialogs/empty states, MPRIS/keys, M5 X11 pass). Algorithms stay in `cargo test` / `qmltestrunner`.

## Frontend Rules (binding for any UI work)

1. **Load the skill trio first** — `craft-beautiful-frontend`, `iconography-frontend-ui`, `responsive-design` — and follow them plus their references. Web/CSS specifics translate to QML: tokens live in `docs/DESIGN.md` + the Theme singleton (never ad-hoc hexes), motion uses QML Behaviors within the budgets in `docs/DESIGN.md`, responsive means the window classes in `docs/DESIGN_BRIEF.md` (desktop-first, 960×640 min; no mobile breakpoints).
2. **Follow `docs/mockup.png` for layout/chrome/components; follow `docs/DESIGN_BRIEF.md` adaptation map for content.** The mockup shows streaming content — implement the same components bound to local data (hero→local greeting, Trending→Recently Played, mixes→playlists/Recently Added, Liked→Favorites, lyrics tabs→Up Next; bell/avatar removed). Where mockup and `docs/DESIGN.md` disagree on a V1-out-of-scope pattern, `DESIGN.md` wins.
3. **Real components, real data only.** Implement the components in the mockup bound to real Rust `QAbstractListModel`s. No lorem-ipsum, no invented screens, no new component concepts. Placeholders allowed only where specified: generated artwork placeholder, "Unknown" metadata, EmptyState variants in `docs/DESIGN.md`.
4. **`docs/DESIGN.md` tokens are normative.** Prose explains; frontmatter decides. Never contradict token values. Run its lint (0 errors) when touching it.
5. **Progressive disclosure everywhere** (see brief + `docs/INFORMATION_ARCHITECTURE.md` disclosure map): hover/focus reveals row actions with always-visible touch targets, drawers over new pages, "See all"/"More" escalation, context menus over toolbars, ≤7 primary choices per view.
6. **Quality gates per UI change** (from `craft-beautiful-frontend`): type scale respected · 4px spacing · AA contrast · one primary action per view · 44px targets (dense-list 40px exception per `DESIGN.md`) · motion in budget + reduce-motion honored · focus ring visible · empty/loading/error/success states for every async surface · no color-only meaning.
7. **Icons:** single Lucide-style line family (2px @24px, rounded), 16/20/24px sizes, icon+text default; icon-only transport controls carry accessible names + tooltips and 3:1 contrast. No emoji-as-icon, no mixed families.

## Generated Assets

- If imagery must be generated (empty-state illustrations, hero backdrops, placeholder textures — **never album artwork**: unknown art stays the generated monogram placeholder per "unknown > incorrect"), use OpenRouter MCP image generation with the `openai/gpt-image-2.5-sunburst` model.
- Save outputs under `assets/` with a sidecar note of the prompt + model + date, and reference them from the commit message.

## Code Style

- Rust: `rustfmt` default + `clippy -D warnings`; `thiserror`/`anyhow` error style per crate; `tokio` for workers; `tracing`, never `println!` in library code.
- QML: `qmlformat` clean, `qmllint` clean; component names match `docs/DESIGN_BRIEF.md` inventory (`TrackRow`, `AlbumCard`, `MiniPlayer`, …); no business logic in QML — no direct imports of FS/DB/media.
- Naming: domain `Track/Album/Artist/Queue/Playlist`; UI labels per IA glossary (`Songs` view, `Up Next` panel, `Music folders`).

## Language Skill Triggers (binding)

- Any Rust work (`.rs`, `Cargo.toml`): load `ms-rust` + `rust-best-practices` + `rust-reference` first. Load `rust-optimise` only for M6/profile-backed optimization — never on intuition.
- Any QML/CMake work: load per `docs/rules/qt-qml.md` — always `qt-qml` for `.qml` edits and `qt-project` for CMake/targets; add `qt6-qml-development` (exact name) for Qt6 APIs/bridge patterns, `qt-ui-design` for screen implementation, `qt-qml-review` before committing QML, `qt-qml-test`/`qt-qml-test-run` for QML tests, `qt-qml-docs` for component docs, `qt-qml-profiler` for M6/perf hunts. Never `qt-cpp-review`/`qt-cpp-docs` for app code (no C++ — `cxx-qt`/Rust only).

## Build and Deployment

- Single entry: CMake + Corrosion (`cmake -B build && cmake --build build`). CI must cover native build + `cargo test` + `qmllint`/`qmlformat` + PKGBUILD/`makepkg` check.
- V1 artifact: AUR PKGBUILD with exact pacman `depends`/`makedepends` (Qt6, GStreamer + plugin sets, SQLite). No dev-package burden on users.
- Pre-migration DB backup; rollback = previous package version (`pacman -U`) + DB backup restore.

## Security Considerations

- Canonicalize every library path; reject symlinks escaping roots; argv-only process/file APIs (no shell); embedded images treated as opaque bytes with decode caps (see SPEC §10.1); no network calls in V1 code paths.
- V1 collects nothing: no telemetry, no accounts, no secrets to manage.

## Troubleshooting

- `cxx-qt`/Qt version skew → check the pinned versions in Cargo + CMake first; only `tunex-app` may include Qt headers.
- QML shows blank lists → the Rust model isn't emitting through `AppEvent` on the Qt thread; workers must never touch `QObject`s.
- GStreamer "missing plugin" on Arch → install the corresponding `gst-plugins-*` set; record it in PKGBUILD deps.
- Wayland decoration/input quirks → verify against X11 session too (both are V1 targets, M5 gate).
