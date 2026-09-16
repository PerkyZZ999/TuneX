import QtQuick
import QtQuick.Controls.Basic
import TuneX

// QueuePanel: Now Playing list plus the persistent-player summary. Docked as
// the 288px right column at ≥1280px (chrome at 76%); inside the compact
// Drawer it is transparent so the drawer's subtle glass shows through
// (rows stay transparent over it, never glass).
// Transport, shuffle/repeat, reorder, and the playing marker stay here;
// MiniPlayer is the narrow-width bar. Progress scrubs through QueueModel.seekMs.
Rectangle {
    id: root

    required property QueueModel queue
    required property PlaylistModel playlists
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
    property int visualizerMode: 0
    property string spectrumCsv: ""
    property string waveformCsv: ""
    readonly property string shownTitle: Format.fallback(root.titleText, qsTr("Unknown Title"))
    readonly property string shownArtist: Format.fallback(root.artistText, qsTr("Unknown Artist"))
    readonly property string monogram: Format.monogram(root.shownTitle)
    readonly property string positionText: Format.duration(root.positionMs)
    readonly property string durationText: Format.durationOrDash(root.durationMs)
    readonly property bool hasCurrent: root.titleText !== "" || root.transportState > 0
    readonly property string mimeIds: queueSelection.mimeIds(root.queue)
    // Cached source rows of the queue selection for internal drags. The
    // stamp read keeps this subscribed; delegates read the string instead
    // of re-sorting per row.
    readonly property string rowsCsv: {
        queueSelection.stamp;
        return queueSelection.sorted().join(",");
    }

    signal browseRequested
    signal closeRequested
    signal expandRequested

    // Durations shape through the Format singleton.

    function clearSelection() {
        return queueSelection.clear();
    }

    function removeSelected() {
        queueSelection.removeSelected(root.queue);
    }

    TrackListSelection {
        id: queueSelection
    }

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
    color: root.embedded ? Appearance.chromeFill : "transparent"
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

    Timer {
        interval: 50
        running: root.tracking
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
            text: qsTr("Remove from Now Playing")
            onTriggered: root.queue.removeAt(rowMenu.rowIndex)
        }
    }

    // Clearing is destructive with no undo, so it confirms like the
    // playlist delete does. The current track keeps playing.
    GlassDialog {
        id: clearDialog

        title: qsTr("Clear Now Playing?")
        acceptLabel: qsTr("Clear")
        onAccepted: root.queue.clearQueue()

        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            text: qsTr("Remove all %1 queued tracks? The current track keeps playing.").arg(queueList.count)
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBody
            color: Theme.foreground
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
                accessibleName: qsTr("Close Now Playing")
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

            ArtStage {
                id: artWell

                queue: root.queue
                anchors.horizontalCenter: parent.horizontalCenter
                width: root.embedded ? parent.height : Theme.artThumb
                height: width
                source: root.artUrl
                monogram: root.hasCurrent ? root.monogram : qsTr("TuneX")
                monogramSize: root.embedded ? Theme.fontHeadline : Theme.fontTitle
                radius: Theme.radiusMd
                mode: root.visualizerMode
                spectrumCsv: root.spectrumCsv
                waveformCsv: root.waveformCsv
                Accessible.ignored: true
            }
        }

        Column {
            visible: root.embedded
            width: parent.width
            spacing: 0

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
                // While scrubbing, sync holds the last landed value, so the
                // label previews the drag target instead (emphasized so the
                // preview never reads as the landed position).
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
                id: panelDuration

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

                anchors.left: panelPosition.right
                anchors.leftMargin: Theme.spaceXs
                anchors.right: panelDuration.left
                anchors.rightMargin: Theme.spaceXs
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

        Row {
            id: volumeRow

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

                width: parent.width - muteButton.width - Theme.spaceXs
                from: 0
                to: 100
                stepSize: 1
                Accessible.name: qsTr("Volume")
                tipText: qsTr("%1%").arg(Math.round(value))
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
                iconName: Format.repeatIcon(root.repeatModeValue)
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
            id: upNextHeader

            width: parent.width
            height: Theme.targetMin
            spacing: Theme.spaceSm

            Text {
                width: parent.width - clearButton.width - Theme.spaceSm
                anchors.verticalCenter: parent.verticalCenter
                elide: Text.ElideRight
                text: queueList.count > 0 ? qsTr("Now Playing (%1)").arg(queueList.count) : qsTr("Now Playing")
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
                onClicked: clearDialog.open()
            }
        }

        // Errors pair the alert glyph with words, like the overlay
        // (never colour alone).
        Row {
            id: errorText

            visible: root.errorLine !== ""
            width: parent.width
            height: visible ? implicitHeight : 0
            clip: true
            spacing: Theme.spaceSm

            Icon {
                anchors.verticalCenter: parent.verticalCenter
                name: "alert"
                iconSize: 16
                stroke: Theme.error
            }

            Text {
                width: parent.width - Theme.navIconSize - Theme.spaceSm
                anchors.verticalCenter: parent.verticalCenter
                wrapMode: Text.WordWrap
                text: root.errorLine
                textFormat: Text.PlainText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontCaption
                color: Theme.error
            }
        }

        Item {
            id: content

            width: parent.width
            height: parent.height - headerRow.height - nowPlayingHit.height - progressItem.height - volumeRow.height - transportRow.height - upNextHeader.height - errorText.height - selectionBar.height - Theme.spaceMd * 8

            SelectionBar {
                id: selectionBar

                anchors.top: parent.top
                width: parent.width
                queue: root.queue
                playlists: root.playlists
                count: queueSelection.count
                trackIds: root.mimeIds
                showRemove: true
                fromLibrary: false
                onCleared: queueSelection.clear()
                onRemoveRequested: root.removeSelected()
                onPlayInPlaceRequested: {
                    const rows = queueSelection.sorted();
                    if (rows.length > 0)
                        root.queue.playAt(rows[0]);
                }
            }

            EmptyState {
                visible: queueList.count === 0 && !root.embedded
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: selectionBar.bottom
                anchors.bottom: parent.bottom
                title: qsTr("Now Playing is empty")
                note: qsTr("Play any song, album, or artist and it will queue up here.")
                actionLabel: qsTr("Browse library")
                onActionRequested: root.browseRequested()
            }

            // Same guidance as the drawer empty state, minus the
            // illustration that would crowd the narrow rail.
            EmptyState {
                visible: queueList.count === 0 && root.embedded
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: selectionBar.bottom
                anchors.bottom: parent.bottom
                title: qsTr("Now Playing is empty")
                note: qsTr("Play any song, album, or artist and it will queue up here.")
                actionLabel: qsTr("Browse library")
                onActionRequested: root.browseRequested()
            }

            ListView {
                id: queueList
                ScrollBar.vertical: ListScrollBar {}

                visible: count > 0
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: selectionBar.bottom
                anchors.bottom: parent.bottom
                model: root.queue
                activeFocusOnTab: true
                clip: true
                spacing: Theme.listRowGap
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
                Accessible.selectable: true
                Accessible.name: qsTr("Now Playing")
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
                    if (queueSelection.count > 0) {
                        root.removeSelected();
                        return;
                    }
                    if (queueList.currentIndex >= 0 && queueList.currentIndex < queueList.count)
                        root.queue.removeAt(queueList.currentIndex);
                }
                Keys.onPressed: event => {
                    if ((event.modifiers & Qt.ControlModifier) && event.key === Qt.Key_A) {
                        queueSelection.selectAll(queueList.count);
                        event.accepted = true;
                        return;
                    }
                    if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier))) {
                        const at = queueList.currentIndex >= 0 ? queueList.currentIndex : 0;
                        if (at < queueList.count) {
                            queueList.currentIndex = at;
                            rowMenu.rowIndex = at;
                            rowMenu.popup();
                            event.accepted = true;
                        }
                    }
                }
                Keys.onUpPressed: event => {
                    if (event.modifiers & Qt.AltModifier) {
                        const from = queueList.currentIndex;
                        if (from > 0) {
                            root.queue.moveItem(from, from - 1);
                            queueList.currentIndex = from - 1;
                        }
                        event.accepted = true;
                        return;
                    }
                    if (event.modifiers & Qt.ShiftModifier) {
                        const next = Math.max(0, queueList.currentIndex - 1);
                        queueList.currentIndex = next;
                        queueSelection.setRange(next);
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
                        return;
                    }
                    if (event.modifiers & Qt.ShiftModifier) {
                        const next = Math.min(queueList.count - 1, queueList.currentIndex + 1);
                        queueList.currentIndex = next;
                        queueSelection.setRange(next);
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
                    compact: true
                    reorderable: true
                    selected: queueSelection.contains(index)
                    dragTrackIds: selected && root.mimeIds !== "" ? root.mimeIds : (model.trackId >= 0 ? String(model.trackId) : "")
                    dragOrigin: "queue"
                    dragRows: selected ? root.rowsCsv : String(index)
                    onPlayRequested: (trackId, rowIndex) => {
                        queueSelection.clear();
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
                    onToggleSelectRequested: row => {
                        queueList.currentIndex = row;
                        queueSelection.toggle(row);
                    }
                    onRangeSelectRequested: row => {
                        queueList.currentIndex = row;
                        queueSelection.setRange(row);
                    }
                }
            }

            // Positional landing: the insertion line shows the index, an
            // internal drag moves rows there, an external one inserts.
            TrackDropArea {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: selectionBar.bottom
                anchors.bottom: parent.bottom
                z: 2
                targetView: queueList
                showHint: false
                onTracksDroppedAt: (ids, index, origin, rows) => {
                    if (origin === "queue" && rows !== "")
                        root.queue.moveItems(rows, index);
                    else
                        root.queue.enqueueTrackIdsAt(ids, index);
                    queueSelection.clear();
                    // Leave the keyboard cursor where the rows landed.
                    if (queueList.count > 0)
                        queueList.currentIndex = Math.max(0, Math.min(index, queueList.count - 1));
                }
            }
        }
    }
}
