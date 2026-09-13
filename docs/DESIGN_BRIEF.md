# Design Brief: TuneX V1 Player Experience

## Problem

I have thousands of music files sitting on my disk, and every Linux player makes me feel like I'm filing taxes. Either it's a dull spreadsheet with a play button, or it's a streaming app wearing a trench coat — begging me to log in, showing me "Trending Now" for music I don't own, hiding my own library three clicks deep. I just want to open an app that feels like *my* music room: calm, dark, beautiful, instant. I want to find an album in seconds, press play, and have it sound gapless — without an account, without the network, without visual noise shouting at me.

## Solution

TuneX V1 is a nocturnal listening room, not a storefront. Opening the app lands you home: a greeting, one clear way to start music, your recently played records as large artwork, and a persistent player that never leaves you. Your library is organized the way collectors think — artists, albums, songs, genres, folders — with search that answers as you type. The interface stays out of the way: artwork leads, chrome recedes, and every control answers in under a blink. It works identically on a plane with Wi-Fi off.

## Experience Principles

Three principles maximum. Each resolves a tension from the evidence (SPEC + mockups + local-first constraints).

1. **Calm ownership over storefront energy** — Nothing shouts, badges, or "discovers" for you. No trending rails, no notification bells, no avatars. The library you own is the whole universe of the interface. (Resolves: mockup1's streaming patterns vs. local-first truth.)
2. **Artwork leads, chrome recedes** — Album art is the primary visual; panels, rails, and buttons are quiet dark glass that frames it. If a surface competes with the artwork, the surface loses. (Resolves: premium atmosphere vs. visual noise.)
3. **Every pixel responds** — Controls acknowledge instantly and motion only ever explains a state change (playing, loading, queued, found). Nothing animates decoratively; nothing makes you wait. (Resolves: fluid GPU canvas vs. functional-first engineering — perceived speed is a feature.)

## Aesthetic Direction

- **Philosophy**: Frosted Obsidian / Modern Audio Workstation (per SPEC §18). Dark-first, atmospheric, restrained depth — closer to a high-end listening bar at night than to a dashboard.
- **Tone**: Calm, nocturnal, confident, premium. Warm-neutral text on blue-black surfaces; a single electric-blue accent used sparingly, like a pilot light.
- **Reference points**: `docs/mockup.png` is the canonical visual reference — three-column shell (icon+label rail / content with hero + chip strip + card rails / Now Playing panel), pill search, blue accent, card/row/transport styling. **Layout, chrome, and component styling follow the mockup; content follows the adaptation map below** (local library, never streaming content). SPEC §§18–23 (glass hierarchy, token system, animation restraint).
- **Mockup adaptation map (look like the mockup, behave like TuneX):** hero copy → local greeting + single "Play Something" action (mockup's secondary "Discover" button omitted — rails + search cover it) · genre chip strip → Genres filter backed by library data · "Trending Now" rail → Recently Played · "Made for You / Discover Weekly / Daily Mix" cards → user playlists + Recently Added (same card component, local data only) · "Liked Songs" → Favorites · Lyrics/About/Related tabs → Up Next queue (lyrics deferred post-V1) · bell/avatar chrome → removed, no accounts (settings gear stays).
- **Anti-references**: streaming *service* patterns must NOT ship in V1 — accounts, avatars, commerce, algorithmic recommendations, trending charts. Also not: neon gamer aesthetics, giant rounded cards everywhere, low-contrast gray-on-black body text, GNOME-Adwaita-default look, web-app-in-a-window feel.

## Existing Patterns

Greenfield UI — no code, no tokens, no components exist yet (verified 2026-09-09: repo contains only docs). The brief extends the mockups, it does not replace them:

- Typography: none in code. The mockup uses a geometric grotesque for display + neutral sans for UI. Decision locked in `DESIGN.md`: Inter (display weight 700, UI 400/500), system fallback chain documented there.
- Colors: SPEC §20 conceptual palette only (obsidian bg, charcoal surface, royal/electric blue primary, cool cyan secondary, high-contrast neutral text, desaturated gray muted). Concrete hexes locked in `DESIGN.md` frontmatter; the mockup's blue-black night tones are the calibration target.
- Spacing: 4px-base scale (4 / 8 / 12 / 16 / 24 / 32 / 48), locked in `DESIGN.md`.
- Components: none exist. The inventory below is the complete V1 build list; all status New, all implemented as custom QML per SPEC §19 (no Qt Quick Controls default styling visible to the user).

## Component Inventory

| Component | Status | Notes |
| --------- | ------ | ----- |
| TuneXWindow + theme tokens | New | App frame, min 960×640, tokens singleton consumed by everything |
| NavigationRail | New | Icon + label items; sections: main, library, playlists |
| TopBar (back/fwd, SearchBar, settings entry) | New | Search field is the global command point |
| SearchBar | New | Pill, debounced ~150ms, focus shortcut |
| HeroCard (home greeting) | New | Artwork/gradient backdrop, greeting by time of day, single primary action |
| AlbumCard / ArtistCard / PlaylistCard | New | Artwork-first, 12px radius, two-line meta |
| TrackRow / TrackList (virtualized) | New | Opaque rows, hover + playing states, duration + explicit position |
| Artwork (async, multi-resolution) | New | Placeholder → thumbnail → full; never UI-thread decode |
| MiniPlayer (persistent transport) | New | Art thumb, title/artist, favorite, controls, progress, volume, queue toggle. Bottom bar when right panel is hidden (<1280px); right-panel summary at full width |
| NowPlaying panel | New | Strong-glass signature surface: large art, meta, progress, transport |
| ProgressBar / VolumeControl | New | Thin track, blue fill, scrub without stutter |
| QueuePanel (Up Next) | New | Reorder-capable list, currently-playing marker |
| PlaylistDetail header + rows | New | Reuses TrackList; header with art mosaic + actions |
| GlassPanel / GlassSurface dialog + menu | New | Dialogs and context menus only; rows stay opaque |
| EmptyState (no library / no results / missing files) | New | Explicit, actionable (add folder, rescan, reveal in files) |
| ScanProgress indicator | New | Counts + % in library view; non-blocking |
| SettingsView | New | List-detail: Library (music folders), Playback, Appearance, Shortcuts |
| SettingsToggle | New | 44px row switch; whole row is the target |
| TrayPopup | New | Compact tray card: now-playing + transport; Quit / Show TuneX |

## Key Interactions

- **Play in under 30 seconds (first run):** empty state → Add folder opens Settings → Library (native dialog) → live scan counts → first artwork appears → Play Something shuffles something immediately. Scanning never blocks browsing.
- **Transport feedback:** play/pause icon morphs instantly on press (<50ms), before the pipeline confirms; artwork crossfades ~180ms on track change; progress thumb grows on hover for grab-ability.
- **Search as you type:** keystroke → debounced query → grouped results (Songs / Albums / Artists) replace content in place; Escape clears and returns focus to the list; stale results never overwrite newer ones.
- **Queue as instrument:** "Play next" / "Add to Up Next" from any row menu; drag or keyboard-reorder in Up Next; the playing row is always marked; clearing the queue never stops the current track.
- **Now Playing expand:** persistent-player click (mini-player bar or right-panel summary) or shortcut opens the glass Now Playing surface over content (same state, larger art); close returns to exact scroll position.
- **Honest states:** missing art → generated placeholder; unknown tags → "Unknown" (never guessed); missing files → dimmed row + "File missing" with reveal/rescan actions; corrupt file → toast naming the file, playback continues to next.
- **Micro-motion budget:** 120–220ms, ease-out, one property per transition. No entrance choreography, no parallax, no animated gradients.

## Progressive Disclosure

The interface follows progressive disclosure throughout (Hick's + Miller's laws): essential actions are visible; advanced and secondary controls reveal on demand. One explicit affordance per hidden layer — never mystery meat.

- Track rows show title/artist/duration; hover (or focus) reveals play + ⋯ actions. Touch and keyboard users get an always-visible 44px ⋯ target — hover-only controls are a bug.
- Queue lives in the Up Next drawer; Now Playing expands to overlay. Secondary surfaces never get top-level nav items.
- Search groups show top hits with "See all" escalation; chip strips collapse extras behind "More".
- Settings shows common options first; advanced (watcher status, cache controls, shortcut reference) sits one level deeper per section.
- Context menus carry row/playlist operations instead of toolbars of icon buttons.
- No view presents more than ~7 primary choices; disclosure state is always keyboard- and screen-reader-reachable (drawer focus trap + Esc, `aria-expanded` equivalents via Qt Accessibility).

## Frontend Skill Alignment

All UI work must load and follow three skills (see `AGENTS.md`): `craft-beautiful-frontend` (quality gates: type scale, 4px spacing, AA contrast, one primary action per view, 44px targets, motion budget, empty/loading/error/success states for every async surface), `iconography-frontend-ui` (single Lucide-style line-icon family, icon+text default, icon-only only with accessible name + tooltip, 3:1 non-text contrast), `responsive-design` (content-based window classes, fluid artwork grid, no mobile breakpoints — desktop-first with 960×640 minimum). Web/CSS specifics in those skills translate to QML: tokens live in `DESIGN.md` + the Theme singleton (not CSS), icons ship as SVG assets, motion uses QML Behaviors within the budgets in `DESIGN.md`.

## Responsive Behavior

Desktop-only product (no mobile breakpoints). Window classes:

- **≥1280px:** full three-column home — rail 240px, fluid content, Now Playing/Up Next panel 320px (the persistent player; no bottom bar at this width, per the mockup).
- **1024–1279px:** right panel becomes an overlay drawer (toggle from the top bar); bottom mini-player bar appears as the persistent player; content grid drops one column.
- **800–1023px:** single column; rail collapses to icon strip 64px with tooltips; Now Playing is full overlay.
- **Minimum 960×640**, enforced by window. High-DPI scales via Qt devicePixelRatio; artwork grid uses fluid 160–220px cards, never fixed counts. No component changes *meaning* across sizes — only placement and column counts.

## Accessibility Requirements

- Text contrast WCAG AA (4.5:1) for body/labels; metadata caption never below AA against its surface — enforced by `DESIGN.md` token pairs.
- Full keyboard operation: Space play/pause, `/` or Ctrl+K search, arrows navigate lists, Enter plays, Esc backs out; every control reachable with a visible focus ring (never outline:none without replacement).
- Screen reader: accessible names on all transport/list controls via Qt Accessibility; track rows expose title–artist–duration; progress exposes position/duration.
- No color-only meaning: playing state pairs accent bar + "Now playing" label; errors pair icon + text.
- Toggles in Settings: reduce motion (disables crossfades/transitions) and reduce blur/transparency (falls back to opaque surfaces) — both honored before first paint where possible.
- Software-GL fallback remains fully usable, minus blur.

## Out of Scope

Explicitly not in this brief (matches SPEC §32): lyrics UI, audio visualizers/waveforms, smart/Discover-style algorithmic rails, metadata editor, output-device picker, queue-across-restart, composer/genre destination pages, light theme (V1 is dark-only), online enrichment UI, accounts/avatars/social, podcasts/video/mobile, advanced DSP. The "Made for You" style rails in the mockups are deferred — home uses Recently Played / Recently Added / jump-back-in rails built only from local history.
