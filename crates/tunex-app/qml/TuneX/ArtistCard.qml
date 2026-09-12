import QtQuick
import TuneX

// ArtistCard (S2 W-016, activated in S6 W-044): circular monogram, name,
// collection counts. The crest stays circular — an artist has no single
// cover, so a monogram is the honest placeholder (DESIGN.md Shapes).
//
// The card plays the artist: there is no artist detail view in V1, and a
// grid of cards that do nothing contradicts "every pixel responds", so the
// hover play badge states the action the click performs.
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

    signal activated(string name)

    width: GridView.view.cellWidth
    height: GridView.view.cellHeight
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: root.artistName + ", " + root.metaLine
    Accessible.description: qsTr("Play this artist")
    Accessible.onPressAction: root.activated(root.artistName)
    Keys.onReturnPressed: root.activated(root.artistName)
    Keys.onEnterPressed: root.activated(root.artistName)
    Keys.onSpacePressed: root.activated(root.artistName)

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
            onClicked: root.activated(root.artistName)
        }

        Column {
            anchors.fill: parent
            anchors.margins: Theme.spaceSm
            spacing: Theme.spaceSm

            Item {
                width: parent.width
                height: parent.width

                Rectangle {
                    anchors.fill: parent
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

                    Rectangle {
                        anchors.fill: parent
                        radius: width / 2
                        color: Theme.foreground
                        opacity: hoverArea.containsMouse ? 0.04 : 0

                        Behavior on opacity {
                            NumberAnimation {
                                duration: Appearance.duration(Theme.motionHover)
                                easing.type: Easing.OutCubic
                            }
                        }
                    }
                }

                PlayBadge {
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    // The crest is a circle inscribed in this square, so the
                    // corner sits outside it; inset until the badge rests on
                    // the arc instead of floating past it.
                    anchors.margins: Theme.spaceSm
                    shown: hoverArea.containsMouse || root.activeFocus
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
                font.pixelSize: Theme.fontBodySm
                color: Theme.muted
            }
        }
    }
}
