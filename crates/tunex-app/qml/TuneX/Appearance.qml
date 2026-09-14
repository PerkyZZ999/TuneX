pragma Singleton
import QtQuick
import TuneX

// Appearance (S6): runtime appearance state shared by every surface. App
// seeds the `[appearance]` preferences once at startup (config.toml via
// QueueModel) and points `canvas` at the shell content that glass
// backdrops sample. Overlays are siblings of the canvas, never children, so
// a capture can never feed back into itself. Holds references and flags
// only — never parents items (singleton rule).
QtObject {
    property Item canvas: null
    // Opaque-surface fallback: glass renders as solid surface + border.
    property bool reduceTransparency: false
    // Instant state swaps: every transition duration collapses to 0.
    property bool reduceMotion: false
    // Rail, top bar, and docked player: `{colors.panel}` at the documented
    // chrome tint, or solid panel when transparency is off. The content
    // pane is the canvas — a near-black stage inside that frame.
    readonly property color chromeFill: reduceTransparency ? Theme.panel : Qt.rgba(Theme.panel.r, Theme.panel.g, Theme.panel.b, Theme.chromeTint)
    readonly property color contentFill: Theme.background

    // Every transition duration goes through here (motion budget in
    // Theme). Progress bars never animate: they follow the engine.
    function duration(ms: int): int {
        return reduceMotion ? 0 : ms;
    }
}
