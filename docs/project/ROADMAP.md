# Roadmap (vertical slices — each ships user-visible value)

## Slice S1 — Shell + real playback (M0+M1)
- **Acceptance:** CMake+Corrosion builds window; cxx-qt model+signal round-trip proven; playbin3 plays/pauses/seeks MP3+FLAC+OGG+M4A+Opus+WAV with volume; queue next/prev works in-memory; PKGBUILD builds + installs on Arch; `playerctl` skeleton responds.
- **Requirements:** R-001, R-009, R-010 (in-memory), R-011 (volume), R-013 (skeleton), R-016 (skeleton), R-NFR-01/05
- **Validation:** codec matrix manual + `cargo test` queue/engine + `makepkg` + playerctl check
- **Status:** todo

## Slice S2 — Library scan + browse + art (M2+M3 core)
- **Acceptance:** Add/remove roots (native dialog), background scan with live counts+% while UI usable, browse artists/albums/tracks with lazy art + placeholders, restart preserves library, rename reconciles without dup.
- **Requirements:** R-002, R-003, R-004, R-005, R-006, R-007, R-015 (paths), R-NFR-01/02/03
- **Validation:** 12k fixture scan manual + rename/delete fault injection + restart check + `cargo test` stable_key/migrations
- **Status:** todo

## Slice S3 — Search + queue depth + playlists (M3+M4 core)
- **Acceptance:** Instant FTS5 search across 7 fields with debounce + stale-cancel; queue full ops + shuffle/repeat + gapless album; playlists full CRUD + play + restart persistence.
- **Requirements:** R-008, R-010 (full), R-012, R-NFR-01
- **Validation:** typing manual + queue/FTS5 unit tests + gapless listen check
- **Status:** todo

## Slice S4 — Player UI complete (M4)
- **Acceptance:** Mini-player + expanded/now-playing (blurred art, controls, progress, volume), keyboard map (space/search/esc/nav), best-effort position restore, animations short/purposeful.
- **Requirements:** R-010, R-011 (position), R-014, R-018 (functional, pre-polish), R-NFR-04
- **Validation:** keyboard walkthrough + offline run + a11y/contrast/blur-off check
- **Status:** todo

## Slice S5 — Linux integration hardens (M5)
- **Acceptance:** Full MPRIS (seek/volume/metadata/status + art URL where feasible), notifications, Wayland + X11 verified, high-DPI, native dialogs, AUR package verified on clean Arch.
- **Requirements:** R-013 (full), R-016 (final), R-017, R-NFR-02/04
- **Validation:** playerctl full matrix + KDE widget check + Wayland/X11 sessions + network-off DoD rehearsal + clean-chroot PKGBUILD build
- **Status:** todo

## Slice S6 — Visual polish + perf pass (M6, aspirational numbers)
- **Acceptance:** Glass hierarchy per SPEC §21, MultiEffect/shadow/glow restraint, transitions/micro-interactions, artwork backgrounds; profile with QML profiler + tracing; record numbers vs targets without blocking release on them.
- **Requirements:** R-018 (final), R-NFR-06 (measure, not gate)
- **Validation:** profiler traces + 50k scroll/search observation + release checklist
- **Status:** todo

## Traceability (req → slices)
- R-001→S1, R-002→S2, R-003→S2, R-004→S2, R-005→S2, R-006→S2 (+S1 harness), R-007→S2, R-008→S3, R-009→S1, R-010→S1+S3, R-011→S1+S4, R-012→S3, R-013→S1+S5, R-014→S4, R-015→S2+S4, R-016→S1+S5 (PKGBUILD), R-017→S5, R-018→S4+S6.
