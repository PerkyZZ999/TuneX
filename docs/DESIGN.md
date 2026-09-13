---
version: alpha
name: "TuneX Frosted Obsidian"
description: "Dark-first nocturnal design system for TuneX, a local-first Linux music player. Artwork leads, chrome recedes; charcoal neutrals under one royal-blue brand; translucent chrome over an artwork-tinted room."
colors:
  background: "#0B0B0E"
  surface: "#141419"
  surface-raised: "#1D1D23"
  chrome: "#0F0F13"
  hover: "#232329"
  selected: "#2A2A33"
  foreground: "#F4F4F7"
  muted: "#A3A3AE"
  border: "#32323B"
  primary: "#2B5CE6"
  primary-hover: "#3766EE"
  on-primary: "#FFFFFF"
  accent: "#6E9BFF"
  accent-secondary: "#4C86FF"
  success: "#4ED07A"
  warning: "#F5B544"
  error: "#FF8585"
  focus: "#9BBBFF"
typography:
  display-lg:
    fontFamily: Inter
    fontSize: 32px
    fontWeight: 700
    lineHeight: 1.18
    letterSpacing: -0.01em
  headline-md:
    fontFamily: Inter
    fontSize: 22px
    fontWeight: 700
    lineHeight: 1.25
  title-md:
    fontFamily: Inter
    fontSize: 16px
    fontWeight: 600
    lineHeight: 1.33
  body-md:
    fontFamily: Inter
    fontSize: 14px
    fontWeight: 400
    lineHeight: 1.5
  body-sm:
    fontFamily: Inter
    fontSize: 12px
    fontWeight: 400
    lineHeight: 1.45
    fontFeature: "tnum"
  label-md:
    fontFamily: Inter
    fontSize: 12px
    fontWeight: 500
    lineHeight: 1.3
  label-sm:
    fontFamily: Inter
    fontSize: 12px
    fontWeight: 500
    lineHeight: 1.3
  caption-md:
    fontFamily: Inter
    fontSize: 12px
    fontWeight: 400
    lineHeight: 1.4
    fontFeature: "tnum"
  eyebrow-sm:
    fontFamily: Inter
    fontSize: 12px
    fontWeight: 600
    lineHeight: 1.2
    letterSpacing: 0.08em
rounded:
  xs: 4px
  sm: 8px
  md: 12px
  lg: 16px
  xl: 24px
  pill: 999px
spacing:
  xs: 4px
  sm: 8px
  md: 16px
  lg: 24px
  xl: 32px
  xxl: 48px
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    typography: "{typography.label-md}"
    rounded: "{rounded.pill}"
    padding: 12px
  button-primary-hover:
    backgroundColor: "{colors.primary-hover}"
    textColor: "{colors.on-primary}"
    typography: "{typography.label-md}"
    rounded: "{rounded.pill}"
    padding: 12px
  button-secondary:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.foreground}"
    typography: "{typography.label-md}"
    rounded: "{rounded.pill}"
    padding: 12px
  button-secondary-hover:
    backgroundColor: "{colors.hover}"
    textColor: "{colors.foreground}"
    typography: "{typography.label-md}"
    rounded: "{rounded.pill}"
    padding: 12px
  nav-item:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    typography: "{typography.label-md}"
    rounded: "{rounded.sm}"
    padding: 8px
  nav-item-selected:
    backgroundColor: "{colors.selected}"
    textColor: "{colors.foreground}"
    typography: "{typography.label-md}"
    rounded: "{rounded.sm}"
    padding: 8px
  track-row:
    backgroundColor: "transparent"
    textColor: "{colors.foreground}"
    typography: "{typography.body-md}"
    rounded: "{rounded.sm}"
    padding: 8px
  track-row-hover:
    backgroundColor: "{colors.hover}"
    textColor: "{colors.foreground}"
    typography: "{typography.body-md}"
    rounded: "{rounded.sm}"
    padding: 8px
  search-field:
    backgroundColor: "{colors.chrome}"
    textColor: "{colors.foreground}"
    typography: "{typography.body-md}"
    rounded: "{rounded.pill}"
    padding: 12px
  glass-panel:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.foreground}"
    typography: "{typography.body-md}"
    rounded: "{rounded.lg}"
    padding: 16px
  album-card:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.foreground}"
    typography: "{typography.body-md}"
    rounded: "{rounded.md}"
    padding: 12px
  chip:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.muted}"
    typography: "{typography.label-sm}"
    rounded: "{rounded.pill}"
    padding: 8px
  chip-selected:
    backgroundColor: "{colors.selected}"
    textColor: "{colors.foreground}"
    typography: "{typography.label-sm}"
    rounded: "{rounded.pill}"
    padding: 8px
  text-link:
    backgroundColor: "transparent"
    textColor: "{colors.accent}"
    typography: "{typography.body-md}"
  progress-fill:
    backgroundColor: "{colors.accent-secondary}"
    height: 4px
  divider:
    backgroundColor: "{colors.border}"
    height: 1px
  toast-error:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.error}"
    typography: "{typography.body-sm}"
    rounded: "{rounded.md}"
    padding: 12px
  status-success:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.success}"
    typography: "{typography.caption-md}"
    rounded: "{rounded.md}"
    padding: 12px
  status-warning:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.warning}"
    typography: "{typography.caption-md}"
    rounded: "{rounded.md}"
    padding: 12px
  icon-button:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    typography: "{typography.label-md}"
    rounded: "{rounded.sm}"
    padding: 8px
  icon-button-hover:
    backgroundColor: "{colors.hover}"
    textColor: "{colors.foreground}"
    typography: "{typography.label-md}"
    rounded: "{rounded.sm}"
    padding: 8px
---

# TuneX Frosted Obsidian

## Overview

TuneX is a local-first Linux music player for collectors with 100s to 50k+ tracks. Its personality is a high-end listening bar at night: calm, nocturnal, confident, premium. Visual mood is blue-black darkness with one electric-blue pilot light — never a neon arcade, never a gray office tool.

Density is medium-high: collectors scan hundreds of rows, so lists are compact but artwork stays large enough to recognize at a glance. Sessions are long (hours of playback) with short bursts of navigation (find → play → leave it alone).

The canonical visual reference is `docs/mockup.png`: layout, chrome, and component styling follow it; content is adapted to the local library per the adaptation map in `DESIGN_BRIEF.md` (streaming rails become local rails — same components, real data). Where this document and the mockup disagree on a V1-out-of-scope pattern (lyrics tabs, algorithmic rails, account chrome), this document wins.

Guiding principles (from the design brief — they arbitrate anything unspecified):

1. **Calm ownership over storefront energy.** The owned library is the whole universe; nothing shouts or recommends.
2. **Artwork leads, chrome recedes.** If a surface competes with album art, the surface loses.
3. **Every pixel responds.** Motion only ever explains a state change; perceived speed is a feature.

## Colors

Roles, darkest to brightest. Solid tokens below back every component; translucency/blur are described in Elevation & Depth and applied over these bases.

- **Canvas `{colors.background}` (#0B0B0E):** charcoal near-black, deliberately hue-free. Every neutral in this system is greyscale so the one royal blue reads as the brand instead of tinting the whole room; a navy-cast neutral makes an accent of the same family disappear into it. Window background everywhere; content never sits on pure black.
- **Surfaces `{colors.surface}` / `{colors.surface-raised}`:** cards, panels, drawers, dialogs. Raised is one step up for hoverable containers and toasts. Track rows deliberately stay transparent over the canvas — opaque rows would chop long lists into visual noise.
- **Wells `{colors.chrome}`:** recessed inputs (search field) — darker than cards so fields read as "type here".
- **Interaction states `{colors.hover}` / `{colors.selected}`:** row hover and nav selection. Selection always pairs the surface with an accent indicator bar plus `{colors.foreground}` text — never color alone.
- **Text `{colors.foreground}` / `{colors.muted}`:** high-contrast neutral for titles, rows, controls; neutral gray strictly for metadata (artist under title, durations, captions) at the 12px caption floor. Muted-on-background is 7.9:1, foreground 17.9:1; every token in this palette clears AA against the canvas.
- **Dividers `{colors.border}`:** 1px hairlines between rail sections and list groups. Borders, not shadows, separate most surfaces.
- **Primary `{colors.primary}` + hover `{colors.primary-hover}` with `{colors.on-primary}` white text:** royal blue, the single conversion color — primary buttons ("Play Something", "Add folder"), selected chips, active progress. White on primary is 5.6:1; on hover 4.9:1.
- **Signal accents `{colors.accent}` / `{colors.accent-secondary}`:** accent for text-level signals (eyebrow labels, links, "See all", playing-row marker, focus-adjacent highlights); accent-secondary for progress fill and gradient end-stops over artwork. Both stay inside the royal-blue family — no cyan, no teal — so the interface reads as one brand hue against grey. Accent is never a large-area fill and never body text.
- **Feedback `{colors.success}` / `{colors.warning}` / `{colors.error}`:** scan-complete and healthy states, scan warnings (permission-skipped files), errors and missing-file copy. Always paired with icon + words.
- **Focus `{colors.focus}`:** 2px keyboard focus ring on all interactive elements (see Components).
- **Theme behavior:** V1 is dark-only. No light tokens exist; do not invent light variants. Future themes add new token sets rather than reinterpreting these.

## Typography

Inter throughout (weights 400/500/600/700), fallback `"Inter, 'Noto Sans', system-ui, sans-serif"` in QML font stacks. One family keeps a dense media UI coherent; character comes from scale and artwork, not font novelty. Sizes are px tokens consumed by the Theme singleton (consistent cross-platform rendering for a media app; revisit if OS font-scale issues arise).

- `{typography.display-lg}` — hero greeting only ("Music feels different here."). 700, tight tracking. Never in lists or dialogs.
- `{typography.headline-md}` — view titles (artist/album names in detail headers, Now Playing title at large size).
- `{typography.title-md}` — section/rail headers ("Recently Played"), dialog titles, card titles at grid size.
- `{typography.body-md}` — track rows, menu items, primary UI text. Default reading size is 14px. Hierarchy stays display > headline > title > body > labels/captions; captions never drop below 12px.
- `{typography.body-sm}` — secondary lines (artist under title in rows, toasts). Tabular numbers on (`tnum`) so durations don't jitter.
- `{typography.label-md}` / `{typography.label-sm}` — buttons, nav items, chips, controls. Medium weight carries small sizes on dark.
- `{typography.caption-md}` — metadata: durations, counts, scan %, status lines. Always `{colors.muted}`, never smaller than 12px. `tnum` on for times/counts.
- `{typography.eyebrow-sm}` — tiny uppercase kickers ("Recently Played" alternatives, section eyebrows) in `{colors.accent}` with wide tracking. Decorative-only; never the sole carrier of meaning.
- Times, counts, and progress labels always use the `tnum` tokens so columns don't shift while scanning or playing.

## Layout

Shell grid (≥1280px): 240px nav rail · fluid content with 24px gutters · 320px Now Playing/Up Next panel as the persistent player (as in the mockup — no bottom bar at this width). Below 1280px the panel becomes a drawer and a 76px bottom mini-player bar carries the persistent player. Content max measure is fluid — artwork grids reflow 160–220px cards; text columns cap ~72ch in detail headers.

- **Rhythm:** 4px base unit; section gaps 24–32px (`lg`–`xl`); card internal padding 12–16px; row padding 8px vertical so 50k lists stay scannable. Page gutters 24px, never less than 16px.
- **Grouping:** hairline borders + whitespace group; cards only for artwork-bearing content (albums, playlists, hero). Plain rows group by alignment alone.
- **Breakpoints (window classes, desktop-only):** ≥1280 full three-column; 1024–1279 right panel becomes overlay drawer; 800–1023 rail collapses to 64px icon strip; minimum window 960×640. Placement changes, meaning never does.
- **Density:** one density for all sizes. No compact/comfortable toggle in V1 — the base density already serves both scanning and lounging.
- **Z-order:** content < right drawer < Now Playing overlay < menus < dialogs < toasts. Overlays dim the canvas with a translucent dark scrim, never a blur of interactive content beneath except Now Playing's own artwork backdrop.

## Elevation & Depth

Depth model: **tonal layering first, translucency second, shadows last.** Most hierarchy comes from stepping surface tones over the canvas plus 1px borders. This keeps 60fps scrolling on integrated GPUs and keeps the UI legible with blur disabled.

- **Glass is hierarchical, not decorative.** Strong glass: Now Playing overlay only (artwork backdrop → 24–40px backdrop blur → dark translucent tint → 1px top highlight → soft shadow → content). Subtle glass: dialogs, context menus, drawer (12–20px blur). Translucent chrome: the nav rail, top bar, and docked player are tinted-translucent (no blur) over the ambient wash. Opaque: track rows, cards, mini-player. If a mockup shows glass on a list row, the mockup is wrong.
- **Translucent chrome is tint, not blur.** The rail, top bar and docked player are on screen for the whole session, so a live backdrop blur under them would cost a blur pass every frame forever. They carry `{colors.surface}` at ~76% instead, which is see-through enough to read as glass and free to draw. Blur stays with the overlays that appear, act, and leave.
- **Ambient wash.** Translucency over a flat canvas reveals a flat canvas and reads as a lighter grey panel, so the shell paints the playing track's own artwork behind everything — blurred to abstraction, desaturated, held at ~14% and faded down the view so it colours the room without competing with the artwork actually on screen. It repaints on track change, never per frame, and with no track playing or transparency reduced it is simply absent. The room takes the record's colour; the record stays the only picture.
- **Glass stack (normative order):** background artwork/gradient → blur → dark tint (60–75% background) → 1px border/highlight → shadow → foreground content. Never blur without the tint — raw blurred art behind text fails contrast.
- **Shadows:** one restrained scale — dialogs/drawers `0 8px 32px rgba(0,0,0,0.45)`; hero `0 4px 24px rgba(0,0,0,0.35)`; cards none (borders do the work). No colored shadows except the primary transport button's faint blue glow (`0 0 24px rgba(91,140,255,0.35)`).
- **Glow discipline:** glow appears exactly twice — the playing transport button and the selected-nav indicator. Everywhere else, glow is a bug.
- **Reduced-transparency fallback:** with the blur toggle off, glass surfaces, translucent chrome, and the ambient wash all render as solid `{colors.surface}` / `{colors.surface-raised}` with the same borders — the wash draws nothing at all. Layout and hierarchy must survive this — test every overlay both ways.

## Shapes

Small, crisp, consistent radius scale. TuneX feels engineered, not bubbly: corners are present but quiet, so artwork and type carry the warmth.

- `{rounded.xs}` 4px — focus-adjacent ticks, progress track ends, tiny badges.
- `{rounded.sm}` 8px — rows, nav items, icon buttons, menus. The workhorse for anything clickable in a list.
- `{rounded.md}` 12px — album/playlist cards and thumbnails, toasts.
- `{rounded.lg}` 16px — hero card, dialogs, Now Playing panel, drawer.
- `{rounded.xl}` 24px — reserved for the expanded artwork frame if a future iteration needs it; do not use elsewhere in V1.
- `{rounded.pill}` — buttons, chips, search field. Pills signal "control"; rectangles-with-small-radius signal "content row". Never pill a card, never square a button.
- Artwork is always 1:1 with the card radius (never circular except the Now Playing crest if art is missing and a placeholder monogram is used).

## Components

Contracts for the V1 primitives. QML implements these as custom components (no visible stock Qt Quick Controls styling). All interactive elements show a 2px `{colors.focus}` ring on keyboard focus; pressed states scale nothing — color shift only.

**Icon system (normative):** one line-icon family in the Lucide style — 2px stroke at 24px, rounded caps/corners, straight-on metaphors. Sizes 16px (dense rows, validation), 20px (compact buttons), 24px (default: nav, buttons, transport); touch/keyboard targets stay 44px+ via transparent padding. Icon + text is the default (rail items, buttons, empty states, validation); icon-only is allowed only for universal transport/media symbols and always carries an accessible name plus hover tooltip. Never mix stroke weights or families in one view; never use emoji or file-type/brand glyphs as UI actions; icons meet 3:1 non-text contrast in dark. Dynamic icons (play↔pause, mute, shuffle/repeat state) update their accessible name with the state.

- **Primary button (`button-primary` → `button-primary-hover`):** pill, 44px min height, label-md. Reserved for the one main action per view (Play, Add folder, Save). One per view — a second primary action is a design error; demote to secondary. Disabled: 38% opacity, desaturated, no hover. Loading: icon swaps to spinner, width locked so layout never shifts.
- **Secondary button:** same geometry on `{colors.surface-raised}`; hover steps to `{colors.hover}`. Row-level and dialog-cancel actions.
- **Nav item (`nav-item` → `nav-item-selected`):** full-width 40px rows, icon + label-md; selected adds the surface plus a 3px accent bar at the leading edge. Unselected text is muted; icons inherit text color.
- **Track row (`track-row` → `track-row-hover`):** 56px, transparent; columns art-thumb 40px (sm radius) · title/body-md + artist/body-sm-muted · duration/caption-muted right. Playing row: accent 3px bar + title in foreground (not accent-colored text) + animated equalizer tick allowed only with motion on. Missing-file rows dim to 50% with "File missing" caption + actions.
- **Search field (`search-field`):** pill well with magnifier icon; typed text foreground, placeholder muted at full opacity (never transparent-ized placeholder text); Esc clears; focus ring replaces any glow.
- **Glass panel (`glass-panel`):** dialogs and menus only. Dialogs: title-md + body + primary/secondary actions; menus: body-md items 40px with hover surface. Menus open toward available space, never off-window.
- **Album/artist/playlist cards (`album-card`):** art on top (1:1, md radius), title/body-md truncated 1 line, subtitle/body-sm-muted 1 line; hover lifts nothing — art gets a 4% brighten + play-affordance overlay button. No text wider than the art.
- **Chips (`chip` → `chip-selected`):** filter pills (search groups, view filters). Selected steps up to the selected surface with foreground text — max one selected per group in V1.
- **Progress + volume (`progress-fill` on xs track):** 4px track in `{colors.hover}`, cyan fill, 12px thumb appearing on hover/focus/drag. Dragging scrubs the engine live; time labels use tabular caption and never overlap the thumb at min width.
- **Now Playing surface:** lg glass panel; artwork ≥320px md radius with 180ms crossfade on track change; headline title, muted artist, favorite heart toggle; transport row (shuffle · prev · play/pause 64px primary circle · next · repeat) + volume + queue toggle.
- **Toast (`toast-error`, `status-success`, `status-warning`):** bottom-center above the persistent player, surface-raised, icon + one-line copy + optional action ("Reveal", "Rescan", "Retry"). Auto-dismiss 5s except errors with actions (persist until dismissed).
- **Empty states:** centered art-glyph + title-md + one body-sm line + one primary action. Three variants only: no-library (Add folder), no-results (Clear search), empty-playlist (Browse library).
- **Loading:** skeleton blocks in card/row shape for library and search operations over ~300ms; determinate progress (counts + %) for scans; spinner only inside the triggering button. Never a blank screen.
- **Artwork component:** 1:1, three resolutions (64/256/512) with generated placeholder (initial-letter monogram on surface-raised); fade-in 120ms on load; broken/oversized art falls back silently to placeholder.

## Do's and Don'ts

- Do give every view exactly one primary action; don't place two blue pill buttons side by side.
- Do keep track rows opaque/transparent over canvas; don't put blur, glass, or shadows on list rows.
- Do use `{colors.accent}` for signals (playing marker, links, kickers); don't fill large areas with it or set body copy in it.
- Do pair every color-coded state with text or icon (playing bar + label, error icon + copy); don't communicate status by hue alone.
- Do truncate titles/artists to 1–2 lines with ellipsis; don't shrink type to fit or clip without affordance.
- Do write honest copy ("File missing", "Unknown artist", "Searched your library"); don't fake metadata, artwork, or availability.
- Do keep motion to 120–220ms single-property transitions (see below); don't add entrance choreography, parallax, or animated gradients.
- Do test each overlay with transparency off; don't ship a surface that only works blurred.
- Do name domain things per the IA glossary (Songs view, Up Next panel, Music folders); don't use "Liked Songs", "Discover", "Trending", or any service vocabulary.
- Do disclose progressively: hover/focus reveals row actions (with an always-visible touch target), drawers hold queue and Now Playing, "See all"/"More" escalate lists, context menus replace action toolbars; don't show more than ~7 primary choices per view or hide anything behind an unlabeled affordance.
- Do keep dialogs and menus on the glass panel contract; don't invent new surface colors per dialog.
- Don't add bells, avatars, badges, or algorithmic rails — the out-of-scope patterns stay out of V1 even where the mockup sketches them.

## Motion & Accessibility

Motion budget (all QML behaviors, standard easing `OutCubic`): hover/focus 120ms color-only; artwork crossfade 180ms opacity; row insert/remove 160ms; drawer/overlay open 200ms translate+fade; progress follows the engine (no easing). Reduce-motion toggle forces all durations to 0 (instant state swaps) except progress, which keeps following playback position.

- Minimum 44px hit areas for all interactive targets (icons 16/20/24px optical inside padded hit areas); the sole exception is dense TrackList inline actions at 40px with ≥8px separation. Never smaller.
- Focus order matches visual order; the persistent-player transport is reachable without opening Now Playing.
- Screen-reader names on all icon-only buttons; rows expose "title, artist, duration"; progress exposes position/duration; toasts announce politely.
- Body and label text hold WCAG AA (primary/white ~5.2:1, muted/canvas ~7:1, accent/canvas ~5.9:1 per token pairs above); captions never carry essential meaning alone.
