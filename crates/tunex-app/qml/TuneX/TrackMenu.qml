import QtQuick
import QtQuick.Controls.Basic
import TuneX 1.0

// TrackMenu (S3 W-022, playlist items in W-024): row actions for library
// and search song lists — play now, play next, add to Up Next, add to a
// playlist (or a fresh auto-named one). One instance per view, retargeted
// per row (`trackId` then `popup()`); the queue and detail views own their
// own menus with move/remove actions instead.
Menu {
    id: root

    required property QueueModel queue
    required property PlaylistModel playlists
    property int trackId: -1

    onAboutToShow: root.playlists.refresh()

    MenuItem {
        text: qsTr("Play now")
        onTriggered: root.queue.playTrackNow(root.trackId)
    }

    MenuItem {
        text: qsTr("Play next")
        onTriggered: root.queue.playTrackNext(root.trackId)
    }

    MenuItem {
        text: qsTr("Add to Up Next")
        onTriggered: root.queue.enqueueTrack(root.trackId)
    }

    MenuSeparator {}

    Menu {
        title: qsTr("Add to playlist")

        Repeater {
            model: root.playlists

            MenuItem {
                text: model.name
                onTriggered: root.playlists.addTrack(model.playlistId, root.trackId)
            }
        }

        MenuSeparator {}

        MenuItem {
            text: qsTr("New playlist")
            onTriggered: {
                const id = root.playlists.createPlaylistAuto();
                if (id >= 0)
                    root.playlists.addTrack(id, root.trackId);
            }
        }

        background: GlassBackdrop {
            cornerRadius: Theme.radiusLg
            transparencyOff: root.queue.reduceTransparency()
            disableBlur: true
        }
    }

    background: GlassBackdrop {
        cornerRadius: Theme.radiusLg
        transparencyOff: root.queue.reduceTransparency()
        disableBlur: true
    }
}
