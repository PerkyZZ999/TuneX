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
    // One-line confirmation for playlist adds (handled by the shell toast):
    // every add names the count and the list, errors surface loudly.
    signal notice(string text, bool isError)

    // `name` empty means the auto-named list from "New playlist".
    function confirmPlaylistAdd(added, name) {
        if (added > 0) {
            if (name === "")
                root.notice(qsTr("%n track(s) added to the new playlist", "", added), false);
            else
                root.notice(qsTr("%n track(s) added to “%1”", "", added).arg(name), false);
        } else {
            const err = root.playlists.errorText();
            root.notice(err !== "" ? err : qsTr("Could not add to “%1”").arg(name), true);
        }
        root.close();
    }

    onAboutToShow: root.playlists.refresh()

    // Open the menu for the cursor row of a track list: bounds-checks the
    // view, seeds its cursor, and carries the selection's ids when the row
    // is part of it. Every track list opens its row menu through here.
    function openFor(at, view, selection, model) {
        if (at < 0 || at >= view.count)
            return;

        view.currentIndex = at;
        root.trackId = model.trackIdAt(at);
        root.trackIds = selection.contains(at) ? selection.mimeIds(model) : "";
        root.popup();
    }

    TagEditDialog {
        id: tagDialog

        library: root.library
        onTagsSaved: root.indexChanged()
    }

    // Removing deletes the index row while playlists dangle, so it
    // confirms like every other destructive action — naming the track.
    GlassDialog {
        id: removeDialog

        property int pendingTrackId: -1
        readonly property string pendingTitle: root.library.trackValue(pendingTrackId, "title")

        title: qsTr("Remove from library?")
        acceptLabel: qsTr("Remove")
        onAccepted: {
            root.library.removeTrack(removeDialog.pendingTrackId);
            root.indexChanged();
        }

        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            text: removeDialog.pendingTitle !== "" ? qsTr("Remove “%1” from the library? Playlists keep a dangling entry.").arg(removeDialog.pendingTitle) : qsTr("Remove this track from the library? Playlists keep a dangling entry.")
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBody
            color: Theme.foreground
        }
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
                    let added = 0;
                    if (root.hasSet)
                        added = root.playlists.addTracks(model.playlistId, root.trackIds);
                    else
                        added = root.playlists.addTrack(model.playlistId, root.trackId);
                    root.confirmPlaylistAdd(added, model.name);
                }
            }
        }

        GlassMenuSeparator {}

        GlassMenuItem {
            text: qsTr("New playlist")
            onTriggered: {
                const id = root.playlists.createPlaylistAuto();
                if (id >= 0) {
                    let added = 0;
                    if (root.hasSet)
                        added = root.playlists.addTracks(id, root.trackIds);
                    else
                        added = root.playlists.addTrack(id, root.trackId);
                    root.confirmPlaylistAdd(added, "");
                } else {
                    const err = root.playlists.errorText();
                    root.notice(err !== "" ? err : qsTr("Could not create a playlist"), true);
                    root.close();
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
            removeDialog.pendingTrackId = root.trackId;
            removeDialog.open();
        }
    }
}
