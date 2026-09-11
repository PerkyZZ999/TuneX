import QtQuick
import QtQuick.Controls.Basic
import TuneX

// GlassMenu (S6 W-038): context menus on the glass-panel contract (DESIGN.md
// Components) — subtle glass with the overlay shadow, lg radius, 40px
// body-md GlassMenuItem rows with a hover surface, hairline separators.
// Sub-menu entries render through the same delegate. Menus open toward
// available space and stay inside the window (margins), never off-window.
Menu {
    id: root

    topPadding: Theme.spaceXs
    bottomPadding: Theme.spaceXs
    margins: Theme.spaceSm
    delegate: GlassMenuItem {}

    enter: Transition {
        NumberAnimation {
            property: "opacity"
            from: 0
            to: 1
            duration: Appearance.duration(Theme.motionHover)
            easing.type: Easing.OutCubic
        }
    }

    exit: Transition {
        NumberAnimation {
            property: "opacity"
            from: 1
            to: 0
            duration: Appearance.duration(Theme.motionHover)
            easing.type: Easing.OutCubic
        }
    }

    background: GlassBackdrop {
        implicitWidth: 224
        cornerRadius: Theme.radiusLg
    }
}
