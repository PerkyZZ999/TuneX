import QtQuick
import QtQuick.Controls.Basic
import TuneX 1.0

// Bridge proof UI (S1 W-002).
// Rows below come from the Rust TrackListModel; the button exercises the
// QML → Rust slot path and rowsInserted proves Rust → QML signals.
// Replaced by the real home screen in W-003; do not build on this layout.
Window {
    id: root

    minimumWidth: 960
    minimumHeight: 640
    width: 960
    height: 640
    visible: true
    title: qsTr("TuneX")

    Column {
        anchors.fill: parent
        anchors.margins: 24
        spacing: 12

        Text {
            text: qsTr("Bridge proof — rows below come from Rust")
        }

        Text {
            id: countText

            text: qsTr("Tracks: %1").arg(trackList.count)
        }

        Text {
            id: signalText

            text: qsTr("Add a track to test the signal path")
        }

        Button {
            text: qsTr("Add proof track")
            onClicked: trackModel.appendTrack(qsTr("Proof track %1").arg(trackList.count + 1), "TuneX")
        }

        ListView {
            id: trackList

            width: parent.width
            height: 200

            model: TrackListModel {
                id: trackModel

                onRowsInserted: (parent, first, last) => {
                    return signalText.text = qsTr("Signal received: row %1").arg(last);
                }
            }

            delegate: Row {
                required property string title
                required property string artist

                spacing: 8

                Text {
                    text: title
                }

                Text {
                    text: artist
                }

            }

        }

    }

}
