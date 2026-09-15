import QtQuick
import TuneX

// SettingsCycleRow: one cycling settings row — title plus current value,
// optional trailing action button, whole row clickable. Repeat,
// ReplayGain, output, Now Playing view, and EQ preset shared one
// hand-rolled copy each.
Item {
    id: root

    property string title: ""
    property string value: ""
    // Empty hides the trailing button (the view and preset rows carry none).
    property string iconName: ""
    property string accessibleName: ""
    property string buttonAccessibleName: ""
    property bool checkable: false
    property bool checked: false

    signal activated

    implicitHeight: Math.max(Theme.targetMin, copy.implicitHeight + Theme.spaceSm * 2)
    height: implicitHeight
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: root.accessibleName
    Keys.onSpacePressed: root.activated()
    Keys.onReturnPressed: root.activated()
    Keys.onEnterPressed: root.activated()

    Rectangle {
        anchors.fill: parent
        radius: Theme.radiusSm
        color: rowMouse.containsMouse || parent.activeFocus ? Theme.hover : "transparent"
        border.width: parent.activeFocus ? 2 : 0
        border.color: Theme.focus
    }

    Column {
        id: copy

        anchors.left: parent.left
        anchors.right: root.iconName !== "" ? actionButton.left : parent.right
        anchors.rightMargin: root.iconName !== "" ? Theme.spaceMd : 0
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.spaceXs

        Text {
            text: root.title
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBody
            font.weight: Font.Medium
            color: Theme.foreground
        }

        Text {
            text: root.value
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBodySm
            color: Theme.muted
        }
    }

    IconButton {
        id: actionButton

        visible: root.iconName !== ""
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        iconName: root.iconName
        accessibleName: root.buttonAccessibleName
        checkable: root.checkable
        checked: root.checked
        onActivated: root.activated()
    }

    MouseArea {
        id: rowMouse

        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.activated()
    }
}
