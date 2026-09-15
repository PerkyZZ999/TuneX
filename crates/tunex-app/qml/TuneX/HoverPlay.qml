import QtQuick
import TuneX

// HoverPlay: the card hover affordance — a 4% brighten veil plus the play
// badge. Album, playlist, and artist cards shared one hand-rolled copy
// each; the wide playlist tile carries the veil only.
Item {
    id: root

    property bool hovered: false
    // False on surfaces with no room for a badge (the wide playlist tile).
    property bool showBadge: true
    property int badgeInset: Theme.spaceXs
    // Circular crest (artist monogram) instead of a rounded card.
    property bool circular: false

    anchors.fill: parent

    Rectangle {
        anchors.fill: parent
        radius: root.circular ? width / 2 : Theme.radiusSm
        color: Theme.foreground
        opacity: root.hovered ? 0.04 : 0

        Behavior on opacity {
            NumberAnimation {
                duration: Appearance.duration(Theme.motionHover)
                easing.type: Easing.OutCubic
            }
        }
    }

    PlayBadge {
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: root.badgeInset
        shown: root.showBadge && root.hovered
    }
}
