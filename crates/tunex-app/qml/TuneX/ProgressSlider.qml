import QtQuick
import QtQuick.Controls.Basic
import TuneX

// ProgressSlider (S6 W-038, S14 W-067): the ProgressBar / VolumeControl
// track — 4px hover track, accent-secondary fill, 12px thumb on
// hover/focus/drag and 8px at rest. The control itself is 44px tall so
// the groove is a real grab, not a 4px target. Hosts act on `moved` only.
Slider {
    id: root

    readonly property bool engaged: root.hovered || root.pressed || root.activeFocus
    // Seek hosts set this to 5000 so Left/Right skip five seconds.
    // Volume leaves it at 1 (percent steps).
    property real keyStep: 1

    implicitHeight: Theme.targetMin
    padding: 0
    live: true
    wheelEnabled: true
    stepSize: root.keyStep
    snapMode: Slider.NoSnap

    background: Item {
        x: root.leftPadding
        y: root.topPadding
        implicitWidth: 96
        implicitHeight: Theme.targetMin
        width: root.availableWidth
        height: root.availableHeight

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            implicitHeight: Theme.progressTrack
            height: Theme.progressTrack
            radius: Theme.radiusXs
            color: Theme.hover

            Rectangle {
                width: root.visualPosition * parent.width
                height: parent.height
                radius: Theme.radiusXs
                color: Theme.accentSecondary
            }
        }
    }

    handle: Rectangle {
        x: root.leftPadding + root.visualPosition * (root.availableWidth - width)
        y: root.topPadding + (root.availableHeight - height) / 2
        implicitWidth: Theme.thumbSize
        implicitHeight: Theme.thumbSize
        width: root.engaged ? Theme.thumbSize : Theme.spaceSm
        height: width
        radius: width / 2
        color: Theme.foreground
        border.color: Theme.focus
        border.width: root.activeFocus ? 2 : 0
        visible: root.enabled
    }
}
