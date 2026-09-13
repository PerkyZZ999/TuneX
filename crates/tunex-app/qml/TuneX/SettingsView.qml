import QtQuick
import QtQuick.Dialogs
import TuneX

// SettingsView (IA list-detail): Library, Playback, Appearance, Shortcuts.
// Presentation only — folders/playback/appearance write through the bridged
// models. Close-to-tray lives under Playback with the other session prefs.
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
    property bool closeToTray: true
    property bool trayAvailable: false
    property bool reduceMotionOn: false
    property bool reduceTransparencyOn: false
    readonly property list<string> sections: ["library", "playback", "appearance", "shortcuts"]
    readonly property list<string> shortcutKeys: [qsTr("Space"), qsTr("Media Play / Pause / Next / Previous"), qsTr("Volume Up / Down / Mute"), qsTr("/ or Ctrl+K"), qsTr("Esc"), qsTr("Alt+Left / Alt+Right"), qsTr("↑ ↓ ← →"), qsTr("j / k"), qsTr("Enter"), qsTr("Type in Songs"), qsTr("Tab / Shift+Tab in Search")]
    readonly property list<string> shortcutActions: [qsTr("Play / pause"), qsTr("Play, next, previous"), qsTr("Volume and mute"), qsTr("Search"), qsTr("Close Now Playing, then search, then back"), qsTr("Back / forward"), qsTr("Move list and grid cursor"), qsTr("Move list and grid cursor"), qsTr("Play the current song, or open the current album or artist"), qsTr("Jump to the first title with that prefix (400 ms reset)"), qsTr("Cycle Songs, Albums, and Artists result groups")]
    readonly property string repeatLabel: {
        if (root.repeatModeValue === 1)
            return qsTr("All tracks");
        if (root.repeatModeValue === 2)
            return qsTr("One track");
        return qsTr("Off");
    }

    function sectionLabel(key) {
        if (key === "playback")
            return qsTr("Playback");
        if (key === "appearance")
            return qsTr("Appearance");
        if (key === "shortcuts")
            return qsTr("Shortcuts");
        return qsTr("Library");
    }

    function sectionIcon(key) {
        if (key === "playback")
            return "sliders-horizontal";
        if (key === "appearance")
            return "blend";
        if (key === "shortcuts")
            return "keyboard";
        return "folder";
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
        root.closeToTray = root.tray.closeToTray();
        root.trayAvailable = root.tray.isAvailable();
        root.reduceMotionOn = root.queue.reduceMotion();
        root.reduceTransparencyOn = root.queue.reduceTransparency();
    }

    function requestAddFolder() {
        folderPicker.open();
    }

    anchors.fill: parent
    onVisibleChanged: {
        if (root.visible)
            root.sync();
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
                            text: qsTr("Playback is gapless between consecutive tracks. The last track and position come back when you reopen TuneX, if the file is still there.")
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
                            description: qsTr("Play Up Next in shuffled order.")
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
