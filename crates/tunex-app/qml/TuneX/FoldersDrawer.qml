import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import TuneX 1.0

// FoldersDrawer (S2 W-017, glass in S6 W-038): music-folder management over
// LibraryManager — native add dialog, per-folder remove, manual rescan,
// live scan progress. Subtle glass over a dark scrim; one primary action
// (Add folder), everything else secondary. The manager polls on this
// drawer's own timer while open; the shell refreshes views when a run
// finishes (App.syncPlayer → takeFinished).
Drawer {
    id: root

    required property LibraryManager manager
    // Local mirror of manager state (functions carry no notifiers).
    property bool scanning: false
    property string statusLine: ""
    property string errorLine: ""
    property int folderTotal: 0

    function sync() {
        root.manager.poll();
        root.scanning = root.manager.isScanning();
        root.statusLine = root.manager.statusText();
        root.folderTotal = root.manager.folderCount();
        root.errorLine = root.manager.errorText();
    }

    width: 360
    height: parent.height
    edge: Qt.RightEdge
    onOpened: root.sync()

    Overlay.modal: Rectangle {
        color: Qt.alpha(Theme.background, Theme.scrimOpacity)
    }

    Timer {
        interval: 300
        running: root.opened
        repeat: true
        onTriggered: root.sync()
    }

    FolderDialog {
        id: folderPicker

        title: qsTr("Choose a music folder")
        onAccepted: {
            const picked = decodeURIComponent(String(selectedFolder).replace("file://", ""));
            root.manager.addFolder(picked);
            root.sync();
        }
    }

    Column {
        anchors.fill: parent
        anchors.margins: Theme.spaceLg
        spacing: Theme.spaceMd

        Item {
            width: parent.width
            height: Theme.targetMin

            Text {
                anchors.left: parent.left
                anchors.right: closeButton.left
                anchors.rightMargin: Theme.spaceSm
                anchors.verticalCenter: parent.verticalCenter
                elide: Text.ElideRight
                text: qsTr("Music folders")
                textFormat: Text.PlainText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontTitle
                font.weight: Font.DemiBold
                color: Theme.foreground
                Accessible.role: Accessible.Heading
                Accessible.name: text
            }

            IconButton {
                id: closeButton

                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                iconName: "x"
                accessibleName: qsTr("Close music folders")
                onActivated: root.close()
            }
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
            Accessible.role: Accessible.StaticText
            Accessible.name: root.statusLine
        }

        // Scan activity: a 4px track with a travelling segment while a run
        // is live (the status line above carries the counts). Reduce-motion
        // keeps the segment still; it is never the only signal.
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
                    running: root.scanning && root.opened && !Appearance.reduceMotion
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

        Repeater {
            model: root.folderTotal

            Item {
                id: folderRow

                required property int index

                width: parent.width
                height: Theme.targetMin

                Icon {
                    id: folderGlyph

                    anchors.left: parent.left
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
                    text: root.manager.folderAt(folderRow.index)
                    textFormat: Text.PlainText
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontBody
                    color: Theme.foreground
                }

                PrimaryButton {
                    id: removeButton

                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    primary: false
                    text: qsTr("Remove")
                    Accessible.name: qsTr("Remove %1").arg(root.manager.folderAt(folderRow.index))
                    onClicked: {
                        root.manager.removeFolder(root.manager.folderAt(folderRow.index));
                        root.sync();
                    }
                }
            }
        }

        Row {
            spacing: Theme.spaceSm

            PrimaryButton {
                glyph: "folder-plus"
                text: qsTr("Add folder…")
                onClicked: folderPicker.open()
            }

            PrimaryButton {
                primary: false
                glyph: "refresh"
                text: qsTr("Rescan")
                Accessible.name: qsTr("Rescan music folders now")
                enabled: !root.scanning
                onClicked: {
                    root.manager.rescan();
                    root.sync();
                }
            }
        }
    }

    background: GlassBackdrop {
        restingRect: Qt.rect(root.parent.width - root.width, 0, root.width, root.height)
    }
}
