import QtQuick
import QtQuick.Controls.Basic
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
    readonly property string shownTitle: Format.fallback(root.titleText, qsTr("Unknown Album"))
    readonly property string shownArtist: Format.fallback(root.artistText, qsTr("Unknown Artist"))
    readonly property string monogram: Format.monogram(root.shownTitle)
    readonly property string countLine: Format.plural(root.trackCount, qsTr("1 song"), qsTr("%1 songs"))
    readonly property string durationLine: root.durationMs > 0 ? Format.duration(root.durationMs) : ""
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

    // Durations shape through the Format singleton.

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
        trackSelection.clear();
    }

    function clearSelection() {
        return trackSelection.clear();
    }

    anchors.fill: parent

    LibraryTrackModel {
        id: songs
    }

    TrackListSelection {
        id: trackSelection
    }

    AlbumListModel {
        id: more
    }

    ArtPump {
        id: moreArtPump

        models: [more]
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

                // The artist name is a real link: accent on hover/focus,
                // underline while keyboard-focused, reachable by Tab.
                Text {
                    id: artistLink

                    width: parent.width
                    wrapMode: Text.WordWrap
                    text: root.metaLine
                    textFormat: Text.PlainText
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontBody
                    font.underline: artistLink.activeFocus
                    color: artistMouse.containsMouse || artistLink.activeFocus ? Theme.accent : Theme.muted
                    activeFocusOnTab: true
                    Accessible.role: Accessible.Link
                    Accessible.name: root.artistText
                    Accessible.onPressAction: root.artistRequested(root.artistText)
                    Keys.onReturnPressed: root.artistRequested(root.artistText)
                    Keys.onEnterPressed: root.artistRequested(root.artistText)
                    Keys.onSpacePressed: root.artistRequested(root.artistText)

                    Behavior on color {
                        ColorAnimation {
                            duration: Appearance.duration(Theme.motionHover)
                            easing.type: Easing.OutCubic
                        }
                    }

                    MouseArea {
                        id: artistMouse

                        anchors.fill: parent
                        hoverEnabled: true
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
            ScrollBar.vertical: ListScrollBar {}

            width: parent.width
            height: parent.height - Theme.nowPlayingArt - Theme.targetMin * 2 - Theme.spaceMd * 6 - (moreView.visible ? moreView.height + Theme.spaceMd : 0)
            model: songs
            clip: true
            spacing: Theme.listRowGap
            activeFocusOnTab: true
            highlightMoveDuration: Appearance.duration(Theme.motionHover)
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
            Keys.onPressed: event => {
                if ((event.modifiers & Qt.ControlModifier) && event.key === Qt.Key_A) {
                    trackSelection.selectAll(tracksView.count);
                    event.accepted = true;
                    return;
                }
                if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier))) {
                    trackMenu.openFor(tracksView.currentIndex >= 0 ? tracksView.currentIndex : 0, tracksView, trackSelection, songs);
                    event.accepted = true;
                }
            }
            Keys.onUpPressed: event => {
                if (event.modifiers & Qt.ShiftModifier) {
                    const next = Math.max(0, tracksView.currentIndex - 1);
                    tracksView.currentIndex = next;
                    trackSelection.setRange(next);
                    event.accepted = true;
                }
            }
            Keys.onDownPressed: event => {
                if (event.modifiers & Qt.ShiftModifier) {
                    const next = Math.min(tracksView.count - 1, tracksView.currentIndex + 1);
                    tracksView.currentIndex = next;
                    trackSelection.setRange(next);
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
                selected: trackSelection.contains(index)
                dragTrackIds: selected && trackSelection.mimeIds(songs) !== "" ? trackSelection.mimeIds(songs) : String(model.trackId)
                onPlayRequested: (trackId, rowIndex, dangling) => {
                    trackSelection.clear();
                    tracksView.currentIndex = index;
                    if (!dangling && songs.isPlayableAt(index))
                        root.queue.playTrackNow(trackId);
                }
                onMenuRequested: (trackId, rowIndex, dangling) => trackMenu.openFor(index, tracksView, trackSelection, songs)
                onToggleSelectRequested: row => {
                    tracksView.currentIndex = row;
                    trackSelection.toggle(row);
                }
                onRangeSelectRequested: row => {
                    tracksView.currentIndex = row;
                    trackSelection.setRange(row);
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
                // Keyboard users Tab through the rail: keep the focused
                // card scrolled into view.
                onActiveFocusChanged: {
                    if (activeFocus)
                        moreView.positionViewAtIndex(index, ListView.Contain);
                }
                onActivated: id => root.albumRequested(id)
            }
        }
    }
}
