import QtQuick
import TuneX

// PlaylistsView (S3 W-024, glass in S6 W-038): sidebar of user playlists
// plus the detail pane for the selection — Play all / Queue all with rename
// and delete one ⋯ menu deep (progressive disclosure), entries with
// missing/dangling badges, and empty states for no playlists, no
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
    property bool playlistIsSmart: false

    signal notice(string text, bool isError)

    function selectPlaylist(id, name) {
        root.playlistId = id;
        root.playlistName = name;
        root.playlistIsSmart = playlists.isSmart(id);
        root.errorLine = "";
        entrySelection.clear();
        entries.refreshPlaylist(id);
        root.errorLine = entries.errorText();
        root.syncSidebarCursor();
    }

    function clearSelection() {
        return entrySelection.clear();
    }

    function entryMimeIds() {
        const rows = entrySelection.sorted();
        const ids = [];
        for (let i = 0; i < rows.length; i++) {
            const id = entries.trackIdAt(rows[i]);
            if (id >= 0)
                ids.push(id);
        }
        return ids.join(",");
    }

    // Source rows for an internal drag (comma positions). The detail target
    // moves entry rows, never track ids, so repeats stay independent.
    function entryRowsCsv() {
        entrySelection.stamp;
        return entrySelection.sorted().join(",");
    }

    TrackListSelection {
        id: entrySelection
    }

    // The sidebar highlight follows the open playlist, however it opened
    // (rail entry, create dialog, or a click here).
    function syncSidebarCursor() {
        for (let row = 0; row < playlistsView.count; row++) {
            if (playlists.playlistIdAt(row) === root.playlistId) {
                playlistsView.currentIndex = row;
                return;
            }
        }
        playlistsView.currentIndex = -1;
    }

    function refreshAll() {
        playlists.reloadIndex();
        playlists.refresh();
        entries.reloadIndex();
        root.syncSidebarCursor();
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

    function openSmart() {
        smartDialog.openBlank();
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

    SmartPlaylistDialog {
        id: smartDialog

        onRuleAccepted: (name, kind, value, excludeMissing) => {
            const id = playlists.createSmartPlaylist(name, kind, value, excludeMissing);
            root.errorLine = playlists.errorText();
            if (id >= 0)
                root.selectPlaylist(id, name);
        }
    }

    GlassDialog {
        id: deleteDialog

        title: qsTr("Delete playlist?")
        acceptLabel: qsTr("Delete")
        onAccepted: {
            playlists.deletePlaylist(root.playlistId);
            root.errorLine = playlists.errorText();
            if (root.errorLine === "") {
                root.playlistId = -1;
                root.playlistName = "";
                root.playlistIsSmart = false;
                entries.clear();
            }
        }

        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            text: qsTr("Delete “%1” and its entries? This cannot be undone.").arg(root.playlistName)
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBody
            color: Theme.foreground
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

    GlassMenu {
        id: entryMenu

        property int rowIndex: -1
        property int rowTrackId: -1
        property bool rowPlayable: false

        GlassMenuItem {
            text: qsTr("Play now")
            enabled: entryMenu.rowPlayable
            onTriggered: root.queue.playTrackNow(entryMenu.rowTrackId)
        }

        GlassMenuItem {
            text: qsTr("Play next")
            enabled: entryMenu.rowPlayable
            onTriggered: root.queue.playTrackNext(entryMenu.rowTrackId)
        }

        GlassMenuItem {
            text: qsTr("Add to Now Playing")
            enabled: entryMenu.rowPlayable
            onTriggered: root.queue.enqueueTrack(entryMenu.rowTrackId)
        }

        GlassMenuSeparator {}

        GlassMenuItem {
            visible: !root.playlistIsSmart
            text: qsTr("Move up")
            onTriggered: {
                entries.moveItem(entryMenu.rowIndex, entryMenu.rowIndex - 1);
                root.errorLine = entries.errorText();
            }
        }

        GlassMenuItem {
            visible: !root.playlistIsSmart
            text: qsTr("Move down")
            onTriggered: {
                entries.moveItem(entryMenu.rowIndex, entryMenu.rowIndex + 1);
                root.errorLine = entries.errorText();
            }
        }

        GlassMenuItem {
            visible: !root.playlistIsSmart
            text: qsTr("Remove from playlist")
            onTriggered: {
                entries.removeAt(entryMenu.rowIndex);
                playlists.refresh();
                root.errorLine = entries.errorText();
            }
        }
    }

    GlassMenu {
        id: playlistMenu

        GlassMenuItem {
            text: qsTr("Rename…")
            onTriggered: {
                root.renaming = true;
                nameDialog.openFor(root.playlistName);
            }
        }

        GlassMenuItem {
            text: qsTr("Delete…")
            onTriggered: deleteDialog.open()
        }
    }

    Column {
        anchors.fill: parent
        anchors.margins: Theme.spaceLg
        spacing: Theme.spaceLg

        PageBanner {
            id: playlistsBanner

            width: parent.width
            title: qsTr("Playlists")
            source: "qrc:/qt/qml/TuneX/banner-playlists.png"
        }

        Row {
            width: parent.width
            height: parent.height - playlistsBanner.height - Theme.spaceLg
            spacing: Theme.spaceLg

            // Sidebar: playlists with counts plus the create entry.
            Column {
                width: Theme.panelWidth
                height: parent.height
                spacing: Theme.spaceMd

                // Secondary: Play all owns the view's one primary action.
                PrimaryButton {
                    id: newButton

                    width: parent.width
                    primary: false
                    glyph: "plus"
                    text: qsTr("New playlist")
                    Accessible.name: qsTr("New playlist")
                    onClicked: root.openCreate()
                }

                PrimaryButton {
                    id: smartButton

                    width: parent.width
                    primary: false
                    text: qsTr("Smart playlist")
                    Accessible.name: qsTr("Smart playlist")
                    onClicked: root.openSmart()
                }

                ListView {
                    id: playlistsView

                    width: parent.width
                    height: parent.height - newButton.height - smartButton.height - Theme.spaceMd * 2
                    model: playlists
                    activeFocusOnTab: true
                    clip: true
                    spacing: Theme.listRowGap
                    highlightMoveDuration: Appearance.duration(Theme.motionHover)
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
                        id: sidebarRow

                        // Model roles as required delegate properties (the rail's
                        // playlist entries take the same shape): the row reads
                        // them, never stores state of its own. `index` joins them
                        // because a delegate with required properties no longer
                        // receives the context properties.
                        required property int index
                        required property string name
                        required property int trackCount
                        required property int playlistId

                        width: playlistsView.width
                        height: Theme.trackRowHeight
                        Accessible.role: Accessible.ListItem
                        Accessible.name: sidebarRow.name

                        // Hover surface, 120ms colour-only like the track rows;
                        // the selected row keeps the view's own highlight.
                        Rectangle {
                            anchors.fill: parent
                            radius: Theme.radiusSm
                            color: sidebarArea.containsMouse ? Theme.hover : "transparent"

                            Behavior on color {
                                ColorAnimation {
                                    duration: Appearance.duration(Theme.motionHover)
                                    easing.type: Easing.OutCubic
                                }
                            }
                        }

                        MouseArea {
                            id: sidebarArea

                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                playlistsView.currentIndex = sidebarRow.index;
                                root.selectPlaylist(sidebarRow.playlistId, sidebarRow.name);
                            }
                        }

                        Column {
                            anchors.left: parent.left
                            anchors.leftMargin: Theme.spaceMd
                            anchors.right: parent.right
                            anchors.rightMargin: Theme.spaceMd
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: 0

                            Text {
                                width: parent.width
                                elide: Text.ElideRight
                                clip: true
                                text: sidebarRow.name
                                textFormat: Text.PlainText
                                font.family: Theme.fontFamily
                                font.pixelSize: Theme.fontBody
                                font.weight: root.playlistId === sidebarRow.playlistId ? Font.DemiBold : Font.Normal
                                lineHeight: Theme.listLineHeight
                                color: Theme.foreground
                            }

                            Text {
                                width: parent.width
                                elide: Text.ElideRight
                                text: sidebarRow.trackCount === 1 ? qsTr("1 song") : qsTr("%1 songs").arg(sidebarRow.trackCount)
                                textFormat: Text.PlainText
                                font.family: Theme.fontFamily
                                font.pixelSize: Theme.fontCaption
                                lineHeight: Theme.listLineHeight
                                color: Theme.muted
                            }
                        }

                        TrackDropArea {
                            anchors.fill: parent
                            enabled: !root.playlists.isSmart(sidebarRow.playlistId)
                            showHint: false
                            onTracksDropped: ids => {
                                if (root.playlists.isSmart(sidebarRow.playlistId))
                                    return;
                                const added = root.playlists.addTracks(sidebarRow.playlistId, ids);
                                if (added > 0)
                                    root.notice(qsTr("%n track(s) added to “%1”", "", added).arg(sidebarRow.name), false);
                                else
                                    root.notice(root.playlists.errorText(), true);
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

                            glyph: "play"
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

                        // Rename and delete are secondary: one ⋯ entry, not a
                        // toolbar (IA progressive-disclosure map).
                        IconButton {
                            id: moreButton

                            anchors.verticalCenter: parent.verticalCenter
                            iconName: "ellipsis"
                            accessibleName: qsTr("More actions for %1").arg(root.playlistName)
                            onActivated: playlistMenu.popup(moreButton, 0, moreButton.height)
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
                        note: root.playlistIsSmart ? qsTr("No tracks match this rule yet.") : qsTr("Add songs from any row menu, then play the whole list here.")
                    }

                    ListView {
                        id: entriesView

                        visible: root.playlistId >= 0 && count > 0
                        anchors.fill: parent
                        model: entries
                        activeFocusOnTab: true
                        clip: true
                        spacing: Theme.listRowGap
                        highlightMoveDuration: Appearance.duration(Theme.motionHover)
                        header: SelectionBar {
                            width: entriesView.width
                            queue: root.queue
                            playlists: root.playlists
                            count: entrySelection.count
                            trackIds: {
                                entrySelection.stamp;
                                return root.entryMimeIds();
                            }
                            showRemove: !root.playlistIsSmart
                            onCleared: entrySelection.clear()
                            onNotice: (text, isError) => root.notice(text, isError)
                            onRemoveRequested: {
                                const rows = entrySelection.sorted().reverse();
                                for (let i = 0; i < rows.length; i++)
                                    entries.removeAt(rows[i]);
                                playlists.refresh();
                                entrySelection.clear();
                            }
                        }
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
                        Accessible.selectable: true
                        Keys.onPressed: event => {
                            if ((event.modifiers & Qt.ControlModifier) && event.key === Qt.Key_A) {
                                entrySelection.selectAll(entriesView.count);
                                event.accepted = true;
                                return;
                            }
                            if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier))) {
                                const at = entriesView.currentIndex >= 0 ? entriesView.currentIndex : 0;
                                if (at < entriesView.count) {
                                    entriesView.currentIndex = at;
                                    entryMenu.rowIndex = at;
                                    entryMenu.rowTrackId = entries.trackIdAt(at);
                                    entryMenu.rowPlayable = entries.isPlayableAt(at);
                                    entryMenu.popup();
                                    event.accepted = true;
                                }
                            }
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
                                if (from > 0 && !root.playlistIsSmart) {
                                    entries.moveItem(from, from - 1);
                                    entriesView.currentIndex = from - 1;
                                }
                                event.accepted = true;
                                return;
                            }
                            if (event.modifiers & Qt.ShiftModifier) {
                                const next = Math.max(0, entriesView.currentIndex - 1);
                                entriesView.currentIndex = next;
                                entrySelection.setRange(next);
                                event.accepted = true;
                            }
                        }
                        Keys.onDownPressed: event => {
                            if (event.modifiers & Qt.AltModifier) {
                                const from = entriesView.currentIndex;
                                if (from >= 0 && from + 1 < entriesView.count && !root.playlistIsSmart) {
                                    entries.moveItem(from, from + 1);
                                    entriesView.currentIndex = from + 1;
                                }
                                event.accepted = true;
                                return;
                            }
                            if (event.modifiers & Qt.ShiftModifier) {
                                const next = Math.min(entriesView.count - 1, entriesView.currentIndex + 1);
                                entriesView.currentIndex = next;
                                entrySelection.setRange(next);
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
                            reorderable: !root.playlistIsSmart
                            selected: {
                                entrySelection.stamp;
                                return entrySelection.contains(index);
                            }
                            dragTrackIds: selected && root.entryMimeIds() !== "" ? root.entryMimeIds() : String(model.trackId)
                            dragOrigin: "playlist:" + root.playlistId
                            dragRows: {
                                entrySelection.stamp;
                                return selected ? root.entryRowsCsv() : String(index);
                            }
                            onPlayRequested: (trackId, rowIndex, dangling) => {
                                entrySelection.clear();
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
                            onReorderRequested: (from, to) => {
                                if (!root.playlistIsSmart)
                                    entries.moveItem(from, to);
                            }
                            onToggleSelectRequested: row => {
                                entriesView.currentIndex = row;
                                entrySelection.toggle(row);
                            }
                            onRangeSelectRequested: row => {
                                entriesView.currentIndex = row;
                                entrySelection.setRange(row);
                            }
                        }
                    }

                    // Positional landing: the insertion line shows the index,
                    // an internal drag moves entry rows there, an external
                    // one inserts. Smart playlists refuse (menu does too).
                    TrackDropArea {
                        anchors.fill: parent
                        z: 2
                        enabled: root.playlistId >= 0 && !root.playlistIsSmart
                        targetView: entriesView
                        showHint: false
                        onTracksDroppedAt: (ids, index, origin, rows) => {
                            if (root.playlistId < 0 || root.playlistIsSmart)
                                return;
                            if (origin === "playlist:" + root.playlistId && rows !== "")
                                entries.moveItems(rows, index);
                            else
                                entries.insertTracks(ids, index);
                            root.errorLine = entries.errorText();
                            entrySelection.clear();
                        }
                    }
                }
            }
        }
    }
}
