# Roadmap (vertical slices — each ships user-visible value)

## Slice S1 — Shell + real playback (M0+M1)
- **Acceptance:** CMake+Corrosion builds window; cxx-qt model+signal round-trip proven; playbin3 plays/pauses/seeks MP3+FLAC+OGG+M4A+Opus+WAV with volume; queue next/prev works in-memory; PKGBUILD builds + installs on Arch; `playerctl` skeleton responds.
- **Requirements:** R-001, R-009, R-010 (in-memory), R-011 (volume), R-013 (skeleton), R-016 (skeleton), R-NFR-01/05
- **Validation:** codec matrix manual + `cargo test` queue/engine + `makepkg` + playerctl check
- **Status:** done (W-001a/b–W-010, 2026-09-10; both feasibility risks closed, no loop-back; known deferrals: full `makepkg` build → S5, `playerctl` binary absent so smoke ran via `qdbus6`/`busctl`)

## Slice S2 — Library scan + browse + art (M2+M3 core)
- **Acceptance:** Add/remove roots (native dialog), background scan with live counts+% while UI usable, browse artists/albums/tracks with lazy art + placeholders, restart preserves library, rename reconciles without dup.
- **Requirements:** R-002, R-003, R-004, R-005, R-006, R-007, R-015 (paths), R-NFR-01/02/03
- **Validation:** 12k fixture scan manual + rename/delete fault injection + restart check + `cargo test` stable_key/migrations
- **Status:** done (W-011–W-018, 2026-09-10; 12k→12k rows in 2.5 s, 100/100 rename + 100/100 missing reconcile, KWin DoD with 5 evidence shots, no loop-back; known deferrals: background art warming → S3/S4 views resolve lazily, full `makepkg` build → S5)

## Slice S3 — Search + queue depth + playlists (M3+M4 core)
- **Acceptance:** Instant FTS5 search across 7 fields with debounce + stale-cancel; queue full ops + shuffle/repeat + gapless album; playlists full CRUD + play + restart persistence.
- **Requirements:** R-008, R-010 (full), R-012, R-NFR-01
- **Validation:** typing manual + queue/FTS5 unit tests + gapless listen check
- **Status:** done (W-019–W-025, 2026-09-10; FTS5+queue+playlists GUI-proven, no loop-back; known deferrals: untagged fixtures do not FTS-match display "Unknown", full `makepkg` → S5, async seek → S4 before the player slider)

## Slice S4 — Player UI complete (M4)
- **Acceptance:** Mini-player + expanded/now-playing (blurred art, controls, progress, volume), keyboard map (space/search/esc/nav), best-effort position restore, animations short/purposeful.
- **Requirements:** R-010, R-011 (position), R-014, R-018 (functional, pre-polish), R-NFR-04
- **Validation:** keyboard walkthrough + offline run + a11y/contrast/blur-off check
- **Status:** done (W-026–W-031, 2026-09-10; MiniPlayer + Now Playing overlay + keyboard map + last-track restore GUI-proven, a11y/contrast/blur-off checked, no loop-back)

## Slice S5 — Linux integration hardens (M5)
- **Acceptance:** Full MPRIS (seek/volume/metadata/status + art URL where feasible), notifications, Wayland + X11 verified, high-DPI, native dialogs, AUR package verified on clean Arch.
- **Requirements:** R-013 (full), R-016 (final), R-017, R-NFR-02/04
- **Validation:** playerctl full matrix + KDE widget check + Wayland/X11 sessions + network-off DoD rehearsal + clean-chroot PKGBUILD build
- **Status:** done (W-032–W-037, 2026-09-10; MPRIS matrix + notify emission + Wayland/X11/HiDPI/dialogs + local makepkg + net-off rehearsal green, no loop-back; known follow-ups: true `extra-x86_64-build`, true X11-Plasma visuals, toast pixels — all need rooted/real-desktop runs, no code risk)

## Slice S6 — Visual polish + perf pass (M6, aspirational numbers)
- **Acceptance:** Glass hierarchy per SPEC §21, MultiEffect/shadow/glow restraint, transitions/micro-interactions, artwork backgrounds; profile with QML profiler + tracing; record numbers vs targets without blocking release on them.
- **Requirements:** R-018 (final), R-NFR-06 (measure, not gate)
- **Validation:** profiler traces + 50k scroll/search observation + release checklist
- **Status:** done (W-038–W-045, 2026-09-13; glass hierarchy, motion budgets, artwork pipeline, measured M6 numbers, charcoal identity; S6 gate PASS, no loop-back; known follow-ups unchanged from S5: true `extra-x86_64-build`, true X11-Plasma visuals, toast pixels)

## Slice S7 — Tray + Settings (post-S6)
- **Acceptance:** StatusNotifierItem tray with compact now-playing controls; Settings list-detail (library paths, playback, appearance, shortcuts, close-to-tray) matches DESIGN.md; scanned-folder add/remove only in Settings; type scale and chrome one step denser via tokens; window-close honors close-to-tray when a tray host is present.
- **Requirements:** R-002 (Settings surface), R-015, R-018 (type scale)
- **Validation:** `cargo test` config/tray persist + hide-on-close; `scripts/qml-lint.sh`; `design.md lint`; CMake when Qt floor allows
- **Status:** done (W-046, W-047, 2026-09-13)

## Slice S8 — Session memory and player UX (post-V1)
- **Acceptance:** Window geometry and library tab/sort persist; MiniPlayer and Up Next scrub; list/grid keyboard (arrows, j/k, Enter, Songs type-to-select); Settings → Shortcuts is a complete map; missing-file reveal/remove; first-run hero waits for the first album.
- **Requirements:** R-014, R-015
- **Validation:** `cargo test` config/geometry/view prefs + delete_track; `scripts/qml-lint.sh`
- **Status:** done (W-048–W-051, 2026-09-13)

## Slice S9 — Browse depth (post-V1)
- **Acceptance:** Album and artist landing pages (grid → detail → back; artist card opens detail, Play is the header primary); Genres and Composers under Your Library (Unknown stays Unknown); search recents (cap 10) and Tab / Shift+Tab cycle Songs → Albums → Artists groups.
- **Requirements:** R-007, R-019 (promoted L-014), R-008, R-014
- **Validation:** `cargo test` album/artist/facet queries + recent searches; `scripts/qml-lint.sh`
- **Status:** done (W-052–W-055, 2026-09-13)

## Slice S10 — Queue as an instrument (post-V1)
- **Acceptance:** Drag-reorder Up Next onto `moveItem`; queue-row menu Play next / Move to end / Remove; playing-row equalizer tick (static when reduce-motion); ordered URIs + cursor persist in SQLite and restore paused without auto-play; missing files stay dangling.
- **Requirements:** R-010 (promoted L-012 + L-003 named UX)
- **Validation:** `cargo test` queue persist/restore + play_next_at; `scripts/qml-lint.sh`
- **Status:** done (W-056–W-057, 2026-09-13)

## Slice S11 — Playback enrichment (post-V1)
- **Acceptance:** ReplayGain off/track/album from tags (unity when missing); 0–12 s volume-envelope crossfade on the existing handoff; PipeWire/GStreamer output picker (System default) without dropping the queue.
- **Requirements:** R-020 (promoted L-001, L-002, L-013); D-005 updated
- **Validation:** `cargo test` replaygain/crossfade/output; `scripts/qml-lint.sh`
- **Status:** done (W-058–W-060, 2026-09-13)

## Slice S12 — Library authorship (post-V1)
- **Acceptance:** Play history on URI advance (restore does not count) powers Home Recently Played; smart playlists from a GlassDialog rule builder evaluate in SQL on open/play; tag editor writes lofty tags then upserts that path (empty fields unchanged; permission failures loud).
- **Requirements:** R-021 (promoted L-004, L-005)
- **Validation:** `cargo test` play_history / smart playlists / tag write; `scripts/qml-lint.sh`
- **Status:** done (W-061–W-063, 2026-09-13)

## Slice S13 — Lyrics, profiles, opt-in MusicBrainz (post-V1)
- **Acceptance:** Now Playing lyrics toggle (sidecar `.lrc` + embedded unsynced, offline-only); named library profiles with their own index and folders; MusicBrainz / Cover Art Archive opt-in default off, missing-only, never required.
- **Requirements:** R-022 (promoted L-009, L-010); D-010 extra source; D-014 product-loop exception
- **Validation:** `cargo test` lyrics/profiles/enrichment-off; `scripts/qml-lint.sh`; enrichment off for `scripts/netoff-rehearsal.sh`
- **Status:** done (W-064–W-066, 2026-09-13)

## Slice S14 — Seek, notifications, Now Playing chrome, DnD, multi-select
- **Acceptance:** Seek slider grabs and lands (coalesced FLUSH); track-change toasts only when unfocused; right rail is Now Playing; playlist + Now Playing reorder and accept dropped TrackRows; TrackRow multi-select with Play / Add / New playlist. Library browse lists stay Sort By only.
- **Requirements:** R-007, R-009, R-010, R-012, R-014, R-015
- **Validation:** `cargo test` seek coalesce + notify gate; `scripts/qml-lint.sh`; KWin scrub / drop / Ctrl+A
- **Status:** done (W-067–W-070, W-073, 2026-09-14)

## Slice S15 — Equalizer
- **Acceptance:** 10-band EQ in the tunex-audio bin before ReplayGain; named presets + Custom; Settings → Playback; missing `equalizer-10bands` stays flat; reduce-motion does not disable audio EQ.
- **Requirements:** R-020; D-005 updated
- **Validation:** `cargo test` preset/config + playback with EQ; `scripts/qml-lint.sh`
- **Status:** done (W-071, 2026-09-14)

## Slice S16 — Artwork / visualizer modes
- **Acceptance:** Artwork | Spectrum | Waveform | Visualizer on the Now Playing well; View menu + Settings → Appearance; pad probe after EQ (no second sink); reduce-motion → Artwork; lyrics toggle still wins.
- **Requirements:** R-023 (promoted L-008)
- **Validation:** `cargo test` spectrum/PCM parse; `scripts/qml-lint.sh`; KWin mode switch
- **Status:** done (W-072, 2026-09-14)

## Traceability (req → slices)
- R-001→S1, R-002→S2+S7, R-003→S2, R-004→S2, R-005→S2, R-006→S2 (+S1 harness), R-007→S2+S9+S14, R-008→S3+S9, R-009→S1+S14, R-010→S1+S3+S10+S14, R-011→S1+S4+S14, R-012→S3+S12+S14, R-013→S1+S5, R-014→S4+S8+S9+S14, R-015→S2+S4+S7+S8+S13+S14, R-016→S1+S5 (PKGBUILD), R-017→S5, R-018→S4+S6+S7, R-019→S9, R-020→S11+S15, R-021→S12, R-022→S13, R-023→S16.
