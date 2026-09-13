import QtQuick
import TuneX

// PlayBadge (S6 W-044): the play affordance DESIGN.md asks artwork cards to
// reveal on hover — "hover lifts nothing; art gets a 4% brighten +
// play-affordance overlay button". It is an indicator, not a second hit
// target: the whole card already activates, so this carries no pointer
// handling and stays out of the accessibility tree.
//
// No glow. Glow appears exactly twice in TuneX (the playing transport button
// and the selected-nav bar); anywhere else it is a bug.
Item {
    id: root

    // Revealed while the card is hovered or keyboard-focused.
    property bool shown: false
    property int size: Theme.playBadge

    implicitWidth: root.size
    implicitHeight: root.size
    opacity: root.shown ? 1 : 0
    visible: opacity > 0
    Accessible.ignored: true

    Behavior on opacity {
        NumberAnimation {
            duration: Appearance.duration(Theme.motionHover)
            easing.type: Easing.OutCubic
        }
    }

    Rectangle {
        anchors.fill: parent
        radius: width / 2
        color: Theme.primary

        Icon {
            anchors.centerIn: parent
            // Optical centering: a play triangle's visual mass sits left of
            // its bounding box, so nudge it back toward the circle's middle.
            anchors.horizontalCenterOffset: 1
            name: "play"
            iconSize: 16
            filled: true
            stroke: Theme.primaryText
        }
    }
}
