import QtQuick

// Home view (S1 W-003): greeting hero plus the honest empty-library state.
// Rails, search, and playback surfaces arrive with S2+ slices.
Column {
    anchors.fill: parent
    anchors.margins: Theme.spaceLg
    spacing: Theme.spaceMd

    Text {
        text: qsTr("Good evening,")
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontBody
        color: Theme.muted
    }

    Text {
        text: qsTr("Music feels different here.")
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontDisplay
        font.weight: Font.Bold
        color: Theme.foreground
    }

    Text {
        text: qsTr("Your music. Your space. Always local.")
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontBody
        color: Theme.muted
    }

    Item {
        width: parent.width
        height: Theme.spaceXl
    }

    Text {
        text: qsTr("No music yet")
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontTitle
        font.weight: Font.DemiBold
        color: Theme.foreground
    }

    Text {
        width: parent.width
        wrapMode: Text.WordWrap
        text: qsTr("Library scanning lands in S2. Point TuneX at a music folder and your collection will appear here.")
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontBody
        color: Theme.muted
    }

}
