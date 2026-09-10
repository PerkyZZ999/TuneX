# Requirements — TuneX V1 (source: docs/SPEC.md v1.1)

## MVP

### R-001 — Application shell
- **Description:** Launchable Qt Quick window with nav (library/search/player/now-playing), theme tokens, logging, XDG config. QML presentation only.
- **Priority:** must
- **Acceptance:**
  - [ ] Window opens via CMake/Corrosion build + PKGBUILD install with no console errors
  - [ ] Nav switches views without restart; theme tokens centralized
- **Validation method:** manual + build

### R-002 — Library folders
- **Description:** Add/remove library root folders via native dialog; persisted.
- **Priority:** must
- **Acceptance:**
  - [ ] Add 1+ dirs, restart → roots preserved; remove stops watching
  - [ ] Symlink escaping root rejected
- **Validation method:** manual + test

### R-003 — Background scanning
- **Description:** Recursive scan, supported formats only, progress UI, UI stays usable, incremental rescan + notify watcher debounced.
- **Priority:** must
- **Acceptance:**
  - [ ] 12k-track fixture scans with live counts + % while scrolling/searching without freeze
  - [ ] Modify/rename/delete file → library reconciles (missing-flag, no dup on rename)
- **Validation method:** manual + test

### R-004 — Metadata extraction
- **Description:** Title/artist/album/album-artist/composer/genre/date/track/disc/duration/bitrate/sample-rate/channels/codec/format behind replaceable trait. Unknown stays unknown.
- **Priority:** must
- **Acceptance:**
  - [ ] MP3/FLAC/OGG/M4A/Opus/WAV fixtures show correct fields; missing tags show Unknown, never fabricated
- **Validation method:** test

### R-005 — Artwork
- **Description:** Embedded > cover.jpg > folder.jpg > front.jpg; XDG cache 64/256/512 + orig, blake3 key, ~2GB LRU, decode caps, lazy/background, placeholder fallback.
- **Priority:** must
- **Acceptance:**
  - [ ] Grid shows art without UI-thread decode; restart reuses cache; oversized/broken art → placeholder, no crash
- **Validation method:** manual + test

### R-006 — Persistence + migrations
- **Description:** SQLite WAL, versioned migrations from day 1, restart restores library/settings/queue-position-best-effort.
- **Priority:** must
- **Acceptance:**
  - [ ] Kill + restart → library + settings intact; migration v1→v2 rehearsed
- **Validation method:** test + manual

### R-007 — Browse library
- **Description:** Browse tracks/albums/artists with artwork grid + track lists, virtualized for large libs.
- **Priority:** must
- **Acceptance:**
  - [ ] 50k-track seed browsable, scroll holds, album → tracks correct
- **Validation method:** manual

### R-008 — Instant search
- **Description:** FTS5 external-content + BM25 over title/artist/album/album-artist/composer/genre/filename, ~150ms debounce, LIMIT 200/group, stale-cancel.
- **Priority:** must
- **Acceptance:**
  - [x] Typing filters tracks/albums/artists live; offline works
- **Validation method:** manual + test

### R-009 — Core playback
- **Description:** load/play/pause/stop/seek via playbin3 PlayerEngine abstraction.
- **Priority:** must
- **Acceptance:**
  - [ ] Play → pause → seek → resume sample-accurate UI position; corrupt file → actionable error, app survives
- **Validation method:** manual + test

### R-010 — Queue + modes
- **Description:** Independent queue: add/remove/reorder/clear/play-next/play-now/shuffle/repeat; gapless via about-to-finish; survives navigation (not restart in V1).
- **Priority:** must
- **Acceptance:**
  - [x] Queue ops reflect instantly; gapless transition has no audible gap on album fixture
- **Validation method:** manual

### R-011 — Volume / mute / position
- **Description:** Volume/mute via pipeline; best-effort last-track+position restore.
- **Priority:** must
- **Acceptance:**
  - [x] Slider/mute immediate; restart restores volume + last track if file exists
- **Validation method:** manual

### R-012 — Playlists
- **Description:** Local playlists: create/rename/delete/add/remove/reorder/play; dangling-as-missing.
- **Priority:** must
- **Acceptance:**
  - [x] Full CRUD + play persists across restart
- **Validation method:** manual + test

### R-013 — MPRIS + media keys
- **Description:** D-Bus MPRIS play/pause/stop/next/prev/seek/volume/metadata/status; KDE widgets + keys work.
- **Priority:** must
- **Acceptance:**
  - [ ] `playerctl` controls TuneX and shows metadata while playing
- **Validation method:** manual

### R-014 — Keyboard
- **Description:** Space play/pause, media next/prev/volume, search shortcut, nav + esc/back, standard text editing.
- **Priority:** must
- **Acceptance:**
  - [x] All bindings work focused in library/search/player; list documented
- **Validation method:** manual

### R-015 — Settings
- **Description:** XDG settings: library paths, playback, volume, shuffle/repeat, appearance/accent, animation pref, audio default, shortcuts.
- **Priority:** must
- **Acceptance:**
  - [ ] Change → restart → retained
- **Validation method:** manual

### R-016 — Installable PKGBUILD / AUR (MVP locked 2026-09-09)
- **Description:** Arch PKGBUILD/AUR primary artifact with pacman depends (Qt6, GStreamer + plugins, SQLite). Flatpak deferred post-V1.
- **Priority:** must
- **Acceptance:**
  - [ ] Fresh Arch VM/container `makepkg -si` installs and plays local files offline with system deps only
- **Validation method:** manual

### R-017 — Offline-first
- **Description:** All R-001–R-015 work with networking disabled.
- **Priority:** must
- **Acceptance:**
  - [ ] Disable network → rescan/browse/search/play/queue/playlists all pass
- **Validation method:** manual

### R-018 — Dark premium UI
- **Description:** Dark-first frosted-obsidian, restrained glass (strong only where hierarchical), centralized tokens, short purposeful animations.
- **Priority:** must
- **Acceptance:**
  - [ ] No neon/low-contrast text; track rows opaque; dialogs/menus/now-playing glass per SPEC §21
- **Validation method:** manual

## NFR (all must, validated at slice + M6)
- **R-NFR-01 Threading:** no expensive work on UI thread (scan/meta/art/DB/search off-thread). Validation: manual jank check + tracing.
- **R-NFR-02 Reliability:** missing art → placeholder; incomplete meta → Unknown/filename fallback where explicit; missing file → missing-flag; device loss → preserve state + recover. Validation: fault-injection manual.
- **R-NFR-03 Security:** canonicalize + symlink-escape reject, argv-only, decode caps, images as opaque bytes. Validation: test + review.
- **R-NFR-04 Accessibility:** keyboard-full, focus-visible, contrast, blur-off/reduced-motion, software-GL usable. Validation: manual.
- **R-NFR-05 Errors:** Unsupported/corrupt/permission/missing/playback/meta/DB/device errors are explicit + actionable, never crash. Validation: manual + test.
- **R-NFR-06 Perf (aspirational, M6 only):** 1.5s cold 50k, search p95 <50ms, 60fps scroll, <50ms controls, >500 files/min. Desired, not V1-blocking.

## Later (out of V1)
- L-001 ReplayGain, L-002 crossfade, L-003 advanced queue, L-004 smart playlists, L-005 metadata editing, L-006 sorting/filtering, L-007 shortcut customization, L-008 visualizer/waveform/spectrum/shaders, L-009 MusicBrainz/Cover Art, L-010 lyrics, L-011 Last.fm, L-012 queue-across-restart, L-013 output-device picker, L-014 composer/genre pages.

## Non-goals
- Streaming/accounts/cloud, social, podcasts, video, mobile, DAW/editor, store, AI, advanced DSP — per BRIEF.
