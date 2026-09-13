import QtQuick
import TuneX

// PlaylistCard (DESIGN_BRIEF inventory): artwork-first 12px card, two-line
// meta, bound to PlaylistModel roles. Placeholder glyph until mosaic art.
Item {
    id: root

    property int playlistId: -1
    property string title: ""
    property int trackCount: 0
    property int explicitWidth: 0
    // Wide tile-beside-text layout for the Home rail; the grid keeps the
    // tall artwork-over-text card.
    property bool horizontal: false
    property int horizontalHeight: Theme.miniPlayerHeight
    readonly property string countLine: root.trackCount === 1 ? qsTr("1 song") : qsTr("%1 songs").arg(root.trackCount)

    signal activated(int id, string name)

    width: root.explicitWidth > 0 ? root.explicitWidth : GridView.view.cellWidth
    height: {
        if (root.horizontal)
            return root.horizontalHeight;

        return root.explicitWidth > 0 ? root.explicitWidth + Theme.cardMetaHeight : GridView.view.cellHeight;
    }
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: root.title + ", " + root.countLine
    Accessible.onPressAction: root.activated(root.playlistId, root.title)
    Keys.onReturnPressed: root.activated(root.playlistId, root.title)
    Keys.onEnterPressed: root.activated(root.playlistId, root.title)
    Keys.onSpacePressed: root.activated(root.playlistId, root.title)

    // Same carded surface as the album grid: tonal step plus a hairline, no
    // shadow (DESIGN.md Elevation & Depth).
    Rectangle {
        anchors.fill: parent
        anchors.margins: Theme.spaceXs
        radius: Theme.radiusMd
        color: root.activeFocus ? Theme.selected : Theme.surface
        border.color: root.activeFocus ? Theme.focus : Theme.border
        border.width: root.activeFocus ? 2 : 1

        MouseArea {
            id: hoverArea

            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.activated(root.playlistId, root.title)
        }

        // Wide variant: tile on the leading edge, text beside it. The two
        // Home rails then read as different kinds of thing at a glance
        // instead of two identical grids, which is how the mockup separates
        // its rails.
        Row {
            visible: root.horizontal
            anchors.fill: parent
            anchors.margins: Theme.spaceXs
            spacing: Theme.spaceXs

            Rectangle {
                width: parent.height
                height: parent.height
                radius: Theme.radiusSm
                color: Theme.surfaceRaised

                Icon {
                    anchors.centerIn: parent
                    name: "list"
                    iconSize: 20
                    stroke: Theme.accent
                }

                Rectangle {
                    anchors.fill: parent
                    radius: Theme.radiusSm
                    color: Theme.foreground
                    opacity: hoverArea.containsMouse ? 0.04 : 0

                    Behavior on opacity {
                        NumberAnimation {
                            duration: Appearance.duration(Theme.motionHover)
                            easing.type: Easing.OutCubic
                        }
                    }
                }
            }

            Column {
                width: parent.width - parent.height - Theme.spaceSm
                anchors.verticalCenter: parent.verticalCenter
                spacing: 2

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
                    font.pixelSize: Theme.fontBodySm
                    color: Theme.muted
                }
            }
        }

        Column {
            visible: !root.horizontal
            anchors.fill: parent
            anchors.margins: Theme.spaceXs
            spacing: Theme.spaceXs

            Rectangle {
                width: parent.width
                height: parent.width
                radius: Theme.radiusSm
                color: Theme.surfaceRaised

                Icon {
                    anchors.centerIn: parent
                    name: "list"
                    iconSize: 24
                    stroke: Theme.accent
                }

                Rectangle {
                    anchors.fill: parent
                    radius: Theme.radiusSm
                    color: Theme.foreground
                    opacity: hoverArea.containsMouse ? 0.04 : 0

                    Behavior on opacity {
                        NumberAnimation {
                            duration: Appearance.duration(Theme.motionHover)
                            easing.type: Easing.OutCubic
                        }
                    }
                }

                PlayBadge {
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    anchors.margins: Theme.spaceSm
                    shown: hoverArea.containsMouse || root.activeFocus
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
                font.pixelSize: Theme.fontBodySm
                color: Theme.muted
            }
        }
    }
}
