# Discovery & feasibility — TuneX (spikes skipped per user 2026-09-09)

## Constraints inventoried
- Tech: Rust + Qt6 Quick + cxx-qt + CMake/Corrosion; GStreamer playbin3; PipeWire default; SQLite WAL + rusqlite_migration + FTS5; notify debounced; tokio AppEvent bus. Wayland-first, X11-compat, high-DPI, a11y-conscious.
- Time/budget: unspecified — assumed iterative MVP slices, no hard date. Assumption.
- Legal: Qt Community dual-license (LGPLv3/GPLv3, some GPL-only modules); app GPLv3 recommended + dynamic linking. No user data collection in V1 (local-first) → minimal privacy surface.
- Integrations: AUR PKGBUILD primary for V1 (Flatpak deferred post-V1). MPRIS/D-Bus, media keys, notifications, native file dialogs. No network services in V1.
- Operational: XDG config/cache/data; offline-mandatory core flows.

## Security / privacy / a11y / data lifecycle (proportionate)
- Untrusted files: canonicalize paths, reject symlink escape, argv-only APIs, embedded images as opaque bytes, artwork decode caps (see SPEC §10.1/§29).
- Privacy: no accounts/telemetry in V1; nothing leaves machine.
- A11y: keyboard + media keys, focus-visible, sufficient contrast, blur-off/reduced-motion setting, software-GL fallback.
- Data: SQLite is source of truth for index, files are truth for audio. Missing-file → missing-flag + reconcile, playlists keep dangling-as-missing. Migrations versioned from M2.

## Top risks
| # | Risk | L / I | Mitigation / accept |
|---|------|-------|---------------------|
| R1 | cxx-qt + CMake/Corrosion build fragility (Qt version skew, codegen, AUR/pacman deps) | H / H | Mitigate: M0 proof (1 model + 1 signal round-trip) + CI native+makepkg from day 1; pin cxx-qt/Qt; keep only tunex-app touching Qt. No spike per user — accept residual M0 risk, fail fast in S1. |
| R2 | GStreamer gapless/seek reliability across codecs + PipeWire device loss | M / H | Mitigate: playbin3 + about-to-finish, bus-thread AppEvents, preserve-state-on-device-loss; test matrix MP3/FLAC/OGG/M4A/Opus/WAV in S1. |
| R3 | Library scan/metadata throughput + artwork decode jank on 50k libs | M / M | Accept for V1 function-first: background tokio workers, lazy art, progress UI. Optimize M6 vs aspirational budgets. |
| R4 | QML perf/a11y of glass/blur on iGPU + software GL | M / M | Mitigate: restrained glass (opaque rows, glass only for hierarchy), MultiEffect sparingly, blur-off switch, profile M6. |
| R5 | Scope creep (enrichment, DSP, smart playlists, visualizers) delaying V1 | M / M | Mitigate: locked out-of-V1 list + vertical slices; any addition needs product-loop + human confirm. |

## Feasibility stance
**Go with conditions:** M0/S1 must prove cxx-qt round-trip + playbin3 playback + PKGBUILD build. If any of those three fails, stop and revisit D-001/D-002/D-005 before proceeding to library slices.

## Sources / assumption labels
- Observed: empty repo (no commits, only docs/SPEC.md), 2026-09-09.
- Locked by user: SPEC v1.1 stack (§40), cxx-qt, perf-aspirational policy.
- Assumptions: no hard deadline; GPLv3 app license (needs human confirm); 50k-track design point.
