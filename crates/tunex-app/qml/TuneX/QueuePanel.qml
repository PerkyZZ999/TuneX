import QtQuick
import TuneX

// QueuePanel (S3 W-022, S4 W-026, S6 W-038): Up Next list plus the
// persistent-player summary. Docked as the opaque 320px right column at
// ≥1280px; inside the compact Drawer it is transparent so the drawer's
// subtle glass shows through (rows stay transparent over it, never glass).
// Transport, shuffle/repeat, reorder, and the playing marker stay here;
// MiniPlayer is the narrow-width bar. Progress scrubs through QueueModel.seekMs.
Rectangle {
    id: root

    required property QueueModel queue
    // Docked in the shell (no Close); false when hosted in the compact drawer.
    property bool embedded: false
    // Drive the display-sync timer. The compact drawer host sets this to
    // `opened` so a closed drawer does not poll a second copy of the panel.
    property bool tracking: true
    // Local mirrors of model state (functions carry no notifiers).
    property int transportState: 0
    property bool shuffleOn: false
    property int repeatModeValue: 0
    property string repeatLabel: qsTr("Repeat: Off")
    property string errorLine: ""
    property int seenCursor: -2
    property string titleText: ""
    property string artistText: ""
    property int positionMs: 0
    property int durationMs: 0
    property bool muted: false
    // Cached cover of the playing track, empty until it resolves.
    property url artUrl
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
    readonly property bool hasCurrent: root.titleText !== "" || root.transportState > 0

    signal browseRequested
    signal closeRequested
    signal expandRequested

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
        root.repeatModeValue = root.queue.repeatMode();
        root.repeatLabel = root.repeatText(root.repeatModeValue);
        root.errorLine = root.queue.errorText();
        root.titleText = root.queue.currentTitle();
        root.artistText = root.queue.currentArtist();
        root.artUrl = root.queue.currentArtUrl();
        root.positionMs = root.queue.positionMs();
        root.durationMs = root.queue.durationMs();
        root.muted = root.queue.isMuted();
        if (!volumeSlider.pressed)
            volumeSlider.value = root.queue.volumePct();
        if (!seekSlider.pressed)
            seekSlider.value = root.positionMs;

        const cursor = root.queue.currentIndex();
        if (cursor !== root.seenCursor) {
            root.seenCursor = cursor;
            if (cursor >= 0) {
                // Seed the keyboard cursor from the playing cursor (no focus
                // move): Enter/Delete/Alt handlers stay on the playing row.
                queueList.currentIndex = cursor;
                queueList.positionViewAtIndex(cursor, ListView.Contain);
            }
        }
    }

    // Docked: translucent chrome over the shell's ambient wash (solid again
    // when transparency is reduced). In the compact drawer it stays fully
    // transparent so the drawer's own glass is the only surface.
    color: {
        if (!root.embedded)
            return "transparent";

        if (Appearance.reduceTransparency)
            return Theme.surface;

        return Qt.rgba(Theme.surface.r, Theme.surface.g, Theme.surface.b, Theme.chromeTint);
    }
    onTrackingChanged: {
        if (root.tracking)
            root.sync();
    }

    Rectangle {
        visible: root.embedded
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        width: 1
        color: Theme.border
        Accessible.ignored: true
    }

    Timer {
        interval: 300
        running: root.tracking
        repeat: true
        onTriggered: root.sync()
    }

    // Dismiss the row menu on any model reset: it captures a positional
    // index that resets, reorders, and track advances invalidate.
    Connections {
        function onModelReset() {
            rowMenu.dismiss();
        }

        target: root.queue
    }

    GlassMenu {
        id: rowMenu

        property int rowIndex: -1

        GlassMenuItem {
            text: qsTr("Play now")
            onTriggered: root.queue.playAt(rowMenu.rowIndex)
        }

        GlassMenuItem {
            text: qsTr("Play next")
            onTriggered: root.queue.playNextAt(rowMenu.rowIndex)
        }

        GlassMenuItem {
            text: qsTr("Move to end")
            onTriggered: root.queue.moveToEnd(rowMenu.rowIndex)
        }

        GlassMenuItem {
            text: qsTr("Remove from Up Next")
            onTriggered: root.queue.removeAt(rowMenu.rowIndex)
        }
    }

    Column {
        anchors.fill: parent
        anchors.margins: Theme.spaceLg
        spacing: Theme.spaceMd

        Item {
            id: headerRow

            visible: !root.embedded
            width: parent.width
            height: visible ? Theme.targetMin : 0

            IconButton {
                anchors.right: parent.right
                iconName: "x"
                accessibleName: qsTr("Close Up Next")
                onActivated: root.closeRequested()
            }
        }

        MouseArea {
            id: nowPlayingHit

            visible: root.hasCurrent || root.embedded
            width: parent.width
            height: {
                if (!visible)
                    return 0;

                if (root.embedded)
                    return Math.min(parent.width, 248);

                return Theme.artThumb;
            }
            cursorShape: Qt.PointingHandCursor
            Accessible.role: Accessible.Button
            Accessible.name: qsTr("Open Now Playing")
            onClicked: root.expandRequested()

            Artwork {
                id: artWell

                anchors.horizontalCenter: parent.horizontalCenter
                width: root.embedded ? parent.height : Theme.artThumb
                height: width
                source: root.artUrl
                monogram: root.hasCurrent ? root.monogram : qsTr("TuneX")
                monogramSize: root.embedded ? Theme.fontHeadline : Theme.fontTitle
                radius: Theme.radiusMd
                Accessible.ignored: true
            }
        }

        Column {
            visible: root.embedded
            width: parent.width
            spacing: Theme.spaceXs

            // Left-aligned under the artwork, as the mockup sets them: the
            // title reads as a heading for the panel rather than a caption
            // centred under a picture.
            Text {
                width: parent.width
                elide: Text.ElideRight
                text: root.hasCurrent ? root.shownTitle : qsTr("Nothing playing")
                textFormat: Text.PlainText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontTitle
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

        Item {
            id: progressItem

            visible: root.hasCurrent
            width: parent.width
            height: visible ? Theme.targetMin : 0

            Text {
                id: panelPosition

                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                text: root.positionText
                textFormat: Text.PlainText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontCaption
                color: Theme.muted
            }

            Text {
                id: panelDuration

                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                text: root.durationText
                textFormat: Text.PlainText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontCaption
                color: Theme.muted
            }

            ProgressSlider {
                id: seekSlider

                anchors.left: panelPosition.right
                anchors.leftMargin: Theme.spaceXs
                anchors.right: panelDuration.left
                anchors.rightMargin: Theme.spaceXs
                anchors.verticalCenter: parent.verticalCenter
                from: 0
                to: Math.max(1, root.durationMs)
                enabled: root.durationMs > 0 && root.hasCurrent
                Accessible.name: qsTr("Playback position %1 of %2").arg(root.positionText).arg(root.durationText)
                onMoved: root.queue.seekMs(Math.round(value))
            }
        }

        Row {
            id: volumeRow

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

                width: parent.width - muteButton.width - Theme.spaceXs
                from: 0
                to: 100
                stepSize: 1
                Accessible.name: qsTr("Volume")
                onMoved: root.queue.setVolumePct(Math.round(value))
            }
        }

        Row {
            id: transportRow

            width: parent.width
            spacing: Theme.spaceXs

            IconButton {
                iconName: "shuffle"
                accessibleName: root.shuffleOn ? qsTr("Shuffle: On") : qsTr("Shuffle: Off")
                checkable: true
                checked: root.shuffleOn
                onActivated: {
                    root.queue.toggleShuffle();
                    root.sync();
                }
            }

            Item {
                width: Math.max(0, (parent.width - Theme.targetMin * 4 - Theme.playPrimary - Theme.spaceXs * 4) / 2)
                height: 1
            }

            IconButton {
                iconName: "skip-back"
                accessibleName: qsTr("Previous")
                enabled: queueList.count > 0
                onActivated: root.queue.previousTrack(root.queue.positionMs())
            }

            PlayButton {
                playing: root.transportState === 2
                enabled: queueList.count > 0 || root.transportState === 2 || root.transportState === 3
                onActivated: {
                    root.queue.playPause();
                    root.transportState = root.transportState === 2 ? 3 : 2;
                }
            }

            IconButton {
                iconName: "skip-forward"
                accessibleName: qsTr("Next")
                enabled: queueList.count > 0
                onActivated: root.queue.nextTrack()
            }

            Item {
                width: Math.max(0, (parent.width - Theme.targetMin * 4 - Theme.playPrimary - Theme.spaceXs * 4) / 2)
                height: 1
            }

            IconButton {
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

        Item {
            id: togglesRow

            width: parent.width
            height: 0
        }

        Row {
            id: upNextHeader

            width: parent.width
            height: Theme.targetMin
            spacing: Theme.spaceSm

            Text {
                width: parent.width - clearButton.width - Theme.spaceSm
                anchors.verticalCenter: parent.verticalCenter
                elide: Text.ElideRight
                text: queueList.count > 0 ? qsTr("Up Next (%1)").arg(queueList.count) : qsTr("Up Next")
                textFormat: Text.PlainText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontTitle
                font.weight: Font.DemiBold
                color: Theme.foreground
            }

            PrimaryButton {
                id: clearButton

                primary: false
                text: qsTr("Clear")
                enabled: queueList.count > 0
                Accessible.name: qsTr("Clear the queue (keeps playing)")
                onClicked: root.queue.clearQueue()
            }
        }

        Text {
            id: errorText

            visible: root.errorLine !== ""
            width: parent.width
            height: visible ? implicitHeight : 0
            clip: true
            wrapMode: Text.WordWrap
            text: root.errorLine
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontCaption
            color: Theme.error
        }

        Item {
            id: content

            width: parent.width
            height: parent.height - headerRow.height - nowPlayingHit.height - progressItem.height - volumeRow.height - transportRow.height - togglesRow.height - upNextHeader.height - errorText.height - Theme.spaceMd * 8

            EmptyState {
                visible: queueList.count === 0 && !root.embedded
                title: qsTr("Up Next is empty")
                note: qsTr("Play any song, album, or artist and it will queue up here.")
                actionLabel: qsTr("Browse library")
                onActionRequested: root.browseRequested()
            }

            Text {
                visible: queueList.count === 0 && root.embedded
                width: parent.width
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
                text: qsTr("Nothing queued yet.")
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontBody
                color: Theme.muted
            }

            ListView {
                id: queueList

                visible: count > 0
                anchors.fill: parent
                model: root.queue
                activeFocusOnTab: true
                clip: true
                highlightMoveDuration: Appearance.duration(Theme.motionHover)
                // Row insert/remove, 160ms per the DESIGN.md motion budget.
                // These only run when the model reports a single row moving,
                // which is why `QueueModel` narrows its signals instead of
                // resetting for every queue change.
                add: Transition {
                    NumberAnimation {
                        properties: "opacity"
                        from: 0
                        to: 1
                        duration: Appearance.duration(Theme.motionRow)
                        easing.type: Easing.OutCubic
                    }
                }
                remove: Transition {
                    NumberAnimation {
                        properties: "opacity"
                        to: 0
                        duration: Appearance.duration(Theme.motionRow)
                        easing.type: Easing.OutCubic
                    }
                }
                displaced: Transition {
                    NumberAnimation {
                        properties: "y"
                        duration: Appearance.duration(Theme.motionRow)
                        easing.type: Easing.OutCubic
                    }
                }
                Accessible.role: Accessible.List
                Accessible.name: qsTr("Up Next")
                Keys.onReturnPressed: {
                    const at = queueList.currentIndex >= 0 ? queueList.currentIndex : 0;
                    if (at < queueList.count)
                        root.queue.playAt(at);
                }
                Keys.onEnterPressed: {
                    const at = queueList.currentIndex >= 0 ? queueList.currentIndex : 0;
                    if (at < queueList.count)
                        root.queue.playAt(at);
                }
                Keys.onDeletePressed: {
                    if (queueList.currentIndex >= 0 && queueList.currentIndex < queueList.count)
                        root.queue.removeAt(queueList.currentIndex);
                }
                Keys.onUpPressed: event => {
                    if (event.modifiers & Qt.AltModifier) {
                        const from = queueList.currentIndex;
                        if (from > 0) {
                            root.queue.moveItem(from, from - 1);
                            queueList.currentIndex = from - 1;
                        }
                        event.accepted = true;
                    }
                }
                Keys.onDownPressed: event => {
                    if (event.modifiers & Qt.AltModifier) {
                        const from = queueList.currentIndex;
                        if (from >= 0 && from + 1 < queueList.count) {
                            root.queue.moveItem(from, from + 1);
                            queueList.currentIndex = from + 1;
                        }
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
                    durationMs: model.durationMs
                    isCurrent: model.isCurrent
                    isPlaying: model.isCurrent && root.transportState === 2
                    missing: model.missing
                    reorderable: true
                    onPlayRequested: (trackId, rowIndex) => {
                        queueList.currentIndex = rowIndex;
                        queueList.forceActiveFocus();
                        root.queue.playAt(rowIndex);
                    }
                    onMenuRequested: (trackId, rowIndex) => {
                        queueList.currentIndex = rowIndex;
                        rowMenu.rowIndex = rowIndex;
                        rowMenu.popup();
                    }
                    onReorderRequested: (from, to) => {
                        queueList.currentIndex = to;
                        root.queue.moveItem(from, to);
                    }
                }
            }
        }
    }
}
