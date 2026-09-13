# Architecture — TuneX

## Overview
Fast Rust engine + beautiful Qt Quick shell. QML renders; Rust decides. `tunex-app` bridges via cxx-qt; `tunex-library` indexes; `tunex-player` plays; SQLite is index truth, files are audio truth. All heavy work off UI thread via tokio AppEvent bus; GStreamer bus isolated. AUR PKGBUILD is the V1 artifact (Flatpak deferred). Perf numbers aspirational until M6.

## Components
| Component | Responsibility | Notes |
|-----------|----------------|-------|
| QML shell (`qml/`) | Nav, theme tokens, lists/grids, player UI, search field, dialogs | `qt_add_qml_module`; models are Rust QAbstractListModels; qmllint/qmlformat gated |
| tunex-app | cxx-qt QObjects, QML models, AppEvent fan-out, watcher debounce, settings (XDG), MPRIS/D-Bus, StatusNotifierItem tray, config | Only crate touching Qt; owns Qt-thread affinity |
| tunex-library | Roots, recursive scan, metadata trait, SQLite + migrations + FTS5 search controller, artwork pipeline, playlists/favorites/history | Largest crate by design; split later per D-008 |
| tunex-player | PlayerEngine over playbin3, queue + shuffle/repeat, volume/mute, position/bus events | Emits TrackChanged/State/Position/Duration/Error/EndOfTrack as AppEvents |
| tunex-core | Domain types (Track/Album/Artist/Queue/Playlist/...), PlayerState machine, errors, AppEvent enum, config schema | No Qt/GStreamer/SQLite/notify deps |
| SQLite | artists/albums/tracks/genres/folders/playlists/playlist_tracks/favorites/play_history/library_roots/scan_state/artwork/settings + FTS5 external-content | WAL, versioned migrations, BM25 |
| GStreamer playbin3 | Decode/seek/buffer/gapless/volume | about-to-finish preload; volume/tag tap reserved |
| notify watcher | FS events → enqueue paths | Debounce/coalesce in app; never heavy work on callback |

## Data flow
```
QML action → cxx-qt slot → tunex-app → tokio worker (library/player)
  → SQLite / FS / GStreamer → AppEvent broadcast → tunex-app Qt signal
  → QML model refresh / player state / progress bar
Search: QML field (150ms debounce) → search controller → FTS5 BM25 → ranked models (stale-cancel)
Scan: roots → walker → meta/art workers → SQLite txns → progress AppEvents → grid increments
Play: queue.next → engine.preload(about-to-finish) → bus → TrackChanged/Position → QML + MPRIS
```

## Trust & boundaries
- Authn/z: none (local single-user). No secrets in V1.
- Data sensitivity: file paths + listening history stay local; no egress.
- Retention/deletion: remove root → stop watch + GC orphan index rows; missing files flagged not silently dropped.
- External calls: none required. Future enrichment (MusicBrainz/lyrics) sandboxed + opt-in, post-V1.
- Boundary enforced: QML cannot import FS/DB/media; review any new cxx-qt method for logic leakage.

## Failure modes
| Failure | Detection | Response / recovery |
|---------|-----------|---------------------|
| Unsupported/corrupt media | GStreamer bus + meta error | Skip + actionable toast/log; app survives (R-NFR-05) |
| Permission denied / missing file | Scan/play error | Mark missing, reconcile, keep playlist entry as missing |
| DB failure / migration fail | Startup check | Refuse upgrade-safe open, backup + explicit error, never silent wipe |
| Device unavailable | Bus message | Preserve queue/position, retry on return |
| Oversized/corrupt art | Decode guard | Placeholder, background evict |
| Watcher flood | Queue depth metric | Coalesce + backpressure, progress stays live |

## Test strategy
- Unit/component: core state machines (queue/shuffle/repeat), stable_key/reconcile, FTS5 ranking, playlist ops; `cargo test`.
- Integration/contract: scan→DB→search→play headless fixtures (MP3/FLAC/OGG/M4A/Opus/WAV); migration v1→v2; cxx-qt model round-trip test.
- End-to-end/manual: DoD demo script (install→add→scan→browse→search→play→MPRIS→queue→playlist→restart→offline); playerctl checks; network-off run.
- Risk-specific: codec matrix in S1; 12k/50k seed for jank observation (numbers not gating pre-M6); Arch container/VM `makepkg -si` run per slice.

## Operability
- Config: `~/.config/tunex/` (TOML/serde); cache `~/.cache/tunex/art/`; data `~/.local/share/tunex/` (DB). Documented keys per R-015.
- Secrets: none in V1.
- Logging/metrics: `tracing` structured logs; scan counters + progress %; GStreamer bus logged at debug.
- Rollback sketch: DB backup pre-migration; `pacman -U` previous package version rollback; no remote state to reconcile.
- Build: CMake+Corrosion single entry + PKGBUILD; CI native + `makepkg` check; qmllint/qmlformat required.

## Alignment to locks
- D-001→core Rust; D-002→QML/SceneGraph/RHI; D-003→cxx-qt models/signals; D-004→CMake/Corrosion; D-005→playbin3/about-to-finish; D-006→SQLite/FTS5; D-007→tokio AppEvent; D-008→4 crates; D-009→stable_key; D-010→art pipeline; D-011b→PKGBUILD/AUR V1; D-012→M6 perf; D-013→GPLv3; D-014→offline + boundary.
