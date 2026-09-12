import QtQuick
import TuneX

// LibraryView (S2 W-016): browse the indexed library — Songs list, Albums
// grid, Artists grid. Models load from the index on completion; a missing
// index shows the empty state (never an error). Album cards drill into the
// songs tab filtered to that album; artist drill-down arrives in a later
// slice. Scan triggering, progress, and artwork warming arrived with W-017, which
// re-calls these same refresh() entry points when a scan completes.
Item {
    id: root

    required property QueueModel queue
    required property PlaylistModel playlists
    required property LibraryManager library
    // Album drill-down: -1 means the full songs tab.
    property int albumId: -1
    property string albumTitle: ""
    property string tab: "songs"
    readonly property bool drilled: root.albumId >= 0
    // View counts (the models expose rows, not a count property).
    readonly property bool libraryEmpty: songsView.count === 0 && albumsView.count === 0 && artistsView.count === 0
    // Fluid artwork columns shared by both grids (160–220px cards).
    readonly property int gridCell: Math.max(160, Math.floor(content.width / Math.max(1, Math.floor(content.width / 190))))

    signal foldersRequested
    // An artist card was activated: V1 has no artist detail view, so the
    // shell plays that artist's tracks.
    signal artistRequested(string name)

    // Display name for one tab key (the tab strip models keys, not labels).
    function tabLabel(key) {
        if (key === "albums")
            return qsTr("Albums");

        if (key === "artists")
            return qsTr("Artists");

        return qsTr("Songs");
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
        songs.refresh();
    }

    function refresh() {
        artists.refresh();
        albums.refresh();
        songs.refresh();
    }

    onTabChanged: {
        // A drill hides its Back button off the songs tab: unwind it when
        // the tab leaves (results resubmit; the target tab shows them).
        if (root.tab !== "songs" && root.drilled)
            root.leaveDrillKeepTab();
    }
    anchors.fill: parent
    Component.onCompleted: {
        artists.refresh();
        albums.refresh();
        songs.refresh();
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

    TrackMenu {
        id: trackMenu

        queue: root.queue
        playlists: root.playlists
    }

    Column {
        anchors.fill: parent
        anchors.margins: Theme.spaceLg
        spacing: Theme.spaceMd

        // Header: tab strip (three Hick-compliant text choices) plus the
        // folders entry on the trailing edge.
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
                    model: ["songs", "albums", "artists"]

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
                onClicked: root.foldersRequested()
            }
        }

        // Drill header: album title, queue actions, plus the way back.
        // Collapsed (zero height) when not drilling — positioners keep
        // invisible space. Hidden while the library is empty (a drill cannot
        // outlive its rows).
        Row {
            id: drillRow

            visible: root.drilled && root.tab === "songs" && !root.libraryEmpty
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

        Item {
            id: content

            width: parent.width
            height: parent.height - tabRow.height - drillRow.height - Theme.spaceMd * 2

            EmptyState {
                visible: root.libraryEmpty
                title: qsTr("No music yet")
                note: qsTr("Add a music folder and your artists, albums, and songs will appear here.")
                actionLabel: qsTr("Add music folder")
                onActionRequested: root.foldersRequested()
            }

            // Songs tab: virtualized list over the capped songs query.
            ListView {
                id: songsView

                visible: root.tab === "songs" && !root.libraryEmpty
                anchors.fill: parent
                model: songs
                focus: root.tab === "songs" && !root.libraryEmpty
                activeFocusOnTab: true
                clip: true
                highlightMoveDuration: Appearance.duration(Theme.motionHover)
                Accessible.role: Accessible.List
                Accessible.name: root.drilled ? root.albumTitle : qsTr("Songs")
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

                    readonly property bool shown: !root.drilled && songsView.count >= 500

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
                cellHeight: cellWidth + 64
                highlightMoveDuration: Appearance.duration(Theme.motionHover)
                Accessible.role: Accessible.List
                Accessible.name: qsTr("Albums")

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
                cellHeight: cellWidth + 64
                highlightMoveDuration: Appearance.duration(Theme.motionHover)
                Accessible.role: Accessible.List
                Accessible.name: qsTr("Artists")

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
        }
    }
}
