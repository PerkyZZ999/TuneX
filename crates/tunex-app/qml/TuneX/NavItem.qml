import QtQuick

// One selectable row in the navigation rail: icon slot (text glyph until the
// Lucide-style icon set lands) plus label, accent bar when selected.
// Fully keyboard operable with an accessible name.
Item {
    id: root

    required property string label
    required property bool selected

    signal activated()

    width: parent.width
    height: 40
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: label
    Accessible.selected: selected
    Keys.onSpacePressed: root.activated()
    Keys.onEnterPressed: root.activated()
    Keys.onReturnPressed: root.activated()

    Rectangle {
        anchors.fill: parent
        radius: Theme.radiusSm
        color: root.selected ? Theme.selected : (navMouse.containsMouse || root.activeFocus ? Theme.hover : "transparent")
    }

    Rectangle {
        width: 3
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.leftMargin: 4
        radius: 2
        color: Theme.accent
        visible: root.selected
    }

    Text {
        anchors.verticalCenter: parent.verticalCenter
        anchors.left: parent.left
        anchors.leftMargin: Theme.spaceMd
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
        onPressed: root.forceActiveFocus()
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
