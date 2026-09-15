import QtQuick
import TuneX

// CapFooter: the "showing the first 200 matches" cap note topping capped
// result lists. The three search groups shared one hand-written copy
// each; the copy is the contract, so it lives in exactly one place.
Text {
    id: root

    property bool capped: false

    visible: root.capped
    width: parent.width
    height: visible ? implicitHeight : 0
    horizontalAlignment: Text.AlignHCenter
    text: qsTr("Showing the first 200 matches.")
    textFormat: Text.PlainText
    font.family: Theme.fontFamily
    font.pixelSize: Theme.fontCaption
    color: Theme.muted
}
