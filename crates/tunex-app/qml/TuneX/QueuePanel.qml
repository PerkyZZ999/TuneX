import QtQuick
import QtQuick.Controls.Basic
import TuneX 1.0

// QueuePanel (S3 W-022, S4 W-026): Up Next list plus the persistent-player
// summary. Hosted as the 320px right column at ≥1280px, or inside a right
// Drawer below that. Transport, shuffle/repeat, reorder, and the playing
// marker stay here; MiniPlayer is the narrow-width bar. Progress here is
// read-only; scrub lives on the Now Playing overlay (W-028).
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
    property string repeatLabel: qsTr("Repeat: Off")
    property string errorLine: ""
    property int seenCursor: -2
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
    readonly property bool hasCurrent: root.titleText !== "" || root.transportState > 0

    signal browseRequested()
    signal closeRequested()
    signal expandRequested()

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
        if (!volumeSlider.pressed)
            volumeSlider.value = root.queue.volumePct();

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

    color: Theme.surface
    onTrackingChanged: {
        if (root.tracking)
            root.sync();

    }

    Rectangle {
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

    Menu {
        id: rowMenu

        property int rowIndex: -1

        MenuItem {
            text: qsTr("Play now")
            onTriggered: root.queue.playAt(rowMenu.rowIndex)
        }

        MenuItem {
            text: qsTr("Move up")
            onTriggered: root.queue.moveItem(rowMenu.rowIndex, rowMenu.rowIndex - 1)
        }

        MenuItem {
            text: qsTr("Move down")
            onTriggered: root.queue.moveItem(rowMenu.rowIndex, rowMenu.rowIndex + 1)
        }

        MenuItem {
            text: qsTr("Remove from Up Next")
            onTriggered: root.queue.removeAt(rowMenu.rowIndex)
        }

        background: GlassBackdrop {
            cornerRadius: Theme.radiusLg
            transparencyOff: root.queue.reduceTransparency()
            disableBlur: true
        }

    }

    Column {
        anchors.fill: parent
        anchors.margins: Theme.spaceLg
        spacing: Theme.spaceMd

        Row {
            id: headerRow

            visible: !root.embedded
            width: parent.width
            height: visible ? Theme.targetMin : 0
            clip: true

            Item {
                width: parent.width - closeButton.width
                height: 1
            }

            PrimaryButton {
                id: closeButton

                visible: !root.embedded
                primary: false
                text: qsTr("Close")
                onClicked: root.closeRequested()
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

            Rectangle {
                id: artWell

                anchors.horizontalCenter: parent.horizontalCenter
                width: root.embedded ? parent.height : Theme.artThumb
                height: width
                radius: Theme.radiusMd
                color: Theme.surfaceRaised
                Accessible.ignored: true

                Text {
                    anchors.centerIn: parent
                    text: root.hasCurrent ? root.monogram : qsTr("TuneX")
                    textFormat: Text.PlainText
                    font.family: Theme.fontFamily
                    font.pixelSize: root.embedded ? Theme.fontHeadline : Theme.fontTitle
                    font.weight: Font.DemiBold
                    color: Theme.muted
                }

            }

        }

        Column {
            visible: root.embedded
            width: parent.width
            spacing: Theme.spaceXs

            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
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
                horizontalAlignment: Text.AlignHCenter
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
            height: visible ? Theme.spaceLg : 0
            Accessible.role: Accessible.StaticText
            Accessible.name: qsTr("Playback position %1 of %2").arg(root.positionText).arg(root.durationText)

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

            Rectangle {
                anchors.left: panelPosition.right
                anchors.leftMargin: Theme.spaceXs
                anchors.right: panelDuration.left
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

        Row {
            id: volumeRow

            width: parent.width
            spacing: Theme.spaceXs

            IconButton {
                id: muteButton

                iconName: "volume"
                accessibleName: root.muted ? qsTr("Unmute") : qsTr("Mute")
                checked: root.muted
                onActivated: {
                    root.queue.setMuted(!root.muted);
                    root.muted = !root.muted;
                }
            }

            Slider {
                id: volumeSlider

                width: parent.width - muteButton.width - Theme.spaceXs
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

        }

        Row {
            id: transportRow

            width: parent.width
            spacing: Theme.spaceXs

            IconButton {
                iconName: "shuffle"
                accessibleName: root.shuffleOn ? qsTr("Shuffle: On") : qsTr("Shuffle: Off")
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

            Item {
                width: Theme.playPrimary
                height: Theme.playPrimary
                Accessible.role: Accessible.Button
                Accessible.name: root.transportState === 2 ? qsTr("Pause") : qsTr("Play")
                enabled: queueList.count > 0 || root.transportState === 2 || root.transportState === 3
                Keys.onSpacePressed: playHit.clicked(null)
                activeFocusOnTab: true

                Rectangle {
                    anchors.fill: parent
                    radius: width / 2
                    color: Theme.primary
                    border.width: parent.activeFocus ? 2 : 0
                    border.color: Theme.focus
                }

                Icon {
                    anchors.centerIn: parent
                    name: root.transportState === 2 ? "pause" : "play"
                    iconSize: 24
                    stroke: Theme.primaryText
                }

                MouseArea {
                    id: playHit

                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        root.queue.playPause();
                        root.transportState = root.transportState === 2 ? 3 : 2;
                    }
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
                iconName: "repeat"
                accessibleName: root.repeatLabel
                checked: root.queue.repeatMode() !== 0
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
                highlightMoveDuration: 120
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
                Keys.onUpPressed: (event) => {
                    if (event.modifiers & Qt.AltModifier) {
                        const from = queueList.currentIndex;
                        if (from > 0) {
                            root.queue.moveItem(from, from - 1);
                            queueList.currentIndex = from - 1;
                        }
                        event.accepted = true;
                    }
                }
                Keys.onDownPressed: (event) => {
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
                }

            }

        }

    }

}
