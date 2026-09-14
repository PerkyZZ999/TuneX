pragma Singleton
import QtQuick

// Centralized design tokens (docs/DESIGN.md "TuneX Frosted Obsidian").
// Every color, radius, spacing, and type value in the UI must come from here;
// never hardcode visual constants in components.
// The neutrals are charcoal, not navy: every base tone is hue-free so the one
// royal blue reads as the brand instead of competing with a blue room. Ratios
// beside each token are measured against the canvas.
QtObject {
    // Canvas and surfaces. Each step is a real lift so the shell has depth:
    // near-black stage, charcoal chrome, then cards, then raised controls.
    readonly property color background: "#0B0B0B"
    readonly property color surface: "#222222"
    readonly property color surfaceRaised: "#2A2A2A"
    // Chrome only (rail, top bar, docked player) at chromeTint — not content.
    readonly property color panel: "#1A1A1A"
    readonly property color chrome: "#000000"
    readonly property color hover: "#343434"
    readonly property color selected: "#3D3D3D"
    // Text and dividers — greyscale only. Blue lives on primary/accent/focus.
    readonly property color foreground: "#F4F4F4"   // ~17.9:1 on canvas
    readonly property color muted: "#A3A3A3"        // ~7.6:1 on canvas
    readonly property color border: "#2E2E2E"
    // Actions and signals — the royal blue family, and nothing else.
    readonly property color primary: "#2B5CE6"      // white on it 5.6:1
    readonly property color primaryHover: "#3766EE" // white on it 4.9:1
    // DESIGN.md token on-primary (`onX` names are reserved in QML).
    readonly property color primaryText: "#FFFFFF"
    readonly property color accent: "#6E9BFF"       // 7.3:1
    readonly property color accentSecondary: "#4C86FF" // 5.8:1, progress fill
    readonly property color success: "#4ED07A"      // 10.0:1
    readonly property color warning: "#F5B544"      // 10.8:1
    readonly property color error: "#FF8585"        // 8.4:1
    readonly property color focus: "#9BBBFF"        // 10.2:1
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
    readonly property int spaceLg: 20
    readonly property int spaceXl: 28
    readonly property int spaceXxl: 40
    // Type scale (Inter with system fallback; px per DESIGN.md).
    readonly property string fontFamily: "Inter"
    readonly property int fontDisplay: 28
    readonly property int fontHeadline: 18
    readonly property int fontTitle: 14
    readonly property int fontBody: 13
    readonly property int fontBodySm: 12
    readonly property int fontLabel: 12
    readonly property int fontLabelSm: 12
    readonly property int fontCaption: 12
    readonly property int fontEyebrow: 12
    // Shell geometry.
    readonly property int windowMinWidth: 960
    readonly property int windowMinHeight: 640
    readonly property int railWidth: 216
    readonly property int railNarrow: 64
    readonly property int panelWidth: 288
    // Labeled and icon-strip rail rows share 32px compact chrome and 16px
    // icons. Compact mode only hides the label (64px strip).
    readonly property int navItemHeight: 32
    readonly property int navIconSize: 16
    readonly property int navAccent: 5
    readonly property int historyButton: 24
    readonly property int searchFieldHeight: 36
    // Dialog measure: title + one field or two lines of body copy.
    readonly property int dialogWidth: 400
    // DESIGN.md: three-column shell at ≥1280; 64px opaque mini-player below.
    readonly property int shellWide: 1280
    readonly property int shellCompact: 1024
    readonly property int miniPlayerHeight: 64
    readonly property int progressTrack: 4
    readonly property int targetMin: 44
    readonly property int buttonHeight: 32
    readonly property int artThumb: 40
    readonly property int topBarHeight: 48
    // Same 32px compact chrome as labeled nav rows (Penpot Home density).
    readonly property int trackRowHeight: 32
    readonly property real listLineHeight: 1.2
    readonly property int brandMark: 24
    readonly property int cardMetaHeight: 56
    readonly property int heroHeight: 280
    readonly property int chipHeight: 28
    readonly property int playBadge: 32
    readonly property int emptyArt: 128
    readonly property int gridMin: 152
    readonly property int gridTarget: 176
    // Now Playing overlay (DESIGN.md: art ≥280, play 52, blur 24–40, 200ms).
    readonly property int nowPlayingArt: 280
    readonly property int playPrimary: 52
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
    // Shell chrome translucency. The rail, top bar and docked player are
    // always on screen, so they are tinted-translucent rather than blurred:
    // a live backdrop blur under permanently visible chrome would cost a
    // blur pass every frame for an effect nobody can point at. What they let
    // through is the ambient wash below, which is the part the eye reads as
    // glass. The content pane is the opaque canvas — a near-black stage.
    readonly property real chromeTint: 0.76
    // Ambient wash: the playing track's own artwork, blurred to abstraction
    // and held this far down so it colours the room without ever competing
    // with content (DESIGN.md: artwork leads, chrome recedes). It repaints on
    // track change, not per frame.
    readonly property real ambientOpacity: 0.14
    readonly property int ambientBlur: 64
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
