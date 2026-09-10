import QtQuick

// AlbumCard (S2 W-016): artwork-first card, 12px radius, two-line meta.
// Artwork arrives with the W-017 background worker; until then the card
// renders its generated monogram placeholder (unknown art stays a
// placeholder, never a guess).
Item {
    id: root

    required property int albumId
    required property string title
    required property string artist
    required property int year
    required property int trackCount
    // Bound at instantiation (`cardIndex: index`): lets activation sync the
    // view's currentIndex so the highlight follows the drilled album.
    required property int cardIndex
    // First letters of the first two words, uppercase.
    readonly property string monogram: {
        const words = root.title.split(/\s+/).filter(function(word) {
            return word.length > 0;
        });
        const letters = words.slice(0, 2).map(function(word) {
            return word[0].toUpperCase();
        });
        return letters.join("");
    }
    readonly property string metaLine: root.artist + (root.year > 0 ? " • " + root.year : "") + " • " + qsTr("%n song(s)", "", root.trackCount)

    signal activated(int id)

    width: GridView.view.cellWidth
    height: GridView.view.cellHeight
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: root.title + ", " + root.artist
    Accessible.onPressAction: root.activated(root.albumId)
    Keys.onReturnPressed: root.activated(root.albumId)
    Keys.onEnterPressed: root.activated(root.albumId)

    Column {
        anchors.fill: parent
        anchors.margins: Theme.spaceSm
        spacing: Theme.spaceXs

        Rectangle {
            width: parent.width
            height: parent.width
            radius: Theme.radiusMd
            color: root.activeFocus ? Theme.selected : Theme.surfaceRaised
            border.color: root.activeFocus ? Theme.focus : "transparent"
            border.width: root.activeFocus ? 2 : 0

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

            MouseArea {
                id: hoverArea

                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.activated(root.albumId)
            }

            // Hover veil: opacity on a flat rect only (no subtree blend),
            // binding-driven so focus styling is never destroyed.
            Rectangle {
                anchors.fill: parent
                radius: Theme.radiusMd
                color: Theme.hover
                opacity: hoverArea.containsMouse ? 0.45 : 0
            }

        }

        Text {
            width: parent.width
            elide: Text.ElideRight
            text: root.title
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBody
            font.weight: Font.DemiBold
            color: Theme.foreground
        }

        Text {
            width: parent.width
            elide: Text.ElideRight
            text: root.metaLine
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontCaption
            color: Theme.muted
        }

    }

}
