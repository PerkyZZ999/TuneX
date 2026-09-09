# Brief — TuneX

## Problem
Linux users who own local music files have no player that is simultaneously fast, beautiful, native-feeling, and offline-first. Existing options are web wrappers (heavy, alien UX), dated Qt Widgets／GTK players (functional but unpolished), or streaming clients (require accounts/network, ignore local libraries).

## Users
- Primary: Linux desktop users (Wayland-first, X11-compat) with local collections (100s–50k+ tracks) who want Spotify-grade browsing/search with native playback.
- Secondary: Audiophiles wanting gapless local playback via PipeWire/GStreamer; Arch/AUR users wanting a native pacman citizen.

## Outcome
A fast Rust engine (library + GStreamer playback + SQLite/FTS5) wrapped in a GPU-accelerated Qt Quick dark-glass experience. Point at a folder → browsable, searchable, playable in seconds, fully offline.

## Success signal
V1 Definition of Done (SPEC §39, observable demo):
1. Install on Arch via AUR PKGBUILD. 2. Add music dirs. 3. Scan without UI freeze + live progress. 4. Browse artists/albums/tracks with artwork. 5. Search feels instant. 6. Play/pause/seek/next/prev/volume/queue/shuffle/repeat gaplessly. 7. Playlists CRUD. 8. Media-key/MPRIS control. 9. Restart preserves library + settings. 10. All core flows work with networking disabled.

## Non-goals
- No Spotify replacement service, streaming, accounts, cloud sync, social.
- No podcasts, video, mobile clients, DAW/editor, music store.
- No AI features, advanced DSP, complex smart playlists, advanced visualization in V1.
- No queue-across-restart, output-device picker, composer/genre pages, or artwork heuristics beyond embedded + cover/folder/front.jpg in V1.

## Constraints (known)
- Stack locked: Rust + Qt 6 Quick/QML + RHI/Vulkan + cxx-qt + CMake/Corrosion + GStreamer playbin3 + PipeWire default + SQLite/FTS5 + notify + tokio (see DECISIONS.md).
- Local-first: network never required for core flows.
- QML = presentation only; Rust = logic. No direct FS/DB/GStreamer from QML.
- Perf is aspirational in V1 (functional first, optimize M6): 1.5s cold 50k, search p95 <50ms, 60fps scroll, >500 files/min — desired, not blocking.
- License path: GPLv3 app recommended + LGPL Qt dynamic linking; avoid GPL-only Qt modules without approval.

## Open questions
- None blocking. Minor: exact dark palette finalized in visual prototyping (M0 tokens → M6 polish).
