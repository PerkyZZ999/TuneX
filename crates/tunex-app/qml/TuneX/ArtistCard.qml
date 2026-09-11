import QtQuick

// ArtistCard (S2 W-016): circular monogram, name, collection counts.
// Display-only in S2 — artist drill-down arrives with S3 search — so the
// card carries no pointer handling (no dead controls).
Item {
    id: root

    // Plain (not required) properties, set from model roles at instantiation:
    // `required` construction-time initialization races the delegate context
    // in this setup and locks role bindings to their defaults (W-018 gate).
    property string artistName: ""
    property int albumCount: 0
    property int trackCount: 0
    readonly property string monogram: {
        const words = root.artistName.split(/\s+/).filter(function (word) {
            return word.length > 0;
        });
        const letters = words.slice(0, 2).map(function (word) {
            return word[0].toUpperCase();
        });
        return letters.join("");
    }
    // No translation files ship in V1, so %n plurals would render literally;
    // translators get explicit singular/plural pairs instead.
    readonly property string albumsLine: root.albumCount === 1 ? qsTr("1 album") : qsTr("%1 albums").arg(root.albumCount)
    readonly property string songsLine: root.trackCount === 1 ? qsTr("1 song") : qsTr("%1 songs").arg(root.trackCount)
    readonly property string metaLine: albumsLine + " • " + songsLine

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
