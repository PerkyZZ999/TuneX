pragma Singleton
import QtQuick

// Centralized design tokens (docs/DESIGN.md "TuneX Frosted Obsidian").
// Every color, radius, spacing, and type value in the UI must come from here;
// never hardcode visual constants in components.
QtObject {
    // Canvas and surfaces.
    readonly property color background: "#0A0D14"
    readonly property color surface: "#121724"
    readonly property color surfaceRaised: "#1A2133"
    readonly property color chrome: "#141B2E"
    readonly property color hover: "#1B2338"
    readonly property color selected: "#1E2A4A"
    // Text and dividers.
    readonly property color foreground: "#F2F5FA"
    readonly property color muted: "#9AA5BA"
    readonly property color border: "#232C42"
    // Actions and signals.
    readonly property color primary: "#2F62E8"
    readonly property color primaryHover: "#3567E6"
    // DESIGN.md token on-primary (`onX` names are reserved in QML).
    readonly property color primaryText: "#FFFFFF"
    readonly property color accent: "#5B8CFF"
    readonly property color accentSecondary: "#6FD3FF"
    readonly property color success: "#52D273"
    readonly property color warning: "#F5B544"
    readonly property color error: "#FF8585"
    readonly property color focus: "#8FB4FF"
    // Shape scale.
    readonly property int radiusXs: 4
    readonly property int radiusSm: 8
    readonly property int radiusMd: 12
    readonly property int radiusLg: 16
    readonly property int radiusXl: 24
    readonly property int radiusPill: 999
    // Spacing scale (4px base).
    readonly property int spaceXs: 4
    readonly property int spaceSm: 8
    readonly property int spaceMd: 16
    readonly property int spaceLg: 24
    readonly property int spaceXl: 32
    readonly property int spaceXxl: 48
    // Type scale (Inter with system fallback; px per DESIGN.md).
    readonly property string fontFamily: "Inter"
    readonly property int fontDisplay: 34
    readonly property int fontHeadline: 24
    readonly property int fontTitle: 18
    readonly property int fontBody: 16
    readonly property int fontBodySm: 13
    readonly property int fontLabel: 14
    readonly property int fontLabelSm: 13
    readonly property int fontCaption: 12
    // Shell geometry.
    readonly property int windowMinWidth: 960
    readonly property int windowMinHeight: 640
    readonly property int railWidth: 240
    readonly property int railNarrow: 64
    readonly property int panelWidth: 320
    // Dialog measure: title + one field or two lines of body copy.
    readonly property int dialogWidth: 440
    // DESIGN.md: three-column shell at ≥1280; 76px opaque mini-player below.
    readonly property int shellWide: 1280
    readonly property int shellCompact: 1024
    readonly property int miniPlayerHeight: 76
    readonly property int progressTrack: 4
    readonly property int targetMin: 44
    readonly property int artThumb: 48
    // Now Playing overlay (DESIGN.md: art ≥320, play 64, blur 24–40, 200ms).
    readonly property int nowPlayingArt: 320
    readonly property int playPrimary: 64
    readonly property int overlayMs: 200
    readonly property int artCrossfadeMs: 180
    readonly property int thumbSize: 12
    readonly property real overlayTint: 0.68
    // Blur. DESIGN.md sizes blur like CSS `blur()`; MultiEffect's blurMax is
    // not that scale (measured in KWin, S6 W-038: blurMax 32 softens an edge
    // ~10px). Calibrated: blurMax 64 ≈ CSS 16px (subtle glass, 12–20px);
    // blurMax 64 × multiplier 1 ≈ CSS 32px (strong glass, 24–40px).
    readonly property int strongBlur: 64
    readonly property real strongBlurMultiplier: 1
    // Subtle glass for drawers, dialogs, and context menus (DESIGN.md
    // Elevation & Depth: 12–20px blur, 60–75% dark tint). Strong glass stays
    // exclusive to Now Playing; rows, rail, cards, mini-player stay opaque.
    readonly property int glassBlur: 64
    readonly property real glassTint: 0.7
    // Elevation (DESIGN.md: one restrained shadow scale, black). Blur values
    // are the CSS radius × 1.2, the documented RectangularShadow match.
    // Overlay: dialogs/drawers/menus `0 8px 32px rgba(0,0,0,0.45)`.
    readonly property color shadow: "#000000"
    readonly property int shadowOverlayBlur: 38
    readonly property int shadowOverlayY: 8
    readonly property real shadowOverlayOpacity: 0.45
    // Hero `0 4px 24px rgba(0,0,0,0.35)`; cards carry none.
    readonly property int shadowHeroBlur: 29
    readonly property int shadowHeroY: 4
    readonly property real shadowHeroOpacity: 0.35
    // Glow, exactly twice: the playing transport button
    // (`0 0 24px rgba(91,140,255,0.35)`, accent) and the selected-nav bar.
    readonly property int glowBlur: 29
    readonly property real glowOpacity: 0.35
    readonly property int glowNavBlur: 10
    readonly property real glowNavOpacity: 0.55
    // Translucent dark scrim behind modal overlays (never a blur of the UI).
    readonly property real scrimOpacity: 0.6
    // Motion budget (DESIGN.md Motion & Accessibility; OutCubic everywhere).
    // Durations go through Appearance.duration() so reduce-motion zeroes them.
    readonly property int motionHover: 120
    readonly property int motionRow: 160
}
