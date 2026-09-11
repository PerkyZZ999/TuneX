import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Effects
import TuneX

// PlayButton (S6 W-038): the primary transport control — a filled primary
// circle with the play/pause glyph, shared by the docked panel, the
// MiniPlayer, and Now Playing. It carries one of the two sanctioned glows
// (DESIGN.md: `0 0 24px rgba(91,140,255,0.35)`), and only while playing, so
// the glow echoes the state the glyph and accessible name already state.
// Pressed states shift colour only; nothing scales.
Item {
    id: root

    property bool playing: false
    property int diameter: Theme.playPrimary
    // One source for the label the button answers by, so the tooltip and the
    // accessible name cannot drift apart.
    readonly property string actionLabel: root.playing ? qsTr("Pause") : qsTr("Play")

    signal activated

    width: root.diameter
    height: root.diameter
    implicitWidth: root.diameter
    implicitHeight: root.diameter
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: root.actionLabel
    Accessible.onPressAction: root.activated()
    Keys.onSpacePressed: root.activated()
    Keys.onReturnPressed: root.activated()
    Keys.onEnterPressed: root.activated()
    ToolTip.visible: hit.containsMouse
    ToolTip.text: root.actionLabel
    ToolTip.delay: 400

    RectangularShadow {
        anchors.fill: disc
        visible: root.playing && root.enabled
        radius: disc.radius
        blur: Theme.glowBlur
        color: Qt.alpha(Theme.accent, Theme.glowOpacity)
    }

    Rectangle {
        id: disc

        anchors.fill: parent
        radius: width / 2
        color: hit.containsMouse ? Theme.primaryHover : Theme.primary
        opacity: root.enabled ? 1 : 0.38

        Behavior on color {
            ColorAnimation {
                duration: Appearance.duration(Theme.motionHover)
                easing.type: Easing.OutCubic
            }
        }
    }

    // Focus ring sits outside the disc so it reads against the page.
    Rectangle {
        anchors.centerIn: parent
        width: root.diameter + Theme.spaceSm
        height: width
        radius: width / 2
        color: "transparent"
        border.width: 2
        border.color: Theme.focus
        visible: root.activeFocus
    }

    Icon {
        anchors.centerIn: parent
        // Optical centre: the triangle's mass sits left of its box.
        anchors.horizontalCenterOffset: root.playing ? 0 : 1
        name: root.playing ? "pause" : "play"
        iconSize: root.diameter >= Theme.playPrimary ? 24 : 20
        stroke: Theme.primaryText
        filled: true
    }

    MouseArea {
        id: hit

        anchors.fill: parent
        enabled: root.enabled
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.activated()
    }
}
