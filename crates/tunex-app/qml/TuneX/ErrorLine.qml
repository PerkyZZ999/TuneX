import QtQuick
import TuneX

// ErrorLine: one shared error row — alert glyph plus words (never colour
// alone), caption size in the error token. Hosts bind `text` (empty hides
// the row and collapses its height, so layout math on `height` keeps
// working) and set width/anchors as usual; `showIcon` covers the text-only
// dialog variant.
Row {
    id: root

    property string text: ""
    property bool showIcon: true
    property int fontPixelSize: Theme.fontCaption

    visible: root.text !== ""
    height: visible ? implicitHeight : 0
    clip: true
    spacing: Theme.spaceSm

    Icon {
        anchors.verticalCenter: parent.verticalCenter
        visible: root.showIcon
        name: "alert"
        iconSize: 16
        stroke: Theme.error
    }

    Text {
        anchors.verticalCenter: parent.verticalCenter
        width: root.width - (root.showIcon ? 16 + root.spacing : 0)
        wrapMode: Text.WordWrap
        text: root.text
        textFormat: Text.PlainText
        font.family: Theme.fontFamily
        font.pixelSize: root.fontPixelSize
        color: Theme.error
    }
}
