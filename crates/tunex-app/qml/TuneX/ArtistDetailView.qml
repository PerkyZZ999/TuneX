import QtQuick
import QtQuick.Controls.Basic
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
    readonly property string shownName: Format.fallback(root.artistName, qsTr("Unknown Artist"))
    readonly property string monogram: Format.monogram(root.shownName)
    readonly property string albumsLine: Format.plural(root.albumCount, qsTr("1 album"), qsTr("%1 albums"))
    readonly property string songsLine: Format.plural(root.trackCount, qsTr("1 song"), qsTr("%1 songs"))
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
        trackSelection.clear();
    }

    function clearSelection() {
        return trackSelection.clear();
    }

    function openSongMenu(at) {
        if (at < 0 || at >= tracksView.count)
            return;
        tracksView.currentIndex = at;
        trackMenu.trackId = songs.trackIdAt(at);
        trackMenu.trackIds = trackSelection.contains(at) ? trackSelection.mimeIds(songs) : "";
        trackMenu.popup();
    }

    anchors.fill: parent

    AlbumListModel {
        id: albums
    }

    LibraryTrackModel {
        id: songs
    }

    TrackListSelection {
        id: trackSelection
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
        ScrollBar.vertical: ListScrollBar {}

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
                text: qsTr("Tracks")
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontTitle
                font.weight: Font.DemiBold
                color: Theme.foreground
            }

            ListView {
                id: tracksView

                visible: count > 0
                width: parent.width
                height: visible ? count * Theme.trackRowHeight + Math.max(0, count - 1) * Theme.listRowGap + (trackSelection.count > 0 ? Theme.targetMin : 0) : 0
                model: songs
                interactive: false
                clip: true
                spacing: Theme.listRowGap
                activeFocusOnTab: true
                header: SelectionBar {
                    width: tracksView.width
                    queue: root.queue
                    playlists: root.playlists
                    count: trackSelection.count
                    trackIds: trackSelection.mimeIds(songs)
                    onCleared: trackSelection.clear()
                }
                Accessible.role: Accessible.List
                Accessible.selectable: true
                Accessible.name: qsTr("Artist tracks")
                Keys.onPressed: event => {
                    if ((event.modifiers & Qt.ControlModifier) && event.key === Qt.Key_A) {
                        trackSelection.selectAll(tracksView.count);
                        event.accepted = true;
                        return;
                    }
                    if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier))) {
                        root.openSongMenu(tracksView.currentIndex >= 0 ? tracksView.currentIndex : 0);
                        event.accepted = true;
                    }
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
                        trackSelection.stamp;
                        return trackSelection.contains(index);
                    }
                    dragTrackIds: selected && trackSelection.mimeIds(songs) !== "" ? trackSelection.mimeIds(songs) : String(model.trackId)
                    onPlayRequested: (trackId, rowIndex, dangling) => {
                        trackSelection.clear();
                        if (!dangling && songs.isPlayableAt(index))
                            root.queue.playTrackNow(trackId);
                    }
                    onMenuRequested: (trackId, rowIndex, dangling) => root.openSongMenu(index)
                    onToggleSelectRequested: row => trackSelection.toggle(row)
                    onRangeSelectRequested: row => trackSelection.setRange(row)
                }
            }
        }
    }
}
