import QtQuick
import TuneX

// BrowseRow: one folder/genre/composer row — leading glyph, title plus a
// muted subtitle, hover surface. The Library folder and facet lists shared
// one hand-rolled copy each (including the same never-true focus term in
// the hover tint — rows take no focus, so it is gone here).
Item {
    id: root

    property string iconName: "folder"
    property string title: ""
    property string subtitle: ""
    // Announced instead of title + subtitle (usually identical).
    property string accessibleName: ""

    signal activated

    width: ListView.view.width
    height: Theme.trackRowHeight
    Accessible.role: Accessible.ListItem
    Accessible.name: root.accessibleName !== "" ? root.accessibleName : root.title
    Accessible.onPressAction: root.activated()

    Rectangle {
        anchors.fill: parent
        radius: Theme.radiusSm
        color: rowMouse.containsMouse ? Theme.hover : "transparent"

        Behavior on color {
            ColorAnimation {
                duration: Appearance.duration(Theme.motionHover)
                easing.type: Easing.OutCubic
            }
        }
    }

    MouseArea {
        id: rowMouse

        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.activated()
    }

    Icon {
        id: glyph

        anchors.left: parent.left
        anchors.leftMargin: Theme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        name: root.iconName
        iconSize: Theme.navIconSize
        stroke: Theme.muted
    }

    Column {
        anchors.left: glyph.right
        anchors.leftMargin: Theme.spaceSm
        anchors.right: parent.right
        anchors.rightMargin: Theme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        spacing: 0

        Text {
            width: parent.width
            elide: Text.ElideRight
            text: root.title
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBody
            lineHeight: Theme.listLineHeight
            color: Theme.foreground
        }

        Text {
            width: parent.width
            elide: Text.ElideRight
            text: root.subtitle
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBodySm
            lineHeight: Theme.listLineHeight
            color: Theme.muted
        }
    }
}
