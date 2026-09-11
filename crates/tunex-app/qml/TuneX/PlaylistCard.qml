import QtQuick
import TuneX 1.0

// PlaylistCard (DESIGN_BRIEF inventory): artwork-first 12px card, two-line
// meta, bound to PlaylistModel roles. Placeholder glyph until mosaic art.
Item {
    id: root

    property int playlistId: -1
    property string title: ""
    property int trackCount: 0
    property int explicitWidth: 0
    readonly property string countLine: root.trackCount === 1 ? qsTr("1 song") : qsTr("%1 songs").arg(root.trackCount)

    signal activated(int id, string name)

    width: root.explicitWidth > 0 ? root.explicitWidth : GridView.view.cellWidth
    height: root.explicitWidth > 0 ? root.explicitWidth + 64 : GridView.view.cellHeight
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: root.title + ", " + root.countLine
    Accessible.onPressAction: root.activated(root.playlistId, root.title)
    Keys.onReturnPressed: root.activated(root.playlistId, root.title)
    Keys.onEnterPressed: root.activated(root.playlistId, root.title)
    Keys.onSpacePressed: root.activated(root.playlistId, root.title)

    Column {
        anchors.fill: parent
        anchors.margins: Theme.spaceSm
        spacing: Theme.spaceXs

        Rectangle {
            width: parent.width
            height: parent.width
            radius: Theme.radiusMd
            color: root.activeFocus ? Theme.selected : Theme.surfaceRaised
            border.color: root.activeFocus ? Theme.focus : "transparent"
            border.width: root.activeFocus ? 2 : 0

            Icon {
                anchors.centerIn: parent
                name: "list"
                iconSize: 28
                stroke: Theme.accent
            }

            MouseArea {
                id: hoverArea

                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.activated(root.playlistId, root.title)
            }

            Rectangle {
                anchors.fill: parent
                radius: Theme.radiusMd
                color: Theme.hover
                opacity: hoverArea.containsMouse ? 0.45 : 0
            }
        }

        Text {
            width: parent.width
            elide: Text.ElideRight
            text: root.title
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBody
            font.weight: Font.DemiBold
            color: Theme.foreground
        }

        Text {
            width: parent.width
            elide: Text.ElideRight
            text: root.countLine
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontCaption
            color: Theme.muted
        }
    }
}
