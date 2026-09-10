import QtQuick
import QtQuick.Controls.Basic
import TuneX 1.0

// QueuePanel (S3 W-022): the Up Next drawer — now-playing transport,
// shuffle/repeat toggles, and the reorder-capable queue list with the
// playing row marked (accent bar + bold title + screen-reader label, never
// color alone). Rows play on click; the ⋯ menu moves or removes them, and
// Alt+Up/Down reorders from the keyboard. The model polls on the App timer
// (playback advances with the drawer closed); this timer only mirrors
// display state and follows the cursor while open.
Drawer {
    id: root

    required property QueueModel queue
    // Local mirrors of model state (functions carry no notifiers).
    property int transportState: 0
    property bool shuffleOn: false
    property string repeatLabel: qsTr("Repeat: Off")
    property string errorLine: ""
    property int seenCursor: -2

    signal browseRequested()

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

    width: Theme.panelWidth
    height: parent.height
    edge: Qt.RightEdge
    onOpened: root.sync()

    Timer {
        interval: 300
        running: root.opened
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

                text: qsTr("Close")
                onClicked: root.close()
            }

        }

        // Panel transport (interim home until the S4 persistent player):
        // previous, play/pause with text state, next.
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
            height: parent.height - headerRow.height - transportRow.height - togglesRow.height - errorText.height - Theme.spaceMd * 4

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
