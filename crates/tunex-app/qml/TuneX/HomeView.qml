import QtQuick
import TuneX

// Home (DESIGN_BRIEF + IA): hero, library-backed chips, album rail,
// playlist rail, library shortcuts. Real models only — no invented rows.
Item {
    id: root

    required property QueueModel queue
    required property PlaylistModel playlists
    required property LibraryManager library
    property string chipKey: "all"
    property bool canContinue: false
    // The rail counts even while hidden; a delegate-less counter view would
    // not (QQmlDelegateModel reports 0 rows without a delegate).
    readonly property bool libraryEmpty: albumRail.count === 0
    // Rails fill their row with whole cards rather than cutting the last one
    // in half at the panel edge: pick the count that lands nearest the target
    // card size, then divide the row between them.
    readonly property int contentWidth: root.width - Theme.spaceLg * 2
    readonly property int railColumns: Math.max(2, Math.round(root.contentWidth / 196))
    readonly property int railCell: Math.floor((root.contentWidth - Theme.spaceMd * (root.railColumns - 1)) / root.railColumns)
    // Playlist rail: wide tiles, about four across like the mockup's second
    // rail, and never so narrow that a name has no room.
    readonly property int playlistCellHeight: 76
    readonly property int playlistColumns: Math.max(2, Math.min(4, Math.floor(root.contentWidth / 220)))
    readonly property int playlistCell: Math.floor((root.contentWidth - Theme.spaceMd * (root.playlistColumns - 1)) / root.playlistColumns)

    signal browseRequested(string tab)
    signal settingsRequested(bool pickFolder)
    signal playlistsRequested
    signal playlistOpened(int playlistId, string name)

    function greetingStatus() {
        if (root.library.isScanning())
            return root.library.statusText();
        return "";
    }

    function playSomething() {
        if (root.canContinue) {
            root.queue.playPause();
            return;
        }
        root.queue.clearQueue();
        for (let i = 0; i < 40; i++) {
            const id = songs.trackIdAt(i);
            if (id < 0)
                break;
            root.queue.enqueueTrack(id);
        }
        if (!root.queue.isShuffle())
            root.queue.toggleShuffle();
        root.queue.playAt(0);
    }

    function refresh() {
        albums.refresh();
        songs.refresh();
        playlists.refresh();
    }

    function activateChip(key) {
        root.chipKey = key;
        if (key === "all")
            return;
        if (key === "folders") {
            root.browseRequested("folders");
            return;
        }
        if (key === "playlists") {
            root.playlistsRequested();
            return;
        }
        root.browseRequested(key);
    }

    anchors.fill: parent
    Component.onCompleted: root.refresh()

    AlbumListModel {
        id: albums
    }

    // Covers land one row at a time; stops itself when none are pending.
    Timer {
        id: homeArtPump

        interval: 120
        repeat: true
        onTriggered: {
            if (!albums.pollArt())
                homeArtPump.stop();
        }
    }

    LibraryTrackModel {
        id: songs
    }

    Flickable {
        id: scroller

        anchors.fill: parent
        clip: true
        contentWidth: width
        contentHeight: page.height
        boundsBehavior: Flickable.StopAtBounds
        Accessible.role: Accessible.Pane
        Accessible.name: qsTr("Home")

        Column {
            id: page

            x: Theme.spaceLg
            width: scroller.width - Theme.spaceLg * 2
            spacing: Theme.spaceLg

            HeroCard {
                width: parent.width
                libraryEmpty: root.libraryEmpty
                canContinue: root.canContinue
                statusLine: root.greetingStatus()
                onPlayRequested: root.playSomething()
                onAddFolderRequested: root.settingsRequested(true)
                onBrowseRequested: root.browseRequested("albums")
            }

            Row {
                width: parent.width
                spacing: Theme.spaceSm

                Chip {
                    label: qsTr("All")
                    selected: root.chipKey === "all"
                    onActivated: root.activateChip("all")
                }

                Chip {
                    label: qsTr("Artists")
                    selected: root.chipKey === "artists"
                    onActivated: root.activateChip("artists")
                }

                Chip {
                    label: qsTr("Albums")
                    selected: root.chipKey === "albums"
                    onActivated: root.activateChip("albums")
                }

                Chip {
                    label: qsTr("Songs")
                    selected: root.chipKey === "songs"
                    onActivated: root.activateChip("songs")
                }

                Chip {
                    label: qsTr("Playlists")
                    selected: root.chipKey === "playlists"
                    onActivated: root.activateChip("playlists")
                }

                Chip {
                    label: qsTr("Folders")
                    selected: root.chipKey === "folders"
                    onActivated: root.activateChip("folders")
                }
            }

            Column {
                width: parent.width
                spacing: Theme.spaceSm
                visible: !root.libraryEmpty

                Item {
                    width: parent.width
                    height: Theme.targetMin

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: qsTr("Recently Added")
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontTitle
                        font.weight: Font.DemiBold
                        color: Theme.foreground
                    }

                    TextLink {
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.right: parent.right
                        text: qsTr("See all")
                        accessibleName: qsTr("See all albums")
                        onActivated: root.browseRequested("albums")
                    }
                }

                ListView {
                    id: albumRail

                    width: parent.width
                    height: root.railCell + 64
                    orientation: ListView.Horizontal
                    clip: true
                    spacing: Theme.spaceMd
                    model: albums
                    boundsBehavior: Flickable.StopAtBounds
                    Accessible.role: Accessible.List
                    Accessible.name: qsTr("Recently Added")

                    delegate: AlbumCard {
                        albumId: model.albumId
                        title: model.title
                        artist: model.artist
                        year: model.year
                        trackCount: model.trackCount
                        artUrl: model.artUrl
                        explicitWidth: root.railCell
                        Component.onCompleted: {
                            albums.requestArt(index);
                            homeArtPump.start();
                        }
                        onActivated: id => {
                            root.queue.clearQueue();
                            root.queue.enqueueAlbum(id);
                            root.queue.playAt(0);
                        }
                    }
                }
            }

            Column {
                width: parent.width
                spacing: Theme.spaceSm

                Item {
                    width: parent.width
                    height: Theme.targetMin

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: qsTr("Playlists")
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontTitle
                        font.weight: Font.DemiBold
                        color: Theme.foreground
                    }

                    TextLink {
                        visible: playlistsRail.count > 0
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.right: parent.right
                        text: qsTr("See all")
                        accessibleName: qsTr("See all playlists")
                        onActivated: root.playlistsRequested()
                    }
                }

                Text {
                    visible: playlistsRail.count <= 0
                    width: parent.width
                    wrapMode: Text.WordWrap
                    text: qsTr("No playlists yet — create one from the rail.")
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontBody
                    color: Theme.muted
                }

                ListView {
                    id: playlistsRail

                    visible: count > 0
                    width: parent.width
                    height: visible ? root.playlistCellHeight : 0
                    orientation: ListView.Horizontal
                    clip: true
                    spacing: Theme.spaceMd
                    model: playlists
                    boundsBehavior: Flickable.StopAtBounds
                    Accessible.role: Accessible.List
                    Accessible.name: qsTr("Playlists")

                    delegate: PlaylistCard {
                        playlistId: model.playlistId
                        title: model.name
                        trackCount: model.trackCount
                        horizontal: true
                        horizontalHeight: root.playlistCellHeight
                        explicitWidth: root.playlistCell
                        onActivated: (id, name) => {
                            return root.playlistOpened(id, name);
                        }
                    }
                }
            }
        }
    }
}
