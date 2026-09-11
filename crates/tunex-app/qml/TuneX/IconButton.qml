import QtQuick
import QtQuick.Controls.Basic

// Circular icon-only control. 44px hit target; optical glyph is smaller.
Item {
    id: root

    property string iconName: "play"
    property string accessibleName: ""
    property bool checked: false
    property bool circle: true
    property int glyphSize: 20
    property color glyphColor: {
        if (!root.enabled)
            return Theme.muted;

        if (root.checked)
            return Theme.accent;

        return Theme.foreground;
    }

    signal activated()

    width: Theme.targetMin
    height: Theme.targetMin
    implicitWidth: Theme.targetMin
    implicitHeight: Theme.targetMin
    opacity: root.enabled ? 1 : 0.38
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: root.accessibleName
    Accessible.checkable: true
    Accessible.checked: root.checked
    Accessible.onPressAction: root.activated()
    Keys.onSpacePressed: root.activated()
    Keys.onReturnPressed: root.activated()
    Keys.onEnterPressed: root.activated()
    ToolTip.visible: area.containsMouse && root.accessibleName !== ""
    ToolTip.text: root.accessibleName
    ToolTip.delay: 400

    Rectangle {
        anchors.centerIn: parent
        width: root.circle ? 36 : parent.width
        height: root.circle ? 36 : parent.height
        radius: root.circle ? width / 2 : Theme.radiusSm
        color: {
            if (area.containsMouse || root.activeFocus)
                return Theme.hover;

            if (root.checked)
                return Theme.selected;

            return "transparent";
        }
        border.width: root.activeFocus ? 2 : 0
        border.color: Theme.focus
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
