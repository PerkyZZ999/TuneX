import QtQuick
import QtQuick.Controls.Basic
import TuneX 1.0

// PlaylistsView (S3 W-024): sidebar of user playlists plus the detail pane
// for the selection — header actions (play/queue/rename/delete), entries
// with missing/dangling badges, and empty states for no playlists, no
// selection, and empty lists. Refreshes on every show so row-menu adds from
// other views land immediately.
Item {
    id: root

    required property QueueModel queue
    required property PlaylistModel playlists
    property int playlistId: -1
    property string playlistName: ""
    property string errorLine: ""
    property bool renaming: false

    function selectPlaylist(id, name) {
        root.playlistId = id;
        root.playlistName = name;
        root.errorLine = "";
        entries.refreshPlaylist(id);
        root.errorLine = entries.errorText();
    }

    function refreshAll() {
        playlists.refresh();
        if (root.playlistId >= 0) {
            entries.refreshPlaylist(root.playlistId);
            root.errorLine = entries.errorText();
        } else {
            root.errorLine = playlists.errorText();
        }
    }

    function playAll() {
        root.queue.clearQueue();
        const added = root.queue.enqueuePlaylist(root.playlistId);
        if (added > 0)
            root.queue.playAt(0);
    }

    function openCreate() {
        root.renaming = false;
        nameDialog.openFor("");
    }

    anchors.fill: parent
    onVisibleChanged: {
        if (root.visible)
            root.refreshAll();
    }
    Component.onCompleted: playlists.refresh()

    PlaylistTrackModel {
        id: entries
    }

    PlaylistNameDialog {
        id: nameDialog

        parent: Overlay.overlay
        anchors.centerIn: Overlay.overlay
        modal: true
        queue: root.queue
        onNameAccepted: name => {
            if (root.playlistId >= 0 && root.renaming) {
                playlists.renamePlaylist(root.playlistId, name);
                root.errorLine = playlists.errorText();
                if (root.errorLine === "")
                    root.playlistName = name;
            } else {
                const id = playlists.createPlaylist(name);
                root.errorLine = playlists.errorText();
                if (id >= 0)
                    root.selectPlaylist(id, name);
            }
        }
    }

    Dialog {
        id: deleteDialog

        parent: Overlay.overlay
        anchors.centerIn: Overlay.overlay
        modal: true
        title: qsTr("Delete playlist?")
        standardButtons: Dialog.Ok | Dialog.Cancel
        onAccepted: {
            playlists.deletePlaylist(root.playlistId);
            root.errorLine = playlists.errorText();
            if (root.errorLine === "") {
                root.playlistId = -1;
                root.playlistName = "";
                entries.clear();
            }
        }

        Text {
            width: 320
            wrapMode: Text.WordWrap
            text: qsTr("Delete “%1” and its entries? This cannot be undone.").arg(root.playlistName)
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBody
            color: Theme.foreground
        }

        background: GlassBackdrop {
            cornerRadius: Theme.radiusLg
            transparencyOff: root.queue.reduceTransparency()
        }
    }

    Connections {
        function onModelReset() {
            entryMenu.dismiss();
        }

        target: playlists
    }

    Connections {
        function onModelReset() {
            entryMenu.dismiss();
        }

        target: entries
    }

    Menu {
        id: entryMenu

        property int rowIndex: -1
        property int rowTrackId: -1
        property bool rowPlayable: false

        MenuItem {
            text: qsTr("Play now")
            enabled: entryMenu.rowPlayable
            onTriggered: root.queue.playTrackNow(entryMenu.rowTrackId)
        }

        MenuItem {
            text: qsTr("Play next")
            enabled: entryMenu.rowPlayable
            onTriggered: root.queue.playTrackNext(entryMenu.rowTrackId)
        }

        MenuItem {
            text: qsTr("Add to Up Next")
            enabled: entryMenu.rowPlayable
            onTriggered: root.queue.enqueueTrack(entryMenu.rowTrackId)
        }

        MenuSeparator {}

        MenuItem {
            text: qsTr("Move up")
            onTriggered: {
                entries.moveItem(entryMenu.rowIndex, entryMenu.rowIndex - 1);
                root.errorLine = entries.errorText();
            }
        }

        MenuItem {
            text: qsTr("Move down")
            onTriggered: {
                entries.moveItem(entryMenu.rowIndex, entryMenu.rowIndex + 1);
                root.errorLine = entries.errorText();
            }
        }

        MenuItem {
            text: qsTr("Remove from playlist")
            onTriggered: {
                entries.removeAt(entryMenu.rowIndex);
                playlists.refresh();
                root.errorLine = entries.errorText();
            }
        }

        background: GlassBackdrop {
            cornerRadius: Theme.radiusLg
            transparencyOff: root.queue.reduceTransparency()
            disableBlur: true
        }
    }

    Row {
        anchors.fill: parent
        anchors.margins: Theme.spaceLg
        spacing: Theme.spaceLg

        // Sidebar: playlists with counts plus the create entry.
        Column {
            width: Theme.panelWidth
            height: parent.height
            spacing: Theme.spaceMd

            PrimaryButton {
                id: newButton

                width: parent.width
                text: qsTr("New playlist")
                Accessible.name: qsTr("New playlist")
                onClicked: root.openCreate()
            }

            ListView {
                id: playlistsView

                width: parent.width
                height: parent.height - newButton.height - Theme.spaceMd
                model: playlists
                activeFocusOnTab: true
                clip: true
                highlightMoveDuration: 120
                Accessible.role: Accessible.List
                Accessible.name: qsTr("Playlists")
                Keys.onReturnPressed: {
                    const at = playlistsView.currentIndex >= 0 ? playlistsView.currentIndex : 0;
                    if (at < playlistsView.count)
                        root.selectPlaylist(playlists.playlistIdAt(at), playlists.playlistNameAt(at));
                }
                Keys.onEnterPressed: {
                    const at = playlistsView.currentIndex >= 0 ? playlistsView.currentIndex : 0;
                    if (at < playlistsView.count)
                        root.selectPlaylist(playlists.playlistIdAt(at), playlists.playlistNameAt(at));
                }

                highlight: Rectangle {
                    color: Theme.selected
                    radius: Theme.radiusSm
                }

                delegate: Item {
                    property string name: model.name
                    property int trackCount: model.trackCount
                    property int playlistId: model.playlistId

                    width: playlistsView.width
                    height: 56
                    Accessible.role: Accessible.ListItem
                    Accessible.name: name

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            playlistsView.currentIndex = index;
                            root.selectPlaylist(playlistId, name);
                        }
                    }

                    Column {
                        anchors.left: parent.left
                        anchors.leftMargin: Theme.spaceMd
                        anchors.right: parent.right
                        anchors.rightMargin: Theme.spaceMd
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 2

                        Text {
                            width: parent.width
                            elide: Text.ElideRight
                            clip: true
                            text: name
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontBody
                            font.weight: root.playlistId === playlistId ? Font.DemiBold : Font.Normal
                            color: Theme.foreground
                        }

                        Text {
                            width: parent.width
                            elide: Text.ElideRight
                            text: trackCount === 1 ? qsTr("1 song") : qsTr("%1 songs").arg(trackCount)
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontCaption
                            color: Theme.muted
                        }
                    }
                }
            }
        }

        // Detail pane: header actions plus the entry list.
        Column {
            width: parent.width - Theme.panelWidth - Theme.spaceLg
            height: parent.height
            spacing: Theme.spaceMd

            Column {
                id: detailHeader

                visible: root.playlistId >= 0
                width: parent.width
                height: visible ? implicitHeight : 0
                spacing: Theme.spaceSm

                Text {
                    width: parent.width
                    elide: Text.ElideRight
                    clip: true
                    text: root.playlistName
                    textFormat: Text.PlainText
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontTitle
                    font.weight: Font.DemiBold
                    color: Theme.foreground
                }

                Row {
                    spacing: Theme.spaceSm

                    PrimaryButton {
                        id: playAllButton

                        text: qsTr("Play all")
                        enabled: entriesView.count > 0
                        onClicked: root.playAll()
                    }

                    PrimaryButton {
                        id: queueAllButton

                        primary: false
                        text: qsTr("Queue all")
                        enabled: entriesView.count > 0
                        onClicked: root.queue.enqueuePlaylist(root.playlistId)
                    }

                    PrimaryButton {
                        id: renameButton

                        primary: false
                        text: qsTr("Rename")
                        onClicked: {
                            root.renaming = true;
                            nameDialog.openFor(root.playlistName);
                        }
                    }

                    PrimaryButton {
                        id: deleteButton

                        primary: false
                        text: qsTr("Delete")
                        onClicked: deleteDialog.open()
                    }
                }
            }

            Text {
                id: errorText

                visible: root.errorLine !== ""
                width: parent.width
                height: visible ? implicitHeight : 0
                clip: true
                wrapMode: Text.WordWrap
                text: root.errorLine
                textFormat: Text.PlainText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontCaption
                color: Theme.error
            }

            Item {
                width: parent.width
                height: parent.height - detailHeader.height - errorText.height - Theme.spaceMd * 2

                EmptyState {
                    visible: playlistsView.count === 0 && root.playlistId < 0
                    title: qsTr("No playlists yet")
                    note: qsTr("Create a playlist, then add songs from any row menu.")
                    actionLabel: qsTr("New playlist")
                    onActionRequested: root.openCreate()
                }

                EmptyState {
                    visible: playlistsView.count > 0 && root.playlistId < 0
                    title: qsTr("No playlist selected")
                    note: qsTr("Choose a playlist on the left, or create one to get started.")
                    actionLabel: qsTr("New playlist")
                    onActionRequested: root.openCreate()
                }

                EmptyState {
                    visible: root.playlistId >= 0 && entriesView.count === 0
                    title: qsTr("This playlist is empty")
                    note: qsTr("Add songs from any row menu, then play the whole list here.")
                }

                ListView {
                    id: entriesView

                    visible: root.playlistId >= 0 && count > 0
                    anchors.fill: parent
                    model: entries
                    activeFocusOnTab: true
                    clip: true
                    highlightMoveDuration: 120
                    Accessible.role: Accessible.List
                    Accessible.name: root.playlistName
                    Keys.onReturnPressed: {
                        const at = entriesView.currentIndex >= 0 ? entriesView.currentIndex : 0;
                        if (at < entriesView.count && entries.isPlayableAt(at))
                            root.queue.playTrackNow(entries.trackIdAt(at));
                    }
                    Keys.onEnterPressed: {
                        const at = entriesView.currentIndex >= 0 ? entriesView.currentIndex : 0;
                        if (at < entriesView.count && entries.isPlayableAt(at))
                            root.queue.playTrackNow(entries.trackIdAt(at));
                    }
                    Keys.onDeletePressed: {
                        if (entriesView.currentIndex >= 0 && entriesView.currentIndex < entriesView.count) {
                            entries.removeAt(entriesView.currentIndex);
                            playlists.refresh();
                            root.errorLine = entries.errorText();
                        }
                    }
                    Keys.onUpPressed: event => {
                        if (event.modifiers & Qt.AltModifier) {
                            const from = entriesView.currentIndex;
                            if (from > 0) {
                                entries.moveItem(from, from - 1);
                                entriesView.currentIndex = from - 1;
                            }
                            event.accepted = true;
                        }
                    }
                    Keys.onDownPressed: event => {
                        if (event.modifiers & Qt.AltModifier) {
                            const from = entriesView.currentIndex;
                            if (from >= 0 && from + 1 < entriesView.count) {
                                entries.moveItem(from, from + 1);
                                entriesView.currentIndex = from + 1;
                            }
                            event.accepted = true;
                        }
                    }

                    highlight: Rectangle {
                        color: Theme.selected
                        radius: Theme.radiusSm
                    }

                    delegate: TrackRow {
                        trackId: model.trackId
                        rowIndex: index
                        title: model.title
                        artist: model.artist
                        durationMs: model.durationMs
                        missing: model.missing
                        dangling: model.dangling
                        onPlayRequested: (trackId, rowIndex, dangling) => {
                            entriesView.currentIndex = rowIndex;
                            entriesView.forceActiveFocus();
                            if (!dangling && entries.isPlayableAt(rowIndex))
                                root.queue.playTrackNow(trackId);
                        }
                        onMenuRequested: (trackId, rowIndex, dangling) => {
                            entriesView.currentIndex = rowIndex;
                            entryMenu.rowIndex = rowIndex;
                            entryMenu.rowTrackId = trackId;
                            entryMenu.rowPlayable = !dangling && entries.isPlayableAt(rowIndex);
                            entryMenu.popup();
                        }
                    }
                }
            }
        }
    }
}
