import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Effects
import TuneX 1.0

// One selectable row in the navigation rail: Lucide glyph + label, accent
// bar when selected. Compact mode hides the label (64px icon strip). The
// selected bar carries one of the two sanctioned glows (DESIGN.md Glow
// discipline). Mouse presses do not take focus, so the 2px ring appears
// for keyboard focus only.
Item {
    id: root

    property string label: ""
    property string iconName: "home"
    property bool selected: false
    property bool compact: false

    signal activated

    width: parent.width
    height: Theme.targetMin
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: root.label
    Accessible.selected: root.selected
    Keys.onSpacePressed: root.activated()
    Keys.onEnterPressed: root.activated()
    Keys.onReturnPressed: root.activated()
    ToolTip.visible: root.compact && navMouse.containsMouse
    ToolTip.text: root.label
    ToolTip.delay: 400

    Rectangle {
        anchors.fill: parent
        radius: Theme.radiusSm
        color: root.selected ? Theme.selected : (navMouse.containsMouse || root.activeFocus ? Theme.hover : "transparent")
    }

    RectangularShadow {
        anchors.fill: accentBar
        visible: root.selected
        radius: accentBar.radius
        blur: Theme.glowNavBlur
        color: Qt.alpha(Theme.accent, Theme.glowNavOpacity)
    }

    Rectangle {
        id: accentBar

        width: 3
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.leftMargin: 4
        radius: 2
        color: Theme.accent
        visible: root.selected
    }

    Icon {
        id: glyph

        anchors.verticalCenter: parent.verticalCenter
        anchors.left: parent.left
        anchors.leftMargin: root.compact ? (parent.width - width) / 2 : Theme.spaceMd
        name: root.iconName
        iconSize: 20
        stroke: root.selected ? Theme.foreground : Theme.muted
    }

    Text {
        visible: !root.compact
        anchors.verticalCenter: parent.verticalCenter
        anchors.left: glyph.right
        anchors.leftMargin: Theme.spaceSm
        anchors.right: parent.right
        anchors.rightMargin: Theme.spaceSm
        elide: Text.ElideRight
        text: root.label
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontLabel
        font.weight: Font.Medium
        color: root.selected ? Theme.foreground : Theme.muted
    }

    MouseArea {
        id: navMouse

        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.activated()
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.radiusSm
        color: "transparent"
        border.width: 2
        border.color: Theme.focus
        visible: root.activeFocus
    }
}
