import QtQuick
import QtQuick.Controls.Basic
import TuneX 1.0

// LibraryView (S2 W-016): browse the indexed library — Songs list, Albums
// grid, Artists grid. Models load from the index on completion; a missing
// index shows the empty state (never an error). Album cards drill into the
// songs tab filtered to that album; artist drill-down arrives with S3.
// Scan triggering, progress, and artwork warming arrive with W-017, which
// re-calls these same refresh() entry points when a scan completes.
Item {
    id: root

    anchors.fill: parent

    // Album drill-down: -1 means the full songs tab.
    property int albumId: -1
    property string albumTitle: ""
    property string tab: "songs"
    readonly property bool drilled: root.albumId >= 0
    // View counts (the models expose rows, not a count property).
    readonly property bool libraryEmpty: songsView.count === 0 && albumsView.count === 0 && artistsView.count === 0
    // Fluid artwork columns shared by both grids (160–220px cards).
    readonly property int gridCell: Math.max(160, Math.floor(content.width / Math.max(1, Math.floor(content.width / 190))))

    function drillIntoAlbum(id, title) {
        root.albumId = id;
        root.albumTitle = title;
        songs.refreshAlbum(id);
        root.tab = "songs";
    }

    function leaveDrill() {
        root.albumId = -1;
        root.albumTitle = "";
        songs.refresh();
        root.tab = "albums";
    }

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

    LibraryTrackModel {
        id: songs
    }

    Column {
        anchors.fill: parent
        anchors.margins: Theme.spaceLg
        spacing: Theme.spaceMd

        // Tab strip: three choices (Hick-compliant), text labels (abstract
        // concepts read better than icons alone).
        Row {
            id: tabRow

            spacing: Theme.spaceXs

            Repeater {
                model: [{
                    "key": "songs",
                    "label": qsTr("Songs")
                }, {
                    "key": "albums",
                    "label": qsTr("Albums")
                }, {
                    "key": "artists",
                    "label": qsTr("Artists")
                }]

                Button {
                    required property var modelData

                    text: modelData.label
                    checkable: true
                    checked: root.tab === modelData.key
                    Accessible.name: modelData.label
                    onClicked: root.tab = modelData.key
                }

            }

        }

        // Drill header: album title plus the way back. Collapsed (zero
        // height) when not drilling — positioners keep invisible space.
        // Hidden while the library is empty (a drill cannot outlive its rows).
        Row {
            id: drillRow

            visible: root.drilled && root.tab === "songs" && !root.libraryEmpty
            width: parent.width
            height: visible ? implicitHeight : 0
            clip: true
            spacing: Theme.spaceSm

            Button {
                id: drillBack

                text: qsTr("Back to albums")
                onClicked: root.leaveDrill()
            }

            Text {
                width: parent.width - drillBack.width - Theme.spaceSm
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
                note: qsTr("Scan a music folder and your artists, albums, and songs will appear here.")
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
                highlightMoveDuration: 120
                Accessible.role: Accessible.List
                Accessible.name: root.drilled ? root.albumTitle : qsTr("Songs")

                highlight: Rectangle {
                    color: Theme.selected
                    radius: Theme.radiusSm
                }

                delegate: TrackRow {
                    title: model.title
                    artist: model.artist
                    trackNumber: model.trackNumber
                    durationMs: model.durationMs
                    missing: model.missing
                }

                footer: Text {
                    visible: !root.drilled && songsView.count >= 500
                    width: songsView.width
                    height: visible ? implicitHeight : 0
                    horizontalAlignment: Text.AlignHCenter
                    text: qsTr("Showing the first 500 songs — search arrives in S3.")
                    textFormat: Text.PlainText
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontCaption
                    color: Theme.muted
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
                highlightMoveDuration: 120
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
                    cardIndex: index
                    onActivated: (id) => {
                        albumsView.currentIndex = cardIndex;
                        root.drillIntoAlbum(id, title);
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
                highlight: Rectangle {
                    color: "transparent"
                    radius: Theme.radiusMd
                    border.color: artistsView.activeFocus ? Theme.focus : "transparent"
                    border.width: 2
                }
                highlightMoveDuration: 120
                Accessible.role: Accessible.List
                Accessible.name: qsTr("Artists")

                delegate: ArtistCard {
                    artistName: model.name
                    albumCount: model.albumCount
                    trackCount: model.trackCount
                }

            }

        }

    }

}
