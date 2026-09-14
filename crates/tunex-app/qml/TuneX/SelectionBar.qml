import QtQuick
import TuneX

// Transient multi-select chrome (S14 W-073). Hidden at count 0 so Play all
// stays the page primary. One primary here: Play selected.
Rectangle {
    id: root

    required property QueueModel queue
    required property PlaylistModel playlists
    property int count: 0
    property string trackIds: ""
    property bool showRemove: false
    property bool fromLibrary: true

    signal cleared
    signal removeRequested
    signal playInPlaceRequested

    visible: root.count > 0
    implicitHeight: flow.implicitHeight + Theme.spaceXs * 2
    height: visible ? Math.max(Theme.targetMin, implicitHeight) : 0
    radius: Theme.radiusSm
    color: Theme.surface
    border.width: 1
    border.color: Theme.border
    Accessible.role: Accessible.ToolBar
    Accessible.name: qsTr("%1 selected").arg(root.count)

    Flow {
        id: flow

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        anchors.leftMargin: Theme.spaceSm
        anchors.rightMargin: Theme.spaceSm
        spacing: Theme.spaceXs

        Text {
            height: Theme.buttonHeight
            verticalAlignment: Text.AlignVCenter
            text: qsTr("%1 selected").arg(root.count)
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBodySm
            color: Theme.foreground
        }

        PrimaryButton {
            text: qsTr("Play selected")
            onClicked: {
                if (root.fromLibrary)
                    root.queue.playTrackIds(root.trackIds);
                else
                    root.playInPlaceRequested();
                root.cleared();
            }
        }

        PrimaryButton {
            visible: root.fromLibrary
            width: visible ? implicitWidth : 0
            primary: false
            text: qsTr("Add to Now Playing")
            onClicked: {
                root.queue.enqueueTrackIds(root.trackIds);
                root.cleared();
            }
        }

        PrimaryButton {
            visible: root.fromLibrary
            width: visible ? implicitWidth : 0
            primary: false
            text: qsTr("New playlist")
            onClicked: {
                const id = root.playlists.createPlaylistAuto();
                if (id >= 0)
                    root.playlists.addTracks(id, root.trackIds);
                root.cleared();
            }
        }

        PrimaryButton {
            visible: root.showRemove
            width: visible ? implicitWidth : 0
            primary: false
            text: qsTr("Remove")
            onClicked: root.removeRequested()
        }

        IconButton {
            iconName: "x"
            accessibleName: qsTr("Clear selection")
            onActivated: root.cleared()
        }
    }
}
