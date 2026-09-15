import QtQuick
import TuneX

// MiniPlayer (S4 W-026, icon transport in S6 W-038): opaque 64px bottom
// transport for windows below 1280px (DESIGN.md: opaque, never glass).
// Hidden until the first play; idle after that shows honest "Nothing
// playing" copy. Progress scrubs through QueueModel.seekMs. Volume and
// mute write through QueueModel.
Rectangle {
    id: root

    required property QueueModel queue
    property bool queueOpen: false
    property int transportState: 0
    property string titleText: ""
    property string artistText: ""
    property int positionMs: 0
    property int durationMs: 0
    property bool muted: false
    readonly property string shownTitle: Format.fallback(root.titleText, qsTr("Unknown Title"))
    readonly property string shownArtist: Format.fallback(root.artistText, qsTr("Unknown Artist"))
    // Cached cover of the playing track, empty until it resolves.
    property url artUrl
    readonly property string monogram: Format.monogram(root.shownTitle)
    readonly property string positionText: Format.duration(root.positionMs)
    readonly property string durationText: root.durationMs > 0 ? Format.duration(root.durationMs) : "—"
    readonly property bool hasCurrent: root.titleText !== "" || root.transportState > 0

    signal queueToggleRequested
    signal expandRequested

    // Durations shape through the Format singleton.

    function sync() {
        root.transportState = root.queue.playbackState();
        root.titleText = root.queue.currentTitle();
        root.artUrl = root.queue.currentArtUrl();
        root.artistText = root.queue.currentArtist();
        root.positionMs = root.queue.positionMs();
        root.durationMs = root.queue.durationMs();
        root.muted = root.queue.isMuted();
        if (!volumeSlider.pressed)
            volumeSlider.value = root.queue.volumePct();
        if (!seekSlider.pressed)
            seekSlider.value = root.positionMs;
    }

    color: Theme.panel
    Accessible.role: Accessible.Pane
    Accessible.name: root.hasCurrent ? qsTr("Now playing, %1, %2").arg(root.shownTitle).arg(root.shownArtist) : qsTr("Nothing playing")

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        height: 1
        color: Theme.border
        Accessible.ignored: true
    }

    MouseArea {
        id: expandHit

        anchors.left: parent.left
        anchors.right: transport.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        cursorShape: Qt.PointingHandCursor
        Accessible.role: Accessible.Button
        Accessible.name: qsTr("Open Now Playing")
        onClicked: root.expandRequested()
    }

    Artwork {
        id: art

        anchors.left: parent.left
        anchors.leftMargin: Theme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        width: Theme.artThumb
        height: Theme.artThumb
        source: root.artUrl
        monogram: root.monogram
        radius: Theme.radiusMd
        monogramSize: Theme.fontTitle
        Accessible.ignored: true
    }

    Column {
        id: meta

        anchors.left: art.right
        anchors.leftMargin: Theme.spaceSm
        anchors.verticalCenter: parent.verticalCenter
        width: 160
        spacing: 0

        Text {
            width: parent.width
            elide: Text.ElideRight
            text: root.hasCurrent ? root.shownTitle : qsTr("Nothing playing")
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBody
            font.weight: Font.DemiBold
            color: Theme.foreground
        }

        Text {
            width: parent.width
            elide: Text.ElideRight
            text: root.hasCurrent ? root.shownArtist : qsTr("Play something from your library")
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBodySm
            color: Theme.muted
        }
    }

    Row {
        id: transport

        anchors.left: meta.right
        anchors.leftMargin: Theme.spaceSm
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.spaceXs

        IconButton {
            anchors.verticalCenter: parent.verticalCenter
            iconName: "skip-back"
            accessibleName: qsTr("Previous track")
            onActivated: root.queue.previousTrack(root.queue.positionMs())
        }

        PlayButton {
            anchors.verticalCenter: parent.verticalCenter
            diameter: Theme.targetMin
            playing: root.transportState === 2
            onActivated: {
                root.queue.playPause();
                root.transportState = root.transportState === 2 ? 3 : 2;
            }
        }

        IconButton {
            anchors.verticalCenter: parent.verticalCenter
            iconName: "skip-forward"
            accessibleName: qsTr("Next track")
            onActivated: root.queue.nextTrack()
        }
    }

    Item {
        id: progressBlock

        anchors.left: transport.right
        anchors.leftMargin: Theme.spaceMd
        anchors.right: muteButton.left
        anchors.rightMargin: Theme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        height: Theme.targetMin

        Text {
            id: positionLabel

            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            // Previews the drag target while scrubbing (see QueuePanel).
            text: seekSlider.pressed ? Format.duration(Math.round(seekSlider.value)) : root.positionText
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontCaption
            font.weight: seekSlider.pressed ? Font.DemiBold : Font.Normal
            font.features: {
                "tnum": 1
            }
            color: seekSlider.pressed ? Theme.foreground : Theme.muted
        }

        Text {
            id: durationLabel

            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            text: root.durationText
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontCaption
            font.features: {
                "tnum": 1
            }
            color: Theme.muted
        }

        ProgressSlider {
            id: seekSlider

            anchors.left: positionLabel.right
            anchors.leftMargin: Theme.spaceSm
            anchors.right: durationLabel.left
            anchors.rightMargin: Theme.spaceSm
            anchors.verticalCenter: parent.verticalCenter
            from: 0
            to: Math.max(1, root.durationMs)
            keyStep: 5000
            enabled: root.durationMs > 0 && root.hasCurrent
            Accessible.name: qsTr("Playback position %1 of %2").arg(root.positionText).arg(root.durationText)
            onMoved: root.queue.seekMs(Math.round(value))
            onPressedChanged: {
                if (!pressed)
                    root.queue.seekMs(Math.round(value));
            }
        }
    }

    IconButton {
        id: muteButton

        anchors.right: volumeSlider.left
        anchors.verticalCenter: parent.verticalCenter
        iconName: Format.muteIcon(root.muted)
        accessibleName: root.muted ? qsTr("Unmute") : qsTr("Mute")
        checkable: true
        checked: root.muted
        onActivated: {
            root.queue.setMuted(!root.muted);
            root.muted = !root.muted;
        }
    }

    ProgressSlider {
        id: volumeSlider

        anchors.right: queueButton.left
        anchors.rightMargin: Theme.spaceSm
        anchors.verticalCenter: parent.verticalCenter
        width: 104
        from: 0
        to: 100
        stepSize: 1
        Accessible.name: qsTr("Volume")
        tipText: qsTr("%1%").arg(Math.round(value))
        onMoved: root.queue.setVolumePct(Math.round(value))
    }

    IconButton {
        id: queueButton

        anchors.right: parent.right
        anchors.rightMargin: Theme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        iconName: "list-music"
        accessibleName: root.queueOpen ? qsTr("Close Now Playing") : qsTr("Open Now Playing")
        checkable: true
        checked: root.queueOpen
        onActivated: root.queueToggleRequested()
    }
}
