import QtQuick
import TuneX

// TrackMenu (S3 W-022, playlist items in W-024, glass in S6 W-038): row
// actions for library and search song lists — play now, play next, queue in
// Up Next, add to a playlist (or a fresh auto-named one). One instance per
// view, retargeted per row (`trackId` then `popup()`); the queue and detail
// views own their own menus with move/remove actions instead.
GlassMenu {
    id: root

    required property QueueModel queue
    required property PlaylistModel playlists
    required property LibraryManager library
    property int trackId: -1

    signal indexChanged

    onAboutToShow: root.playlists.refresh()

    TagEditDialog {
        id: tagDialog

        library: root.library
        onTagsSaved: root.indexChanged()
    }

    GlassMenuItem {
        text: qsTr("Play now")
        Accessible.name: qsTr("Play now")
        onTriggered: root.queue.playTrackNow(root.trackId)
    }

    GlassMenuItem {
        text: qsTr("Play next")
        Accessible.name: qsTr("Play next")
        onTriggered: root.queue.playTrackNext(root.trackId)
    }

    GlassMenuItem {
        text: qsTr("Queue in Up Next")
        Accessible.name: qsTr("Queue in Up Next")
        onTriggered: root.queue.enqueueTrack(root.trackId)
    }

    GlassMenuSeparator {}

    GlassMenu {
        title: qsTr("Add to playlist")

        Repeater {
            model: root.playlists

            GlassMenuItem {
                visible: !root.playlists.isSmart(model.playlistId)
                text: model.name
                onTriggered: root.playlists.addTrack(model.playlistId, root.trackId)
            }
        }

        GlassMenuSeparator {}

        GlassMenuItem {
            text: qsTr("New playlist")
            onTriggered: {
                const id = root.playlists.createPlaylistAuto();
                if (id >= 0)
                    root.playlists.addTrack(id, root.trackId);
            }
        }
    }

    GlassMenuSeparator {}

    GlassMenuItem {
        text: qsTr("Edit tags…")
        Accessible.name: qsTr("Edit tags")
        onTriggered: tagDialog.openFor(root.trackId)
    }

    GlassMenuItem {
        text: qsTr("Show in folder")
        Accessible.name: qsTr("Show in folder")
        onTriggered: root.library.revealTrack(root.trackId)
    }

    GlassMenuItem {
        text: qsTr("Remove from library")
        Accessible.name: qsTr("Remove from library")
        onTriggered: {
            root.library.removeTrack(root.trackId);
            root.indexChanged();
        }
    }
}
