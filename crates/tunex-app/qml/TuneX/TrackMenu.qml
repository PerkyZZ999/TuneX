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
    property string trackIds: ""
    readonly property bool hasSet: root.trackIds !== ""

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
        onTriggered: {
            if (root.hasSet)
                root.queue.playTrackIds(root.trackIds);
            else
                root.queue.playTrackNow(root.trackId);
        }
    }

    GlassMenuItem {
        text: qsTr("Play next")
        Accessible.name: qsTr("Play next")
        onTriggered: {
            if (root.hasSet)
                root.queue.playTrackIdsNext(root.trackIds);
            else
                root.queue.playTrackNext(root.trackId);
        }
    }

    GlassMenuItem {
        text: qsTr("Add to Now Playing")
        Accessible.name: qsTr("Add to Now Playing")
        onTriggered: {
            if (root.hasSet)
                root.queue.enqueueTrackIds(root.trackIds);
            else
                root.queue.enqueueTrack(root.trackId);
        }
    }

    GlassMenuSeparator {}

    GlassMenu {
        title: qsTr("Add to playlist")

        Repeater {
            model: root.playlists

            GlassMenuItem {
                visible: !root.playlists.isSmart(model.playlistId)
                text: model.name
                onTriggered: {
                    if (root.hasSet)
                        root.playlists.addTracks(model.playlistId, root.trackIds);
                    else
                        root.playlists.addTrack(model.playlistId, root.trackId);
                }
            }
        }

        GlassMenuSeparator {}

        GlassMenuItem {
            text: qsTr("New playlist")
            onTriggered: {
                const id = root.playlists.createPlaylistAuto();
                if (id >= 0) {
                    if (root.hasSet)
                        root.playlists.addTracks(id, root.trackIds);
                    else
                        root.playlists.addTrack(id, root.trackId);
                }
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
