pragma Singleton
import QtQuick

// Glass (S6 W-038): shared backdrop reference for subtle-glass overlays.
// App sets `canvas` once to the shell content column; GlassBackdrop surfaces
// (drawers, dialogs, context menus) sample it live. Overlays are siblings of
// the canvas, never children, so the capture can never feed back into
// itself. Holds a reference only — never parents items (singleton rule).
QtObject {
    property Item canvas: null
}
