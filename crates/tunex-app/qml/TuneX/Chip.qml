import QtQuick
import TuneX

// Filter chip (DESIGN.md chip / chip-selected). Selected steps the surface;
// meaning also lives in the label (never color-only).
Item {
    id: root

    property string label: ""
    property bool selected: false

    signal activated

    implicitWidth: chipText.implicitWidth + Theme.spaceSm * 2
    implicitHeight: Theme.chipHeight
    width: implicitWidth
    height: implicitHeight
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: root.label
    Accessible.checkable: true
    Accessible.checked: root.selected
    Accessible.onPressAction: root.activated()
    Keys.onSpacePressed: root.activated()
    Keys.onReturnPressed: root.activated()
    Keys.onEnterPressed: root.activated()

    Rectangle {
        anchors.fill: parent
        radius: Theme.radiusXs
        color: root.selected ? Theme.selected : (chipMouse.containsMouse ? Theme.hover : "transparent")
        Behavior on color {
            ColorAnimation {
                duration: Appearance.duration(Theme.motionHover)
                easing.type: Easing.OutCubic
            }
        }
        border.width: root.activeFocus ? 2 : 1
        border.color: root.activeFocus ? Theme.focus : (root.selected ? Theme.selected : Theme.border)
    }

    Text {
        id: chipText

        anchors.centerIn: parent
        text: root.label
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontLabelSm
        font.weight: Font.Medium
        color: root.selected ? Theme.foreground : Theme.muted
    }

    MouseArea {
        id: chipMouse

        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.activated()
    }
}
