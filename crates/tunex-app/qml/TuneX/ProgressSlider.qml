import QtQuick
import QtQuick.Controls.Basic
import TuneX

// ProgressSlider (S6 W-038): the ProgressBar / VolumeControl track from the
// DESIGN_BRIEF inventory — 4px `hover` track, cyan `accentSecondary` fill,
// thumb 12px on hover/focus/drag and 8px at rest. Seek and volume share it.
// Hosts act on `moved` only, so model polls that set `value` never echo
// back as commands; the fill never animates (progress follows the engine).
Slider {
    id: root

    readonly property bool engaged: root.hovered || root.pressed || root.activeFocus

    implicitHeight: Theme.targetMin

    background: Rectangle {
        x: root.leftPadding
        y: root.topPadding + (root.availableHeight - height) / 2
        implicitWidth: 96
        implicitHeight: Theme.progressTrack
        width: root.availableWidth
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
