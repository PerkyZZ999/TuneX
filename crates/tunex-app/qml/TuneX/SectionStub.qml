import QtQuick
import TuneX

// Honest section placeholder for the nav skeleton (S1 W-003): names the
// section and the slice that builds it. Replaced by real views in S2+.
Column {
    required property string title
    required property string note

    anchors.fill: parent
    anchors.margins: Theme.spaceLg
    spacing: Theme.spaceSm

    Text {
        text: title
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontHeadline
        font.weight: Font.Bold
        color: Theme.foreground
    }

    Text {
        width: parent.width
        wrapMode: Text.WordWrap
        text: note
        textFormat: Text.PlainText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontBody
        color: Theme.muted
    }
}
