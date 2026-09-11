import QtQuick
import QtQuick.Controls.Basic
import TuneX

// IconButton: icon-only control for universal transport/media symbols and
// close (DESIGN.md: icon-only needs an accessible name plus a tooltip).
// 44px hit target by default; dense track rows pass the 40px exception.
// Hover/focus step the `hover` surface, press steps `selected`, a checked
// toggle tints the glyph accent and keeps the `selected` surface — colour
// only, nothing scales. Only real toggles set `checkable`, so plain buttons
// never announce a checked state.
Item {
    id: root

    property string iconName: "play"
    property string accessibleName: ""
    property bool checked: false
    property bool checkable: false
    property bool circle: true
    property int size: Theme.targetMin
    property int glyphSize: 20
    property color glyphColor: {
        if (!root.enabled)
            return Theme.muted;

        if (root.checked)
            return Theme.accent;

        return Theme.foreground;
    }

    signal activated

    width: root.size
    height: root.size
    implicitWidth: root.size
    implicitHeight: root.size
    opacity: root.enabled ? 1 : 0.38
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: root.accessibleName
    Accessible.checkable: root.checkable
    Accessible.checked: root.checkable && root.checked
    Accessible.onPressAction: root.activated()
    Keys.onSpacePressed: root.activated()
    Keys.onReturnPressed: root.activated()
    Keys.onEnterPressed: root.activated()
    ToolTip.visible: area.containsMouse && root.accessibleName !== ""
    ToolTip.text: root.accessibleName
    ToolTip.delay: 400

    Rectangle {
        anchors.centerIn: parent
        width: root.circle ? root.size - Theme.spaceSm : parent.width
        height: root.circle ? root.size - Theme.spaceSm : parent.height
        radius: root.circle ? width / 2 : Theme.radiusSm
        color: {
            if (area.pressed || (root.checked && !area.containsMouse))
                return Theme.selected;

            if (area.containsMouse || root.activeFocus)
                return Theme.hover;

            return "transparent";
        }
        border.width: root.activeFocus ? 2 : 0
        border.color: Theme.focus

        Behavior on color {
            ColorAnimation {
                duration: Appearance.duration(Theme.motionHover)
                easing.type: Easing.OutCubic
            }
        }
    }

    Icon {
        anchors.centerIn: parent
        name: root.iconName
        iconSize: root.glyphSize
        stroke: root.glyphColor
    }

    MouseArea {
        id: area

        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.activated()
    }
}
