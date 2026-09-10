import QtQuick
import QtQuick.Controls.Basic
import TuneX 1.0

// QueuePanel (S3 W-022, S4 W-026): Up Next list plus the persistent-player
// summary. Hosted as the 320px right column at ≥1280px, or inside a right
// Drawer below that. Transport, shuffle/repeat, reorder, and the playing
// marker stay here; MiniPlayer is the narrow-width bar. Progress is
// read-only until W-027.
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

    }

    Column {
        anchors.fill: parent
        anchors.margins: Theme.spaceLg
        spacing: Theme.spaceMd

        Row {
            id: headerRow

            width: parent.width
            clip: true
            spacing: Theme.spaceSm

            Text {
                width: parent.width - clearButton.width - closeButton.width - Theme.spaceSm * 2
                anchors.verticalCenter: parent.verticalCenter
                elide: Text.ElideRight
                text: queueList.count > 0 ? qsTr("Up Next (%1)").arg(queueList.count) : qsTr("Up Next")
                textFormat: Text.PlainText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontTitle
                font.weight: Font.DemiBold
                color: Theme.foreground
            }

            Button {
                id: clearButton

                text: qsTr("Clear")
                enabled: queueList.count > 0
                Accessible.name: qsTr("Clear the queue (keeps playing)")
                onClicked: root.queue.clearQueue()
            }

            Button {
                id: closeButton

                visible: !root.embedded
                width: visible ? implicitWidth : 0
                text: qsTr("Close")
                onClicked: root.closeRequested()
            }

        }

        Row {
            id: nowPlayingRow

            visible: root.hasCurrent
            width: parent.width
            height: visible ? Theme.artThumb : 0
            spacing: Theme.spaceSm

            Rectangle {
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
                width: parent.width - Theme.artThumb - Theme.spaceSm
                anchors.verticalCenter: parent.verticalCenter
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

        // Panel transport: previous, play/pause with text state, next.
        Row {
            id: transportRow

            width: parent.width
            spacing: Theme.spaceSm

            Button {
                text: qsTr("Previous")
                enabled: queueList.count > 0
                onClicked: root.queue.previousTrack(root.queue.positionMs())
            }

            Button {
                text: root.transportState === 2 ? qsTr("Pause") : qsTr("Play")
                enabled: queueList.count > 0 || root.transportState === 2 || root.transportState === 3
                onClicked: {
                    root.queue.playPause();
                    // Optimistic flip: the real state arrives via the poll
                    // timers, so assume the toggle landed (polls confirm).
                    root.transportState = root.transportState === 2 ? 3 : 2;
                }
            }

            Button {
                text: qsTr("Next")
                enabled: queueList.count > 0
                onClicked: root.queue.nextTrack()
            }

        }

        // Shuffle/repeat toggles: text state, never color-only.
        Row {
            id: togglesRow

            width: parent.width
            spacing: Theme.spaceSm

            Button {
                text: root.shuffleOn ? qsTr("Shuffle: On") : qsTr("Shuffle: Off")
                checkable: true
                checked: root.shuffleOn
                onClicked: {
                    root.queue.toggleShuffle();
                    root.sync();
                }
            }

            Button {
                text: root.repeatLabel
                onClicked: {
                    root.queue.cycleRepeat();
                    root.sync();
                }
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
            height: parent.height - headerRow.height - nowPlayingRow.height - progressItem.height - volumeRow.height - transportRow.height - togglesRow.height - errorText.height - Theme.spaceMd * 7

            EmptyState {
                visible: queueList.count === 0
                title: qsTr("Up Next is empty")
                note: qsTr("Play any song, album, or artist and it will queue up here.")
                actionLabel: qsTr("Browse library")
                onActionRequested: root.browseRequested()
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
