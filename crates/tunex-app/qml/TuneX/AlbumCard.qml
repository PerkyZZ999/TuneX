import QtQuick
import TuneX

// AlbumCard (S2 W-016, artwork in S6 W-040): artwork-first card, 12px
// radius, two-line meta. The cover is resolved lazily — the card asks for it
// as it appears and `Artwork` fades it in over the generated monogram, which
// stays for albums that have none (unknown art is never a guess).
Item {
    id: root

    // Plain (not required) properties, set from model roles at instantiation:
    // `required` construction-time initialization races the delegate context
    // in this setup and locks role bindings to their defaults (W-018 gate).
    property int albumId: -1
    property string title: ""
    property string artist: ""
    property int year: 0
    property int trackCount: 0
    // Cached cover, empty until the lazy resolver answers (or forever).
    property url artUrl
    // Bound at instantiation (`cardIndex: index`): lets activation sync the
    // view's currentIndex so the highlight follows the drilled album.
    property int cardIndex: -1
    // Set when the card is not a GridView delegate (home rails).
    property int explicitWidth: 0
    // First letters of the first two words, uppercase.
    readonly property string monogram: Format.monogram(root.title)
    // No translation files ship in V1, so %n plurals would render literally
    // ("40 song(s)"); translators get an explicit singular/plural pair.
    readonly property string countLine: Format.plural(root.trackCount, qsTr("1 song"), qsTr("%1 songs"))
    readonly property string metaLine: Format.meta([root.artist, root.year > 0 ? String(root.year) : "", countLine])

    signal activated(int id)

    width: root.explicitWidth > 0 ? root.explicitWidth : GridView.view.cellWidth
    height: root.explicitWidth > 0 ? root.explicitWidth + Theme.cardMetaHeight : GridView.view.cellHeight
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: qsTr("%1, %2").arg(root.title).arg(root.artist)
    Accessible.onPressAction: root.activated(root.albumId)
    Keys.onReturnPressed: root.activated(root.albumId)
    Keys.onEnterPressed: root.activated(root.albumId)
    Keys.onSpacePressed: root.activated(root.albumId)

    // The card itself: a surface step over the canvas with a hairline border,
    // as the mockup draws it. Hierarchy here is tonal, never a shadow —
    // cards carry no elevation (DESIGN.md Elevation & Depth).
    Rectangle {
        anchors.fill: parent
        anchors.margins: Theme.spaceXs
        radius: Theme.radiusMd
        color: root.activeFocus ? Theme.selected : Theme.surface
        border.color: root.activeFocus ? Theme.focus : Theme.border
        border.width: root.activeFocus ? 2 : 1

        MouseArea {
            id: hoverArea

            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.activated(root.albumId)
        }

        Column {
            anchors.fill: parent
            anchors.margins: Theme.spaceXs
            spacing: Theme.spaceXs

            Item {
                width: parent.width
                height: parent.width

                Artwork {
                    anchors.fill: parent
                    source: root.artUrl
                    monogram: root.monogram
                    radius: Theme.radiusSm
                }

                // Hover affordance per DESIGN.md: the art brightens slightly
                // and a play button appears — the card never lifts or shifts.
                HoverPlay {
                    hovered: hoverArea.containsMouse || root.activeFocus
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
                font.pixelSize: Theme.fontBodySm
                color: Theme.muted
            }
        }
    }
}
