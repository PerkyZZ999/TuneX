import QtQuick
import QtQuick.Controls.Basic
import TuneX 1.0

// MiniPlayer (S4 W-026): opaque 76px bottom transport for windows below
// 1280px. Hidden until the first play. Progress is read-only (seek is
// W-027); volume and mute write through QueueModel. Artwork is the
// generated monogram until the art worker lands.
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
        const words = root.shownTitle.split(/\s+/).filter(function(word) {
            return word.length > 0;
        });
        const letters = words.slice(0, 2).map(function(word) {
            return word[0].toUpperCase();
        });
        return letters.join("");
    }
    readonly property string positionText: root.formatTime(root.positionMs)
    readonly property string durationText: root.durationMs > 0 ? root.formatTime(root.durationMs) : "—"
    readonly property real progress: root.durationMs > 0 ? Math.min(1, root.positionMs / root.durationMs) : 0

    signal queueToggleRequested()

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
    Accessible.name: qsTr("Now playing") + ", " + root.shownTitle + ", " + root.shownArtist

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        height: 1
        color: Theme.border
        Accessible.ignored: true
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
            text: root.shownTitle
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBody
            font.weight: Font.DemiBold
            color: Theme.foreground
        }

        Text {
            width: parent.width
            elide: Text.ElideRight
            text: root.shownArtist
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

        Button {
            width: Theme.targetMin
            height: Theme.targetMin
            padding: 4
            font.pixelSize: Theme.fontCaption
            text: qsTr("Prev")
            Accessible.name: qsTr("Previous track")
            onClicked: root.queue.previousTrack(root.queue.positionMs())
        }

        Button {
            id: playButton

            width: Theme.targetMin
            height: Theme.targetMin
            text: root.transportState === 2 ? qsTr("Pause") : qsTr("Play")
            Accessible.name: root.transportState === 2 ? qsTr("Pause") : qsTr("Play")
            onClicked: {
                root.queue.playPause();
                root.transportState = root.transportState === 2 ? 3 : 2;
            }

            background: Rectangle {
                radius: width / 2
                color: Theme.primary
                border.color: Theme.focus
                border.width: playButton.activeFocus ? 2 : 0
            }

            contentItem: Text {
                text: playButton.text
                textFormat: Text.PlainText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontCaption
                font.weight: Font.DemiBold
                color: Theme.primaryText
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
            }

        }

        Button {
            width: Theme.targetMin
            height: Theme.targetMin
            padding: 4
            font.pixelSize: Theme.fontCaption
            text: qsTr("Next")
            Accessible.name: qsTr("Next track")
            onClicked: root.queue.nextTrack()
        }

    }

    Item {
        id: progressBlock

        anchors.left: transport.right
        anchors.leftMargin: Theme.spaceSm
        anchors.right: muteButton.left
        anchors.rightMargin: Theme.spaceSm
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
            color: Theme.muted
        }

        Rectangle {
            id: progressTrack

            anchors.left: positionLabel.right
            anchors.leftMargin: Theme.spaceXs
            anchors.right: durationLabel.left
            anchors.rightMargin: Theme.spaceXs
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

    Button {
        id: muteButton

        anchors.right: volumeSlider.left
        anchors.rightMargin: Theme.spaceXs
        anchors.verticalCenter: parent.verticalCenter
        width: Theme.targetMin
        height: Theme.targetMin
        padding: 2
        font.pixelSize: Theme.fontCaption
        checkable: true
        checked: root.muted
        text: qsTr("Mute")
        Accessible.name: root.muted ? qsTr("Unmute") : qsTr("Mute")
        onClicked: {
            root.queue.setMuted(!root.muted);
            root.muted = !root.muted;
        }
    }

    Slider {
        id: volumeSlider

        anchors.right: queueButton.left
        anchors.rightMargin: Theme.spaceSm
        anchors.verticalCenter: parent.verticalCenter
        width: 96
        height: Theme.targetMin
        from: 0
        to: 100
        stepSize: 1
        Accessible.name: qsTr("Volume")
        onMoved: root.queue.setVolumePct(Math.round(value))

        background: Rectangle {
            x: volumeSlider.leftPadding
            y: volumeSlider.topPadding + (volumeSlider.availableHeight - height) / 2
            implicitWidth: 96
            implicitHeight: Theme.progressTrack
            width: volumeSlider.availableWidth
            height: Theme.progressTrack
            radius: Theme.radiusXs
            color: Theme.hover

            Rectangle {
                width: volumeSlider.visualPosition * parent.width
                height: parent.height
                radius: Theme.radiusXs
                color: Theme.accentSecondary
            }

        }

        handle: Rectangle {
            x: volumeSlider.leftPadding + volumeSlider.visualPosition * (volumeSlider.availableWidth - width)
            y: volumeSlider.topPadding + (volumeSlider.availableHeight - height) / 2
            implicitWidth: 12
            implicitHeight: 12
            width: volumeSlider.hovered || volumeSlider.pressed || volumeSlider.activeFocus ? 12 : 8
            height: width
            radius: width / 2
            color: Theme.foreground
            border.color: Theme.focus
            border.width: volumeSlider.activeFocus ? 2 : 0
        }

    }

    Button {
        id: queueButton

        anchors.right: parent.right
        anchors.rightMargin: Theme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        height: Theme.targetMin
        padding: 4
        font.pixelSize: Theme.fontCaption
        checkable: true
        checked: root.queueOpen
        text: qsTr("Up Next")
        Accessible.name: root.queueOpen ? qsTr("Close Up Next queue") : qsTr("Open Up Next queue")
        onClicked: root.queueToggleRequested()
    }

}
