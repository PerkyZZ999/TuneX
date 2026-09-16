import QtQuick
import TuneX

// PlaylistWriter: the one add-to-playlist sequence. Menus, selection bars,
// and sidebar drops all create-or-pick a list, add ids, and confirm through
// the NoticeBus with the same count-vs-error phrasing — which previously
// drifted per surface. Pure coordinator over one PlaylistModel; closing
// menus stays with the calling surface.
QtObject {
    id: root

    required property PlaylistModel playlists

    // Add one track to an existing list; announces the outcome.
    function addTrack(playlistId, name, trackId) {
        const added = root.playlists.addTrack(playlistId, trackId);
        confirm(added, name);
        return added;
    }

    // Add an ids set to an existing list; announces the outcome.
    function addTracks(playlistId, name, ids) {
        const added = root.playlists.addTracks(playlistId, ids);
        confirm(added, name);
        return added;
    }

    // Create an auto-named list holding one track; announces. Returns the
    // new id, or -1 when creation failed.
    function addTrackToNew(trackId) {
        const id = root.playlists.createPlaylistAuto();
        if (id < 0) {
            const err = root.playlists.errorText();
            NoticeBus.send(err !== "" ? err : qsTr("Could not create a playlist"), true);
            return -1;
        }
        addTrack(id, "", trackId);
        return id;
    }

    // Create an auto-named list holding an ids set; announces. Returns the
    // new id, or -1 when creation failed.
    function addTracksToNew(ids) {
        const id = root.playlists.createPlaylistAuto();
        if (id < 0) {
            const err = root.playlists.errorText();
            NoticeBus.send(err !== "" ? err : qsTr("Could not create a playlist"), true);
            return -1;
        }
        addTracks(id, "", ids);
        return id;
    }

    // Shared confirmation phrasing: names the count and the list, surfaces
    // errors loudly. `name` empty means the auto-named list.
    function confirm(added, name) {
        if (added > 0) {
            if (name === "")
                NoticeBus.send(qsTr("%n track(s) added to the new playlist", "", added), false);
            else
                NoticeBus.send(qsTr("%n track(s) added to “%1”", "", added).arg(name), false);
        } else {
            const err = root.playlists.errorText();
            NoticeBus.send(err !== "" ? err : qsTr("Could not add to “%1”").arg(name), true);
        }
    }
}
