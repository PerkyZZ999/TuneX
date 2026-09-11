// GlassBackdrop (S6 W-038): the subtle-glass surface behind transient
// overlays — drawers, dialogs, and context menus (DESIGN.md Elevation &
// Depth). Normative stack, bottom to top: shadow → canvas snapshot → ~16px
// blur → 70% dark tint → 1px edge → host content. Strong glass stays
// exclusive to Now Playing; rows, rail, cards, and the mini-player stay
// opaque. With Appearance.reduceTransparency it renders the solid
// glass-panel contract (surface + border) and samples nothing.
//
// The snapshot is taken once per show/resize (live: false), so an open
// overlay costs one blur pass, not one per frame, and closed overlays cost
// nothing. The shell canvas paints its own opaque background, so the
// blurred snapshot fully covers the sharp content beneath it (the W-038
// audit caught rows ghosting through a transparent capture). Hosts that
// slide in (drawers) pass `restingRect`, their final geometry in canvas
// coordinates, so the snapshot never samples the off-screen start position.

import QtQuick
import QtQuick.Effects
import TuneX 1.0

Item {
    id: root

    property real cornerRadius: 0
    // Resting geometry in canvas coordinates; empty maps this item instead.
    property rect restingRect: Qt.rect(0, 0, 0, 0)
    property bool shadowEnabled: true
    readonly property bool glass: !Appearance.reduceTransparency && Appearance.canvas !== null

    function refresh() {
        if (!root.glass || !root.visible || root.width <= 0 || root.height <= 0)
            return;

        let area = root.restingRect;
        if (area.width <= 0 || area.height <= 0) {
            const origin = root.mapToItem(Appearance.canvas, 0, 0);
            area = Qt.rect(origin.x, origin.y, root.width, root.height);
        }
        capture.sourceRect = area;
        capture.scheduleUpdate();
    }

    implicitWidth: 0
    implicitHeight: 0
    onVisibleChanged: Qt.callLater(root.refresh)
    onWidthChanged: Qt.callLater(root.refresh)
    onHeightChanged: Qt.callLater(root.refresh)
    onRestingRectChanged: Qt.callLater(root.refresh)

    // Overlay elevation: `0 8px 32px rgba(0,0,0,0.45)` — the one shadow
    // step for dialogs, drawers, and menus.
    RectangularShadow {
        anchors.fill: parent
        visible: root.shadowEnabled
        radius: root.cornerRadius
        offset.y: Theme.shadowOverlayY
        blur: Theme.shadowOverlayBlur
        color: Qt.alpha(Theme.shadow, Theme.shadowOverlayOpacity)
    }

    ShaderEffectSource {
        id: capture

        anchors.fill: parent
        implicitWidth: 0
        implicitHeight: 0
        visible: false
        sourceItem: root.glass ? Appearance.canvas : null
        live: false
        hideSource: false
    }

    // Rounded surfaces mask the blur to their corner radius.
    Rectangle {
        id: cornerMask

        anchors.fill: parent
        radius: root.cornerRadius
        visible: false
        layer.enabled: root.glass && root.cornerRadius > 0
    }

    MultiEffect {
        anchors.fill: parent
        visible: root.glass
        source: capture
        autoPaddingEnabled: false
        blurEnabled: true
        blurMax: Theme.glassBlur
        blur: 1
        maskEnabled: root.cornerRadius > 0
        maskSource: cornerMask
    }

    Rectangle {
        anchors.fill: parent
        radius: root.cornerRadius
        color: root.glass ? Qt.alpha(Theme.background, Theme.glassTint) : Theme.surface
        border.color: Theme.border
        border.width: 1
    }

    // Top-edge highlight on flush panels (Now Playing pattern); rounded
    // surfaces rely on the border above instead.
    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        height: 1
        visible: root.glass && root.cornerRadius === 0
        color: Qt.alpha(Theme.foreground, 0.12)
        Accessible.ignored: true
    }
}
