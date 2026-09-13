import QtQuick
import TuneX

// SettingsToggle: one settings row — label + optional description + a
// switch. The whole 44px row is the hit target (DESIGN.md), so the switch
// is a visual, not a second tiny target. Checked state is also in the
// accessible name (never colour-only).
Item {
    id: root

    property string title: ""
    property string description: ""
    property bool checked: false

    signal toggled

    implicitHeight: Math.max(Theme.targetMin, content.implicitHeight + Theme.spaceSm * 2)
    height: implicitHeight
    opacity: root.enabled ? 1 : 0.38
    activeFocusOnTab: root.enabled
    Accessible.role: Accessible.CheckBox
    Accessible.name: root.description !== "" ? root.title + ". " + root.description : root.title
    Accessible.checkable: true
    Accessible.checked: root.checked
    Accessible.onPressAction: {
        if (root.enabled)
            root.toggled();
    }
    Keys.onSpacePressed: {
        if (root.enabled)
            root.toggled();
    }
    Keys.onReturnPressed: {
        if (root.enabled)
            root.toggled();
    }
    Keys.onEnterPressed: {
        if (root.enabled)
            root.toggled();
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.radiusSm
        color: (area.containsMouse || root.activeFocus) && root.enabled ? Theme.hover : "transparent"
        border.width: root.activeFocus ? 2 : 0
        border.color: Theme.focus

        Behavior on color {
            ColorAnimation {
                duration: Appearance.duration(Theme.motionHover)
                easing.type: Easing.OutCubic
            }
        }
    }

    Column {
        id: content

        anchors.left: parent.left
        anchors.right: track.left
        anchors.rightMargin: Theme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.spaceXs

        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            text: root.title
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBody
            font.weight: Font.Medium
            color: Theme.foreground
        }

        Text {
            visible: root.description !== ""
            width: parent.width
            wrapMode: Text.WordWrap
            text: root.description
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBodySm
            color: Theme.muted
        }
    }

    Rectangle {
        id: track

        width: Theme.targetMin
        height: Theme.spaceXl
        radius: Theme.radiusPill
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        color: root.checked ? Theme.primary : Theme.surfaceRaised
        border.width: 1
        border.color: root.checked ? Theme.primary : Theme.border
        Accessible.ignored: true

        Behavior on color {
            ColorAnimation {
                duration: Appearance.duration(Theme.motionHover)
                easing.type: Easing.OutCubic
            }
        }

        Rectangle {
            width: Theme.spaceXl - Theme.spaceXs
            height: width
            radius: width / 2
            anchors.verticalCenter: parent.verticalCenter
            x: root.checked ? parent.width - width - Theme.spaceXs : Theme.spaceXs
            color: Theme.primaryText

            Behavior on x {
                NumberAnimation {
                    duration: Appearance.duration(Theme.motionHover)
                    easing.type: Easing.OutCubic
                }
            }
        }
    }

    MouseArea {
        id: area

        anchors.fill: parent
        enabled: root.enabled
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggled()
    }
}
