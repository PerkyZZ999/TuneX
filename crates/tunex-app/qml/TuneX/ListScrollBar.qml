import QtQuick
import QtQuick.Controls.Basic
import TuneX

// ListScrollBar: the one themed scrollbar for every scrollable list.
// Attached per view as `ScrollBar.vertical: ListScrollBar {}`. A quiet
// thumb at rest that firms up on hover/press; position only, it never
// carries meaning on its own.
ScrollBar {
    id: root

    policy: ScrollBar.AsNeeded

    contentItem: Rectangle {
        radius: Theme.radiusXs
        color: root.pressed ? Theme.muted : Theme.border
        opacity: root.hovered || root.pressed ? 0.9 : 0.55

        Behavior on opacity {
            NumberAnimation {
                duration: Appearance.duration(Theme.motionHover)
                easing.type: Easing.OutCubic
            }
        }
    }
}
