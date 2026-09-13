import QtQuick
import TuneX

// LibraryView (S2 W-016): browse the indexed library — Songs list, Albums
// grid, Artists grid, Folders list. Models load from the index on completion;
// a missing index shows the empty state (never an error). Album cards drill
// into the songs tab; folder rows drill into tracks in that directory.
// V1-basic sort chips drive Rust ORDER BY (session-stable). Scanned-folder
// add/remove lives in Settings → Library.
Item {
    id: root

    required property QueueModel queue
    required property PlaylistModel playlists
    required property LibraryManager library
    // Album drill-down: -1 means the full songs tab.
    property int albumId: -1
    property string albumTitle: ""
    property string folderPath: ""
    property string folderTitle: ""
    property string tab: "songs"
    property string songsSort: "title"
    property string albumsSort: "title"
    property string artistsSort: "name"
    property string typePrefix: ""
    readonly property bool albumDrilled: root.albumId >= 0
    readonly property bool folderDrilled: root.folderPath !== ""
    readonly property bool drilled: root.albumDrilled || root.folderDrilled
    // View counts (the models expose rows, not a count property).
    readonly property bool libraryEmpty: songsView.count === 0 && albumsView.count === 0 && artistsView.count === 0
    // Fluid artwork columns shared by both grids (160–220px cards).
    readonly property int gridCell: Math.max(Theme.gridMin, Math.floor(content.width / Math.max(1, Math.floor(content.width / Theme.gridTarget))))

    signal settingsRequested(bool pickFolder)
    // An artist card was activated: V1 plays that artist until S9 opens
    // an artist detail view.
    signal artistRequested(string name)

    // Display name for one tab key (the tab strip models keys, not labels).
    function tabLabel(key) {
        if (key === "albums")
            return qsTr("Albums");
        if (key === "artists")
            return qsTr("Artists");

        if (key === "folders")
            return qsTr("Folders");

        return qsTr("Songs");
    }

    function sortSongs(key) {
        root.songsSort = key;
        songs.setSort(key);
        root.library.setSongsSort(key);
    }

    function sortAlbums(key) {
        root.albumsSort = key;
        albums.setSort(key);
        root.library.setAlbumsSort(key);
    }

    function sortArtists(key) {
        root.artistsSort = key;
        artists.setSort(key);
        root.library.setArtistsSort(key);
    }

    function applyRestoredPrefs() {
        const tab = root.library.libraryTab();
        if (tab === "albums" || tab === "artists" || tab === "folders" || tab === "songs")
            root.tab = tab;

        const songsKey = root.library.songsSort();
        if (songsKey !== "")
            root.sortSongs(songsKey);

        const albumsKey = root.library.albumsSort();
        if (albumsKey !== "")
            root.sortAlbums(albumsKey);

        const artistsKey = root.library.artistsSort();
        if (artistsKey !== "")
            root.sortArtists(artistsKey);
    }

    function moveList(view, delta) {
        if (view.count <= 0)
            return;

        const at = view.currentIndex < 0 ? 0 : view.currentIndex + delta;
        view.currentIndex = Math.max(0, Math.min(view.count - 1, at));
        view.positionViewAtIndex(view.currentIndex, ListView.Contain);
    }

    function typeToSelect(text) {
        root.typePrefix += text.toLowerCase();
        typeReset.restart();
        const n = songsView.count;
        for (let i = 0; i < n; i++) {
            if (songs.titleAt(i).toLowerCase().startsWith(root.typePrefix)) {
                songsView.currentIndex = i;
                songsView.positionViewAtIndex(i, ListView.Contain);
                return;
            }
        }
    }

    function openSongMenu(at) {
        if (at < 0 || at >= songsView.count)
            return;

        songsView.currentIndex = at;
        trackMenu.trackId = songs.trackIdAt(at);
        trackMenu.popup();
    }

    function drillIntoAlbum(id, title) {
        root.albumId = id;
        root.albumTitle = title;
        songs.refreshAlbum(id);
        root.tab = "songs";
    }

    function leaveDrill() {
        root.leaveDrillKeepTab();
        root.tab = "albums";
    }

    function leaveDrillKeepTab() {
        root.albumId = -1;
        root.albumTitle = "";
        songs.refreshSongs();
    }

    function drillIntoFolder(path, name) {
        root.folderPath = path;
        root.folderTitle = name;
        songs.refreshFolder(path);
    }

    function leaveFolderDrill() {
        root.folderPath = "";
        root.folderTitle = "";
        songs.refreshSongs();
        root.tab = "folders";
    }

    function leaveFolderDrillKeepTab() {
        root.folderPath = "";
        root.folderTitle = "";
        songs.refreshSongs();
    }

    function refresh() {
        artists.refresh();
        albums.refresh();
        songs.refresh();
        folders.refresh();
    }

    onTabChanged: {
        if (root.tab !== "songs" && root.albumDrilled)
            root.leaveDrillKeepTab();

        if (root.tab !== "folders" && root.folderDrilled)
            root.leaveFolderDrillKeepTab();

        if (root.tab === "songs" || root.tab === "albums" || root.tab === "artists" || root.tab === "folders")
            root.library.setLibraryTab(root.tab);
    }
    anchors.fill: parent
    Component.onCompleted: {
        artists.refresh();
        albums.refresh();
        songs.refresh();
        folders.refresh();
    }

    ArtistListModel {
        id: artists
    }

    AlbumListModel {
        id: albums
    }

    // Covers land one row at a time; this pump publishes them and stops
    // itself as soon as the resolver runs dry.
    Timer {
        id: artPump

        interval: 120
        repeat: true
        onTriggered: {
            if (!albums.pollArt())
                artPump.stop();
        }
    }

    LibraryTrackModel {
        id: songs
    }

    FolderListModel {
        id: folders
    }

    TrackMenu {
        id: trackMenu

        queue: root.queue
        playlists: root.playlists
        library: root.library
        onIndexChanged: root.refresh()
    }

    Timer {
        id: typeReset

        interval: 400
        onTriggered: root.typePrefix = ""
    }

    Column {
        id: header

        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: Theme.spaceLg
        spacing: Theme.spaceMd

        // Header: tab strip (Songs / Albums / Artists / Folders browse) plus
        // a shortcut into Settings → Library for scanned-folder management.
        Item {
            id: tabRow

            width: parent.width
            height: foldersButton.height

            Row {
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.spaceXs

                Repeater {
                    // Keys only, so the model stays a typed string list; the
                    // translated label comes from `root.tabLabel`.
                    model: ["songs", "albums", "artists", "folders"]

                    Chip {
                        required property string modelData

                        label: root.tabLabel(modelData)
                        selected: root.tab === modelData
                        onActivated: root.tab = modelData
                    }
                }
            }

            PrimaryButton {
                id: foldersButton

                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                primary: false
                text: qsTr("Music folders")
                Accessible.name: qsTr("Manage music folders")
                onClicked: root.settingsRequested(false)
            }
        }

        // V1-basic sort: one selected chip per view; order is applied in Rust.
        Row {
            id: sortRow

            readonly property bool showSongsSort: (root.tab === "songs" && !root.albumDrilled) || (root.tab === "folders" && root.folderDrilled)
            readonly property bool showAlbumsSort: root.tab === "albums"
            readonly property bool showArtistsSort: root.tab === "artists"

            visible: !root.libraryEmpty && (showSongsSort || showAlbumsSort || showArtistsSort)
            width: parent.width
            height: visible ? implicitHeight : 0
            spacing: Theme.spaceXs

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: qsTr("Sort")
                textFormat: Text.PlainText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontCaption
                font.weight: Font.DemiBold
                color: Theme.muted
            }

            Chip {
                visible: sortRow.showSongsSort
                label: qsTr("Title")
                selected: root.songsSort === "title"
                onActivated: root.sortSongs("title")
            }

            Chip {
                visible: sortRow.showSongsSort
                label: qsTr("Artist")
                selected: root.songsSort === "artist"
                onActivated: root.sortSongs("artist")
            }

            Chip {
                visible: sortRow.showSongsSort
                label: qsTr("Album")
                selected: root.songsSort === "album"
                onActivated: root.sortSongs("album")
            }

            Chip {
                visible: sortRow.showSongsSort
                label: qsTr("Date")
                selected: root.songsSort === "date"
                onActivated: root.sortSongs("date")
            }

            Chip {
                visible: sortRow.showAlbumsSort
                label: qsTr("Title")
                selected: root.albumsSort === "title"
                onActivated: root.sortAlbums("title")
            }

            Chip {
                visible: sortRow.showAlbumsSort
                label: qsTr("Artist")
                selected: root.albumsSort === "artist"
                onActivated: root.sortAlbums("artist")
            }

            Chip {
                visible: sortRow.showAlbumsSort
                label: qsTr("Date")
                selected: root.albumsSort === "date"
                onActivated: root.sortAlbums("date")
            }

            Chip {
                visible: sortRow.showArtistsSort
                label: qsTr("Name")
                selected: root.artistsSort === "name"
                onActivated: root.sortArtists("name")
            }

            Chip {
                visible: sortRow.showArtistsSort
                label: qsTr("Songs")
                selected: root.artistsSort === "songs"
                onActivated: root.sortArtists("songs")
            }
        }

        // Drill header: album title, queue actions, plus the way back.
        // Collapsed (zero height) when not drilling — positioners keep
        // invisible space. Hidden while the library is empty (a drill cannot
        // outlive its rows).
        Row {
            id: drillRow

            visible: root.albumDrilled && root.tab === "songs" && !root.libraryEmpty
            width: parent.width
            height: visible ? implicitHeight : 0
            clip: true
            spacing: Theme.spaceSm

            PrimaryButton {
                id: drillBack

                primary: false
                text: qsTr("Back to albums")
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
                Accessible.name: qsTr("Add this album to Up Next")
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

        Row {
            id: folderDrillRow

            visible: root.folderDrilled && root.tab === "folders" && !root.libraryEmpty
            width: parent.width
            height: visible ? implicitHeight : 0
            clip: true
            spacing: Theme.spaceSm

            PrimaryButton {
                id: folderBack

                primary: false
                text: qsTr("Back to folders")
                onClicked: root.leaveFolderDrill()
            }

            PrimaryButton {
                id: playFolderButton

                text: qsTr("Play folder")
                Accessible.name: qsTr("Play this folder now")
                onClicked: {
                    root.queue.clearQueue();
                    root.queue.enqueueFolder(root.folderPath, root.songsSort);
                    root.queue.playAt(0);
                }
            }

            PrimaryButton {
                id: queueFolderButton

                primary: false
                text: qsTr("Queue folder")
                Accessible.name: qsTr("Queue this folder in Up Next")
                onClicked: root.queue.enqueueFolder(root.folderPath, root.songsSort)
            }

            Text {
                width: parent.width - folderBack.width - playFolderButton.width - queueFolderButton.width - Theme.spaceSm * 3
                anchors.verticalCenter: parent.verticalCenter
                elide: Text.ElideRight
                text: root.folderTitle
                textFormat: Text.PlainText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontTitle
                font.weight: Font.DemiBold
                color: Theme.foreground
            }
        }
    }

    Item {
        id: content

        anchors.top: header.bottom
        anchors.topMargin: Theme.spaceMd
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.leftMargin: Theme.spaceLg
        anchors.rightMargin: Theme.spaceLg
        anchors.bottomMargin: Theme.spaceLg

        EmptyState {
            visible: root.libraryEmpty
            art: "qrc:/qt/qml/TuneX/empty-library.png"
            title: qsTr("No music yet")
            note: qsTr("Add a music folder and your artists, albums, and songs will appear here.")
            actionLabel: qsTr("Add music folder")
            onActionRequested: root.settingsRequested(true)
        }

        // Songs tab: virtualized list over the capped songs query.
        ListView {
            id: songsView

            visible: ((root.tab === "songs") || (root.tab === "folders" && root.folderDrilled)) && !root.libraryEmpty
            anchors.fill: parent
            model: songs
            focus: ((root.tab === "songs") || (root.tab === "folders" && root.folderDrilled)) && !root.libraryEmpty
            activeFocusOnTab: true
            clip: true
            highlightMoveDuration: Appearance.duration(Theme.motionHover)
            Accessible.role: Accessible.List
            Accessible.name: root.albumDrilled ? root.albumTitle : (root.folderDrilled ? root.folderTitle : qsTr("Songs"))
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
                if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier))) {
                    root.openSongMenu(songsView.currentIndex >= 0 ? songsView.currentIndex : 0);
                    event.accepted = true;
                    return;
                }
                if (event.key === Qt.Key_J && root.typePrefix === "") {
                    root.moveList(songsView, 1);
                    event.accepted = true;
                    return;
                }
                if (event.key === Qt.Key_K && root.typePrefix === "") {
                    root.moveList(songsView, -1);
                    event.accepted = true;
                    return;
                }
                if (event.text.length === 1 && event.text >= " " && !(event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))) {
                    root.typeToSelect(event.text);
                    event.accepted = true;
                }
            }

            highlight: Rectangle {
                color: Theme.selected
                radius: Theme.radiusSm
            }

            delegate: TrackRow {
                trackId: model.trackId
                title: model.title
                artist: model.artist
                trackNumber: model.trackNumber
                durationMs: model.durationMs
                missing: model.missing
                onPlayRequested: (trackId, rowIndex, dangling) => {
                    songsView.currentIndex = index;
                    songsView.forceActiveFocus();
                    if (!dangling && songs.isPlayableAt(index))
                        root.queue.playTrackNow(trackId);
                }
                onMenuRequested: trackId => {
                    songsView.currentIndex = index;
                    trackMenu.trackId = trackId;
                    trackMenu.popup();
                }
            }

            // The cap notice sits in a wrapper: a Text whose own height
            // is bound to its implicitHeight is a binding loop, which Qt
            // reports the moment a library is big enough to show it.
            footer: Item {
                id: songsFooter

                readonly property bool shown: !root.albumDrilled && !root.folderDrilled && songsView.count >= 500

                width: songsView.width
                height: songsFooter.shown ? capNotice.implicitHeight + Theme.spaceMd : 0

                Text {
                    id: capNotice

                    visible: songsFooter.shown
                    anchors.centerIn: parent
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: qsTr("Showing the first 500 songs — search finds the rest.")
                    textFormat: Text.PlainText
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontCaption
                    color: Theme.muted
                }
            }
        }

        // Albums tab: fluid artwork grid over the album query.
        GridView {
            id: albumsView

            visible: root.tab === "albums" && !root.libraryEmpty
            anchors.fill: parent
            model: albums
            focus: root.tab === "albums" && !root.libraryEmpty
            activeFocusOnTab: true
            clip: true
            cellWidth: root.gridCell
            cellHeight: cellWidth + Theme.cardMetaHeight
            highlightMoveDuration: Appearance.duration(Theme.motionHover)
            Accessible.role: Accessible.List
            Accessible.name: qsTr("Albums")
            Keys.onReturnPressed: {
                const at = albumsView.currentIndex >= 0 ? albumsView.currentIndex : 0;
                if (at < albumsView.count)
                    root.drillIntoAlbum(albums.albumIdAt(at), albums.titleAt(at));
            }
            Keys.onEnterPressed: {
                const at = albumsView.currentIndex >= 0 ? albumsView.currentIndex : 0;
                if (at < albumsView.count)
                    root.drillIntoAlbum(albums.albumIdAt(at), albums.titleAt(at));
            }
            Keys.onPressed: event => {
                if (event.key === Qt.Key_J) {
                    root.moveList(albumsView, 1);
                    event.accepted = true;
                } else if (event.key === Qt.Key_K) {
                    root.moveList(albumsView, -1);
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
                artUrl: model.artUrl
                cardIndex: index
                onActivated: id => {
                    albumsView.currentIndex = cardIndex;
                    root.drillIntoAlbum(id, title);
                }
                // Ask as the card appears; answered rows cost nothing.
                Component.onCompleted: {
                    albums.requestArt(index);
                    artPump.start();
                }
            }
        }

        // Artists tab: fluid monogram grid over the artist query.
        // Display-only in S2 (drill-down arrives with S3); the
        // focus-gated ring is a reading cursor, not a selection.
        GridView {
            id: artistsView

            visible: root.tab === "artists" && !root.libraryEmpty
            anchors.fill: parent
            model: artists
            focus: root.tab === "artists" && !root.libraryEmpty
            activeFocusOnTab: true
            clip: true
            cellWidth: root.gridCell
            cellHeight: cellWidth + Theme.cardMetaHeight
            highlightMoveDuration: Appearance.duration(Theme.motionHover)
            Accessible.role: Accessible.List
            Accessible.name: qsTr("Artists")
            Keys.onReturnPressed: {
                const at = artistsView.currentIndex >= 0 ? artistsView.currentIndex : 0;
                if (at < artistsView.count)
                    root.artistRequested(artists.nameAt(at));
            }
            Keys.onEnterPressed: {
                const at = artistsView.currentIndex >= 0 ? artistsView.currentIndex : 0;
                if (at < artistsView.count)
                    root.artistRequested(artists.nameAt(at));
            }
            Keys.onPressed: event => {
                if (event.key === Qt.Key_J) {
                    root.moveList(artistsView, 1);
                    event.accepted = true;
                } else if (event.key === Qt.Key_K) {
                    root.moveList(artistsView, -1);
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
        }

        // Folders tab: browse indexed tracks by parent directory.
        ListView {
            id: foldersView

            visible: root.tab === "folders" && !root.folderDrilled && !root.libraryEmpty
            anchors.fill: parent
            model: folders
            focus: root.tab === "folders" && !root.folderDrilled && !root.libraryEmpty
            activeFocusOnTab: true
            clip: true
            highlightMoveDuration: Appearance.duration(Theme.motionHover)
            Accessible.role: Accessible.List
            Accessible.name: qsTr("Folders")
            Keys.onReturnPressed: {
                const at = foldersView.currentIndex >= 0 ? foldersView.currentIndex : 0;
                if (at < foldersView.count)
                    root.drillIntoFolder(folders.pathAt(at), folders.nameAt(at));
            }
            Keys.onEnterPressed: {
                const at = foldersView.currentIndex >= 0 ? foldersView.currentIndex : 0;
                if (at < foldersView.count)
                    root.drillIntoFolder(folders.pathAt(at), folders.nameAt(at));
            }
            Keys.onPressed: event => {
                if (event.key === Qt.Key_J) {
                    root.moveList(foldersView, 1);
                    event.accepted = true;
                } else if (event.key === Qt.Key_K) {
                    root.moveList(foldersView, -1);
                    event.accepted = true;
                }
            }

            highlight: Rectangle {
                color: Theme.selected
                radius: Theme.radiusSm
            }

            delegate: Item {
                id: folderRow

                // Plain (not required) properties: `required` construction-
                // time initialization races the cxx-qt delegate context
                // and locks role bindings to their defaults (W-018).
                readonly property string countLine: model.trackCount === 1 ? qsTr("1 song") : qsTr("%1 songs").arg(model.trackCount)

                width: ListView.view.width
                height: Theme.trackRowHeight
                Accessible.role: Accessible.ListItem
                Accessible.name: model.name + ", " + folderRow.countLine
                Accessible.onPressAction: root.drillIntoFolder(model.path, model.name)

                Rectangle {
                    anchors.fill: parent
                    radius: Theme.radiusSm
                    color: folderMouse.containsMouse || folderRow.activeFocus ? Theme.hover : "transparent"

                    Behavior on color {
                        ColorAnimation {
                            duration: Appearance.duration(Theme.motionHover)
                            easing.type: Easing.OutCubic
                        }
                    }
                }

                MouseArea {
                    id: folderMouse

                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        foldersView.currentIndex = index;
                        root.drillIntoFolder(model.path, model.name);
                    }
                }

                Icon {
                    id: folderGlyph

                    anchors.left: parent.left
                    anchors.leftMargin: Theme.spaceMd
                    anchors.verticalCenter: parent.verticalCenter
                    name: "folder"
                    iconSize: 20
                    stroke: Theme.muted
                }

                Column {
                    anchors.left: folderGlyph.right
                    anchors.leftMargin: Theme.spaceMd
                    anchors.right: parent.right
                    anchors.rightMargin: Theme.spaceMd
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 2

                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: model.name
                        textFormat: Text.PlainText
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontBody
                        color: Theme.foreground
                    }

                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: folderRow.countLine + " · " + model.path
                        textFormat: Text.PlainText
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontBodySm
                        color: Theme.muted
                    }
                }
            }
        }
    }
}
