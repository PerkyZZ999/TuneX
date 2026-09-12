import QtQuick
import TuneX

// TextLink (S6 W-044): the accent text control that escalates a rail to its
// full view ("See all"). It reads as text but behaves as a control, which is
// exactly where a bare Text plus a MouseArea goes wrong: the label's own
// height is roughly 18px, well under the 44px minimum hit area, and a Text
// takes no focus, so the affordance was unreachable by keyboard.
//
// The hit area is padded out to 44px around the label without moving it, so
// the label still sits flush with its section header.
FocusScope {
    id: root

    property string text: ""
    // Announced instead of the label, which is usually too terse on its own
    // ("See all" -> "See all albums").
    property string accessibleName: root.text

    signal activated

    implicitWidth: Math.max(label.implicitWidth, Theme.targetMin)
    implicitHeight: Theme.targetMin
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: root.accessibleName
    Accessible.onPressAction: root.activated()
    Keys.onReturnPressed: root.activated()
    Keys.onEnterPressed: root.activated()
    Keys.onSpacePressed: root.activated()

    Rectangle {
        anchors.fill: parent
        radius: Theme.radiusSm
        color: "transparent"
        border.color: root.activeFocus ? Theme.focus : "transparent"
        border.width: root.activeFocus ? 2 : 0
    }

    Text {
        id: label

        anchors.centerIn: parent
        text: root.text
        textFormat: Text.PlainText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontLabel
        font.weight: Font.Medium
        color: linkArea.containsMouse ? Theme.foreground : Theme.accent

        Behavior on color {
            ColorAnimation {
                duration: Appearance.duration(Theme.motionHover)
                easing.type: Easing.OutCubic
            }
        }
    }

    MouseArea {
        id: linkArea

        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.activated()
    }
}
