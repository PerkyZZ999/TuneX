import QtQuick
import QtQuick.Controls.Basic
import TuneX

// LibraryView (S2 W-016, S9 landing pages): browse the indexed library —
// Tracks, Albums, Artists, Genres, Composers, Folders. The left rail
// switches those pages; this view's header is a per-page banner plus
// Play all / Sort By. Album and artist cards open detail views;
// genre/composer rows filter tracks. Models load from the index on
// completion; a missing index shows the empty state (never an error).
// Scanned-folder add/remove lives in Settings → Library.
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
    property string page: "browse"
    property string previousPage: "browse"
    property string facetName: ""
    property string songsSort: "title"
    property bool songsSortDesc: false
    property string albumsSort: "title"
    property bool albumsSortDesc: false
    property string artistsSort: "name"
    property bool artistsSortDesc: false
    property string typePrefix: ""
    readonly property bool albumDrilled: root.albumId >= 0
    readonly property bool folderDrilled: root.folderPath !== ""
    readonly property bool drilled: root.albumDrilled || root.folderDrilled
    readonly property bool showingBrowse: root.page === "browse"
    // View counts (the models expose rows, not a count property).
    readonly property bool libraryEmpty: songsView.count === 0 && albumsView.count === 0 && artistsView.count === 0
    // Fluid artwork columns shared by both grids (160–220px cards).
    readonly property int gridCell: Math.max(Theme.gridMin, Math.floor(content.width / Math.max(1, Math.floor(content.width / Theme.gridTarget))))

    signal settingsRequested(bool pickFolder)

    // Display name for one tab key (the tab strip models keys, not labels).
    function tabLabel(key) {
        if (key === "albums")
            return qsTr("Albums");
        if (key === "artists")
            return qsTr("Artists");
        if (key === "folders")
            return qsTr("Folders");
        if (key === "genres")
            return qsTr("Genres");
        if (key === "composers")
            return qsTr("Composers");
        return qsTr("Tracks");
    }

    // Sort button label mirrors the active key + direction so the state is
    // readable without opening the menu (recognition over recall).
    function sortKeyLabel() {
        if (root.tab === "albums") {
            if (root.albumsSort === "artist")
                return qsTr("Artist");
            if (root.albumsSort === "date")
                return qsTr("Date");
            return qsTr("Title");
        }
        if (root.tab === "artists") {
            if (root.artistsSort === "songs")
                return qsTr("Tracks");
            return qsTr("Name");
        }
        if (root.songsSort === "artist")
            return qsTr("Artist");
        if (root.songsSort === "album")
            return qsTr("Album");
        if (root.songsSort === "date")
            return qsTr("Date");
        return qsTr("Title");
    }

    function sortDescending() {
        if (root.tab === "albums")
            return root.albumsSortDesc;
        if (root.tab === "artists")
            return root.artistsSortDesc;
        return root.songsSortDesc;
    }

    function sortLabel() {
        return qsTr("%1 %2").arg(root.sortKeyLabel()).arg(root.sortDescending() ? "↓" : "↑");
    }

    function sortAccessibleLabel() {
        return qsTr("Sort by %1, %2").arg(root.sortKeyLabel()).arg(root.sortDescending() ? qsTr("descending") : qsTr("ascending"));
    }

    function bannerSource() {
        if (root.tab === "albums")
            return "qrc:/qt/qml/TuneX/banner-albums.png";
        if (root.tab === "artists")
            return "qrc:/qt/qml/TuneX/banner-artists.png";
        if (root.tab === "genres")
            return "qrc:/qt/qml/TuneX/banner-genres.png";
        if (root.tab === "composers")
            return "qrc:/qt/qml/TuneX/banner-composers.png";
        if (root.tab === "folders")
            return "qrc:/qt/qml/TuneX/banner-folders.png";
        return "qrc:/qt/qml/TuneX/banner-tracks.png";
    }

    function playAll() {
        root.queue.clearQueue();
        let added = 0;
        if (root.tab === "folders" && root.folderDrilled)
            added = root.queue.enqueueFolder(root.folderPath, root.songsSort, root.songsSortDesc);
        else if (root.tab === "songs")
            added = root.queue.enqueueLibraryList("songs", "", root.songsSort, root.songsSortDesc);
        else if (root.tab === "albums")
            added = root.queue.enqueueLibraryList("albums", "", root.albumsSort, root.albumsSortDesc);
        else if (root.tab === "artists")
            added = root.queue.enqueueLibraryList("artists", "", root.artistsSort, root.artistsSortDesc);
        else if (root.tab === "folders")
            added = root.queue.enqueueLibraryList("folders", "", root.songsSort, root.songsSortDesc);
        else if (root.tab === "genres")
            added = root.queue.enqueueLibraryList("genres", "", "", false);
        else if (root.tab === "composers")
            added = root.queue.enqueueLibraryList("composers", "", "", false);

        if (added > 0)
            root.queue.playAt(0);
    }

    function playFacet() {
        root.queue.clearQueue();
        const kind = root.page === "composer" ? "composer" : "genre";
        const added = root.queue.enqueueLibraryList(kind, root.facetName, root.songsSort, root.songsSortDesc);
        if (added > 0)
            root.queue.playAt(0);
    }

    function clearSelection() {
        return songsSelection.clear() || facetSelection.clear() || albumDetail.clearSelection() || artistDetail.clearSelection();
    }

    TrackListSelection {
        id: songsSelection
    }

    TrackListSelection {
        id: facetSelection
    }

    function sortSongs(key, descending) {
        root.songsSort = key;
        root.songsSortDesc = descending;
        songs.setSort(key);
        songs.setSortDescending(descending);
        root.library.setSongsSort(key);
        root.library.setSongsSortDescending(descending);
        songsSelection.clear();
    }

    function sortAlbums(key, descending) {
        root.albumsSort = key;
        root.albumsSortDesc = descending;
        albums.setSort(key);
        albums.setSortDescending(descending);
        root.library.setAlbumsSort(key);
        root.library.setAlbumsSortDescending(descending);
    }

    function sortArtists(key, descending) {
        root.artistsSort = key;
        root.artistsSortDesc = descending;
        artists.setSort(key);
        artists.setSortDescending(descending);
        root.library.setArtistsSort(key);
        root.library.setArtistsSortDescending(descending);
    }

    function showTab(key) {
        if (root.page !== "browse") {
            root.page = "browse";
            root.previousPage = "browse";
            root.albumId = -1;
            root.albumTitle = "";
            root.facetName = "";
        }
        root.tab = key;
        root.clearSelection();
    }

    function applyRestoredPrefs() {
        const tab = root.library.libraryTab();
        if (tab === "albums" || tab === "artists" || tab === "folders" || tab === "songs" || tab === "genres" || tab === "composers")
            root.showTab(tab);

        const songsKey = root.library.songsSort();
        root.sortSongs(songsKey !== "" ? songsKey : "title", root.library.songsSortDescending());

        const albumsKey = root.library.albumsSort();
        root.sortAlbums(albumsKey !== "" ? albumsKey : "title", root.library.albumsSortDescending());

        const artistsKey = root.library.artistsSort();
        root.sortArtists(artistsKey !== "" ? artistsKey : "name", root.library.artistsSortDescending());
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
        trackMenu.trackIds = songsSelection.contains(at) ? songsSelection.mimeIds(songs) : "";
        trackMenu.popup();
    }

    function openFacetMenu(at) {
        if (at < 0 || at >= facetTracksView.count)
            return;

        facetTracksView.currentIndex = at;
        trackMenu.trackId = songs.trackIdAt(at);
        trackMenu.trackIds = facetSelection.contains(at) ? facetSelection.mimeIds(songs) : "";
        trackMenu.popup();
    }

    function drillIntoAlbum(id, title) {
        root.openAlbum(id);
    }

    function openAlbum(id) {
        root.previousPage = root.page;
        root.albumId = id;
        root.albumTitle = root.library.albumTitle(id);
        albumDetail.albumId = id;
        albumDetail.titleText = root.albumTitle;
        albumDetail.artistText = root.library.albumArtist(id);
        albumDetail.year = root.library.albumYear(id);
        albumDetail.trackCount = root.library.albumTrackCount(id);
        albumDetail.durationMs = root.library.albumDurationMs(id);
        albumDetail.load();
        root.page = "album";
    }

    function openArtist(name) {
        root.previousPage = root.page === "album" ? "browse" : root.page;
        artistDetail.artistName = name;
        artistDetail.albumCount = root.library.artistAlbumCount(name);
        artistDetail.trackCount = root.library.artistTrackCount(name);
        artistDetail.load();
        root.page = "artist";
    }

    function openGenre(name) {
        root.previousPage = "browse";
        root.facetName = name;
        songs.refreshGenre(name);
        root.page = "genre";
    }

    function openComposer(name) {
        root.previousPage = "browse";
        root.facetName = name;
        songs.refreshComposer(name);
        root.page = "composer";
    }

    function openFacetName(name) {
        if (root.tab === "composers")
            root.openComposer(name);
        else
            root.openGenre(name);
    }

    function openFacetAt(at) {
        const model = root.tab === "composers" ? composers : genres;
        root.openFacetName(model.nameAt(at));
    }

    function goBack() {
        if (root.page === "album" && root.previousPage === "artist") {
            root.page = "artist";
            root.previousPage = "browse";
            root.albumId = -1;
            root.albumTitle = "";
            return true;
        }
        if (root.page === "album" || root.page === "artist" || root.page === "genre" || root.page === "composer") {
            root.page = "browse";
            root.previousPage = "browse";
            root.albumId = -1;
            root.albumTitle = "";
            root.facetName = "";
            if (root.tab === "songs")
                songs.refreshSongs();
            return true;
        }
        if (root.folderDrilled) {
            root.leaveFolderDrill();
            return true;
        }
        return false;
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
        genres.refreshGenres();
        composers.refreshComposers();
    }

    onTabChanged: {
        if (root.tab !== "songs" && root.albumDrilled)
            root.leaveDrillKeepTab();

        if (root.tab !== "folders" && root.folderDrilled)
            root.leaveFolderDrillKeepTab();

        if (root.tab === "songs" && !root.albumDrilled && !root.folderDrilled)
            songs.refreshSongs();

        if (root.tab === "songs" || root.tab === "albums" || root.tab === "artists" || root.tab === "folders" || root.tab === "genres" || root.tab === "composers")
            root.library.setLibraryTab(root.tab);
    }
    anchors.fill: parent
    Component.onCompleted: {
        artists.refresh();
        albums.refresh();
        songs.refresh();
        folders.refresh();
        genres.refreshGenres();
        composers.refreshComposers();
    }

    ArtistListModel {
        id: artists
    }

    AlbumListModel {
        id: albums
    }

    // Covers land one row at a time; this pump publishes them and stops
    // itself as soon as the resolver runs dry.
    ArtPump {
        id: artPump

        models: [albums]
    }

    LibraryTrackModel {
        id: songs
    }

    FolderListModel {
        id: folders
    }

    FacetListModel {
        id: genres
    }

    FacetListModel {
        id: composers
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

        PageBanner {
            width: parent.width
            title: root.tabLabel(root.tab)
            source: root.bannerSource()
        }

        // Play all is the one primary on browse; Sort By is secondary and
        // trailing. Keys live in separate menus so hidden items cannot
        // punch empty rows through the popup.
        Item {
            id: sortRow

            readonly property bool showSongsSort: (root.tab === "songs" && !root.albumDrilled) || (root.tab === "folders" && root.folderDrilled)
            readonly property bool showAlbumsSort: root.tab === "albums"
            readonly property bool showArtistsSort: root.tab === "artists"
            readonly property bool showSort: showSongsSort || showAlbumsSort || showArtistsSort
            readonly property bool canPlayAll: {
                if (root.tab === "songs" || (root.tab === "folders" && root.folderDrilled))
                    return songsView.count > 0;
                if (root.tab === "albums")
                    return albumsView.count > 0;
                if (root.tab === "artists")
                    return artistsView.count > 0;
                if (root.tab === "folders")
                    return foldersView.count > 0;
                return facetsView.count > 0;
            }
            // Scope confirmation for the destructive-by-replacement Play
            // all: the count behind it, per tab.
            readonly property int playAllCount: {
                if (root.tab === "songs" || (root.tab === "folders" && root.folderDrilled))
                    return songsView.count;
                if (root.tab === "albums")
                    return albumsView.count;
                if (root.tab === "artists")
                    return artistsView.count;
                if (root.tab === "folders")
                    return foldersView.count;
                return facetsView.count;
            }

            visible: !root.libraryEmpty && root.showingBrowse
            width: parent.width
            height: visible ? playAllButton.height : 0

            PrimaryButton {
                id: playAllButton

                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                glyph: "play"
                text: qsTr("Play all (%1)").arg(sortRow.playAllCount)
                enabled: sortRow.canPlayAll
                Accessible.name: qsTr("Play all %1 shown items").arg(sortRow.playAllCount)
                onClicked: root.playAll()
            }

            PrimaryButton {
                id: sortButton

                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                visible: sortRow.showSort
                primary: false
                glyph: "chevron-down"
                glyphTrailing: true
                text: root.sortLabel()
                Accessible.name: root.sortAccessibleLabel()
                onClicked: {
                    if (sortRow.showAlbumsSort)
                        albumsSortMenu.popup(sortButton, 0, sortButton.height);
                    else if (sortRow.showArtistsSort)
                        artistsSortMenu.popup(sortButton, 0, sortButton.height);
                    else
                        songsSortMenu.popup(sortButton, 0, sortButton.height);
                }
            }

            GlassMenu {
                id: songsSortMenu

                GlassMenuItem {
                    text: qsTr("Title")
                    checkable: true
                    checked: root.songsSort === "title"
                    onTriggered: root.sortSongs("title", root.songsSortDesc)
                }

                GlassMenuItem {
                    text: qsTr("Artist")
                    checkable: true
                    checked: root.songsSort === "artist"
                    onTriggered: root.sortSongs("artist", root.songsSortDesc)
                }

                GlassMenuItem {
                    text: qsTr("Album")
                    checkable: true
                    checked: root.songsSort === "album"
                    onTriggered: root.sortSongs("album", root.songsSortDesc)
                }

                GlassMenuItem {
                    text: qsTr("Date")
                    checkable: true
                    checked: root.songsSort === "date"
                    onTriggered: root.sortSongs("date", root.songsSortDesc)
                }

                GlassMenuSeparator {}

                GlassMenuItem {
                    text: qsTr("Ascending")
                    checkable: true
                    checked: !root.songsSortDesc
                    onTriggered: root.sortSongs(root.songsSort, false)
                }

                GlassMenuItem {
                    text: qsTr("Descending")
                    checkable: true
                    checked: root.songsSortDesc
                    onTriggered: root.sortSongs(root.songsSort, true)
                }
            }

            GlassMenu {
                id: albumsSortMenu

                GlassMenuItem {
                    text: qsTr("Title")
                    checkable: true
                    checked: root.albumsSort === "title"
                    onTriggered: root.sortAlbums("title", root.albumsSortDesc)
                }

                GlassMenuItem {
                    text: qsTr("Artist")
                    checkable: true
                    checked: root.albumsSort === "artist"
                    onTriggered: root.sortAlbums("artist", root.albumsSortDesc)
                }

                GlassMenuItem {
                    text: qsTr("Date")
                    checkable: true
                    checked: root.albumsSort === "date"
                    onTriggered: root.sortAlbums("date", root.albumsSortDesc)
                }

                GlassMenuSeparator {}

                GlassMenuItem {
                    text: qsTr("Ascending")
                    checkable: true
                    checked: !root.albumsSortDesc
                    onTriggered: root.sortAlbums(root.albumsSort, false)
                }

                GlassMenuItem {
                    text: qsTr("Descending")
                    checkable: true
                    checked: root.albumsSortDesc
                    onTriggered: root.sortAlbums(root.albumsSort, true)
                }
            }

            GlassMenu {
                id: artistsSortMenu

                GlassMenuItem {
                    text: qsTr("Name")
                    checkable: true
                    checked: root.artistsSort === "name"
                    onTriggered: root.sortArtists("name", root.artistsSortDesc)
                }

                GlassMenuItem {
                    text: qsTr("Tracks")
                    checkable: true
                    checked: root.artistsSort === "songs"
                    onTriggered: root.sortArtists("songs", root.artistsSortDesc)
                }

                GlassMenuSeparator {}

                GlassMenuItem {
                    text: qsTr("Ascending")
                    checkable: true
                    checked: !root.artistsSortDesc
                    onTriggered: root.sortArtists(root.artistsSort, false)
                }

                GlassMenuItem {
                    text: qsTr("Descending")
                    checkable: true
                    checked: root.artistsSortDesc
                    onTriggered: root.sortArtists(root.artistsSort, true)
                }
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
                    root.queue.enqueueFolder(root.folderPath, root.songsSort, root.songsSortDesc);
                    root.queue.playAt(0);
                }
            }

            PrimaryButton {
                id: queueFolderButton

                primary: false
                text: qsTr("Queue folder")
                Accessible.name: qsTr("Queue this folder in Now Playing")
                onClicked: root.queue.enqueueFolder(root.folderPath, root.songsSort, root.songsSortDesc)
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
            ScrollBar.vertical: ListScrollBar {}

            visible: root.showingBrowse && ((root.tab === "songs") || (root.tab === "folders" && root.folderDrilled)) && !root.libraryEmpty
            anchors.fill: parent
            model: songs
            focus: root.showingBrowse && ((root.tab === "songs") || (root.tab === "folders" && root.folderDrilled)) && !root.libraryEmpty
            activeFocusOnTab: true
            clip: true
            spacing: Theme.listRowGap
            highlightMoveDuration: Appearance.duration(Theme.motionHover)
            header: SelectionBar {
                width: songsView.width
                queue: root.queue
                playlists: root.playlists
                count: songsSelection.count
                trackIds: songsSelection.mimeIds(songs)
                onCleared: songsSelection.clear()
            }
            Accessible.role: Accessible.List
            Accessible.selectable: true
            Accessible.name: root.albumDrilled ? root.albumTitle : (root.folderDrilled ? root.folderTitle : qsTr("Tracks"))
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
                dragTrackIds: selected && songsSelection.mimeIds(songs) !== "" ? songsSelection.mimeIds(songs) : String(model.trackId)
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
                    trackMenu.trackIds = songsSelection.contains(index) ? songsSelection.mimeIds(songs) : "";
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
                    text: qsTr("Showing the first 500 tracks — search finds the rest.")
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
            ScrollBar.vertical: ListScrollBar {}

            visible: root.showingBrowse && root.tab === "albums" && !root.libraryEmpty
            anchors.fill: parent
            model: albums
            focus: root.showingBrowse && root.tab === "albums" && !root.libraryEmpty
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
            ScrollBar.vertical: ListScrollBar {}

            visible: root.showingBrowse && root.tab === "artists" && !root.libraryEmpty
            anchors.fill: parent
            model: artists
            focus: root.showingBrowse && root.tab === "artists" && !root.libraryEmpty
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
                    root.openArtist(artists.nameAt(at));
            }
            Keys.onEnterPressed: {
                const at = artistsView.currentIndex >= 0 ? artistsView.currentIndex : 0;
                if (at < artistsView.count)
                    root.openArtist(artists.nameAt(at));
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
                onActivated: name => root.openArtist(name)
            }
        }

        // Folders tab: browse indexed tracks by parent directory.
        ListView {
            id: foldersView
            ScrollBar.vertical: ListScrollBar {}

            visible: root.showingBrowse && root.tab === "folders" && !root.folderDrilled && !root.libraryEmpty
            anchors.fill: parent
            model: folders
            focus: root.showingBrowse && root.tab === "folders" && !root.folderDrilled && !root.libraryEmpty
            activeFocusOnTab: true
            clip: true
            spacing: Theme.listRowGap
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
                readonly property string countLine: Format.plural(model.trackCount, qsTr("1 song"), qsTr("%1 songs"))

                width: ListView.view.width
                height: Theme.trackRowHeight
                Accessible.role: Accessible.ListItem
                Accessible.name: qsTr("%1, %2").arg(model.name).arg(folderRow.countLine)
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
                    iconSize: Theme.navIconSize
                    stroke: Theme.muted
                }

                Column {
                    anchors.left: folderGlyph.right
                    anchors.leftMargin: Theme.spaceSm
                    anchors.right: parent.right
                    anchors.rightMargin: Theme.spaceMd
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 0

                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: model.name
                        textFormat: Text.PlainText
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontBody
                        lineHeight: Theme.listLineHeight
                        color: Theme.foreground
                    }

                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: folderRow.countLine + " · " + model.path
                        textFormat: Text.PlainText
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontBodySm
                        lineHeight: Theme.listLineHeight
                        color: Theme.muted
                    }
                }
            }
        }

        ListView {
            id: facetsView
            ScrollBar.vertical: ListScrollBar {}

            visible: root.showingBrowse && (root.tab === "genres" || root.tab === "composers") && !root.libraryEmpty
            anchors.fill: parent
            model: root.tab === "composers" ? composers : genres
            focus: root.showingBrowse && (root.tab === "genres" || root.tab === "composers") && !root.libraryEmpty
            activeFocusOnTab: true
            clip: true
            spacing: Theme.listRowGap
            highlightMoveDuration: Appearance.duration(Theme.motionHover)
            Accessible.role: Accessible.List
            Accessible.name: root.tab === "composers" ? qsTr("Composers") : qsTr("Genres")
            Keys.onReturnPressed: {
                const at = facetsView.currentIndex >= 0 ? facetsView.currentIndex : 0;
                if (at < facetsView.count)
                    root.openFacetAt(at);
            }
            Keys.onEnterPressed: {
                const at = facetsView.currentIndex >= 0 ? facetsView.currentIndex : 0;
                if (at < facetsView.count)
                    root.openFacetAt(at);
            }
            Keys.onPressed: event => {
                if (event.key === Qt.Key_J) {
                    root.moveList(facetsView, 1);
                    event.accepted = true;
                } else if (event.key === Qt.Key_K) {
                    root.moveList(facetsView, -1);
                    event.accepted = true;
                }
            }

            highlight: Rectangle {
                color: Theme.selected
                radius: Theme.radiusSm
            }

            delegate: Item {
                id: facetRow

                readonly property string countLine: Format.plural(model.trackCount, qsTr("1 song"), qsTr("%1 songs"))

                width: ListView.view.width
                height: Theme.trackRowHeight
                Accessible.role: Accessible.ListItem
                Accessible.name: qsTr("%1, %2").arg(model.name).arg(facetRow.countLine)
                Accessible.onPressAction: root.openFacetName(model.name)

                Rectangle {
                    anchors.fill: parent
                    radius: Theme.radiusSm
                    color: facetMouse.containsMouse || facetRow.activeFocus ? Theme.hover : "transparent"

                    Behavior on color {
                        ColorAnimation {
                            duration: Appearance.duration(Theme.motionHover)
                            easing.type: Easing.OutCubic
                        }
                    }
                }

                MouseArea {
                    id: facetMouse

                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        facetsView.currentIndex = index;
                        root.openFacetName(model.name);
                    }
                }

                Icon {
                    id: facetGlyph

                    anchors.left: parent.left
                    anchors.leftMargin: Theme.spaceMd
                    anchors.verticalCenter: parent.verticalCenter
                    name: root.tab === "composers" ? "pen" : "tag"
                    iconSize: Theme.navIconSize
                    stroke: Theme.muted
                }

                Column {
                    anchors.left: facetGlyph.right
                    anchors.leftMargin: Theme.spaceSm
                    anchors.right: parent.right
                    anchors.rightMargin: Theme.spaceMd
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 0

                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: model.name
                        textFormat: Text.PlainText
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontBody
                        lineHeight: Theme.listLineHeight
                        color: Theme.foreground
                    }

                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: facetRow.countLine
                        textFormat: Text.PlainText
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontBodySm
                        lineHeight: Theme.listLineHeight
                        color: Theme.muted
                    }
                }
            }
        }
    }

    AlbumDetailView {
        id: albumDetail

        visible: root.page === "album"
        z: 1
        queue: root.queue
        playlists: root.playlists
        library: root.library
        onBackRequested: root.goBack()
        onAlbumRequested: id => root.openAlbum(id)
        onArtistRequested: name => root.openArtist(name)
    }

    ArtistDetailView {
        id: artistDetail

        visible: root.page === "artist"
        z: 1
        queue: root.queue
        playlists: root.playlists
        library: root.library
        onBackRequested: root.goBack()
        onAlbumRequested: id => root.openAlbum(id)
    }

    Item {
        id: facetDetail

        visible: root.page === "genre" || root.page === "composer"
        z: 1
        anchors.fill: parent

        Column {
            anchors.fill: parent
            anchors.margins: Theme.spaceLg
            spacing: Theme.spaceMd

            Row {
                width: parent.width
                spacing: Theme.spaceSm

                IconButton {
                    iconName: "chevron-left"
                    accessibleName: qsTr("Back")
                    onActivated: root.goBack()
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - Theme.targetMin - playFacetButton.width - Theme.spaceSm * 2
                    elide: Text.ElideRight
                    text: root.facetName
                    textFormat: Text.PlainText
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontHeadline
                    font.weight: Font.Bold
                    color: Theme.foreground
                }

                PrimaryButton {
                    id: playFacetButton

                    anchors.verticalCenter: parent.verticalCenter
                    glyph: "play"
                    text: qsTr("Play all (%1)").arg(facetTracksView.count)
                    enabled: facetTracksView.count > 0
                    Accessible.name: qsTr("Play all in %1, %2 items").arg(root.facetName).arg(facetTracksView.count)
                    onClicked: root.playFacet()
                }
            }

            ListView {
                id: facetTracksView
                ScrollBar.vertical: ListScrollBar {}

                width: parent.width
                height: parent.height - Theme.fontHeadline - Theme.spaceMd * 2
                model: songs
                clip: true
                spacing: Theme.listRowGap
                activeFocusOnTab: true
                highlightMoveDuration: Appearance.duration(Theme.motionHover)
                header: SelectionBar {
                    width: facetTracksView.width
                    queue: root.queue
                    playlists: root.playlists
                    count: facetSelection.count
                    trackIds: facetSelection.mimeIds(songs)
                    onCleared: facetSelection.clear()
                }
                Accessible.role: Accessible.List
                Accessible.selectable: true
                Accessible.name: root.facetName
                Keys.onReturnPressed: {
                    const at = facetTracksView.currentIndex >= 0 ? facetTracksView.currentIndex : 0;
                    if (at < facetTracksView.count && songs.isPlayableAt(at))
                        root.queue.playTrackNow(songs.trackIdAt(at));
                }
                Keys.onEnterPressed: {
                    const at = facetTracksView.currentIndex >= 0 ? facetTracksView.currentIndex : 0;
                    if (at < facetTracksView.count && songs.isPlayableAt(at))
                        root.queue.playTrackNow(songs.trackIdAt(at));
                }
                Keys.onPressed: event => {
                    if ((event.modifiers & Qt.ControlModifier) && event.key === Qt.Key_A) {
                        facetSelection.selectAll(facetTracksView.count);
                        event.accepted = true;
                        return;
                    }
                    if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier))) {
                        root.openFacetMenu(facetTracksView.currentIndex >= 0 ? facetTracksView.currentIndex : 0);
                        event.accepted = true;
                    }
                }
                Keys.onUpPressed: event => {
                    if (event.modifiers & Qt.ShiftModifier) {
                        const next = Math.max(0, facetTracksView.currentIndex - 1);
                        facetTracksView.currentIndex = next;
                        facetSelection.setRange(next);
                        event.accepted = true;
                    }
                }
                Keys.onDownPressed: event => {
                    if (event.modifiers & Qt.ShiftModifier) {
                        const next = Math.min(facetTracksView.count - 1, facetTracksView.currentIndex + 1);
                        facetTracksView.currentIndex = next;
                        facetSelection.setRange(next);
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
                        facetSelection.stamp;
                        return facetSelection.contains(index);
                    }
                    dragTrackIds: selected && facetSelection.mimeIds(songs) !== "" ? facetSelection.mimeIds(songs) : String(model.trackId)
                    onPlayRequested: (trackId, rowIndex, dangling) => {
                        facetSelection.clear();
                        facetTracksView.currentIndex = index;
                        if (!dangling && songs.isPlayableAt(index))
                            root.queue.playTrackNow(trackId);
                    }
                    onMenuRequested: (trackId, rowIndex, dangling) => {
                        facetTracksView.currentIndex = index;
                        trackMenu.trackId = trackId;
                        trackMenu.trackIds = facetSelection.contains(index) ? facetSelection.mimeIds(songs) : "";
                        trackMenu.popup();
                    }
                    onToggleSelectRequested: row => {
                        facetTracksView.currentIndex = row;
                        facetSelection.toggle(row);
                    }
                    onRangeSelectRequested: row => {
                        facetTracksView.currentIndex = row;
                        facetSelection.setRange(row);
                    }
                }
            }
        }
    }
}
