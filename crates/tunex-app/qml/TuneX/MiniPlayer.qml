import QtQuick
import TuneX 1.0

// MiniPlayer (S4 W-026, icon transport in S6 W-038): opaque 76px bottom
// transport for windows below 1280px (DESIGN.md: opaque, never glass).
// Hidden until the first play; idle after that shows honest "Nothing
// playing" copy. Progress is read-only; scrub lives on the Now Playing
// overlay. Volume and mute write through QueueModel.
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
    readonly property string shownTitle: root.titleText !== "" ? root.titleText : qsTr("Unknown Title")
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
    readonly property string positionText: root.formatTime(root.positionMs)
    readonly property string durationText: root.durationMs > 0 ? root.formatTime(root.durationMs) : "—"
    readonly property real progress: root.durationMs > 0 ? Math.min(1, root.positionMs / root.durationMs) : 0
    readonly property bool hasCurrent: root.titleText !== "" || root.transportState > 0

    signal queueToggleRequested
    signal expandRequested

    function formatTime(ms) {
        const total = Math.max(0, Math.floor(ms / 1000));
        const minutes = Math.floor(total / 60);
        const seconds = String(total % 60).padStart(2, "0");
        return minutes + ":" + seconds;
    }

    function sync() {
        root.transportState = root.queue.playbackState();
        root.titleText = root.queue.currentTitle();
        root.artistText = root.queue.currentArtist();
        root.positionMs = root.queue.positionMs();
        root.durationMs = root.queue.durationMs();
        root.muted = root.queue.isMuted();
        if (!volumeSlider.pressed)
            volumeSlider.value = root.queue.volumePct();
    }

    color: Theme.surface
    Accessible.role: Accessible.Pane
    Accessible.name: root.hasCurrent ? qsTr("Now playing") + ", " + root.shownTitle + ", " + root.shownArtist : qsTr("Nothing playing")

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

    Rectangle {
        id: art

        anchors.left: parent.left
        anchors.leftMargin: Theme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        width: Theme.artThumb
        height: Theme.artThumb
        radius: Theme.radiusMd
        color: Theme.surfaceRaised
        Accessible.ignored: true

        Text {
            anchors.centerIn: parent
            text: root.monogram
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontTitle
            font.weight: Font.DemiBold
            color: Theme.muted
        }
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
        Accessible.role: Accessible.StaticText
        Accessible.name: qsTr("Playback position %1 of %2").arg(root.positionText).arg(root.durationText)

        Text {
            id: positionLabel

            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            text: root.positionText
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontCaption
            font.features: {
                "tnum": 1
            }
            color: Theme.muted
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

        Rectangle {
            anchors.left: positionLabel.right
            anchors.leftMargin: Theme.spaceSm
            anchors.right: durationLabel.left
            anchors.rightMargin: Theme.spaceSm
            anchors.verticalCenter: parent.verticalCenter
            height: Theme.progressTrack
            radius: Theme.radiusXs
            color: Theme.hover
            Accessible.ignored: true

            Rectangle {
                width: parent.width * root.progress
                height: parent.height
                radius: Theme.radiusXs
                color: Theme.accentSecondary
            }
        }
    }

    IconButton {
        id: muteButton

        anchors.right: volumeSlider.left
        anchors.verticalCenter: parent.verticalCenter
        iconName: root.muted ? "volume-x" : "volume"
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
        onMoved: root.queue.setVolumePct(Math.round(value))
    }

    IconButton {
        id: queueButton

        anchors.right: parent.right
        anchors.rightMargin: Theme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        iconName: "list-music"
        accessibleName: root.queueOpen ? qsTr("Close Up Next") : qsTr("Open Up Next")
        checkable: true
        checked: root.queueOpen
        onActivated: root.queueToggleRequested()
    }
}
