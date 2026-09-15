import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import TuneX

// SettingsView (IA list-detail): Library, Playback, Notifications,
// Appearance, Shortcuts. Presentation only — models own the writes.
Item {
    id: root

    required property QueueModel queue
    required property LibraryManager library
    required property TrayController tray
    property string section: "library"
    property bool scanning: false
    property string statusLine: ""
    property string errorLine: ""
    property int folderTotal: 0
    property bool watching: false
    property bool shuffleOn: false
    property int repeatModeValue: 0
    property int volumePct: 100
    property int replayGainModeValue: 0
    property int crossfadeSecs: 0
    property string outputLabel: qsTr("System")
    property bool closeToTray: true
    property bool trayAvailable: false
    property bool reduceMotionOn: false
    property bool reduceTransparencyOn: false
    property bool notificationsOn: true
    property bool notifyTrackChangeOn: true
    property bool notifyErrorsOn: true
    property bool eqOn: false
    property string eqPreset: "flat"
    property bool eqMissing: false
    property int eqStamp: 0
    property int visualizerMode: 0
    property int visualizerFps: 20
    property int profileRows: 0
    property string activeProfileId: ""
    property bool musicbrainzOn: false

    signal libraryReopened
    readonly property list<string> sections: ["library", "playback", "notifications", "appearance", "shortcuts"]
    readonly property list<string> shortcutKeys: [qsTr("Space"), qsTr("Media Play / Pause / Next / Previous"), qsTr("Volume Up / Down / Mute"), qsTr("/ or Ctrl+K"), qsTr("Esc"), qsTr("Ctrl+A"), qsTr("Shift+click / Shift+↑↓"), qsTr("Alt+Left / Alt+Right"), qsTr("↑ ↓ ← →"), qsTr("j / k"), qsTr("Enter"), qsTr("Type in Tracks"), qsTr("Tab / Shift+Tab in Search")]
    readonly property list<string> shortcutActions: [qsTr("Play / pause"), qsTr("Play, next, previous"), qsTr("Volume and mute"), qsTr("Search"), qsTr("Clear selection, then close Now Playing, then search, then back"), qsTr("Select all loaded rows"), qsTr("Extend the track selection"), qsTr("Back / forward"), qsTr("Move list and grid cursor"), qsTr("Move list and grid cursor"), qsTr("Play the current track, or open the current album or artist"), qsTr("Jump to the first title with that prefix (400 ms reset)"), qsTr("Cycle Tracks, Albums, and Artists result groups")]
    readonly property string repeatLabel: {
        if (root.repeatModeValue === 1)
            return qsTr("All tracks");
        if (root.repeatModeValue === 2)
            return qsTr("One track");
        return qsTr("Off");
    }
    readonly property string replayGainLabel: {
        if (root.replayGainModeValue === 1)
            return qsTr("Track");
        if (root.replayGainModeValue === 2)
            return qsTr("Album");
        return qsTr("Off");
    }

    function sectionLabel(key) {
        if (key === "playback")
            return qsTr("Playback");
        if (key === "notifications")
            return qsTr("Notifications");
        if (key === "appearance")
            return qsTr("Appearance");
        if (key === "shortcuts")
            return qsTr("Shortcuts");
        return qsTr("Library");
    }

    function sectionIcon(key) {
        if (key === "playback")
            return "sliders-horizontal";
        if (key === "notifications")
            return "bell";
        if (key === "appearance")
            return "blend";
        if (key === "shortcuts")
            return "keyboard";
        return "folder";
    }

    function cycleEqPreset() {
        const names = ["flat", "hip-hop", "rock", "jazz", "classic", "vocals", "electronic", "pop"];
        let at = names.indexOf(root.eqPreset);
        at = at < 0 ? 0 : (at + 1) % names.length;
        root.queue.setEqPreset(names[at]);
        root.eqStamp = root.eqStamp + 1;
        root.sync();
    }

    function cycleVisualizer() {
        root.queue.setVisualizerMode((root.visualizerMode + 1) % 4);
        root.sync();
    }

    function watcherLine() {
        if (root.folderTotal === 0)
            return qsTr("Add a music folder to start watching for new files.");
        if (root.watching)
            return qsTr("Watching folders for new files.");
        return qsTr("Watcher idle — rescan still works.");
    }

    function sync() {
        root.library.poll();
        root.scanning = root.library.isScanning();
        root.statusLine = root.library.statusText();
        root.folderTotal = root.library.folderCount();
        root.errorLine = root.library.errorText();
        root.watching = root.library.isWatching();
        root.shuffleOn = root.queue.isShuffle();
        root.repeatModeValue = root.queue.repeatMode();
        if (!volumeSlider.pressed)
            root.volumePct = root.queue.volumePct();
        root.replayGainModeValue = root.queue.replayGainMode();
        if (!crossfadeSlider.pressed)
            root.crossfadeSecs = root.queue.crossfadeSecs();
        root.outputLabel = qsTr("System");
        const wanted = String(root.queue.outputDevice());
        for (let i = 0; i < root.queue.outputCount(); i++) {
            if (String(root.queue.outputIdAt(i)) === wanted) {
                root.outputLabel = root.queue.outputLabelAt(i);
                break;
            }
        }
        root.closeToTray = root.tray.closeToTray();
        root.trayAvailable = root.tray.isAvailable();
        root.reduceMotionOn = root.queue.reduceMotion();
        root.reduceTransparencyOn = root.queue.reduceTransparency();
        root.notificationsOn = root.queue.notificationsEnabled();
        root.notifyTrackChangeOn = root.queue.notifyTrackChange();
        root.notifyErrorsOn = root.queue.notifyPlaybackErrors();
        root.profileRows = root.library.profileCount();
        root.activeProfileId = root.library.activeProfile();
        root.musicbrainzOn = root.library.musicbrainzOn();
        root.eqOn = root.queue.eqEnabled();
        root.eqPreset = String(root.queue.eqPreset());
        root.eqMissing = root.queue.eqMissing();
        root.visualizerMode = root.queue.visualizerMode();
        root.visualizerFps = root.queue.visualizerFps();
    }

    function requestAddFolder() {
        folderPicker.open();
    }

    anchors.fill: parent
    onVisibleChanged: {
        if (root.visible)
            root.sync();
        if (root.visible && root.section === "playback")
            root.queue.refreshOutputs();
    }
    onSectionChanged: {
        if (root.section === "playback")
            root.queue.refreshOutputs();
    }
    Accessible.role: Accessible.Pane
    Accessible.name: qsTr("Settings")

    Timer {
        interval: 300
        running: root.visible
        repeat: true
        onTriggered: root.sync()
    }

    FolderDialog {
        id: folderPicker

        title: qsTr("Choose a music folder")
        onAccepted: {
            const picked = decodeURIComponent(String(selectedFolder).replace("file://", ""));
            root.library.addFolder(picked);
            root.sync();
        }
    }

    GlassDialog {
        id: profileDialog

        function openBlank() {
            nameField.text = "";
            profileDialog.open();
            nameField.forceActiveFocus();
        }

        title: qsTr("New library profile")
        acceptLabel: qsTr("Create")
        acceptEnabled: nameField.text.trim() !== ""
        onAccepted: {
            const id = root.library.createProfile(nameField.text);
            root.errorLine = root.library.errorText();
            if (id !== "") {
                root.library.switchProfile(id);
                root.libraryReopened();
                root.sync();
            }
        }

        TextField {
            id: nameField

            width: parent.width
            implicitHeight: Theme.targetMin
            placeholderText: qsTr("Profile name")
            maximumLength: 80
            color: Theme.foreground
            placeholderTextColor: Theme.muted
            selectionColor: Theme.primary
            selectedTextColor: Theme.primaryText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBody
            leftPadding: Theme.spaceMd
            rightPadding: Theme.spaceMd
            Accessible.name: qsTr("Profile name")
            onAccepted: {
                if (profileDialog.acceptEnabled)
                    profileDialog.accept();
            }

            background: Rectangle {
                radius: Theme.radiusSm
                color: Theme.chrome
                border.color: nameField.activeFocus ? Theme.focus : Theme.border
                border.width: nameField.activeFocus ? 2 : 1
            }
        }
    }

    Column {
        anchors.fill: parent
        anchors.margins: Theme.spaceLg
        spacing: Theme.spaceLg

        Text {
            id: heading

            text: qsTr("Settings")
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontHeadline
            font.weight: Font.Bold
            color: Theme.foreground
            Accessible.role: Accessible.Heading
            Accessible.name: text
        }

        Row {
            width: parent.width
            height: parent.height - heading.height - Theme.spaceLg
            spacing: Theme.spaceXl

            Column {
                width: 220
                height: parent.height
                spacing: Theme.spaceXs

                Repeater {
                    model: root.sections

                    Item {
                        id: sectionRow

                        required property string modelData

                        width: 220
                        height: Theme.targetMin
                        activeFocusOnTab: true
                        Accessible.role: Accessible.Button
                        Accessible.name: root.sectionLabel(sectionRow.modelData)
                        Accessible.selected: root.section === sectionRow.modelData
                        Keys.onSpacePressed: root.section = sectionRow.modelData
                        Keys.onReturnPressed: root.section = sectionRow.modelData
                        Keys.onEnterPressed: root.section = sectionRow.modelData

                        Rectangle {
                            anchors.fill: parent
                            radius: Theme.radiusSm
                            color: root.section === sectionRow.modelData ? Theme.selected : (sectionMouse.containsMouse || sectionRow.activeFocus ? Theme.hover : "transparent")
                            Behavior on color {
                                ColorAnimation {
                                    duration: Appearance.duration(Theme.motionHover)
                                    easing.type: Easing.OutCubic
                                }
                            }
                        }

                        Rectangle {
                            width: 3
                            anchors.top: parent.top
                            anchors.bottom: parent.bottom
                            anchors.left: parent.left
                            anchors.leftMargin: 4
                            radius: 2
                            color: Theme.accent
                            visible: root.section === sectionRow.modelData
                        }

                        Icon {
                            id: sectionGlyph

                            anchors.verticalCenter: parent.verticalCenter
                            anchors.left: parent.left
                            anchors.leftMargin: Theme.spaceMd
                            name: root.sectionIcon(sectionRow.modelData)
                            iconSize: 20
                            stroke: root.section === sectionRow.modelData ? Theme.foreground : Theme.muted
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.left: sectionGlyph.right
                            anchors.leftMargin: Theme.spaceSm
                            anchors.right: parent.right
                            anchors.rightMargin: Theme.spaceSm
                            elide: Text.ElideRight
                            text: root.sectionLabel(sectionRow.modelData)
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontLabel
                            font.weight: Font.Medium
                            color: root.section === sectionRow.modelData ? Theme.foreground : Theme.muted
                        }

                        MouseArea {
                            id: sectionMouse

                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.section = sectionRow.modelData
                        }

                        Rectangle {
                            anchors.fill: parent
                            radius: Theme.radiusSm
                            color: "transparent"
                            border.width: 2
                            border.color: Theme.focus
                            visible: sectionRow.activeFocus
                        }
                    }
                }
            }

            Rectangle {
                width: 1
                height: parent.height
                color: Theme.border
            }

            Flickable {
                width: parent.width - 220 - 1 - Theme.spaceXl * 2
                ScrollBar.vertical: ListScrollBar {}
                height: parent.height
                clip: true
                contentWidth: width
                contentHeight: detailColumn.height
                boundsBehavior: Flickable.StopAtBounds
                Accessible.role: Accessible.Pane
                Accessible.name: root.sectionLabel(root.section)

                Column {
                    id: detailColumn

                    width: parent.width
                    spacing: Theme.spaceLg

                    Column {
                        visible: root.section === "library"
                        width: parent.width
                        spacing: Theme.spaceMd

                        Text {
                            text: qsTr("Music folders")
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontTitle
                            font.weight: Font.DemiBold
                            color: Theme.foreground
                            Accessible.role: Accessible.Heading
                            Accessible.name: text
                        }

                        Text {
                            width: parent.width
                            wrapMode: Text.WordWrap
                            text: root.statusLine
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontBodySm
                            font.features: {
                                "tnum": 1
                            }
                            color: Theme.muted
                        }

                        Text {
                            width: parent.width
                            wrapMode: Text.WordWrap
                            text: root.watcherLine()
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontCaption
                            color: Theme.muted
                        }

                        Rectangle {
                            visible: root.scanning
                            width: parent.width
                            height: Theme.progressTrack
                            radius: Theme.radiusXs
                            color: Theme.hover
                            clip: true
                            Accessible.role: Accessible.ProgressBar
                            Accessible.name: qsTr("Scan in progress")

                            Rectangle {
                                id: scanSegment

                                width: parent.width * 0.3
                                height: parent.height
                                radius: Theme.radiusXs
                                color: Theme.accentSecondary

                                XAnimator on x {
                                    from: -scanSegment.width
                                    to: scanSegment.parent.width
                                    duration: 1200
                                    loops: Animation.Infinite
                                    running: root.scanning && root.visible && root.section === "library" && !Appearance.reduceMotion
                                }
                            }
                        }

                        Row {
                            visible: root.errorLine !== ""
                            width: parent.width
                            spacing: Theme.spaceSm

                            Icon {
                                name: "alert"
                                iconSize: 16
                                stroke: Theme.error
                            }

                            Text {
                                width: parent.width - Theme.spaceMd - Theme.spaceSm
                                wrapMode: Text.WordWrap
                                text: root.errorLine
                                textFormat: Text.PlainText
                                font.family: Theme.fontFamily
                                font.pixelSize: Theme.fontBodySm
                                color: Theme.error
                            }
                        }

                        EmptyState {
                            visible: root.folderTotal === 0
                            width: parent.width
                            centerInParent: false
                            title: qsTr("No music folders")
                            note: qsTr("Add a folder to scan your library. TuneX stays offline and only reads files you pick.")
                            actionLabel: qsTr("Add folder…")
                            onActionRequested: root.requestAddFolder()
                        }

                        Rectangle {
                            visible: root.folderTotal > 0
                            width: parent.width
                            height: folderList.height + Theme.spaceSm * 2
                            radius: Theme.radiusMd
                            color: Theme.surface
                            border.width: 1
                            border.color: Theme.border

                            Column {
                                id: folderList

                                y: Theme.spaceSm
                                width: parent.width
                                spacing: 0

                                Repeater {
                                    model: root.folderTotal

                                    Item {
                                        id: folderRow

                                        required property int index

                                        width: folderList.width
                                        height: Theme.targetMin

                                        Rectangle {
                                            visible: folderRow.index > 0
                                            anchors.left: parent.left
                                            anchors.right: parent.right
                                            anchors.leftMargin: Theme.spaceMd
                                            anchors.rightMargin: Theme.spaceMd
                                            height: 1
                                            color: Theme.border
                                            Accessible.ignored: true
                                        }

                                        Icon {
                                            id: folderGlyph

                                            anchors.left: parent.left
                                            anchors.leftMargin: Theme.spaceMd
                                            anchors.verticalCenter: parent.verticalCenter
                                            name: "folder"
                                            iconSize: 20
                                            stroke: Theme.muted
                                        }

                                        Text {
                                            anchors.left: folderGlyph.right
                                            anchors.leftMargin: Theme.spaceSm
                                            anchors.right: removeButton.left
                                            anchors.rightMargin: Theme.spaceSm
                                            anchors.verticalCenter: parent.verticalCenter
                                            elide: Text.ElideMiddle
                                            text: root.library.folderAt(folderRow.index)
                                            textFormat: Text.PlainText
                                            font.family: Theme.fontFamily
                                            font.pixelSize: Theme.fontBody
                                            color: Theme.foreground
                                        }

                                        PrimaryButton {
                                            id: removeButton

                                            anchors.right: parent.right
                                            anchors.rightMargin: Theme.spaceSm
                                            anchors.verticalCenter: parent.verticalCenter
                                            primary: false
                                            text: qsTr("Remove")
                                            Accessible.name: qsTr("Remove %1").arg(root.library.folderAt(folderRow.index))
                                            onClicked: {
                                                root.library.removeFolder(root.library.folderAt(folderRow.index));
                                                root.sync();
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        Row {
                            visible: root.folderTotal > 0
                            spacing: Theme.spaceSm

                            PrimaryButton {
                                glyph: "folder-plus"
                                text: qsTr("Add folder…")
                                onClicked: root.requestAddFolder()
                            }

                            PrimaryButton {
                                primary: false
                                glyph: "refresh"
                                text: qsTr("Rescan")
                                Accessible.name: qsTr("Rescan music folders now")
                                enabled: !root.scanning
                                onClicked: {
                                    root.library.rescan();
                                    root.sync();
                                }
                            }
                        }

                        Text {
                            text: qsTr("Library profile")
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontTitle
                            font.weight: Font.DemiBold
                            color: Theme.foreground
                            Accessible.role: Accessible.Heading
                            Accessible.name: text
                        }

                        Text {
                            width: parent.width
                            wrapMode: Text.WordWrap
                            text: qsTr("Each profile has its own index and folders. Switching reopens the library without accounts.")
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontBody
                            color: Theme.muted
                        }

                        Flow {
                            width: parent.width
                            spacing: Theme.spaceSm

                            Repeater {
                                model: root.profileRows

                                Chip {
                                    required property int index
                                    label: root.library.profileNameAt(index)
                                    selected: root.library.profileIdAt(index) === root.activeProfileId
                                    onActivated: {
                                        const id = root.library.profileIdAt(index);
                                        if (id === root.activeProfileId)
                                            return;
                                        root.library.switchProfile(id);
                                        root.libraryReopened();
                                        root.sync();
                                    }
                                }
                            }
                        }

                        PrimaryButton {
                            primary: false
                            glyph: "plus"
                            text: qsTr("New profile")
                            Accessible.name: qsTr("New library profile")
                            onClicked: profileDialog.openBlank()
                        }

                        SettingsToggle {
                            width: parent.width
                            title: qsTr("Look up missing tags online")
                            description: qsTr("Opt-in MusicBrainz. Off by default. Never overwrites tags you already have. Core playback stays offline.")
                            checked: root.musicbrainzOn
                            onToggled: {
                                root.library.setMusicbrainzOn(!root.musicbrainzOn);
                                root.sync();
                            }
                        }
                    }

                    Column {
                        visible: root.section === "playback"
                        width: parent.width
                        spacing: Theme.spaceMd

                        Text {
                            text: qsTr("Playback")
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontTitle
                            font.weight: Font.DemiBold
                            color: Theme.foreground
                            Accessible.role: Accessible.Heading
                            Accessible.name: text
                        }

                        Text {
                            width: parent.width
                            wrapMode: Text.WordWrap
                            text: qsTr("Playback is gapless between consecutive tracks. ReplayGain sits after the volume slider. A crossfade of 0 seconds keeps the gapless cut. Reduce motion does not turn the fade off.")
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontBody
                            color: Theme.muted
                        }

                        Text {
                            text: qsTr("Volume")
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontLabel
                            font.weight: Font.Medium
                            color: Theme.foreground
                        }

                        ProgressSlider {
                            id: volumeSlider

                            width: Math.min(parent.width, 320)
                            from: 0
                            to: 100
                            value: root.volumePct
                            Accessible.name: qsTr("Volume")
                            onMoved: root.queue.setVolumePct(Math.round(value))
                        }

                        SettingsToggle {
                            width: parent.width
                            title: qsTr("Shuffle")
                            description: qsTr("Play Now Playing in shuffled order.")
                            checked: root.shuffleOn
                            onToggled: {
                                root.queue.toggleShuffle();
                                root.sync();
                            }
                        }

                        Item {
                            width: parent.width
                            height: Math.max(Theme.targetMin, repeatCopy.implicitHeight + Theme.spaceSm * 2)
                            activeFocusOnTab: true
                            Accessible.role: Accessible.Button
                            Accessible.name: qsTr("Repeat") + ", " + root.repeatLabel
                            Keys.onSpacePressed: {
                                root.queue.cycleRepeat();
                                root.sync();
                            }
                            Keys.onReturnPressed: {
                                root.queue.cycleRepeat();
                                root.sync();
                            }
                            Keys.onEnterPressed: {
                                root.queue.cycleRepeat();
                                root.sync();
                            }

                            Rectangle {
                                anchors.fill: parent
                                radius: Theme.radiusSm
                                color: repeatMouse.containsMouse || parent.activeFocus ? Theme.hover : "transparent"
                                border.width: parent.activeFocus ? 2 : 0
                                border.color: Theme.focus
                            }

                            Column {
                                id: repeatCopy

                                anchors.left: parent.left
                                anchors.right: repeatButton.left
                                anchors.rightMargin: Theme.spaceMd
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: Theme.spaceXs

                                Text {
                                    text: qsTr("Repeat")
                                    textFormat: Text.PlainText
                                    font.family: Theme.fontFamily
                                    font.pixelSize: Theme.fontBody
                                    font.weight: Font.Medium
                                    color: Theme.foreground
                                }

                                Text {
                                    text: root.repeatLabel
                                    textFormat: Text.PlainText
                                    font.family: Theme.fontFamily
                                    font.pixelSize: Theme.fontBodySm
                                    color: Theme.muted
                                }
                            }

                            IconButton {
                                id: repeatButton

                                anchors.right: parent.right
                                anchors.verticalCenter: parent.verticalCenter
                                iconName: root.repeatModeValue === 2 ? "repeat-1" : "repeat"
                                accessibleName: qsTr("Cycle repeat") + ", " + root.repeatLabel
                                checkable: true
                                checked: root.repeatModeValue !== 0
                                onActivated: {
                                    root.queue.cycleRepeat();
                                    root.sync();
                                }
                            }

                            MouseArea {
                                id: repeatMouse

                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    root.queue.cycleRepeat();
                                    root.sync();
                                }
                            }
                        }

                        Item {
                            width: parent.width
                            height: Math.max(Theme.targetMin, replayCopy.implicitHeight + Theme.spaceSm * 2)
                            activeFocusOnTab: true
                            Accessible.role: Accessible.Button
                            Accessible.name: qsTr("ReplayGain") + ", " + root.replayGainLabel
                            Keys.onSpacePressed: {
                                root.queue.cycleReplayGain();
                                root.sync();
                            }
                            Keys.onReturnPressed: {
                                root.queue.cycleReplayGain();
                                root.sync();
                            }
                            Keys.onEnterPressed: {
                                root.queue.cycleReplayGain();
                                root.sync();
                            }

                            Rectangle {
                                anchors.fill: parent
                                radius: Theme.radiusSm
                                color: replayMouse.containsMouse || parent.activeFocus ? Theme.hover : "transparent"
                                border.width: parent.activeFocus ? 2 : 0
                                border.color: Theme.focus
                            }

                            Column {
                                id: replayCopy

                                anchors.left: parent.left
                                anchors.right: replayButton.left
                                anchors.rightMargin: Theme.spaceMd
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: Theme.spaceXs

                                Text {
                                    text: qsTr("ReplayGain")
                                    textFormat: Text.PlainText
                                    font.family: Theme.fontFamily
                                    font.pixelSize: Theme.fontBody
                                    font.weight: Font.Medium
                                    color: Theme.foreground
                                }

                                Text {
                                    text: root.replayGainLabel
                                    textFormat: Text.PlainText
                                    font.family: Theme.fontFamily
                                    font.pixelSize: Theme.fontBodySm
                                    color: Theme.muted
                                }
                            }

                            IconButton {
                                id: replayButton

                                anchors.right: parent.right
                                anchors.verticalCenter: parent.verticalCenter
                                iconName: "volume"
                                accessibleName: qsTr("Cycle ReplayGain") + ", " + root.replayGainLabel
                                onActivated: {
                                    root.queue.cycleReplayGain();
                                    root.sync();
                                }
                            }

                            MouseArea {
                                id: replayMouse

                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    root.queue.cycleReplayGain();
                                    root.sync();
                                }
                            }
                        }

                        Text {
                            text: qsTr("Crossfade")
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontLabel
                            font.weight: Font.Medium
                            color: Theme.foreground
                        }

                        Text {
                            width: parent.width
                            text: root.crossfadeSecs === 0 ? qsTr("Off — gapless cut") : qsTr("%1 seconds").arg(root.crossfadeSecs)
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontBodySm
                            color: Theme.muted
                        }

                        ProgressSlider {
                            id: crossfadeSlider

                            width: Math.min(parent.width, 320)
                            from: 0
                            to: 12
                            stepSize: 1
                            value: root.crossfadeSecs
                            Accessible.name: qsTr("Crossfade")
                            onMoved: {
                                root.queue.setCrossfadeSecs(Math.round(value));
                                root.sync();
                            }
                        }

                        Text {
                            text: qsTr("Equalizer")
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontLabel
                            font.weight: Font.Medium
                            color: Theme.foreground
                        }

                        Text {
                            visible: root.eqMissing
                            width: parent.width
                            wrapMode: Text.WordWrap
                            text: qsTr("equalizer-10bands is unavailable. Playback continues flat.")
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontBodySm
                            color: Theme.warning
                        }

                        SettingsToggle {
                            width: parent.width
                            enabled: !root.eqMissing
                            title: qsTr("Enable equalizer")
                            description: qsTr("Ten bands sit in the existing audio bin, before ReplayGain. Reduce motion does not turn this off.")
                            checked: root.eqOn
                            onToggled: {
                                root.queue.setEqEnabled(!root.eqOn);
                                root.sync();
                            }
                        }

                        Item {
                            width: parent.width
                            height: Math.max(Theme.targetMin, eqPresetCopy.implicitHeight + Theme.spaceSm * 2)
                            enabled: root.eqOn && !root.eqMissing
                            activeFocusOnTab: true
                            Accessible.role: Accessible.Button
                            Accessible.name: qsTr("Equalizer preset") + ", " + root.eqPreset
                            Keys.onSpacePressed: root.cycleEqPreset()
                            Keys.onReturnPressed: root.cycleEqPreset()
                            Keys.onEnterPressed: root.cycleEqPreset()

                            Rectangle {
                                anchors.fill: parent
                                radius: Theme.radiusSm
                                color: eqPresetMouse.containsMouse || parent.activeFocus ? Theme.hover : "transparent"
                                border.width: parent.activeFocus ? 2 : 0
                                border.color: Theme.focus
                            }

                            Column {
                                id: eqPresetCopy

                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: Theme.spaceXs

                                Text {
                                    text: qsTr("Preset")
                                    textFormat: Text.PlainText
                                    font.family: Theme.fontFamily
                                    font.pixelSize: Theme.fontBody
                                    font.weight: Font.Medium
                                    color: Theme.foreground
                                }

                                Text {
                                    text: root.eqPreset
                                    textFormat: Text.PlainText
                                    font.family: Theme.fontFamily
                                    font.pixelSize: Theme.fontBodySm
                                    color: Theme.muted
                                }
                            }

                            MouseArea {
                                id: eqPresetMouse

                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.cycleEqPreset()
                            }
                        }

                        Repeater {
                            model: 10

                            Item {
                                id: eqBandRow

                                required property int index

                                width: parent.width
                                height: Theme.targetMin

                                Text {
                                    anchors.left: parent.left
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 72
                                    text: root.queue.eqBandLabel(eqBandRow.index)
                                    textFormat: Text.PlainText
                                    font.family: Theme.fontFamily
                                    font.pixelSize: Theme.fontCaption
                                    color: Theme.muted
                                }

                                ProgressSlider {
                                    anchors.left: parent.left
                                    anchors.leftMargin: 80
                                    anchors.right: parent.right
                                    anchors.verticalCenter: parent.verticalCenter
                                    from: -24
                                    to: 12
                                    enabled: root.eqOn && !root.eqMissing
                                    Accessible.name: root.queue.eqBandLabel(eqBandRow.index)
                                    value: {
                                        root.eqStamp;
                                        return root.queue.eqBand(eqBandRow.index);
                                    }
                                    onMoved: {
                                        root.queue.setEqBand(eqBandRow.index, value);
                                        root.eqStamp = root.eqStamp + 1;
                                    }
                                }
                            }
                        }

                        PrimaryButton {
                            primary: false
                            text: qsTr("Reset")
                            enabled: root.eqOn && !root.eqMissing
                            onClicked: {
                                root.queue.resetEq();
                                root.eqStamp = root.eqStamp + 1;
                                root.sync();
                            }
                        }

                        Item {
                            width: parent.width
                            height: Math.max(Theme.targetMin, outputCopy.implicitHeight + Theme.spaceSm * 2)
                            activeFocusOnTab: true
                            Accessible.role: Accessible.Button
                            Accessible.name: qsTr("Output") + ", " + root.outputLabel
                            Keys.onSpacePressed: {
                                root.queue.cycleOutput();
                                root.sync();
                            }
                            Keys.onReturnPressed: {
                                root.queue.cycleOutput();
                                root.sync();
                            }
                            Keys.onEnterPressed: {
                                root.queue.cycleOutput();
                                root.sync();
                            }

                            Rectangle {
                                anchors.fill: parent
                                radius: Theme.radiusSm
                                color: outputMouse.containsMouse || parent.activeFocus ? Theme.hover : "transparent"
                                border.width: parent.activeFocus ? 2 : 0
                                border.color: Theme.focus
                            }

                            Column {
                                id: outputCopy

                                anchors.left: parent.left
                                anchors.right: outputButton.left
                                anchors.rightMargin: Theme.spaceMd
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: Theme.spaceXs

                                Text {
                                    text: qsTr("Output")
                                    textFormat: Text.PlainText
                                    font.family: Theme.fontFamily
                                    font.pixelSize: Theme.fontBody
                                    font.weight: Font.Medium
                                    color: Theme.foreground
                                }

                                Text {
                                    text: root.outputLabel
                                    textFormat: Text.PlainText
                                    font.family: Theme.fontFamily
                                    font.pixelSize: Theme.fontBodySm
                                    color: Theme.muted
                                }
                            }

                            IconButton {
                                id: outputButton

                                anchors.right: parent.right
                                anchors.verticalCenter: parent.verticalCenter
                                iconName: "sliders-horizontal"
                                accessibleName: qsTr("Cycle output") + ", " + root.outputLabel
                                onActivated: {
                                    root.queue.cycleOutput();
                                    root.sync();
                                }
                            }

                            MouseArea {
                                id: outputMouse

                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    root.queue.cycleOutput();
                                    root.sync();
                                }
                            }
                        }

                        SettingsToggle {
                            width: parent.width
                            title: qsTr("Keep TuneX in the tray when closing the window")
                            description: root.trayAvailable ? qsTr("Closing hides the window. Quit from the tray card, or turn this off to quit on close.") : qsTr("No system tray on this session, so closing the window will still quit. The preference is saved for the next session that has a tray.")
                            checked: root.closeToTray
                            onToggled: {
                                root.tray.setCloseToTray(!root.closeToTray);
                                root.sync();
                            }
                        }
                    }

                    Column {
                        visible: root.section === "notifications"
                        width: parent.width
                        spacing: Theme.spaceMd

                        Text {
                            text: qsTr("Notifications")
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontTitle
                            font.weight: Font.DemiBold
                            color: Theme.foreground
                            Accessible.role: Accessible.Heading
                            Accessible.name: text
                        }

                        Text {
                            width: parent.width
                            wrapMode: Text.WordWrap
                            text: qsTr("Track changes only toast when TuneX is in the background. Playback errors still toast while you are looking at the window.")
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontBody
                            color: Theme.muted
                        }

                        SettingsToggle {
                            width: parent.width
                            title: qsTr("Desktop notifications")
                            description: qsTr("Send toasts through the session notification server.")
                            checked: root.notificationsOn
                            onToggled: {
                                root.queue.setNotificationsEnabled(!root.notificationsOn);
                                root.sync();
                            }
                        }

                        SettingsToggle {
                            width: parent.width
                            enabled: root.notificationsOn
                            title: qsTr("Track changes")
                            description: qsTr("When a new song starts and TuneX is not focused.")
                            checked: root.notifyTrackChangeOn
                            onToggled: {
                                root.queue.setNotifyTrackChange(!root.notifyTrackChangeOn);
                                root.sync();
                            }
                        }

                        SettingsToggle {
                            width: parent.width
                            enabled: root.notificationsOn
                            title: qsTr("Playback errors")
                            description: qsTr("When a file cannot play, even if TuneX is focused.")
                            checked: root.notifyErrorsOn
                            onToggled: {
                                root.queue.setNotifyPlaybackErrors(!root.notifyErrorsOn);
                                root.sync();
                            }
                        }
                    }

                    Column {
                        visible: root.section === "appearance"
                        width: parent.width
                        spacing: Theme.spaceMd

                        Text {
                            text: qsTr("Appearance")
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontTitle
                            font.weight: Font.DemiBold
                            color: Theme.foreground
                            Accessible.role: Accessible.Heading
                            Accessible.name: text
                        }

                        Text {
                            width: parent.width
                            wrapMode: Text.WordWrap
                            text: qsTr("TuneX is dark-only in V1. The royal blue accent is the brand colour — there is no picker yet.")
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontBody
                            color: Theme.muted
                        }

                        SettingsToggle {
                            width: parent.width
                            title: qsTr("Reduce motion")
                            description: qsTr("Skip fades and slides. Progress still follows the music.")
                            checked: root.reduceMotionOn
                            onToggled: {
                                root.queue.setReduceMotion(!root.reduceMotionOn);
                                Appearance.reduceMotion = !root.reduceMotionOn;
                                root.sync();
                            }
                        }

                        SettingsToggle {
                            width: parent.width
                            title: qsTr("Reduce transparency")
                            description: qsTr("Use solid surfaces instead of glass and the ambient wash.")
                            checked: root.reduceTransparencyOn
                            onToggled: {
                                root.queue.setReduceTransparency(!root.reduceTransparencyOn);
                                Appearance.reduceTransparency = !root.reduceTransparencyOn;
                                root.sync();
                            }
                        }

                        Item {
                            width: parent.width
                            height: Math.max(Theme.targetMin, vizCopy.implicitHeight + Theme.spaceSm * 2)
                            activeFocusOnTab: true
                            Accessible.role: Accessible.Button
                            Accessible.name: qsTr("Now Playing view")
                            Keys.onSpacePressed: root.cycleVisualizer()
                            Keys.onReturnPressed: root.cycleVisualizer()
                            Keys.onEnterPressed: root.cycleVisualizer()

                            Rectangle {
                                anchors.fill: parent
                                radius: Theme.radiusSm
                                color: vizMouse.containsMouse || parent.activeFocus ? Theme.hover : "transparent"
                                border.width: parent.activeFocus ? 2 : 0
                                border.color: Theme.focus
                            }

                            Column {
                                id: vizCopy

                                anchors.fill: parent
                                anchors.margins: Theme.spaceSm
                                spacing: Theme.spaceXs

                                Text {
                                    text: qsTr("Now Playing view")
                                    textFormat: Text.PlainText
                                    font.family: Theme.fontFamily
                                    font.pixelSize: Theme.fontBody
                                    font.weight: Font.Medium
                                    color: Theme.foreground
                                }

                                Text {
                                    text: root.visualizerMode === 1 ? qsTr("Spectrum") : (root.visualizerMode === 2 ? qsTr("Waveform") : (root.visualizerMode === 3 ? qsTr("Visualizer") : qsTr("Artwork")))
                                    textFormat: Text.PlainText
                                    font.family: Theme.fontFamily
                                    font.pixelSize: Theme.fontBodySm
                                    color: Theme.muted
                                }
                            }

                            MouseArea {
                                id: vizMouse

                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.cycleVisualizer()
                            }
                        }

                        Text {
                            text: qsTr("Visualizer frame cap")
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontLabel
                            font.weight: Font.Medium
                            color: Theme.foreground
                        }

                        ProgressSlider {
                            width: Math.min(parent.width, 320)
                            from: 5
                            to: 30
                            stepSize: 1
                            value: root.visualizerFps
                            Accessible.name: qsTr("Visualizer frame cap")
                            onMoved: {
                                root.queue.setVisualizerFps(Math.round(value));
                                root.sync();
                            }
                        }
                    }

                    Column {
                        visible: root.section === "shortcuts"
                        width: parent.width
                        spacing: Theme.spaceMd

                        Text {
                            text: qsTr("Shortcuts")
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontTitle
                            font.weight: Font.DemiBold
                            color: Theme.foreground
                            Accessible.role: Accessible.Heading
                            Accessible.name: text
                        }

                        Text {
                            width: parent.width
                            wrapMode: Text.WordWrap
                            text: qsTr("Bindings are fixed in V1. Customisation comes later.")
                            textFormat: Text.PlainText
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontBody
                            color: Theme.muted
                        }

                        Repeater {
                            model: root.shortcutKeys

                            Item {
                                id: shortcutRow

                                required property int index
                                required property string modelData

                                width: detailColumn.width
                                height: Theme.targetMin

                                Rectangle {
                                    anchors.left: parent.left
                                    anchors.right: parent.right
                                    anchors.bottom: parent.bottom
                                    height: 1
                                    color: Theme.border
                                    Accessible.ignored: true
                                }

                                Text {
                                    anchors.left: parent.left
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: shortcutRow.modelData
                                    textFormat: Text.PlainText
                                    font.family: Theme.fontFamily
                                    font.pixelSize: Theme.fontLabel
                                    font.weight: Font.Medium
                                    color: Theme.foreground
                                }

                                Text {
                                    anchors.right: parent.right
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: root.shortcutActions[shortcutRow.index]
                                    textFormat: Text.PlainText
                                    font.family: Theme.fontFamily
                                    font.pixelSize: Theme.fontBody
                                    color: Theme.muted
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
