import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Effects
import TuneX 1.0

// NowPlayingView (S4 W-028, strong glass in S6 W-038): the expanded overlay
// at any width and the only strong-glass surface (DESIGN.md Elevation &
// Depth): artwork backdrop → ~32px blur → 68% dark tint → 1px top highlight
// → soft shadow under the floating art → content. Same player state as the
// MiniPlayer and docked panel; Close or Esc returns without touching
// scroll (the popup takes focus so Esc always reaches it). Scrub posts
// async seeks (W-027). With Appearance.reduceTransparency the backdrop is
// solid `surface` and nothing blurs.
Popup {
    id: root

    required property QueueModel queue
    property int transportState: 0
    property bool shuffleOn: false
    property int repeatModeValue: 0
    property string repeatLabel: qsTr("Repeat: Off")
    property string errorLine: ""
    property string titleText: ""
    property string artistText: ""
    property int positionMs: 0
    property int durationMs: 0
    property bool muted: false
    readonly property bool hasCurrent: root.titleText !== "" || root.transportState > 0
    readonly property int motionMs: Appearance.duration(Theme.overlayMs)
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

    signal closeRequested
    signal queueToggleRequested

    function formatTime(ms: int): string {
        const total = Math.max(0, Math.floor(ms / 1000));
        const minutes = Math.floor(total / 60);
        const seconds = String(total % 60).padStart(2, "0");
        return minutes + ":" + seconds;
    }

    function repeatText(mode: int): string {
        if (mode === 1)
            return qsTr("Repeat: All");

        if (mode === 2)
            return qsTr("Repeat: One");

        return qsTr("Repeat: Off");
    }

    function sync() {
        root.transportState = root.queue.playbackState();
        root.shuffleOn = root.queue.isShuffle();
        root.repeatModeValue = root.queue.repeatMode();
        root.repeatLabel = root.repeatText(root.repeatModeValue);
        root.errorLine = root.queue.errorText();
        root.titleText = root.queue.currentTitle();
        root.artistText = root.queue.currentArtist();
        root.positionMs = root.queue.positionMs();
        root.durationMs = root.queue.durationMs();
        root.muted = root.queue.isMuted();
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
    focus: true
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    Component.onCompleted: {
        root.sync();
        root.open();
    }
    onClosed: root.closeRequested()
    onTitleTextChanged: {
        if (root.opened && !Appearance.reduceMotion)
            artCrossfade.restart();
    }

    SequentialAnimation {
        id: artCrossfade

        OpacityAnimator {
            target: art
            to: 0
            duration: Appearance.duration(Theme.artCrossfadeMs / 2)
            easing.type: Easing.OutCubic
        }

        OpacityAnimator {
            target: art
            to: 1
            duration: Appearance.duration(Theme.artCrossfadeMs / 2)
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

        // Opaque base: the blur never covers everything (its edges sample
        // transparent space past the item), so without this the shell shows
        // through the tint — the W-038 audit found list rows readable
        // behind the overlay.
        Rectangle {
            anchors.fill: parent
            color: Theme.background
        }

        // Backdrop source, hidden: MultiEffect renders it into its own
        // texture. W-040 swaps in the current track's artwork.
        Item {
            id: backdropSource

            anchors.fill: parent
            visible: false

            Rectangle {
                anchors.fill: parent
                color: Theme.surfaceRaised
            }

            // Placeholder atmosphere until artwork lands: a soft blue
            // bloom behind the crest (the `selected` tone reads through the
            // tint; `hover` was indistinguishable from the base).
            Rectangle {
                anchors.centerIn: parent
                anchors.verticalCenterOffset: -Theme.spaceXxl
                width: Theme.nowPlayingArt * 2
                height: width
                radius: width / 2
                color: Theme.selected
            }
        }

        MultiEffect {
            anchors.fill: parent
            source: backdropSource
            visible: !Appearance.reduceTransparency
            autoPaddingEnabled: false
            blurEnabled: true
            blurMax: Theme.strongBlur
            blurMultiplier: Theme.strongBlurMultiplier
            blur: 1
        }

        Rectangle {
            anchors.fill: parent
            color: Appearance.reduceTransparency ? Theme.surface : Qt.alpha(Theme.background, Theme.overlayTint)
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            height: 1
            visible: !Appearance.reduceTransparency
            color: Qt.alpha(Theme.foreground, 0.12)
            Accessible.ignored: true
        }
    }

    contentItem: Item {
        Item {
            id: sheet

            width: parent.width
            height: parent.height
            y: root.opened ? 0 : Theme.spaceMd
            // Popup is not an Item, so the accessible pane lives here.
            Accessible.role: Accessible.Pane
            Accessible.name: root.hasCurrent ? qsTr("Now Playing") + ", " + root.shownTitle + ", " + root.shownArtist : qsTr("Now Playing, nothing playing")

            IconButton {
                id: closeButton

                anchors.top: parent.top
                anchors.right: parent.right
                anchors.topMargin: Theme.spaceMd
                anchors.rightMargin: Theme.spaceMd
                z: 1
                iconName: "x"
                accessibleName: qsTr("Close Now Playing")
                onActivated: root.close()
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

                        // The art floats on the glass: the one soft shadow
                        // in the strong-glass stack.
                        RectangularShadow {
                            anchors.fill: art
                            radius: art.radius
                            offset.y: Theme.shadowOverlayY
                            blur: Theme.shadowOverlayBlur
                            color: Qt.alpha(Theme.shadow, Theme.shadowOverlayOpacity)
                        }

                        // Missing art is the one circular shape in V1: the
                        // monogram crest (DESIGN.md Shapes).
                        Rectangle {
                            id: art

                            anchors.horizontalCenter: parent.horizontalCenter
                            width: Theme.nowPlayingArt
                            height: Theme.nowPlayingArt
                            radius: width / 2
                            color: Theme.surfaceRaised
                            Accessible.ignored: true

                            Text {
                                visible: root.hasCurrent
                                anchors.centerIn: parent
                                text: root.monogram
                                textFormat: Text.PlainText
                                font.family: Theme.fontFamily
                                font.pixelSize: Theme.fontDisplay
                                font.weight: Font.DemiBold
                                color: Theme.muted
                            }

                            Icon {
                                visible: !root.hasCurrent
                                anchors.centerIn: parent
                                name: "music"
                                iconSize: 48
                                stroke: Theme.muted
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
                            maximumLineCount: 2
                            elide: Text.ElideRight
                            text: root.hasCurrent ? root.shownTitle : qsTr("Nothing playing")
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontHeadline
                            font.weight: Font.Bold
                            color: Theme.foreground
                        }

                        Text {
                            width: parent.width
                            horizontalAlignment: Text.AlignHCenter
                            elide: Text.ElideRight
                            text: root.hasCurrent ? root.shownArtist : qsTr("Play something from your library")
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
                            font.features: {
                                "tnum": 1
                            }
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
                            font.features: {
                                "tnum": 1
                            }
                            color: Theme.muted
                        }

                        ProgressSlider {
                            id: seekSlider

                            anchors.left: overlayPosition.right
                            anchors.leftMargin: Theme.spaceSm
                            anchors.right: overlayDuration.left
                            anchors.rightMargin: Theme.spaceSm
                            anchors.verticalCenter: parent.verticalCenter
                            from: 0
                            to: Math.max(1, root.durationMs)
                            enabled: root.durationMs > 0
                            Accessible.name: qsTr("Playback position %1 of %2").arg(root.positionText).arg(root.durationText)
                            onMoved: root.queue.seekMs(Math.round(value))
                        }
                    }

                    Row {
                        anchors.horizontalCenter: parent.horizontalCenter
                        spacing: Theme.spaceMd

                        IconButton {
                            anchors.verticalCenter: parent.verticalCenter
                            iconName: "shuffle"
                            accessibleName: root.shuffleOn ? qsTr("Shuffle: On") : qsTr("Shuffle: Off")
                            checkable: true
                            checked: root.shuffleOn
                            onActivated: {
                                root.queue.toggleShuffle();
                                root.sync();
                            }
                        }

                        IconButton {
                            anchors.verticalCenter: parent.verticalCenter
                            iconName: "skip-back"
                            glyphSize: 24
                            accessibleName: qsTr("Previous track")
                            onActivated: root.queue.previousTrack(root.queue.positionMs())
                        }

                        PlayButton {
                            id: playButton

                            anchors.verticalCenter: parent.verticalCenter
                            playing: root.transportState === 2
                            onActivated: {
                                root.queue.playPause();
                                root.transportState = root.transportState === 2 ? 3 : 2;
                            }
                        }

                        IconButton {
                            anchors.verticalCenter: parent.verticalCenter
                            iconName: "skip-forward"
                            glyphSize: 24
                            accessibleName: qsTr("Next track")
                            onActivated: root.queue.nextTrack()
                        }

                        IconButton {
                            anchors.verticalCenter: parent.verticalCenter
                            iconName: root.repeatModeValue === 2 ? "repeat-1" : "repeat"
                            accessibleName: root.repeatLabel
                            checkable: true
                            checked: root.repeatModeValue !== 0
                            onActivated: {
                                root.queue.cycleRepeat();
                                root.sync();
                            }
                        }
                    }

                    Row {
                        width: parent.width
                        spacing: Theme.spaceXs

                        IconButton {
                            id: muteButton

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

                            width: parent.width - muteButton.width - queueButton.width - Theme.spaceXs * 2
                            from: 0
                            to: 100
                            stepSize: 1
                            Accessible.name: qsTr("Volume")
                            onMoved: root.queue.setVolumePct(Math.round(value))
                        }

                        IconButton {
                            id: queueButton

                            iconName: "list-music"
                            accessibleName: qsTr("Open Up Next")
                            onActivated: root.queueToggleRequested()
                        }
                    }

                    // Errors pair the alert glyph with words (never colour
                    // alone).
                    Row {
                        anchors.horizontalCenter: parent.horizontalCenter
                        visible: root.errorLine !== ""
                        height: visible ? implicitHeight : 0
                        spacing: Theme.spaceSm

                        Icon {
                            anchors.verticalCenter: parent.verticalCenter
                            name: "alert"
                            iconSize: 16
                            stroke: Theme.error
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            width: Math.min(implicitWidth, column.width - Theme.spaceLg)
                            wrapMode: Text.WordWrap
                            text: root.errorLine
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontCaption
                            color: Theme.error
                        }
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
