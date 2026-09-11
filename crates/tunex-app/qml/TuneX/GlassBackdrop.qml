// GlassBackdrop (S6 W-038): subtle-glass background for transient overlays
// (drawers, dialogs, context menus) per DESIGN.md Elevation & Depth —
// backdrop snapshot, 12-20px blur, 60-75% dark tint, 1px edge treatment.
// Strong glass stays exclusive to Now Playing; rows, rail, cards, and the
// mini-player stay opaque. With transparency off, renders the solid
// glass-panel contract (surface + border). No shadow pass: restraint —
// blur, tint, and highlight carry the elevation alone.

import QtQuick
import QtQuick.Effects
import TuneX 1.0

// The capture is a snapshot (never live): the grab refreshes when this
// background shows or resizes. A live ShaderEffectSource inside a
// content-sized Menu inflates the popup to the canvas size through its
// texture-sized implicit size and risks a cross-surface feedback loop
// that blanks the whole popup — both observed in KWin runs. Snapshot
// also keeps the per-frame cost at zero, which the M6 profile pass wants.
Item {
    // Blur switch. Drawers and dialogs keep the snapshot blur; context
    // menus set this false and render the tint+border panel only: a live
    // effect chain inside a Menu popup blanks the whole menu in KWin runs
    // (dialogs/drawers with the identical background render fine), so
    // menus take the restraint look rather than no menu at all.

    id: root

    property color tint: Theme.background
    property real tintOpacity: Theme.glassTint
    property int cornerRadius: 0
    // Opaque-surface preference; hosts read queue.reduceTransparency().
    property bool transparencyOff: false
    // Blur kill-switch for context menus (see note above).
    property bool disableBlur: false

    function refresh() {
        if (root.disableBlur)
            return ;

        if (capture.visible && root.width > 0 && root.height > 0)
            capture.scheduleUpdate();

    }

    implicitWidth: 0
    implicitHeight: 0
    onVisibleChanged: {
        if (visible)
            Qt.callLater(root.refresh);

    }
    onWidthChanged: root.refresh()
    onHeightChanged: root.refresh()

    // Snapshot of the shell canvas beneath this surface. Grabbed on show
    // and resize only, so open overlays cost one blur pass, not one per
    // frame, and closed overlays cost nothing.
    ShaderEffectSource {
        id: capture

        anchors.fill: parent
        implicitWidth: 0
        implicitHeight: 0
        visible: !root.disableBlur && !root.transparencyOff && Glass.canvas !== null
        sourceItem: Glass.canvas
        live: false
        hideSource: false
        sourceRect: {
            if (Glass.canvas === null)
                return Qt.rect(0, 0, 0, 0);

            const topLeft = root.mapToItem(Glass.canvas, 0, 0);
            return Qt.rect(topLeft.x, topLeft.y, root.width, root.height);
        }
    }

    MultiEffect {
        anchors.fill: parent
        visible: capture.visible
        source: capture
        autoPaddingEnabled: false
        blurEnabled: true
        blurMax: Theme.glassBlur
        blur: 1
    }

    Rectangle {
        id: tintRect

        anchors.fill: parent
        radius: root.cornerRadius
        visible: !root.transparencyOff
        color: root.tint
        opacity: root.tintOpacity
        border.color: Theme.border
        border.width: 1
    }

    Rectangle {
        anchors.fill: parent
        radius: root.cornerRadius
        visible: root.transparencyOff
        color: Theme.surface
        border.color: Theme.border
        border.width: 1
    }

    // Top-edge highlight on flush panels (Now Playing pattern). Rounded
    // surfaces rely on the border above instead.
    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        height: 1
        visible: !root.transparencyOff && root.cornerRadius === 0
        color: Theme.foreground
        opacity: 0.12
        Accessible.ignored: true
    }

}
