import QtQuick
import TuneX

// Artist landing page: header with play, album grid, then tracks.
Item {
    id: root

    required property QueueModel queue
    required property PlaylistModel playlists
    required property LibraryManager library
    property string artistName: ""
    property int albumCount: 0
    property int trackCount: 0
    readonly property string shownName: root.artistName !== "" ? root.artistName : qsTr("Unknown Artist")
    readonly property string monogram: {
        const words = root.shownName.split(/\s+/).filter(function (word) {
            return word.length > 0;
        });
        const letters = words.slice(0, 2).map(function (word) {
            return word[0].toUpperCase();
        });
        return letters.join("");
    }
    readonly property string albumsLine: root.albumCount === 1 ? qsTr("1 album") : qsTr("%1 albums").arg(root.albumCount)
    readonly property string songsLine: root.trackCount === 1 ? qsTr("1 song") : qsTr("%1 songs").arg(root.trackCount)
    readonly property int gridCell: Math.max(Theme.gridMin, Math.floor((width - Theme.spaceLg * 2) / Math.max(1, Math.floor((width - Theme.spaceLg * 2) / Theme.gridTarget))))

    signal backRequested
    signal albumRequested(int albumId)

    function playArtist() {
        root.queue.clearQueue();
        root.queue.enqueueArtist(root.artistName);
        root.queue.playAt(0);
    }

    function load() {
        albums.refreshForArtist(root.artistName, -1);
        songs.refreshArtist(root.artistName);
        artPump.start();
    }

    function openSongMenu(at) {
        if (at < 0 || at >= tracksView.count)
            return;
        tracksView.currentIndex = at;
        trackMenu.trackId = songs.trackIdAt(at);
        trackMenu.popup();
    }

    anchors.fill: parent

    AlbumListModel {
        id: albums
    }

    LibraryTrackModel {
        id: songs
    }

    Timer {
        id: artPump

        interval: 120
        repeat: true
        onTriggered: {
            if (!albums.pollArt())
                artPump.stop();
        }
    }

    TrackMenu {
        id: trackMenu

        queue: root.queue
        playlists: root.playlists
        library: root.library
        onIndexChanged: songs.refreshArtist(root.artistName)
    }

    Flickable {
        id: scroller

        anchors.fill: parent
        clip: true
        contentWidth: width
        contentHeight: page.height
        boundsBehavior: Flickable.StopAtBounds

        Column {
            id: page

            x: Theme.spaceLg
            width: scroller.width - Theme.spaceLg * 2
            spacing: Theme.spaceMd

            Row {
                width: parent.width
                spacing: Theme.spaceSm

                IconButton {
                    iconName: "chevron-left"
                    accessibleName: qsTr("Back")
                    onActivated: root.backRequested()
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: qsTr("Artist")
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontCaption
                    color: Theme.muted
                }
            }

            Row {
                width: parent.width
                spacing: Theme.spaceLg

                Artwork {
                    width: Theme.emptyArt
                    height: Theme.emptyArt
                    monogram: root.monogram
                    radius: width / 2
                    monogramSize: Theme.fontHeadline
                }

                Column {
                    width: parent.width - Theme.emptyArt - Theme.spaceLg
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: Theme.spaceSm

                    Text {
                        width: parent.width
                        wrapMode: Text.WordWrap
                        text: root.shownName
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontHeadline
                        font.weight: Font.DemiBold
                        color: Theme.foreground
                    }

                    Text {
                        text: root.albumsLine + " • " + root.songsLine
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontBody
                        color: Theme.muted
                    }

                    PrimaryButton {
                        glyph: "play"
                        text: qsTr("Play")
                        Accessible.name: qsTr("Play artist")
                        onClicked: root.playArtist()
                    }
                }
            }

            Text {
                visible: albumsView.count > 0
                text: qsTr("Albums")
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontTitle
                font.weight: Font.DemiBold
                color: Theme.foreground
            }

            GridView {
                id: albumsView

                visible: count > 0
                width: parent.width
                height: visible ? Math.ceil(count / Math.max(1, Math.floor(width / root.gridCell))) * (root.gridCell + Theme.cardMetaHeight) : 0
                cellWidth: root.gridCell
                cellHeight: cellWidth + Theme.cardMetaHeight
                model: albums
                interactive: false
                Accessible.role: Accessible.List
                Accessible.name: qsTr("Albums")

                delegate: AlbumCard {
                    albumId: model.albumId
                    title: model.title
                    artist: model.artist
                    year: model.year
                    trackCount: model.trackCount
                    artUrl: model.artUrl
                    Component.onCompleted: {
                        albums.requestArt(index);
                        artPump.start();
                    }
                    onActivated: id => root.albumRequested(id)
                }
            }

            Text {
                visible: tracksView.count > 0
                text: qsTr("Songs")
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontTitle
                font.weight: Font.DemiBold
                color: Theme.foreground
            }

            ListView {
                id: tracksView

                visible: count > 0
                width: parent.width
                height: visible ? count * Theme.trackRowHeight : 0
                model: songs
                interactive: false
                clip: true
                Accessible.role: Accessible.List
                Accessible.name: qsTr("Artist tracks")

                delegate: TrackRow {
                    trackId: model.trackId
                    title: model.title
                    artist: model.artist
                    trackNumber: model.trackNumber
                    durationMs: model.durationMs
                    missing: model.missing
                    onPlayRequested: (trackId, rowIndex, dangling) => {
                        if (!dangling && songs.isPlayableAt(index))
                            root.queue.playTrackNow(trackId);
                    }
                    onMenuRequested: (trackId, rowIndex, dangling) => root.openSongMenu(index)
                }
            }
        }
    }
}
