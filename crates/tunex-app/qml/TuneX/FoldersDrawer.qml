import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import TuneX 1.0

// FoldersDrawer (S2 W-017): music-folder management over LibraryManager —
// native add dialog, per-folder remove, manual rescan, live scan progress.
// The manager polls on this drawer's own timer; view models refresh through
// LibraryView's timer when a run finishes (takeFinished).
Drawer {
    id: root

    required property LibraryManager manager
    required property QueueModel queue
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

        Row {
            width: parent.width
            spacing: Theme.spaceSm

            Text {
                width: parent.width - closeButton.width - Theme.spaceSm
                anchors.verticalCenter: parent.verticalCenter
                text: qsTr("Music folders")
                textFormat: Text.PlainText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontTitle
                font.weight: Font.DemiBold
                color: Theme.foreground
            }

            Button {
                id: closeButton

                text: qsTr("Close")
                onClicked: root.close()
            }

        }

        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            text: root.statusLine
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBodySm
            color: Theme.muted
            Accessible.role: Accessible.StatusIndicator
            Accessible.name: root.statusLine
        }

        ProgressBar {
            visible: root.scanning
            width: parent.width
            indeterminate: true
            Accessible.name: qsTr("Scan progress")
        }

        Text {
            visible: root.errorLine !== ""
            width: parent.width
            wrapMode: Text.WordWrap
            text: root.errorLine
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBodySm
            color: Theme.error
        }

        Repeater {
            model: root.folderTotal

            Row {
                required property int index

                width: parent.width
                spacing: Theme.spaceSm

                Text {
                    width: parent.width - removeButton.width - Theme.spaceSm
                    anchors.verticalCenter: parent.verticalCenter
                    elide: Text.ElideMiddle
                    text: root.manager.folderAt(index)
                    textFormat: Text.PlainText
                    font.family: Theme.fontFamily
                    font.pixelSize: Theme.fontBody
                    color: Theme.foreground
                }

                Button {
                    id: removeButton

                    anchors.verticalCenter: parent.verticalCenter
                    text: qsTr("Remove")
                    onClicked: {
                        root.manager.removeFolder(root.manager.folderAt(index));
                        root.sync();
                    }
                }

            }

        }

        Row {
            spacing: Theme.spaceSm

            Button {
                text: qsTr("Add folder…")
                onClicked: folderPicker.open()
            }

            Button {
                text: qsTr("Rescan now")
                enabled: !root.scanning
                onClicked: {
                    root.manager.rescan();
                    root.sync();
                }
            }

        }

    }

    background: GlassBackdrop {
        transparencyOff: root.queue.reduceTransparency()
    }

}
