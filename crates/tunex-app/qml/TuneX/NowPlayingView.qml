import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Effects
import TuneX

// NowPlayingView (S4 W-028, strong glass in S6 W-038): the expanded overlay
// at any width and the only strong-glass surface (DESIGN.md Elevation &
// Depth): artwork backdrop → ~32px blur → 68% dark tint → 1px top highlight
// → soft shadow under the floating art → content. Same player state as the
// MiniPlayer and docked panel; Close or Esc returns without touching
// scroll (the popup takes focus so Esc always reaches it). Scrub posts
// async seeks (W-027). With Appearance.reduceTransparency the backdrop is
// solid `surface` and nothing blurs. A lyrics toggle (S13) replaces the
// art with a local sidecar/embedded pane — not a tab farm.
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
    property bool lyricsOn: false
    property int lyricsActive: -1
    readonly property bool hasLyrics: root.queue.lyricsLineCount() > 0 || root.queue.lyricsPlain() !== ""
    readonly property bool hasCurrent: root.titleText !== "" || root.transportState > 0
    readonly property int motionMs: Appearance.duration(Theme.overlayMs)
    readonly property string shownTitle: Format.fallback(root.titleText, qsTr("Unknown Title"))
    readonly property string shownArtist: Format.fallback(root.artistText, qsTr("Unknown Artist"))
    // Cached cover of the playing track, empty until it resolves.
    property url artUrl
    property int visualizerMode: 0
    property string spectrumCsv: ""
    property string waveformCsv: ""
    readonly property string monogram: Format.monogram(root.shownTitle)
    readonly property string positionText: Format.duration(root.positionMs)
    readonly property string durationText: Format.durationOrDash(root.durationMs)

    signal closeRequested
    signal queueToggleRequested

    // Durations shape through the Format singleton.

    // Karaoke follow: keep the sung line on screen, gliding inside the
    // motion budget (zeroed by reduce-motion like every animation).
    function followLyrics() {
        if (!root.lyricsOn || root.lyricsActive < 0 || !root.queue.lyricsSynced())
            return;
        const item = lyricsRepeater.itemAt(root.lyricsActive);
        if (!item)
            return;
        const top = lyricsFlick.contentY;
        if (item.y < top || item.y + item.height > top + lyricsFlick.height) {
            lyricsScroll.to = Math.max(0, item.y - lyricsFlick.height / 3);
            lyricsScroll.restart();
        }
    }

    onLyricsActiveChanged: root.followLyrics()
    onLyricsOnChanged: root.followLyrics()

    function sync() {
        root.transportState = root.queue.playbackState();
        root.shuffleOn = root.queue.isShuffle();
        root.repeatModeValue = root.queue.repeatMode();
        root.repeatLabel = Format.repeatLabel(root.repeatModeValue);
        root.errorLine = root.queue.errorText();
        root.titleText = root.queue.currentTitle();
        root.artistText = root.queue.currentArtist();
        root.artUrl = root.queue.currentArtUrl();
        root.visualizerMode = root.queue.visualizerMode();
        root.spectrumCsv = root.queue.spectrumCsv();
        root.waveformCsv = root.queue.waveformCsv();
        root.positionMs = root.queue.positionMs();
        root.durationMs = root.queue.durationMs();
        root.muted = root.queue.isMuted();
        if (!volumeSlider.pressed)
            volumeSlider.value = root.queue.volumePct();

        if (!seekSlider.pressed)
            seekSlider.value = root.positionMs;
        root.lyricsActive = root.queue.lyricsActiveIndex(root.positionMs);
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

    Timer {
        interval: 50
        running: root.opened
        repeat: true
        onTriggered: {
            root.visualizerMode = root.queue.visualizerMode();
            if (root.visualizerMode === 0 || Appearance.reduceMotion)
                return;
            interval = Math.round(1000 / Math.max(5, root.queue.visualizerFps()));
            root.spectrumCsv = root.queue.spectrumCsv();
            root.waveformCsv = root.queue.waveformCsv();
        }
    }
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

            // The track's own cover is the atmosphere (SPEC §21: artwork
            // leads). It is cropped to fill and blurred below, so its detail
            // never competes with the content on top. An empty or broken
            // source simply draws nothing and the bloom below stands in.
            Image {
                id: backdropArt

                anchors.fill: parent
                source: root.artUrl
                sourceSize.width: Math.max(1, Math.round(parent.width / 2))
                sourceSize.height: Math.max(1, Math.round(parent.height / 2))
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                cache: true
                Accessible.ignored: true
            }

            // Fallback atmosphere when a track has no cover: a soft blue
            // bloom behind the crest (the `selected` tone reads through the
            // tint; `hover` was indistinguishable from the base).
            Rectangle {
                anchors.centerIn: parent
                anchors.verticalCenterOffset: -Theme.spaceXxl
                visible: backdropArt.status !== Image.Ready
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
            Accessible.name: root.hasCurrent ? qsTr("Now Playing, %1, %2").arg(root.shownTitle).arg(root.shownArtist) : qsTr("Now Playing, nothing playing")

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
                ScrollBar.vertical: ListScrollBar {}

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
                        ArtStage {
                            id: art

                            visible: !root.lyricsOn
                            queue: root.queue
                            anchors.horizontalCenter: parent.horizontalCenter
                            width: Theme.nowPlayingArt
                            height: Theme.nowPlayingArt
                            source: root.artUrl
                            monogram: root.hasCurrent ? root.monogram : ""
                            monogramSize: Theme.fontDisplay
                            radius: Theme.radiusMd
                            circular: false
                            mode: root.visualizerMode
                            spectrumCsv: root.spectrumCsv
                            waveformCsv: root.waveformCsv
                            Accessible.ignored: true

                            Icon {
                                visible: !root.hasCurrent && root.visualizerMode === 0 && !root.lyricsOn
                                anchors.centerIn: parent
                                name: "music"
                                iconSize: 48
                                stroke: Theme.muted
                            }
                        }

                        Flickable {
                            id: lyricsFlick

                            visible: root.lyricsOn
                            ScrollBar.vertical: ListScrollBar {}
                            anchors.fill: parent
                            clip: true
                            contentWidth: width
                            contentHeight: lyricsColumn.height
                            boundsBehavior: Flickable.StopAtBounds
                            Accessible.role: Accessible.List
                            Accessible.name: qsTr("Lyrics")

                            NumberAnimation {
                                id: lyricsScroll

                                target: lyricsFlick
                                property: "contentY"
                                duration: Appearance.duration(Theme.motionRow)
                                easing.type: Easing.OutCubic
                            }

                            Column {
                                id: lyricsColumn

                                width: parent.width
                                spacing: Theme.spaceSm

                                Text {
                                    visible: !root.hasLyrics
                                    width: parent.width
                                    wrapMode: Text.WordWrap
                                    horizontalAlignment: Text.AlignHCenter
                                    text: qsTr("No local lyrics for this track.")
                                    textFormat: Text.PlainText
                                    font.family: Theme.fontFamily
                                    font.pixelSize: Theme.fontBody
                                    color: Theme.muted
                                }

                                Repeater {
                                    id: lyricsRepeater

                                    model: root.queue.lyricsSynced() ? root.queue.lyricsLineCount() : 0

                                    Text {
                                        required property int index
                                        width: lyricsColumn.width
                                        wrapMode: Text.WordWrap
                                        horizontalAlignment: Text.AlignHCenter
                                        text: root.queue.lyricsLineAt(index)
                                        textFormat: Text.PlainText
                                        font.family: Theme.fontFamily
                                        font.pixelSize: Theme.fontBody
                                        font.weight: index === root.lyricsActive ? Font.DemiBold : Font.Normal
                                        color: index === root.lyricsActive ? Theme.foreground : Theme.muted
                                    }
                                }

                                Text {
                                    visible: !root.queue.lyricsSynced() && root.queue.lyricsPlain() !== ""
                                    width: parent.width
                                    wrapMode: Text.WordWrap
                                    horizontalAlignment: Text.AlignHCenter
                                    text: root.queue.lyricsPlain()
                                    textFormat: Text.PlainText
                                    font.family: Theme.fontFamily
                                    font.pixelSize: Theme.fontBody
                                    color: Theme.foreground
                                }
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
                            keyStep: 5000
                            enabled: root.durationMs > 0
                            Accessible.name: qsTr("Playback position %1 of %2").arg(root.positionText).arg(root.durationText)
                            onMoved: root.queue.seekMs(Math.round(value))
                            onPressedChanged: {
                                if (!pressed)
                                    root.queue.seekMs(Math.round(value));
                            }
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
                            iconName: Format.repeatIcon(root.repeatModeValue)
                            accessibleName: root.repeatLabel
                            checkable: true
                            checked: root.repeatModeValue !== 0
                            onActivated: {
                                root.queue.cycleRepeat();
                                root.sync();
                            }
                        }

                        IconButton {
                            anchors.verticalCenter: parent.verticalCenter
                            // A note, not the list: the queue toggle below
                            // already owns `list-music`.
                            iconName: "music"
                            accessibleName: root.lyricsOn ? qsTr("Hide lyrics") : qsTr("Show lyrics")
                            checkable: true
                            checked: root.lyricsOn
                            onActivated: root.lyricsOn = !root.lyricsOn
                        }
                    }

                    Row {
                        width: parent.width
                        spacing: Theme.spaceXs

                        IconButton {
                            id: muteButton

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

                            width: parent.width - muteButton.width - queueButton.width - Theme.spaceXs * 2
                            from: 0
                            to: 100
                            stepSize: 1
                            Accessible.name: qsTr("Volume")
                            tipText: qsTr("%1%").arg(Math.round(value))
                            onMoved: root.queue.setVolumePct(Math.round(value))
                        }

                        IconButton {
                            id: queueButton

                            iconName: "list-music"
                            accessibleName: qsTr("Open Now Playing list")
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
