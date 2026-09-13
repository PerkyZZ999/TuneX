import QtQuick
import TuneX

// Album landing page: large art header, one primary Play, meta, virtualized
// tracks, then more albums by the same artist.
Item {
    id: root

    required property QueueModel queue
    required property PlaylistModel playlists
    required property LibraryManager library
    property int albumId: -1
    property string titleText: ""
    property string artistText: ""
    property int year: 0
    property int trackCount: 0
    property int durationMs: 0
    property url artUrl
    readonly property string shownTitle: root.titleText !== "" ? root.titleText : qsTr("Unknown Album")
    readonly property string shownArtist: root.artistText !== "" ? root.artistText : qsTr("Unknown Artist")
    readonly property string monogram: {
        const words = root.shownTitle.split(/\s+/).filter(function (word) {
            return word.length > 0;
        });
        const letters = words.slice(0, 2).map(function (word) {
            return word[0].toUpperCase();
        });
        return letters.join("");
    }
    readonly property string countLine: root.trackCount === 1 ? qsTr("1 song") : qsTr("%1 songs").arg(root.trackCount)
    readonly property string durationLine: root.durationMs > 0 ? root.formatTime(root.durationMs) : ""
    readonly property string metaLine: {
        const parts = [root.shownArtist];
        if (root.year > 0)
            parts.push(String(root.year));
        parts.push(root.countLine);
        if (root.durationLine !== "")
            parts.push(root.durationLine);
        return parts.join(" • ");
    }

    signal backRequested
    signal albumRequested(int albumId)
    signal artistRequested(string name)

    function formatTime(ms) {
        const total = Math.max(0, Math.floor(ms / 1000));
        const minutes = Math.floor(total / 60);
        const seconds = String(total % 60).padStart(2, "0");
        return minutes + ":" + seconds;
    }

    function playAlbum() {
        root.queue.clearQueue();
        root.queue.enqueueAlbum(root.albumId);
        root.queue.playAt(0);
    }

    function queueAlbum() {
        root.queue.enqueueAlbum(root.albumId);
    }

    function load() {
        songs.refreshAlbum(root.albumId);
        more.refreshForArtist(root.artistText, root.albumId);
        moreArtPump.start();
    }

    function openSongMenu(at) {
        if (at < 0 || at >= tracksView.count)
            return;
        tracksView.currentIndex = at;
        trackMenu.trackId = songs.trackIdAt(at);
        trackMenu.popup();
    }

    anchors.fill: parent

    LibraryTrackModel {
        id: songs
    }

    AlbumListModel {
        id: more
    }

    Timer {
        id: moreArtPump

        interval: 120
        repeat: true
        onTriggered: {
            if (!more.pollArt())
                moreArtPump.stop();
        }
    }

    TrackMenu {
        id: trackMenu

        queue: root.queue
        playlists: root.playlists
        library: root.library
        onIndexChanged: songs.refreshAlbum(root.albumId)
    }

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
                onActivated: root.backRequested()
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: qsTr("Album")
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontCaption
                color: Theme.muted
            }
        }

        Row {
            width: parent.width
            spacing: Theme.spaceLg
            height: Theme.nowPlayingArt

            Artwork {
                width: Theme.nowPlayingArt
                height: Theme.nowPlayingArt
                source: root.artUrl
                monogram: root.monogram
                radius: Theme.radiusMd
                monogramSize: Theme.fontHeadline
            }

            Column {
                width: parent.width - Theme.nowPlayingArt - Theme.spaceLg
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.spaceSm

                Text {
                    width: parent.width
                    wrapMode: Text.WordWrap
                    text: root.shownTitle
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontHeadline
                    font.weight: Font.DemiBold
                    color: Theme.foreground
                }

                Text {
                    width: parent.width
                    wrapMode: Text.WordWrap
                    text: root.metaLine
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontBody
                    color: Theme.muted

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.artistRequested(root.artistText)
                    }
                }

                Row {
                    spacing: Theme.spaceSm

                    PrimaryButton {
                        glyph: "play"
                        text: qsTr("Play")
                        Accessible.name: qsTr("Play album")
                        onClicked: root.playAlbum()
                    }

                    PrimaryButton {
                        primary: false
                        glyph: "list-music"
                        text: qsTr("Queue")
                        Accessible.name: qsTr("Queue album")
                        onClicked: root.queueAlbum()
                    }
                }
            }
        }

        ListView {
            id: tracksView

            width: parent.width
            height: parent.height - Theme.nowPlayingArt - Theme.targetMin * 2 - Theme.spaceMd * 6 - (moreView.visible ? moreView.height + Theme.spaceMd : 0)
            model: songs
            clip: true
            activeFocusOnTab: true
            highlightMoveDuration: Appearance.duration(Theme.motionHover)
            Accessible.role: Accessible.List
            Accessible.name: qsTr("Album tracks")
            Keys.onReturnPressed: {
                const at = tracksView.currentIndex >= 0 ? tracksView.currentIndex : 0;
                if (at < tracksView.count && songs.isPlayableAt(at))
                    root.queue.playTrackNow(songs.trackIdAt(at));
            }
            Keys.onEnterPressed: {
                const at = tracksView.currentIndex >= 0 ? tracksView.currentIndex : 0;
                if (at < tracksView.count && songs.isPlayableAt(at))
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
                    tracksView.currentIndex = index;
                    if (!dangling && songs.isPlayableAt(index))
                        root.queue.playTrackNow(trackId);
                }
                onMenuRequested: (trackId, rowIndex, dangling) => {
                    tracksView.currentIndex = index;
                    root.openSongMenu(index);
                }
            }
        }

        Text {
            visible: moreView.count > 0
            text: qsTr("More by %1").arg(root.shownArtist)
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontTitle
            font.weight: Font.DemiBold
            color: Theme.foreground
        }

        ListView {
            id: moreView

            visible: count > 0
            width: parent.width
            height: visible ? Theme.gridMin + Theme.cardMetaHeight : 0
            orientation: ListView.Horizontal
            clip: true
            spacing: Theme.spaceMd
            model: more
            boundsBehavior: Flickable.StopAtBounds
            Accessible.role: Accessible.List
            Accessible.name: qsTr("More by this artist")

            delegate: AlbumCard {
                albumId: model.albumId
                title: model.title
                artist: model.artist
                year: model.year
                trackCount: model.trackCount
                artUrl: model.artUrl
                explicitWidth: Theme.gridMin
                Component.onCompleted: {
                    more.requestArt(index);
                    moreArtPump.start();
                }
                onActivated: id => root.albumRequested(id)
            }
        }
    }
}
