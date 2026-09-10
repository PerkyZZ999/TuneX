import QtQuick

// ArtistCard (S2 W-016): circular monogram, name, collection counts.
// Display-only in S2 — artist drill-down arrives with S3 search — so the
// card carries no pointer handling (no dead controls).
Item {
    id: root

    required property string artistName
    required property int albumCount
    required property int trackCount
    readonly property string monogram: {
        const words = root.artistName.split(/\s+/).filter(function(word) {
            return word.length > 0;
        });
        const letters = words.slice(0, 2).map(function(word) {
            return word[0].toUpperCase();
        });
        return letters.join("");
    }
    readonly property string metaLine: qsTr("%n album(s)", "", root.albumCount) + " • " + qsTr("%n song(s)", "", root.trackCount)

    width: GridView.view.cellWidth
    height: GridView.view.cellHeight
    Accessible.role: Accessible.StaticText
    Accessible.name: root.artistName + ", " + root.metaLine

    Column {
        anchors.fill: parent
        anchors.margins: Theme.spaceSm
        spacing: Theme.spaceXs

        Rectangle {
            width: parent.width
            height: parent.width
            radius: width / 2
            color: Theme.surfaceRaised

            Text {
                anchors.centerIn: parent
                text: root.monogram
                textFormat: Text.PlainText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontHeadline
                font.weight: Font.DemiBold
                color: Theme.muted
                Accessible.ignored: true
            }

        }

        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
            text: root.artistName
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBody
            font.weight: Font.DemiBold
            color: Theme.foreground
        }

        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
            text: root.metaLine
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontCaption
            color: Theme.muted
        }

    }

}
