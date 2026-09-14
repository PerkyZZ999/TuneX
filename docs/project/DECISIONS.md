# Decisions — TuneX (source SPEC §40 v1.1)

## D-001 — Rust as application language
- **Status:** locked
- **Date:** 2026-09-09
- **Owner:** user
- **Context:** Long-running desktop app needing safe concurrency, FS tooling, media/DB ecosystem, no GC.
- **Decision:** Rust owns all domain behavior.
- **Rationale:** Memory safety + perf + ecosystem; separates UI from logic.
- **Evidence:** SPEC §3.1, §40.
- **Consequences:** Team must be Rust-capable; Qt interaction only via bridge.
- **Alternatives:** C++ (Qt-native, rejected: safety/velocity), Electron/TS (rejected: non-native, heavy).
- **Reopen criteria:** Only if hiring/skill constraint forces language change — full replan.
- **Confirmed by / date:** user, 2026-09-09 (SPEC)
- **Supersedes / superseded by:** —

## D-002 — Qt 6 + Qt Quick/QML + SceneGraph/RHI (Vulkan pref)
- **Status:** locked
- **Date:** 2026-09-09
- **Owner:** user
- **Context:** Need GPU animation/models/effects, Linux-native Wayland+X11.
- **Decision:** Qt Quick (not Widgets); SceneGraph over RHI; Vulkan preferred with fallback; MultiEffect + custom shaders sparingly; custom QRhi only for visualizer/waveform-grade needs.
- **Rationale:** Only Qt path delivering Spotify-grade fluid UI natively on Linux.
- **Evidence:** SPEC §3.2–3.3, §40.
- **Consequences:** QML design system required; GPU testing matrix needed.
- **Alternatives:** Widgets (rejected: insufficient animation/effects), GTK/Adwaita (rejected: weaker GPU canvas), Tauri/Electron (rejected: non-native).
- **Reopen criteria:** Qt6 Wayland showstopper with no workaround.
- **Confirmed by / date:** user, 2026-09-09
- **Supersedes / superseded by:** —

## D-003 — cxx-qt Rust↔QML bridge
- **Status:** locked
- **Date:** 2026-09-09
- **Owner:** user
- **Context:** Biggest gap in SPEC v1.0; user explicitly chose cxx-qt this turn.
- **Decision:** cxx-qt, pinned; QObjects/models in Rust, QML calls only via bridge; workers → AppEvent → queued signals; version pinned.
- **Rationale:** Real QObject support, QML singletons/models, Qt6-active, explicit affinity.
- **Evidence:** SPEC §3.2.1; user message 2026-09-09.
- **Consequences:** CMake/Corrosion required; only tunex-app touches Qt.
- **Alternatives:** qmetaobject-rs (rejected: weaker Qt6), Slint (rejected: abandons Qt lock).
- **Reopen criteria:** cxx-qt fails S1 round-trip or blocks PKGBUILD/AUR with no workaround.
- **Confirmed by / date:** user, 2026-09-09
- **Supersedes / superseded by:** —

## D-004 — CMake top-level + Corrosion + Cargo
- **Status:** locked
- **Date:** 2026-09-09
- **Owner:** user
- **Context:** Qt/QML + cxx-qt codegen + native/AUR packaging need CMake entrypoint.
- **Decision:** CMake drives, Corrosion builds Cargo workspace, cxx-qt codegen in-cargo, qt_add_qml_module versioned, CI builds native + PKGBUILD check from same entry.
- **Rationale:** Only setup satisfying qmllint/qmlformat + PKGBUILD + cxx-qt together.
- **Evidence:** SPEC §3.7.
- **Consequences:** Contributors need CMake+Qt+GStreamer dev packages for native builds.
- **Alternatives:** Pure Cargo (rejected: QML module/packaging pain).
- **Reopen criteria:** Corrosion/cxx-qt incompatibility with Qt 6.x LTS.
- **Confirmed by / date:** user, 2026-09-09
- **Supersedes / superseded by:** —

## D-005 — GStreamer playbin3 + PipeWire output picker
- **Status:** locked
- **Date:** 2026-09-09
- **Owner:** user
- **Context:** Need codecs/seek/gapless without hand-rolled engine.
- **Decision:** playbin3 + about-to-finish preload; volume/tag tap used for ReplayGain and a short crossfade envelope; PipeWire default output with an optional device picker (empty id = System). Changing device must not drop the queue.
- **Rationale:** Mature gapless path; PipeWire via GStreamer sinks.
- **Evidence:** SPEC §3.4–3.5, §12.1; S11 W-058–W-060.
- **Consequences:** GStreamer + plugins are hard pacman deps in PKGBUILD; Settings → Playback lists sinks from `GstDeviceMonitor`.
- **Alternatives:** Rodio/Symphonia direct (rejected: codec burden), QtMultimedia (rejected: control/licensing).
- **Reopen criteria:** Proven gapless failure across target codecs with no pipeline fix.
- **Confirmed by / date:** user, 2026-09-09; picker added 2026-09-13 (post-V1, same lock).
- **Supersedes / superseded by:** —

## D-006 — SQLite WAL + rusqlite_migration + FTS5
- **Status:** locked
- **Date:** 2026-09-09
- **Owner:** user
- **Context:** Indexed local library + instant search, files stay on disk.
- **Decision:** SQLite stores index/state not audio; WAL + prepared + txns + indexes; rusqlite_migration from M2; FTS5 external-content + triggers + BM25 + LIMIT 200.
- **Rationale:** Boring reliable local DB with best text search story.
- **Evidence:** SPEC §3.6, §11.2–11.3.
- **Consequences:** Schema changes always migrated; search ranking tuned once.
- **Alternatives:** sled/redb/Tantivy (rejected: ops/tuning burden).
- **Reopen criteria:** Proven 50k-lib FTS5 path failure in M6 profiling.
- **Confirmed by / date:** user, 2026-09-09
- **Supersedes / superseded by:** —

## D-007 — tokio + AppEvent bus threading
- **Status:** locked
- **Date:** 2026-09-09
- **Owner:** user
- **Context:** UI must never block on scan/meta/art/DB/search.
- **Decision:** tokio workers; bounded mpsc + broadcast/watch AppEvent; only tunex-app touches QObject; notify callback enqueues only; GStreamer bus on own thread.
- **Rationale:** Explicit affinity, cancellable search, debounced watcher.
- **Evidence:** SPEC §26.
- **Consequences:** All cross-thread APIs must be Send + event-driven.
- **Alternatives:** rayon/std-threads only (rejected: async cancel/timers weaker).
- **Reopen criteria:** Qt-thread interop deadlock attributable to runtime.
- **Confirmed by / date:** user, 2026-09-09
- **Supersedes / superseded by:** —

## D-008 — 4-crate workspace to start
- **Status:** locked
- **Date:** 2026-09-09
- **Owner:** user
- **Context:** 9-crate split risked circular deps early.
- **Decision:** core / library / player / app; split library→metadata/database/search/playlists and app→platform only when boundaries hurt. core has no Qt/GStreamer/SQLite/notify; only app has cxx-qt.
- **Rationale:** Minimize friction, enforce dependency arrows.
- **Evidence:** SPEC §5.
- **Consequences:** library crate is large initially by design.
- **Alternatives:** 9 crates day 1 (rejected).
- **Reopen criteria:** library crate cohesion failure (explicit module pain).
- **Confirmed by / date:** user, 2026-09-09
- **Supersedes / superseded by:** —

## D-009 — Track identity + missing-flag reconcile
- **Status:** locked
- **Date:** 2026-09-09
- **Owner:** user
- **Context:** Renames must not duplicate or lose playlist/history links.
- **Decision:** rowid + stable_key(path+mtime+size); inode/content-hash assist on move; missing=1 then GC; playlists keep dangling-as-missing.
- **Rationale:** Preserves user data across FS churn.
- **Evidence:** SPEC §11.1.
- **Consequences:** Scanner must canonicalize + compare keys, not just paths.
- **Alternatives:** Path-only identity (rejected: dup/loss on rename).
- **Reopen criteria:** Proven cross-FS inode instability requiring new key.
- **Confirmed by / date:** user, 2026-09-09
- **Supersedes / superseded by:** —

## D-010 — Artwork pipeline
- **Status:** locked
- **Date:** 2026-09-09
- **Owner:** user
- **Context:** Artwork is signature visual; must never jank UI.
- **Decision:** Local order is unchanged: embedded > cover/folder/front.jpg; XDG cache 64/256/512+orig; blake3 key; ~2GB LRU; decode caps; lazy/background; placeholder. MusicBrainz / Cover Art Archive is an **opt-in extra source after local miss**, default off (R-022). Remote fills never overwrite user tags or local art.
- **Rationale:** Deterministic, restart-stable, bomb-safe. Network stays never required.
- **Evidence:** SPEC §10.1; post-V1 S13.
- **Consequences:** Other filename heuristics deferred. Opt-in enrichment writes Cover Art Archive bytes into the XDG cache (`remote/` pointers) only when embedded and folder art are missing.
- **Alternatives:** Wider heuristics V1 (rejected: scope); always-on MusicBrainz (rejected: D-014).
- **Reopen criteria:** User signal that V1 misses dominant naming convention.
- **Confirmed by / date:** user, 2026-09-09; extra-source consequence updated 2026-09-13 (S13)
- **Supersedes / superseded by:** —

## D-011 — Flatpak primary packaging
- **Status:** superseded by D-011b
- **Date:** 2026-09-09
- **Owner:** user
- **Context:** Universal Linux distribution without dev-dep burden.
- **Decision:** ~~Flatpak from M0 skeleton; native debs/rpms + AppImage later~~ — replaced per user 2026-09-09.
- **Rationale:** One tested artifact incl. Qt/GStreamer/Plugins.
- **Evidence:** SPEC §31 (pre-change), §38 M0.
- **Consequences:** See D-011b.
- **Alternatives:** Native-only (rejected: fragmentation).
- **Reopen criteria:** Superseded — see D-011b.
- **Confirmed by / date:** user, 2026-09-09
- **Supersedes / superseded by:** superseded by D-011b

## D-011b — PKGBUILD / AUR primary packaging (replaces Flatpak for V1)
- **Status:** locked
- **Date:** 2026-09-09
- **Owner:** user
- **Context:** User requested "Change Flatpak for PKGBUILD" at decision-lock checkpoint; Arch-first workflow.
- **Decision:** V1 primary artifact is Arch `PKGBUILD` (AUR) with pacman system deps (Qt6, GStreamer + plugins, SQLite). PKGBUILD skeleton from M0; CI checks PKGBUILD build. Flatpak + debs/rpms + AppImage deferred post-V1.
- **Rationale:** Native Arch integration, pacman-managed deps, simplest path for author's distro; avoids Flatpak SDK/portal overhead in V1.
- **Evidence:** User checkpoint answer 2026-09-09; SPEC §31 patched.
- **Consequences:** V1 install story is Arch-only; universal coverage postponed; no sandbox portals to handle (direct FS/MPRIS); must declare makedepends/depends precisely; revisit universal packaging when leaving Arch-only audience.
- **Alternatives:** Flatpak primary (rejected for V1 by user, kept as deferred post-V1 option).
- **Reopen criteria:** Decision to support non-Arch users in V1, or AUR maintenance burden proves excessive → revisit Flatpak.
- **Confirmed by / date:** user, 2026-09-09
- **Supersedes / superseded by:** supersedes D-011

## D-012 — Perf aspirational, optimize in M6
- **Status:** locked
- **Date:** 2026-09-09
- **Owner:** user
- **Context:** User explicitly deprioritized perf-blocking; functional first.
- **Decision:** Targets (1.5s cold 50k, p95 search <50ms, 60fps, <50ms controls, >500/min scan) are desired not gating; profile + optimize in M6 with QML profiler + tracing.
- **Rationale:** Protect velocity; avoid premature optimization.
- **Evidence:** SPEC §25; user message 2026-09-09.
- **Consequences:** S1–S5 must still keep work off UI thread (correctness), but need not hit numbers.
- **Alternatives:** Perf-gated slices (rejected by user).
- **Reopen criteria:** User reprioritizes perf earlier.
- **Confirmed by / date:** user, 2026-09-09
- **Supersedes / superseded by:** —

## D-013 — App license GPLv3 recommended
- **Status:** locked
- **Date:** 2026-09-09
- **Owner:** user
- **Context:** Qt Community dual-license; some modules GPL-only; need frictionless native/AUR distribution.
- **Decision:** TuneX app GPLv3 + Qt Community + LGPL dynamic linking; avoid Charts/DataVis/VirtualKeyboard unless approved; final review before distribution.
- **Rationale:** Maximizes Qt module compatibility; standard for community Linux apps.
- **Evidence:** SPEC §35.
- **Consequences:** Copyleft applies to app; alternative MIT/Apache needs module audit.
- **Alternatives:** MIT/Apache-2.0 (viable only after GPL-only module audit).
- **Reopen criteria:** Commercial/dual-license intent or legal advice otherwise.
- **Confirmed by / date:** user, 2026-09-09
- **Supersedes / superseded by:** —

## D-014 — Local-first + strict QML/Rust boundary
- **Status:** locked
- **Date:** 2026-09-09
- **Owner:** user
- **Context:** Core product model; keeps UI replaceable and testable.
- **Decision:** Network never required for core flows (R-001–R-021). QML presentation only, Rust logic only, no FS/DB/GStreamer/HTTP from QML. Opt-in MusicBrainz (R-022) is the explicit product-loop exception: default **off**, worker + timeout + XDG cache, never overwrites user tags/art; `scripts/netoff-rehearsal.sh` must keep passing with enrichment off.
- **Rationale:** Offline reliability + clean testing seams.
- **Evidence:** SPEC §2.1, §4–5; post-V1 S13.
- **Consequences:** Every feature needs Rust API + QML model; no QML shortcuts. Core playback, scan, search, and queue stay fully offline.
- **Alternatives:** QML-side logic (rejected); required network for metadata (rejected).
- **Reopen criteria:** Only via explicit product-loop adding further network scope.
- **Confirmed by / date:** user, 2026-09-09; product-loop exception documented 2026-09-13 (S13)
- **Supersedes / superseded by:** —
