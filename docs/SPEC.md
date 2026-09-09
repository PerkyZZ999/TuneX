# TuneX — SPEC.md

> A Linux-native, local-first, high-performance desktop music player with a modern Spotify-inspired experience.

**Project:** TuneX  
**Target:** Linux desktop  
**Status:** Pre-development / architecture locked  
**Primary design goal:** A beautiful, extremely responsive local music application that feels like a first-class Linux desktop product rather than a web wrapper.

---

## 1. Vision

TuneX is a Linux-native desktop music player for users who own and manage local music files.

The product should combine:

- Spotify-like information architecture and usability
- A distinctly Linux-native desktop experience
- Extremely fast library browsing and search
- Gapless, reliable local playback
- A GPU-accelerated modern interface
- Glassmorphism, translucency, shadows, glows, gradients and subtle depth
- Excellent keyboard and media-key support
- Offline-first operation
- A clean, maintainable Rust core

TuneX is **not** intended to reproduce Spotify's online service model. There are no accounts, subscriptions, mandatory cloud services or mandatory network dependencies.

The local music library is the source of truth.

---

# 2. Product Principles

## 2.1 Local-first

TuneX must remain fully useful without an Internet connection.

Optional online services may enrich metadata later, but:

> Network availability must never be required for core playback or library functionality.

## 2.2 Fast by default

All expensive operations must be asynchronous or incremental.

The UI must never block on:

- filesystem traversal
- metadata extraction
- image decoding
- database queries
- playlist operations
- audio analysis
- library synchronization

## 2.3 Unknown is better than incorrect

Metadata, artwork and derived information must not be fabricated.

If TuneX cannot determine something reliably, it should represent it as unknown rather than displaying misleading information.

## 2.4 Native desktop experience

TuneX should behave like a real Linux application:

- Wayland-first
- X11 compatibility
- native keyboard handling
- media keys
- MPRIS integration
- desktop notifications where appropriate
- native filesystem dialogs
- high-DPI support
- accessibility-conscious UI

## 2.5 Visual quality is a product feature

The UI is not merely a shell around the player.

Rendering, animation, spacing, typography, album artwork and transitions should receive first-class engineering attention.

---

# 3. Technology Stack

## 3.1 Core language

**Rust**

Rust is the primary application language.

Reasons:

- memory safety
- predictable performance
- strong concurrency model
- excellent filesystem tooling
- strong ecosystem for media and metadata
- excellent fit for long-running desktop applications
- no garbage collector
- easy separation between UI and domain logic

Rust owns all application/domain behavior.

---

## 3.2 UI framework

**Qt 6 + Qt Quick + QML**

TuneX will use Qt Quick rather than Qt Widgets.

Qt Quick provides the visual canvas, animation system, models/views and graphical effects needed for the application's design. Qt Quick is specifically designed around a scene graph rendering architecture. citeturn0search4turn0search3

QML is presentation-layer code only.

Business logic must remain in Rust.

### 3.2.1 Rust ↔ QML bridge — LOCKED

**`cxx-qt`**

`cxx-qt` is the locked bridge between Rust and Qt 6 / QML.

Reasons:

- real `QObject` definitions in Rust with Qt meta-object support
- first-class QML singleton / model exposure
- actively maintained Qt 6 support
- compatible with CMake + Corrosion workflow
- keeps ownership/thread-affinity explicit, which TuneX needs for audio + workers

Rules:

- QML never calls filesystem, SQLite, or GStreamer directly. All calls go through `cxx-qt` bridged Rust objects.
- All list data exposed to QML via `QAbstractListModel` subclasses in Rust (tracks, albums, queue, playlists, search results).
- All async Rust work reports back via queued signals / `AppEvent` broadcast, never via direct UI-thread blocking.
- `cxx-qt` version is pinned in Cargo + CMake and updated deliberately.

---

## 3.3 Rendering

**Qt Quick Scene Graph + Qt RHI**

Qt 6's Scene Graph renders through the Qt Rendering Hardware Interface (RHI), which abstracts Vulkan, OpenGL and other graphics APIs. On Linux, Vulkan should be the preferred backend when available, while maintaining fallback compatibility. citeturn0search8turn0search12

TuneX should use:

- Qt Quick Scene Graph
- RHI
- GPU-accelerated composition
- custom QML components
- custom shaders where beneficial
- `MultiEffect` for appropriate blur/shadow/effect use
- custom scene-graph/RHI rendering only where the standard Qt Quick pipeline is insufficient

Qt supports integrating custom QRhi rendering into Qt Quick scenes, including underlay/overlay and texture-based approaches. This gives TuneX a path to advanced visualizations without abandoning Qt Quick. citeturn0search0turn0search13

### Important constraint

Do not use QRhi or private rendering APIs everywhere.

Prefer normal Qt Quick/QML rendering first. Use low-level rendering only for features that materially benefit from it, such as:

- audio visualizers
- advanced waveform rendering
- specialized GPU effects
- future spectrum analysis visualization

---

## 3.4 Audio engine

**GStreamer**

GStreamer is the media pipeline responsible for playback.

TuneX should not implement its own codec/audio decoding engine.

Responsibilities include:

- decoding
- playback
- seeking
- buffering
- gapless playback
- format handling
- volume
- pipeline state
- audio processing where appropriate

The TuneX player layer should abstract GStreamer behind a Rust `PlayerEngine` interface.

---

## 3.5 Linux audio

**PipeWire**

PipeWire is the preferred Linux audio integration layer.

TuneX should integrate cleanly with modern PipeWire-based desktops while allowing the underlying GStreamer pipeline to handle supported output configuration.

Future work may expose:

- output device selection
- volume
- channel configuration
- sample-rate information
- exclusive-mode options where technically appropriate

---

## 3.6 Database

**SQLite**

SQLite is the persistent local library database.

SQLite should store indexed metadata and application state, not the music files themselves.

Recommended extensions/features:

- WAL mode
- prepared statements
- transactions
- indexes
- FTS5 for search

---

## 3.7 Build system — LOCKED

**CMake (top-level) + Corrosion + Cargo**

- CMake is the top-level build driver because Qt/QML, `qmllint`/`qmlformat`, and PKGBUILD/AUR packaging expect it.
- Rust is built via Corrosion (`corrosion_import_crate`) from the Cargo workspace.
- `cxx-qt` Qt codegen runs as part of the Cargo/Corrosion build.
- The QML module (URI `TuneX`) is defined exactly once in `crates/tunex-app/build.rs` (`CxxQtBuilder` `QmlModule`); CMake must NOT redeclare it via `qt_add_qml_module` (duplicate registration). QML sources live in `qml/` and are checked with standalone `qmllint`/`qmlformat`.
- CI must build both native (dev) and PKGBUILD (release check) via this same CMake entrypoint.

---

# 4. Proposed Architecture

```text
┌──────────────────────────────────────────────────────────┐
│                      TuneX UI                            │
│                  Qt Quick / QML                          │
│                                                          │
│  Navigation │ Library │ Search │ Player │ Now Playing   │
└─────────────────────────┬────────────────────────────────┘
                          │
                     Rust/QML API
                          │
┌─────────────────────────▼────────────────────────────────┐
│                    Application Core                       │
│                                                          │
│  Library │ Player │ Queue │ Playlists │ Search │ Config │
└──────────┬───────────────┬───────────────────┬───────────┘
           │               │                   │
           ▼               ▼                   ▼
      SQLite/FTS5      GStreamer           Filesystem
                           │               + notify
                           ▼
                       PipeWire
```

The architecture must preserve a strict boundary:

**QML = presentation**

**Rust = application logic**

---

# 5. Rust Workspace

The project should be organized as a Cargo workspace.

Initial structure (V1 — intentionally small):

```text
tunex/
├── Cargo.toml
├── crates/
│   ├── tunex-core/      # domain types, errors, config, AppEvent bus
│   ├── tunex-library/   # scan + metadata + sqlite + fts5 + artwork + playlists + search
│   ├── tunex-player/    # GStreamer engine + queue + MPRIS state source
│   └── tunex-app/       # wiring, cxx-qt bridge, QML models, watcher, settings, platform
│
├── qml/
│   ├── App.qml
│   ├── components/
│   ├── views/
│   ├── player/
│   ├── library/
│   ├── search/
│   ├── settings/
│   └── theme/
│
├── assets/
├── packaging/
├── tests/
└── docs/
```

Rationale: 4 crates avoid early circular dependencies (Playlist ↔ Search ↔ DB ↔ Library).
Split `tunex-library` into `tunex-metadata / tunex-database / tunex-search / tunex-playlists` and split `tunex-platform` out of `tunex-app` only when boundaries hurt.

Inter-crate rules:

- `tunex-core` has no dependency on Qt, GStreamer, SQLite, or `notify`.
- `tunex-library` and `tunex-player` communicate only via `tunex-core` types + channels.
- Only `tunex-app` depends on `cxx-qt` / Qt.

---

# 6. Application Core

The application core should expose domain-level concepts rather than leaking implementation details into QML.

Important concepts:

```text
Track
Album
Artist
Genre
Folder
Playlist
Queue
Library
PlaybackState
PlaybackPosition
PlayerState
Artwork
Settings
```

Example high-level player states:

```text
Stopped
Loading
Playing
Paused
Seeking
Buffering
Error
```

---

# 7. Library Engine

The Library Engine is one of TuneX's most important subsystems.

It maintains an indexed representation of the user's music collection.

Capabilities:

- add library folders
- remove library folders
- recursively scan folders
- detect supported media files
- extract metadata
- extract embedded artwork
- generate/cache artwork representations
- detect changed files
- detect deleted files
- incremental rescanning
- filesystem watching
- database reconciliation

Scanning must happen in background workers.

The UI should immediately remain usable while scanning continues.

Example:

```text
Scanning Music…

12,843 tracks
1,142 albums
438 artists

██████████████████░░░ 91%
```

---

# 8. Filesystem Monitoring

Use a Rust filesystem notification library such as `notify`.

The watcher should detect:

- created files
- modified files
- renamed files
- deleted files
- moved directories

Filesystem events should be debounced/coalesced before triggering expensive metadata operations.

A file watcher must never directly perform heavy indexing work on its callback thread.

---

# 9. Metadata

TuneX should support common local music metadata formats and audio containers through mature Rust libraries.

Metadata should include, where available:

```text
Title
Artist
Album
Album Artist
Composer
Genre
Date
Track Number
Disc Number
Duration
Bitrate
Sample Rate
Channels
Codec
File Format
Embedded Artwork
```

Metadata extraction should be isolated behind a TuneX metadata interface so the underlying library can be replaced without affecting the rest of the application.

---

# 10. Artwork System

Album artwork is a major visual component of TuneX.

Artwork should support:

- embedded artwork
- folder artwork where appropriate
- cached decoded representations
- multiple display resolutions
- lazy loading
- background decoding
- cache eviction

The UI should never decode large artwork synchronously during rendering.

Suggested cache hierarchy:

```text
Original
   ↓
Decoded cache
   ↓
Thumbnail cache
   ↓
UI texture
```

Artwork should be identified using stable content/file metadata so the cache can survive application restarts.

### 10.1 Artwork rules — LOCKED

- Priority: `embedded > cover.jpg > folder.jpg > front.jpg`. No other heuristics in V1.
- Cache location: XDG cache (`~/.cache/tunex/art/`), thumbnails at 64/256/512px + original.
- Cache key: `blake3(canonical_path + mtime + size)` so edits invalidate correctly and restarts reuse cache.
- LRU cap: ~2 GB default, evicted in background. Configurable later.
- Decode guard: max dimension / byte limit before decode to block image bombs; oversized images downscaled or rejected with placeholder.
- Missing artwork uses generated placeholder. Never fabricate artwork.

---

# 11. Database Model

Initial schema should conceptually contain:

```text
artists
albums
tracks
genres
folders
playlists
playlist_tracks
favorites
play_history
library_roots
scan_state
artwork
settings
```

Relationships:

```text
Artist
 └── Albums
      └── Tracks

Playlist
 └── Playlist Tracks

Library Root
 └── Folders
      └── Tracks
```

FTS5 should power full-text search.

Search should cover:

- track title
- artist
- album
- album artist
- composer
- genre
- filename

### 11.1 Track identity and reconciliation — LOCKED

- `tracks.id` is a SQLite `INTEGER PRIMARY KEY` (internal rowid).
- Stable external key is `stable_key = canonicalized_absolute_path + mtime + size`. Stored on every track/folder row.
- Rename/move detection: on `notify` rename/move events, match by `(file_id/inode where available, size, content-hash fallback)` before treating as delete+create, to preserve playlists, favorites, and play history.
- Deleted files are marked `missing=1` first, then reconciled/garbage-collected; playlists keep dangling entries as missing rather than silently dropping them.

### 11.2 Migrations — LOCKED

- Use `rusqlite_migration` (or equivalent versioned migration crate) from Milestone 2 day 1.
- Every schema change is a numbered migration with `up` SQL. No ad-hoc `CREATE TABLE IF NOT EXISTS` outside migrations.
- `scan_state` stores `schema_version` + `last_scan_root_mtime` for incremental rescans.

### 11.3 FTS5 pattern — LOCKED

- FTS5 as external-content table over `tracks` + `albums` + `artists`, with delete/update triggers.
- Use BM25 ranking, `LIMIT 200` per query group (tracks/albums/artists), prefix matching for incremental typing (`"query"*`).
- Search input debounced ~150ms in QML/Rust controller; cancelled stale queries never overwrite newer results.

---

# 12. Player Engine

The Player Engine provides the stable application-level abstraction around GStreamer.

Required operations:

```text
load(track)
play()
pause()
stop()
seek(position)
next()
previous()
set_volume(value)
set_position(position)
```

State/events:

```text
TrackChanged
PlaybackStateChanged
PositionChanged
DurationChanged
VolumeChanged
PlaybackError
EndOfTrack
```

Playback must support:

- pause/resume
- seeking
- next/previous
- queue
- shuffle
- repeat
- gapless playback
- volume
- mute
- playback position persistence where useful

### 12.1 Gapless + pipeline notes — LOCKED

- Use `playbin3` with `about-to-finish` preloading for gapless transitions. No manual stop/start between queue items.
- Keep a volume element + tag-reading tap in the pipeline from M1 so Phase-2 ReplayGain/crossfade can attach without pipeline rewrite.
- Queue owns `next-track` intent; `PlayerEngine` owns preloading/execution.
- Playback position persistence is best-effort (restore last track + position on restart if file still exists); queue persistence across restarts is out of V1.

---

# 13. Queue

The queue is independent from playlists and library ordering.

It should support:

- add
- remove
- reorder
- clear
- play next
- play now
- shuffle
- repeat modes

The queue must survive normal UI navigation.

---

# 14. Playlists

V1 should support local playlists.

Required:

- create playlist
- rename playlist
- delete playlist
- add track
- remove track
- reorder tracks
- play playlist

Potential future feature:

- smart playlists based on database queries

---

# 15. Search

Search must feel instantaneous.

Architecture:

```text
QML Search Field
        ↓
Rust Search Controller
        ↓
SQLite FTS5
        ↓
Ranked Results
        ↓
QML Models
```

Search should support incremental queries while typing.

Debouncing should be used to avoid unnecessary database work.

---

# 16. MPRIS / D-Bus

TuneX should provide Linux desktop media integration through MPRIS.

Supported operations should include:

- play
- pause
- stop
- next
- previous
- seek
- volume
- metadata
- playback status

This allows TuneX to work with:

- KDE media controls
- desktop media widgets
- keyboard media keys
- compatible desktop integrations

---

# 17. Keyboard & Input

TuneX should support:

- Space — play/pause
- Previous/next media keys
- Volume media keys where available
- Search shortcut
- navigation shortcuts
- escape/back navigation
- standard text editing shortcuts

Exact bindings should remain configurable later.

---

# 18. UI / UX Direction

TuneX should be inspired by the usability patterns of Spotify but should **not** be a visual clone.

The visual language should be:

- dark-first
- modern
- premium
- restrained
- atmospheric
- responsive
- desktop-native

Preferred aesthetic:

**Frosted Obsidian / Modern Audio Workstation**

Visual ingredients:

- translucent surfaces
- subtle glass panels
- controlled blur
- soft shadows
- restrained glows
- gradients
- layered depth
- large artwork
- strong typography
- smooth transitions
- subtle micro-interactions

Avoid:

- excessive neon
- excessive blur
- excessive animation
- giant rounded cards everywhere
- low-contrast text
- visual noise

---

# 19. Custom Visual System

TuneX should build its own reusable QML design system.

Core components:

```text
TuneXWindow
GlassSurface
GlassPanel
GlassCard
Glow
Shadow
BlurSurface
GradientSurface

AlbumCard
ArtistCard
PlaylistCard
TrackRow
TrackList
Artwork
Waveform
SpectrumVisualizer

NavigationRail
Sidebar
TopBar
SearchBar
MiniPlayer
NowPlaying
ProgressBar
VolumeControl
QueuePanel
```

The design system should use centralized theme tokens.

---

# 20. Design Tokens

Initial tokens should include:

```text
Colors
Typography
Spacing
Corner Radius
Elevation
Blur Strength
Glow Strength
Animation Duration
Animation Easing
Opacity
```

Example conceptual palette:

```text
Background:
  Obsidian / near-black

Surface:
  Charcoal / translucent black

Primary:
  Royal / electric blue

Secondary:
  Cool blue / cyan accents

Text:
  High-contrast neutral

Muted:
  Desaturated cool gray
```

The concrete palette is locked in `docs/DESIGN.md` (Frosted Obsidian tokens), calibrated against `docs/mockup.png`.

---

# 21. Glassmorphism Strategy

Glass effects should be implemented deliberately.

Preferred model:

```text
Background Artwork
       ↓
Blur
       ↓
Dark translucent tint
       ↓
Subtle border/highlight
       ↓
Shadow
       ↓
Foreground content
```

Do not make every component glass.

Glass should establish hierarchy.

Examples:

- Now Playing surface: strong glass
- Navigation: subtle glass
- Track rows: mostly opaque
- Context menus: glass
- Dialogs: glass
- Background: artwork/gradient atmosphere

---

# 22. Animation

Animation should communicate state and hierarchy.

Examples:

- album artwork transitions
- page transitions
- hover states
- queue insertion
- player expansion
- search results
- play/pause feedback
- progress transitions
- artwork crossfade

Animations should be short and purposeful.

Avoid animations that delay interaction.

---

# 23. Now Playing Experience

The Now Playing screen should be one of TuneX's signature experiences.

Concept:

```text
┌──────────────────────────────────────────────┐
│                                              │
│              blurred artwork                 │
│                                              │
│            ┌──────────────┐                  │
│            │              │                  │
│            │ Album Cover  │                  │
│            │              │                  │
│            └──────────────┘                  │
│                                              │
│               Track Title                    │
│                Artist                        │
│                                              │
│       ━━━━━━━━━●━━━━━━━━━━━━                 │
│                                              │
│          ◀      ▶      ▶                    │
│                                              │
└──────────────────────────────────────────────┘
```

Potential future enhancement:

A subtle reactive visualization derived from the currently playing audio.

---

# 24. Audio Visualization

Not required for V1.

Architecture should nevertheless leave room for:

- waveform
- spectrum analyzer
- frequency bars
- circular visualizer
- reactive background

Visualization rendering should eventually use GPU-friendly rendering, potentially through custom Qt Quick scene-graph/RHI integration.

---

# 25. Performance Requirements

TuneX should prioritize perceived performance as much as raw benchmark performance.

Aspirational targets (desired, NOT V1-blocking — functionality first, optimize in Milestone 6):

- cold start with 50k cached tracks to interactive <1.5s
- no UI stalls during library scans
- search p95 <50ms on 50k-track library
- scrolling 60fps on iGPU with artwork grid
- artwork loading never blocks the UI
- playback controls respond <50ms
- database work happens off the UI thread
- animations remain smooth under normal load
- scan throughput >500 files/min with live progress

Policy per user decision:

> Get everything functional first. Do not block milestones on perf budgets. Profile and optimize at the end (Milestone 6) using QML profiler + Rust tracing, not assumptions.

Qt provides QML profiling and performance tooling; these should be used during Milestone 6 optimization.

---

# 26. Threading Model

Conceptual model:

```text
UI Thread
  │
  ├── QML
  └── interaction/state presentation

Rust Worker Pool
  │
  ├── filesystem scanning
  ├── metadata extraction
  ├── artwork decoding
  ├── database operations
  └── search/indexing

Audio Pipeline
  │
  └── GStreamer / PipeWire

Scene Graph Render Thread
  │
  └── GPU rendering
```

Runtime — LOCKED:

- `tokio` for Rust background workers (scan, metadata, artwork, DB, search).
- Bounded channels (`tokio::sync::mpsc` + `broadcast`/`watch` for `AppEvent`) between workers and `tunex-app`.
- Only `tunex-app` on the Qt thread touches `cxx-qt` `QObject`s. Workers never touch QML objects directly; they emit `AppEvent` which `tunex-app` forwards as queued Qt signals.
- GStreamer bus messages are polled on their own thread and forwarded as `AppEvent`.
- `notify` watcher callback only enqueues paths; debounced worker does the heavy work.

Never perform expensive blocking operations from the UI thread.

---

# 27. Error Handling

Errors should be explicit and actionable.

Examples:

```text
Unsupported file
Corrupt media
Permission denied
Missing file
Playback failure
Metadata read failure
Database failure
Audio device unavailable
```

Errors should not crash the application.

Recoverable failures should be isolated to the affected operation.

---

# 28. Reliability

TuneX should favor graceful degradation.

Examples:

If artwork is unavailable:

```text
Use generated placeholder.
```

If metadata is incomplete:

```text
Use filename-derived fallback where explicitly appropriate.
```

If a file disappears:

```text
Mark it missing and reconcile the library.
```

If an audio output becomes unavailable:

```text
Preserve player state and recover when possible.
```

---

# 29. Security

TuneX is local-first, but it still processes untrusted files.

The application must:

- avoid executing files
- safely parse metadata
- validate filesystem paths
- avoid unsafe shell invocation
- avoid trusting embedded metadata as executable content
- sandbox optional network integrations where practical
- minimize network access by default
- canonicalize all library paths and reject symlinks escaping library roots
- enforce artwork decode limits (see 10.1) and treat embedded images as opaque bytes, never as code/paths
- never pass filenames/URIs through shell; use argv-style APIs only

---

# 30. Configuration

Application configuration should live in the standard Linux user configuration directory.

Settings should include:

- library paths
- playback behavior
- volume
- shuffle/repeat
- appearance
- theme
- accent color
- animation preference
- audio output preference
- keyboard shortcuts

User data and cache should follow appropriate XDG locations.

---

# 31. Packaging

Primary target (per user 2026-09-09 — Flatpak deferred):

### PKGBUILD / AUR (Arch Linux)

Primary V1 distribution target. Provide `PKGBUILD` for AUR/native Arch install with `pacman` system dependencies (Qt 6, GStreamer + plugins, SQLite). No bundled runtime; rely on Arch repos.

### Flatpak (deferred)

Deferred to post-V1 / S5+. Revisit if universal distribution is needed beyond Arch.

### Other native packages / AppImage

Debian/Ubuntu, Fedora, AppImage remain post-V1 options.

Packaging must declare all runtime dependencies so users do not manually install development packages.

---

# 32. V1 Scope

V1 should intentionally remain focused.

### Must have

- application shell
- library folders
- recursive library scanning
- metadata extraction
- artwork extraction
- SQLite library database
- FTS5 search
- track/album/artist browsing
- playback
- pause/play
- seek
- next/previous
- volume
- queue
- shuffle
- repeat
- playlists
- MPRIS
- media keys
- dark modern UI
- responsive artwork
- persistent settings

### Explicitly out of V1

- online accounts
- streaming services
- cloud synchronization
- lyrics providers
- MusicBrainz integration
- Last.fm integration
- social features
- podcasts
- video
- mobile clients
- AI features
- advanced audio DSP
- complex smart playlists
- advanced visualization
- queue persistence across restarts (queue survives navigation, not restart)
- output device selection UI (PipeWire default output only in V1)
- folder-artwork heuristics beyond `cover.jpg / folder.jpg / front.jpg`
- dedicated composer/genre browsing pages (indexed in search, no dedicated pages in V1)

---

# 33. Post-V1 Roadmap

Potential future features:

## Phase 2

- ReplayGain
- crossfade
- advanced queue management
- smart playlists
- richer metadata editing
- advanced sorting/filtering
- keyboard customization
- audio visualizer

## Phase 3

- MusicBrainz integration
- Cover Art Archive
- lyrics providers
- Last.fm scrobbling
- automatic metadata enrichment

## Phase 4

- advanced waveform analysis
- spectrum visualizer
- custom shaders
- adaptive artwork backgrounds
- richer animation system

---

# 34. Non-Goals

TuneX should not become:

- a Spotify replacement service
- a streaming platform
- a social network
- a media server
- a video player
- a general-purpose DAW
- an audio editor
- an online music store

The product should remain:

> **An exceptional local music player for Linux.**

---

# 35. Licensing Decision

**Recommended: TuneX app as GPLv3 + Qt Community Edition + LGPL-compatible architecture**

Qt is dual-licensed. The Qt Community Edition is primarily available under LGPLv3/GPLv3 with module-specific differences, while commercial Qt is available for proprietary/commercial applications. citeturn0search1turn0search2

For an open-source TuneX, the initial target should be:

**Qt Community Edition + LGPL-compatible architecture**

Rationale for GPLv3 app license:

- removes all friction with Qt Community GPL-only modules if ever needed
- compatible with LGPL Qt via dynamic linking
- standard for Linux-native community apps, native-packaging-friendly

Rules:

- use dynamic linking for LGPL Qt libraries and maintain appropriate license notices/source availability obligations.
- avoid Qt modules that introduce GPL-only requirements unless TuneX's GPLv3 license intentionally covers them (e.g. avoid `QtCharts`, `DataVisualization`, `VirtualKeyboard` unless explicitly approved).
- do not assume every Qt module is LGPL merely because Qt itself offers LGPL licensing; Qt documents specific modules that are GPL-only for open-source users. citeturn0search2

A final licensing review should still be performed before public distribution.

---

# 36. Development Philosophy

TuneX should follow these principles:

### Build the boring infrastructure correctly.

Library indexing, database integrity, playback state and filesystem synchronization should be boring, reliable and testable.

### Make the visual layer exceptional.

The UI is where TuneX should distinguish itself.

### Keep boundaries clean.

Rust should not become entangled with presentation details.

QML should not become a second application backend.

### Prefer mature infrastructure.

Use GStreamer rather than implementing codecs.

Use SQLite rather than inventing a local database.

Use PipeWire rather than bypassing the Linux audio stack.

Use Qt Quick rather than building a complete UI framework.

### Optimize based on profiling.

Do not sacrifice maintainability for hypothetical performance.

---

# 37. Architectural North Star

The final architecture should feel like this:

```text
                 ┌───────────────────────┐
                 │       TuneX UI        │
                 │                       │
                 │      Qt Quick/QML     │
                 │                       │
                 │  Beautiful + Fluid    │
                 └───────────┬───────────┘
                             │
                         Rust API
                             │
                 ┌───────────▼───────────┐
                 │      TuneX Core       │
                 │                       │
                 │ Library │ Player      │
                 │ Queue   │ Search      │
                 │ Playlists │ Settings  │
                 └────┬──────────┬───────┘
                      │          │
                ┌─────▼────┐ ┌───▼────────┐
                │ SQLite    │ │ GStreamer  │
                │ + FTS5    │ │            │
                └───────────┘ └─────┬──────┘
                                    │
                                PipeWire
```

The desired result is:

> **A fast Rust engine wrapped in a beautiful GPU-accelerated Qt Quick experience.**

---

# 38. Initial Milestone Plan

## Milestone 0 — Foundation

- Cargo workspace (4 crates per §5)
- CMake top-level + Corrosion + `cxx-qt` wiring — LOCKED
- application window (`qt_add_qml_module`)
- theme tokens only (no full polish yet)
- Rust ↔ QML bridge proof (1 model + 1 signal round-trip)
- logging (`tracing`)
- configuration (XDG `~/.config/tunex/`, XDG cache/data)
- `qmllint` / `qmlformat` enforcement
- CI (native dev build + PKGBUILD/AUR build check)
- PKGBUILD skeleton early (Arch primary; Flatpak deferred)

## Milestone 1 — Audio Core (playable end-to-end)

- GStreamer `playbin3` integration + `about-to-finish` gapless preload
- load/play/pause/stop
- seeking
- volume
- track completion
- queue (in-memory, no cross-restart persistence in V1)
- minimal SQLite schema + minimal scanner harness so M1 plays real files (full scan in M2)
- MPRIS skeleton (play/pause/next/prev/status) — full metadata in M5

## Milestone 2 — Library

- full SQLite schema + `rusqlite_migration`
- library folders
- scanner (recursive, background, progress)
- metadata extraction (behind TuneX metadata trait)
- artwork extraction (embedded + cover.jpg/folder.jpg only in V1)
- filesystem watcher (`notify`, debounced)

## Milestone 3 — Library UI

- albums
- artists
- tracks
- search
- artwork grid
- track lists

## Milestone 4 — Player UI

- mini-player
- expanded player
- queue
- playback controls
- progress
- volume
- keyboard/media keys

## Milestone 5 — Linux Integration

- MPRIS
- D-Bus integration
- notifications
- Wayland testing
- X11 compatibility

## Milestone 6 — Visual Polish

- glass surfaces
- blur
- shadows
- glow
- transitions
- micro-interactions
- artwork backgrounds
- performance profiling

---

# 39. Definition of Done for V1

TuneX V1 is complete when a user can:

1. Install TuneX on Linux.
2. Add one or more music directories.
3. Let TuneX scan them without freezing the UI.
4. Browse artists, albums and tracks.
5. Search the library instantly.
6. View artwork.
7. Play music reliably.
8. Control playback from the application.
9. Control playback through Linux media keys/MPRIS.
10. Build and manage a queue.
11. Create and manage playlists.
12. Restart TuneX without losing important library/application state.
13. Use TuneX entirely offline.
14. Experience a polished, responsive, GPU-accelerated interface.

---

# 40. Final Technology Decision

**LOCKED**

| Area | Decision |
|---|---|
| Product | TuneX |
| Platform | Linux desktop |
| Language | Rust |
| UI | Qt 6 |
| UI Framework | Qt Quick |
| UI Language | QML |
| Rust ↔ QML bridge | `cxx-qt` (LOCKED) |
| Build system | CMake + Corrosion + Cargo (LOCKED) |
| Async runtime | `tokio` + mpsc/broadcast `AppEvent` (LOCKED) |
| Rendering | Qt Quick Scene Graph |
| Graphics abstraction | Qt RHI |
| Preferred Linux GPU backend | Vulkan |
| Custom rendering | QRhi / Scene Graph where justified |
| Audio engine | GStreamer (`playbin3` + `about-to-finish` gapless) |
| Linux audio | PipeWire (default output in V1) |
| Database | SQLite (WAL, `rusqlite_migration`) |
| Search | SQLite FTS5 (external-content + BM25) |
| Track identity | `stable_key = path + mtime + size`, missing-flag reconcile |
| Artwork cache | XDG `~/.cache/tunex/art/`, blake3 key, ~2GB LRU |
| Filesystem monitoring | `notify` (debounced, enqueue-only callback) |
| IPC | D-Bus / MPRIS |
| Architecture | Rust core + QML presentation (4 crates to start) |
| UI style | Dark / glassmorphic / modern / premium |
| Primary display target | Wayland |
| Compatibility target | X11 |
| Primary package (V1) | PKGBUILD / AUR (Arch) — LOCKED 2026-09-09 |
| Deferred packaging | Flatpak, debs/rpms, AppImage (post-V1) |
| Network dependency | None |
| Core product model | Local-first |
| App license | GPLv3 + LGPL Qt dynamic linking (LOCKED 2026-09-09) |
| Perf policy | Aspirational targets, optimize in M6 (functional first) |
| V1 | Local music playback + library management |

---

## Final Principle

TuneX should not try to win by having the most features.

It should win by being:

**Fast. Beautiful. Native. Reliable. Local.**

A user should be able to install TuneX, point it at their music folder, and immediately feel that they are using a carefully engineered Linux desktop application rather than a web application pretending to be one.
