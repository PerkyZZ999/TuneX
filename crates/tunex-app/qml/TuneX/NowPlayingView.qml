import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Effects
import TuneX 1.0

// NowPlayingView (S4 W-028): expanded overlay — strong glass over content
// at any width. Same player state as MiniPlayer / docked panel; Close or
// Esc returns without touching scroll. Scrub posts async seeks (W-027).
Popup {
    id: root

    required property QueueModel queue
    property int transportState: 0
    property bool shuffleOn: false
    property string repeatLabel: qsTr("Repeat: Off")
    property string errorLine: ""
    property string titleText: ""
    property string artistText: ""
    property int positionMs: 0
    property int durationMs: 0
    property bool muted: false
    property bool reduceTransparency: false
    property bool reduceMotion: false
    readonly property int motionMs: root.reduceMotion ? 0 : Theme.overlayMs
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

    signal closeRequested()
    signal queueToggleRequested()

    function formatTime(ms) {
        const total = Math.max(0, Math.floor(ms / 1000));
        const minutes = Math.floor(total / 60);
        const seconds = String(total % 60).padStart(2, "0");
        return minutes + ":" + seconds;
    }

    function repeatText(mode) {
        if (mode === 1)
            return qsTr("Repeat: All");

        if (mode === 2)
            return qsTr("Repeat: One");

        return qsTr("Repeat: Off");
    }

    function sync() {
        root.transportState = root.queue.playbackState();
        root.shuffleOn = root.queue.isShuffle();
        root.repeatLabel = root.repeatText(root.queue.repeatMode());
        root.errorLine = root.queue.errorText();
        root.titleText = root.queue.currentTitle();
        root.artistText = root.queue.currentArtist();
        root.positionMs = root.queue.positionMs();
        root.durationMs = root.queue.durationMs();
        root.muted = root.queue.isMuted();
        root.reduceTransparency = root.queue.reduceTransparency();
        root.reduceMotion = root.queue.reduceMotion();
        if (!volumeSlider.pressed)
            volumeSlider.value = root.queue.volumePct();

        if (!seekSlider.pressed)
            seekSlider.value = root.positionMs;

    }

    parent: Overlay.overlay
    width: Overlay.overlay.width
    height: Overlay.overlay.height
    padding: 0
    modal: true
    dim: false
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Now Playing") + ", " + root.shownTitle + ", " + root.shownArtist
    Component.onCompleted: {
        root.sync();
        root.open();
    }
    onClosed: root.closeRequested()
    onTitleTextChanged: {
        if (root.opened && !root.reduceMotion)
            artCrossfade.restart();

    }

    SequentialAnimation {
        id: artCrossfade

        NumberAnimation {
            target: art
            property: "opacity"
            to: 0
            duration: root.reduceMotion ? 0 : Theme.artCrossfadeMs / 2
            easing.type: Easing.OutCubic
        }

        NumberAnimation {
            target: art
            property: "opacity"
            to: 1
            duration: root.reduceMotion ? 0 : Theme.artCrossfadeMs / 2
            easing.type: Easing.OutCubic
        }

    }

    enter: Transition {
        NumberAnimation {
            property: "opacity"
            from: 0
            to: 1
            duration: root.motionMs
            easing.type: Easing.OutCubic
        }

    }

    exit: Transition {
        NumberAnimation {
            property: "opacity"
            from: 1
            to: 0
            duration: root.motionMs
            easing.type: Easing.OutCubic
        }

    }

    background: Item {
        anchors.fill: parent

        Item {
            id: backdropSource

            anchors.fill: parent
            visible: false
            layer.enabled: !root.reduceTransparency

            Rectangle {
                anchors.fill: parent
                color: Theme.surfaceRaised
            }

            Rectangle {
                anchors.centerIn: parent
                width: Theme.nowPlayingArt * 2
                height: width
                radius: width / 2
                color: Theme.hover
            }

        }

        MultiEffect {
            anchors.fill: parent
            source: backdropSource
            visible: !root.reduceTransparency
            autoPaddingEnabled: false
            blurEnabled: !root.reduceTransparency
            blurMax: Theme.blurMax
            blur: 1
        }

        Rectangle {
            anchors.fill: parent
            color: root.reduceTransparency ? Theme.surface : Theme.background
            opacity: root.reduceTransparency ? 1 : Theme.overlayTint
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            height: 1
            color: Theme.foreground
            opacity: 0.12
            Accessible.ignored: true
        }

    }

    contentItem: Item {
        Item {
            id: sheet

            width: parent.width
            height: parent.height
            y: root.opened ? 0 : Theme.spaceMd

            Button {
                id: closeButton

                anchors.top: parent.top
                anchors.right: parent.right
                anchors.topMargin: Theme.spaceMd
                anchors.rightMargin: Theme.spaceMd
                height: Theme.targetMin
                z: 1
                text: qsTr("Close")
                Accessible.name: qsTr("Close Now Playing")
                onClicked: root.close()
            }

            Flickable {
                id: scroller

                anchors.fill: parent
                anchors.topMargin: Theme.spaceMd
                contentWidth: width
                contentHeight: column.height + Theme.spaceXxl
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                Accessible.ignored: true

                Column {
                    id: column

                    anchors.horizontalCenter: parent.horizontalCenter
                    width: Math.min(parent.width - Theme.spaceXxl, Theme.nowPlayingArt + Theme.spaceXxl * 2)
                    spacing: Theme.spaceLg
                    topPadding: Theme.spaceXxl

                    Item {
                        id: artWrap

                        width: parent.width
                        height: Theme.nowPlayingArt

                        Rectangle {
                            anchors.centerIn: art
                            width: art.width + Theme.spaceSm
                            height: art.height + Theme.spaceSm
                            radius: width / 2
                            color: Theme.background
                            opacity: 0.45
                            Accessible.ignored: true
                        }

                        Rectangle {
                            id: art

                            anchors.horizontalCenter: parent.horizontalCenter
                            width: Theme.nowPlayingArt
                            height: Theme.nowPlayingArt
                            radius: width / 2
                            color: Theme.surfaceRaised
                            opacity: 1
                            Accessible.ignored: true

                            Text {
                                anchors.centerIn: parent
                                text: root.monogram
                                textFormat: Text.PlainText
                                font.family: Theme.fontFamily
                                font.pixelSize: Theme.fontDisplay
                                font.weight: Font.DemiBold
                                color: Theme.muted
                            }

                        }

                    }

                    Column {
                        width: parent.width
                        spacing: Theme.spaceXs

                        Text {
                            width: parent.width
                            horizontalAlignment: Text.AlignHCenter
                            wrapMode: Text.WordWrap
                            text: root.shownTitle
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontHeadline
                            font.weight: Font.Bold
                            color: Theme.foreground
                        }

                        Text {
                            width: parent.width
                            horizontalAlignment: Text.AlignHCenter
                            wrapMode: Text.WordWrap
                            text: root.shownArtist
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontBody
                            color: Theme.muted
                        }

                    }

                    Item {
                        width: parent.width
                        height: Theme.targetMin

                        Text {
                            id: overlayPosition

                            anchors.left: parent.left
                            anchors.verticalCenter: parent.verticalCenter
                            text: root.positionText
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontCaption
                            color: Theme.muted
                        }

                        Text {
                            id: overlayDuration

                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            text: root.durationText
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontCaption
                            color: Theme.muted
                        }

                        Slider {
                            id: seekSlider

                            anchors.left: overlayPosition.right
                            anchors.leftMargin: Theme.spaceSm
                            anchors.right: overlayDuration.left
                            anchors.rightMargin: Theme.spaceSm
                            anchors.verticalCenter: parent.verticalCenter
                            height: Theme.targetMin
                            from: 0
                            to: Math.max(1, root.durationMs)
                            enabled: root.durationMs > 0
                            Accessible.name: qsTr("Playback position %1 of %2").arg(root.positionText).arg(root.durationText)
                            onMoved: root.queue.seekMs(Math.round(value))

                            background: Rectangle {
                                x: seekSlider.leftPadding
                                y: seekSlider.topPadding + (seekSlider.availableHeight - height) / 2
                                implicitHeight: Theme.progressTrack
                                width: seekSlider.availableWidth
                                height: Theme.progressTrack
                                radius: Theme.radiusXs
                                color: Theme.hover

                                Rectangle {
                                    width: seekSlider.visualPosition * parent.width
                                    height: parent.height
                                    radius: Theme.radiusXs
                                    color: Theme.accentSecondary
                                }

                            }

                            handle: Rectangle {
                                x: seekSlider.leftPadding + seekSlider.visualPosition * (seekSlider.availableWidth - width)
                                y: seekSlider.topPadding + (seekSlider.availableHeight - height) / 2
                                implicitWidth: Theme.thumbSize
                                implicitHeight: Theme.thumbSize
                                width: seekSlider.hovered || seekSlider.pressed || seekSlider.activeFocus ? Theme.thumbSize : Theme.spaceSm
                                height: width
                                radius: width / 2
                                color: Theme.foreground
                                border.color: Theme.focus
                                border.width: seekSlider.activeFocus ? 2 : 0
                            }

                        }

                    }

                    Row {
                        anchors.horizontalCenter: parent.horizontalCenter
                        spacing: Theme.spaceSm

                        Button {
                            height: Theme.targetMin
                            checkable: true
                            checked: root.shuffleOn
                            text: root.shuffleOn ? qsTr("Shuffle: On") : qsTr("Shuffle: Off")
                            Accessible.name: root.shuffleOn ? qsTr("Shuffle on") : qsTr("Shuffle off")
                            onClicked: {
                                root.queue.toggleShuffle();
                                root.sync();
                            }
                        }

                        Button {
                            width: Theme.targetMin
                            height: Theme.targetMin
                            text: qsTr("Prev")
                            Accessible.name: qsTr("Previous track")
                            onClicked: root.queue.previousTrack(root.queue.positionMs())
                        }

                        Item {
                            width: Theme.playPrimary
                            height: Theme.playPrimary

                            Rectangle {
                                anchors.centerIn: parent
                                width: Theme.playPrimary + Theme.spaceLg
                                height: width
                                radius: width / 2
                                color: Theme.accent
                                opacity: root.transportState === 2 ? 0.35 : 0
                                Accessible.ignored: true
                            }

                            Button {
                                id: playButton

                                anchors.fill: parent
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
                                    font.pixelSize: Theme.fontLabel
                                    font.weight: Font.DemiBold
                                    color: Theme.primaryText
                                    horizontalAlignment: Text.AlignHCenter
                                    verticalAlignment: Text.AlignVCenter
                                }

                            }

                        }

                        Button {
                            width: Theme.targetMin
                            height: Theme.targetMin
                            text: qsTr("Next")
                            Accessible.name: qsTr("Next track")
                            onClicked: root.queue.nextTrack()
                        }

                        Button {
                            height: Theme.targetMin
                            text: root.repeatLabel
                            Accessible.name: root.repeatLabel
                            onClicked: {
                                root.queue.cycleRepeat();
                                root.sync();
                            }
                        }

                    }

                    Row {
                        width: parent.width
                        spacing: Theme.spaceXs

                        Button {
                            id: muteButton

                            width: Theme.targetMin
                            height: Theme.targetMin
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

                            width: parent.width - muteButton.width - queueButton.width - Theme.spaceXs * 2
                            height: Theme.targetMin
                            from: 0
                            to: 100
                            stepSize: 1
                            Accessible.name: qsTr("Volume")
                            onMoved: root.queue.setVolumePct(Math.round(value))

                            background: Rectangle {
                                x: volumeSlider.leftPadding
                                y: volumeSlider.topPadding + (volumeSlider.availableHeight - height) / 2
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
                                implicitWidth: Theme.thumbSize
                                implicitHeight: Theme.thumbSize
                                width: volumeSlider.hovered || volumeSlider.pressed || volumeSlider.activeFocus ? Theme.thumbSize : Theme.spaceSm
                                height: width
                                radius: width / 2
                                color: Theme.foreground
                                border.color: Theme.focus
                                border.width: volumeSlider.activeFocus ? 2 : 0
                            }

                        }

                        Button {
                            id: queueButton

                            height: Theme.targetMin
                            text: qsTr("Up Next")
                            Accessible.name: qsTr("Open Up Next queue")
                            onClicked: root.queueToggleRequested()
                        }

                    }

                    Text {
                        visible: root.errorLine !== ""
                        width: parent.width
                        height: visible ? implicitHeight : 0
                        wrapMode: Text.WordWrap
                        text: root.errorLine
                        textFormat: Text.PlainText
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontCaption
                        color: Theme.error
                        horizontalAlignment: Text.AlignHCenter
                    }

                }

            }

            Behavior on y {
                NumberAnimation {
                    duration: root.motionMs
                    easing.type: Easing.OutCubic
                }

            }

        }

    }

}
