import QtQuick
import QtQuick.Controls.Basic
import TuneX

// SearchView (S3 W-020): grouped live results — Songs, then Albums, then
// Artists. The query binds from the shell search field; each group owns a
// model instance with a debounced off-thread worker, and the timer drains
// settled results while visible. States: no-query hint, first-results
// loading, no-results with library scope note, per-tab results with 200-cap
// footers, error line. Album cards drill into that album's tracks; playback
// from results arrives with the queue wiring (W-021/W-022).
Item {
    id: root

    required property QueueModel queue
    required property PlaylistModel playlists
    required property LibraryManager library
    // Raw query text, bound from the shell search field.
    property string query: ""
    property string tab: "songs"
    // Album drill-down: -1 means query results.
    property int albumId: -1
    property string albumTitle: ""
    // Per-group settle flags since the last submit (staggered workers).
    property bool songsSettled: false
    property bool albumsSettled: false
    property bool artistsSettled: false
    property string searchError: ""
    property string rememberedFor: ""
    property list<string> recents: []
    readonly property bool drilled: root.albumId >= 0
    readonly property bool awaitingFirst: root.query !== "" && !root.songsSettled && !root.albumsSettled && !root.artistsSettled
    readonly property bool settledEmpty: root.songsSettled && root.albumsSettled && root.artistsSettled && !root.drilled && songsView.count === 0 && albumsView.count === 0 && artistsView.count === 0
    readonly property bool showContent: root.query !== "" && !root.awaitingFirst && !root.settledEmpty
    // Fluid artwork columns shared by both grids (160–220px cards).
    readonly property int gridCell: Math.max(Theme.gridMin, Math.floor(content.width / Math.max(1, Math.floor(content.width / Theme.gridTarget))))

    signal focusFieldRequested
    signal clearRequested
    signal queryRequested(string text)
    signal albumRequested(int albumId)
    signal artistRequested(string name)

    function loadRecents() {
        const items = [];
        const n = root.library.recentSearchCount();
        for (let i = 0; i < n; i++)
            items.push(root.library.recentSearchAt(i));
        root.recents = items;
    }

    function maybeRemember() {
        const q = root.query.trim();
        if (q.length < 2 || root.rememberedFor === q)
            return;
        if (!root.songsSettled && !root.albumsSettled && !root.artistsSettled)
            return;
        root.library.rememberSearch(q);
        root.rememberedFor = q;
        root.loadRecents();
    }

    function cycleGroup(back) {
        const order = ["songs", "albums", "artists"];
        let at = order.indexOf(root.tab);
        if (at < 0)
            at = 0;
        const step = back ? -1 : 1;
        for (let n = 0; n < order.length; n++) {
            at = (at + step + order.length) % order.length;
            const key = order[at];
            if (key === "songs" && songsView.count > 0) {
                root.tab = key;
                songsView.forceActiveFocus();
                return;
            }
            if (key === "albums" && albumsView.count > 0) {
                root.tab = key;
                albumsView.forceActiveFocus();
                return;
            }
            if (key === "artists" && artistsView.count > 0) {
                root.tab = key;
                artistsView.forceActiveFocus();
                return;
            }
        }
    }

    function clearSelection() {
        return songsSelection.clear();
    }

    function songsMimeIds() {
        const rows = songsSelection.sorted();
        const ids = [];
        for (let i = 0; i < rows.length; i++) {
            const id = songs.trackIdAt(rows[i]);
            if (id >= 0)
                ids.push(id);
        }
        return ids.join(",");
    }

    TrackListSelection {
        id: songsSelection
    }

    function submitAll() {
        songs.search(root.query);
        albums.search(root.query);
        artists.search(root.query);
    }

    function clearAll() {
        songs.clear();
        albums.clear();
        artists.clear();
    }

    function drillIntoAlbum(id, title) {
        root.albumId = id;
        root.albumTitle = title;
        songs.refreshAlbum(id);
        root.tab = "songs";
    }

    function leaveDrill() {
        root.leaveDrillKeepTab();
        root.tab = "songs";
    }

    function leaveDrillKeepTab() {
        root.albumId = -1;
        root.albumTitle = "";
        root.songsSettled = false;
        root.albumsSettled = false;
        root.artistsSettled = false;
        // Mirror the query-change path: drop drill rows before resubmitting
        // so stale album tracks never present as query results.
        root.clearAll();
        root.submitAll();
    }

    function focusResults() {
        if (!root.showContent)
            return;

        // Only move focus when rows exist to receive it; loading and empty
        // states stay readable with focus left in the field.
        if (root.tab === "songs" && songsView.count > 0)
            songsView.forceActiveFocus();
        else if (root.tab === "albums" && albumsView.count > 0)
            albumsView.forceActiveFocus();
        else if (root.tab === "artists" && artistsView.count > 0)
            artistsView.forceActiveFocus();
    }

    function tabLabel(key) {
        if (key === "songs")
            return songsView.count > 0 ? qsTr("Tracks (%1)").arg(songsView.count) : qsTr("Tracks");

        if (key === "albums")
            return albumsView.count > 0 ? qsTr("Albums (%1)").arg(albumsView.count) : qsTr("Albums");

        return artistsView.count > 0 ? qsTr("Artists (%1)").arg(artistsView.count) : qsTr("Artists");
    }

    function playAll() {
        root.queue.clearQueue();
        let added = 0;
        if (root.tab === "songs") {
            const ids = [];
            for (let i = 0; i < songsView.count; i++) {
                if (songs.isPlayableAt(i))
                    ids.push(songs.trackIdAt(i));
            }
            added = root.queue.enqueueTrackIds(ids.join(","));
        } else if (root.tab === "albums") {
            const ids = [];
            for (let i = 0; i < albumsView.count; i++)
                ids.push(albums.albumIdAt(i));
            added = root.queue.enqueueAlbumIds(ids.join(","));
        } else {
            const names = [];
            for (let i = 0; i < artistsView.count; i++)
                names.push(artists.nameAt(i));
            added = root.queue.enqueueArtistNames(names.join("\n"));
        }
        if (added > 0)
            root.queue.playAt(0);
    }

    onTabChanged: {
        // A drill hides its Back button off the songs tab: unwind it when
        // the tab leaves (results resubmit; the target tab shows them).
        if (root.tab !== "songs" && root.drilled)
            root.leaveDrillKeepTab();
    }
    onQueryChanged: {
        // New keystrokes supersede everything: clear the drill, drop stale
        // rows immediately (never flash outdated results), resubmit.
        root.albumId = -1;
        root.albumTitle = "";
        root.songsSettled = false;
        root.albumsSettled = false;
        root.artistsSettled = false;
        root.searchError = "";
        songsSelection.clear();
        if (root.query === "") {
            root.clearAll();
        } else {
            root.clearAll();
            root.submitAll();
        }
    }
    Component.onCompleted: root.loadRecents()
    anchors.fill: parent

    ArtistListModel {
        id: artists
    }

    AlbumListModel {
        id: albums
    }

    LibraryTrackModel {
        id: songs
    }

    TrackMenu {
        id: trackMenu

        queue: root.queue
        playlists: root.playlists
        library: root.library
        onIndexChanged: root.submitAll()
    }

    Timer {
        interval: 100
        running: root.visible
        repeat: true
        onTriggered: {
            // While drilled the songs worker idles unpolled: its pending
            // results would clobber the drill, and the next submit drops
            // them as stale by generation.
            if (!root.drilled && songs.pollSearch())
                root.songsSettled = true;

            if (albums.pollSearch())
                root.albumsSettled = true;

            if (artists.pollSearch())
                root.artistsSettled = true;

            root.searchError = songs.errorText() || albums.errorText() || artists.errorText();
            root.maybeRemember();
        }
    }

    Column {
        anchors.fill: parent
        anchors.margins: Theme.spaceLg
        spacing: Theme.spaceMd

        PageBanner {
            id: searchBanner

            width: parent.width
            title: qsTr("Search")
            source: "qrc:/qt/qml/TuneX/banner-search.png"
        }

        // Result-group chips stay — they split one query, unlike Library
        // pages which the rail already switches.
        Item {
            id: tabRow

            width: parent.width
            height: tabButtons.height

            Row {
                id: tabButtons

                spacing: Theme.spaceXs

                // Static key model: counts resolve inside the delegate text
                // binding, so settling groups never rebuild (and unfocus)
                // the tab buttons.
                Repeater {
                    model: ["songs", "albums", "artists"]

                    Chip {
                        required property string modelData

                        label: root.tabLabel(modelData)
                        selected: root.tab === modelData
                        onActivated: root.tab = modelData
                    }
                }
            }
        }

        Item {
            id: playAllRow

            visible: root.showContent && !root.drilled
            width: parent.width
            height: visible ? playAllButton.height : 0
            readonly property int playAllCount: root.tab === "albums" ? albumsView.count : (root.tab === "artists" ? artistsView.count : songsView.count)

            PrimaryButton {
                id: playAllButton

                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                glyph: "play"
                text: qsTr("Play all (%1)").arg(playAllRow.playAllCount)
                enabled: (root.tab === "songs" && songsView.count > 0) || (root.tab === "albums" && albumsView.count > 0) || (root.tab === "artists" && artistsView.count > 0)
                Accessible.name: qsTr("Play all %1 results").arg(playAllRow.playAllCount)
                onClicked: root.playAll()
            }
        }

        // Error line: first worker failure, cleared on the next submit.
        Text {
            id: errorLine

            visible: root.searchError !== ""
            width: parent.width
            height: visible ? implicitHeight : 0
            clip: true
            elide: Text.ElideRight
            text: root.searchError
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontCaption
            color: Theme.error
        }

        // Drill header: album title plus the way back (re-submits the query).
        Row {
            id: drillRow

            visible: root.drilled && root.tab === "songs"
            width: parent.width
            height: visible ? implicitHeight : 0
            clip: true
            spacing: Theme.spaceSm

            PrimaryButton {
                id: drillBack

                primary: false
                text: qsTr("Back to results")
                onClicked: root.leaveDrill()
            }

            PrimaryButton {
                id: playAlbumButton

                text: qsTr("Play album")
                Accessible.name: qsTr("Play this album now")
                onClicked: {
                    root.queue.clearQueue();
                    root.queue.enqueueAlbum(root.albumId);
                    root.queue.playAt(0);
                }
            }

            PrimaryButton {
                id: queueAlbumButton

                primary: false
                text: qsTr("Queue album")
                Accessible.name: qsTr("Add this album to Now Playing")
                onClicked: root.queue.enqueueAlbum(root.albumId)
            }

            Text {
                width: parent.width - drillBack.width - playAlbumButton.width - queueAlbumButton.width - Theme.spaceSm * 3
                anchors.verticalCenter: parent.verticalCenter
                elide: Text.ElideRight
                text: root.albumTitle
                textFormat: Text.PlainText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontTitle
                font.weight: Font.DemiBold
                color: Theme.foreground
            }
        }

        Item {
            id: content

            width: parent.width
            height: parent.height - searchBanner.height - tabRow.height - playAllRow.height - errorLine.height - drillRow.height - Theme.spaceMd * 5

            EmptyState {
                visible: root.query === ""
                art: "qrc:/qt/qml/TuneX/empty-search.png"
                title: qsTr("Search your library")
                note: qsTr("Find songs, albums, and artists as you type — results appear here.")
                actionLabel: qsTr("Focus search")
                onActionRequested: root.focusFieldRequested()
            }

            // Labeled recents with a way out: chips alone gave no name
            // to the row and no way to remove entries.
            Column {
                visible: root.query === "" && root.recents.length > 0
                anchors.bottom: parent.bottom
                anchors.horizontalCenter: parent.horizontalCenter
                width: Math.min(parent.width, 520)
                spacing: Theme.spaceXs

                Row {
                    width: parent.width
                    spacing: Theme.spaceSm

                    Text {
                        id: recentsLabel

                        anchors.verticalCenter: parent.verticalCenter
                        text: qsTr("RECENT SEARCHES")
                        textFormat: Text.PlainText
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontCaption
                        font.weight: Font.DemiBold
                        font.letterSpacing: 0.8
                        color: Theme.muted
                    }

                    Item {
                        width: parent.width - recentsLabel.implicitWidth - clearRecents.implicitWidth - parent.spacing * 2
                        height: 1
                    }

                    TextLink {
                        id: clearRecents

                        anchors.verticalCenter: parent.verticalCenter
                        text: qsTr("Clear")
                        accessibleName: qsTr("Clear recent searches")
                        onActivated: {
                            root.library.clearRecentSearches();
                            root.loadRecents();
                        }
                    }
                }

                Flow {
                    width: parent.width
                    spacing: Theme.spaceXs

                    Repeater {
                        model: root.recents

                        Chip {
                            required property string modelData

                            label: modelData
                            onActivated: root.queryRequested(modelData)
                        }
                    }
                }
            }

            // First-results loading: one centered spinner until any group
            // settles; groups then stream in progressively, no global wait.
            Column {
                visible: root.awaitingFirst
                anchors.centerIn: parent
                spacing: Theme.spaceSm

                BusyIndicator {
                    anchors.horizontalCenter: parent.horizontalCenter
                    running: root.awaitingFirst
                    Accessible.name: qsTr("Searching")
                }

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: qsTr("Searching…")
                    textFormat: Text.PlainText
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontBody
                    color: Theme.muted
                }
            }

            EmptyState {
                visible: root.settledEmpty
                art: "qrc:/qt/qml/TuneX/empty-search.png"
                title: qsTr("No matches")
                note: qsTr("Nothing in your library matches “%1”.").arg(root.query)
                actionLabel: qsTr("Clear search")
                onActionRequested: root.clearRequested()
            }

            // Songs group: virtualized list over the ranked track hits.
            Item {
                visible: root.showContent && root.tab === "songs"
                anchors.fill: parent

                ListView {
                    id: songsView
                    ScrollBar.vertical: ListScrollBar {}

                    anchors.fill: parent
                    model: songs
                    // No declarative focus: worker-driven flips would yank
                    // focus from the field mid-typing; entry is explicit via
                    // focusResults() plus normal Tab order.
                    activeFocusOnTab: true
                    clip: true
                    spacing: Theme.listRowGap
                    highlightMoveDuration: Appearance.duration(Theme.motionHover)
                    header: SelectionBar {
                        width: songsView.width
                        queue: root.queue
                        playlists: root.playlists
                        count: songsSelection.count
                        trackIds: {
                            songsSelection.stamp;
                            return root.songsMimeIds();
                        }
                        onCleared: songsSelection.clear()
                    }
                    Accessible.role: Accessible.List
                    Accessible.selectable: true
                    Accessible.name: root.drilled ? root.albumTitle : qsTr("Song results")
                    Keys.onReturnPressed: {
                        const at = songsView.currentIndex >= 0 ? songsView.currentIndex : 0;
                        if (at < songsView.count && songs.isPlayableAt(at))
                            root.queue.playTrackNow(songs.trackIdAt(at));
                    }
                    Keys.onEnterPressed: {
                        const at = songsView.currentIndex >= 0 ? songsView.currentIndex : 0;
                        if (at < songsView.count && songs.isPlayableAt(at))
                            root.queue.playTrackNow(songs.trackIdAt(at));
                    }
                    Keys.onPressed: event => {
                        if ((event.modifiers & Qt.ControlModifier) && event.key === Qt.Key_A) {
                            songsSelection.selectAll(songsView.count);
                            event.accepted = true;
                            return;
                        }
                        if (event.key === Qt.Key_Tab) {
                            root.cycleGroup((event.modifiers & Qt.ShiftModifier) !== 0);
                            event.accepted = true;
                            return;
                        }
                        if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier))) {
                            const at = songsView.currentIndex >= 0 ? songsView.currentIndex : 0;
                            if (at < songsView.count) {
                                songsView.currentIndex = at;
                                trackMenu.trackId = songs.trackIdAt(at);
                                trackMenu.trackIds = songsSelection.contains(at) ? root.songsMimeIds() : "";
                                trackMenu.popup();
                                event.accepted = true;
                            }
                        }
                    }
                    Keys.onUpPressed: event => {
                        if (event.modifiers & Qt.ShiftModifier) {
                            const next = Math.max(0, songsView.currentIndex - 1);
                            songsView.currentIndex = next;
                            songsSelection.setRange(next);
                            event.accepted = true;
                        }
                    }
                    Keys.onDownPressed: event => {
                        if (event.modifiers & Qt.ShiftModifier) {
                            const next = Math.min(songsView.count - 1, songsView.currentIndex + 1);
                            songsView.currentIndex = next;
                            songsSelection.setRange(next);
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
                        trackNumber: model.trackNumber
                        durationMs: model.durationMs
                        missing: model.missing
                        selected: {
                            songsSelection.stamp;
                            return songsSelection.contains(index);
                        }
                        dragTrackIds: selected && root.songsMimeIds() !== "" ? root.songsMimeIds() : String(model.trackId)
                        onPlayRequested: (trackId, rowIndex, dangling) => {
                            songsSelection.clear();
                            songsView.currentIndex = index;
                            songsView.forceActiveFocus();
                            if (!dangling && songs.isPlayableAt(index))
                                root.queue.playTrackNow(trackId);
                        }
                        onMenuRequested: trackId => {
                            songsView.currentIndex = index;
                            trackMenu.trackId = trackId;
                            trackMenu.trackIds = songsSelection.contains(index) ? root.songsMimeIds() : "";
                            trackMenu.popup();
                        }
                        onToggleSelectRequested: row => {
                            songsView.currentIndex = row;
                            songsSelection.toggle(row);
                        }
                        onRangeSelectRequested: row => {
                            songsView.currentIndex = row;
                            songsSelection.setRange(row);
                        }
                    }

                    footer: Text {
                        visible: songsView.count >= 200
                        width: songsView.width
                        height: visible ? implicitHeight : 0
                        horizontalAlignment: Text.AlignHCenter
                        text: qsTr("Showing the first 200 matches.")
                        textFormat: Text.PlainText
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontCaption
                        color: Theme.muted
                    }
                }

                BusyIndicator {
                    visible: root.query !== "" && !root.songsSettled && !root.drilled && songsView.count === 0
                    running: root.query !== "" && !root.songsSettled && !root.drilled && songsView.count === 0
                    anchors.centerIn: parent
                    Accessible.name: qsTr("Searching songs")
                }

                EmptyState {
                    visible: songsView.count === 0 && (root.songsSettled || root.drilled)
                    title: root.drilled ? qsTr("No tracks here") : qsTr("No songs match")
                    note: root.drilled ? qsTr("This album has no indexed tracks.") : qsTr("Try a different spelling, or browse the library.")
                    actionLabel: root.drilled ? "" : qsTr("Clear search")
                    onActionRequested: root.clearRequested()
                }
            }

            // Albums group: fluid artwork grid over the album hits.
            Item {
                visible: root.showContent && root.tab === "albums"
                anchors.fill: parent

                GridView {
                    id: albumsView
                    ScrollBar.vertical: ListScrollBar {}

                    anchors.fill: parent
                    model: albums
                    // No declarative focus (see songs view): entry is explicit.
                    activeFocusOnTab: true
                    clip: true
                    cellWidth: root.gridCell
                    cellHeight: cellWidth + Theme.cardMetaHeight
                    highlightMoveDuration: Appearance.duration(Theme.motionHover)
                    Accessible.role: Accessible.List
                    Accessible.name: qsTr("Album results")
                    Keys.onPressed: event => {
                        if (event.key === Qt.Key_Tab) {
                            root.cycleGroup((event.modifiers & Qt.ShiftModifier) !== 0);
                            event.accepted = true;
                        }
                    }

                    highlight: Rectangle {
                        color: "transparent"
                        radius: Theme.radiusMd
                        border.color: albumsView.activeFocus ? Theme.focus : "transparent"
                        border.width: 2
                    }

                    delegate: AlbumCard {
                        albumId: model.albumId
                        title: model.title
                        artist: model.artist
                        year: model.year
                        trackCount: model.trackCount
                        cardIndex: index
                        onActivated: id => {
                            albumsView.currentIndex = cardIndex;
                            root.albumRequested(id);
                        }
                    }

                    footer: Text {
                        visible: albumsView.count >= 200
                        width: albumsView.width
                        height: visible ? implicitHeight : 0
                        horizontalAlignment: Text.AlignHCenter
                        text: qsTr("Showing the first 200 matches.")
                        textFormat: Text.PlainText
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontCaption
                        color: Theme.muted
                    }
                }

                BusyIndicator {
                    visible: root.query !== "" && !root.albumsSettled && albumsView.count === 0
                    running: root.query !== "" && !root.albumsSettled && albumsView.count === 0
                    anchors.centerIn: parent
                    Accessible.name: qsTr("Searching albums")
                }

                EmptyState {
                    visible: root.albumsSettled && albumsView.count === 0
                    title: qsTr("No albums match")
                    note: qsTr("Try a different spelling, or browse the library.")
                    actionLabel: qsTr("Clear search")
                    onActionRequested: root.clearRequested()
                }
            }

            // Artists group: fluid monogram grid over the artist hits.
            Item {
                visible: root.showContent && root.tab === "artists"
                anchors.fill: parent

                GridView {
                    id: artistsView
                    ScrollBar.vertical: ListScrollBar {}

                    anchors.fill: parent
                    model: artists
                    // No declarative focus (see songs view): entry is explicit.
                    activeFocusOnTab: true
                    clip: true
                    cellWidth: root.gridCell
                    cellHeight: cellWidth + Theme.cardMetaHeight
                    highlightMoveDuration: Appearance.duration(Theme.motionHover)
                    Accessible.role: Accessible.List
                    Accessible.name: qsTr("Artist results")
                    Keys.onPressed: event => {
                        if (event.key === Qt.Key_Tab) {
                            root.cycleGroup((event.modifiers & Qt.ShiftModifier) !== 0);
                            event.accepted = true;
                        }
                    }

                    highlight: Rectangle {
                        color: "transparent"
                        radius: Theme.radiusMd
                        border.color: artistsView.activeFocus ? Theme.focus : "transparent"
                        border.width: 2
                    }

                    delegate: ArtistCard {
                        artistName: model.name
                        albumCount: model.albumCount
                        trackCount: model.trackCount
                        onActivated: name => root.artistRequested(name)
                    }

                    footer: Text {
                        visible: artistsView.count >= 200
                        width: artistsView.width
                        height: visible ? implicitHeight : 0
                        horizontalAlignment: Text.AlignHCenter
                        text: qsTr("Showing the first 200 matches.")
                        textFormat: Text.PlainText
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontCaption
                        color: Theme.muted
                    }
                }

                BusyIndicator {
                    visible: root.query !== "" && !root.artistsSettled && artistsView.count === 0
                    running: root.query !== "" && !root.artistsSettled && artistsView.count === 0
                    anchors.centerIn: parent
                    Accessible.name: qsTr("Searching artists")
                }

                EmptyState {
                    visible: root.artistsSettled && artistsView.count === 0
                    title: qsTr("No artists match")
                    note: qsTr("Try a different spelling, or browse the library.")
                    actionLabel: qsTr("Clear search")
                    onActionRequested: root.clearRequested()
                }
            }
        }
    }
}
