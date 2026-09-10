import QtQuick
import QtQuick.Controls.Basic
import TuneX 1.0

// TrackMenu (S3 W-022): row actions for library and search song lists —
// play now, play next, add to Up Next. One instance per view, retargeted
// per row (`trackId` then `popup()`); the queue panel owns its own menu
// with move/remove actions instead.
Menu {
    id: root

    required property QueueModel queue
    property int trackId: -1

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

}
