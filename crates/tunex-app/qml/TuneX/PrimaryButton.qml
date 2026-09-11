import QtQuick
import QtQuick.Controls.Basic
import TuneX

// Pill primary/secondary action. One primary per view (DESIGN.md). Disabled
// fades the whole control to 38% on the desaturated raised surface, label
// and glyph included; the 2px ring follows keyboard focus only.
Button {
    id: root

    property bool primary: true
    property string glyph: ""

    implicitHeight: Theme.targetMin
    leftPadding: Theme.spaceLg
    rightPadding: Theme.spaceLg
    opacity: root.enabled ? 1 : 0.38
    font.family: Theme.fontFamily
    font.pixelSize: Theme.fontLabel
    font.weight: Font.Medium

    background: Rectangle {
        radius: Theme.radiusPill
        color: {
            if (!root.enabled)
                return Theme.surfaceRaised;

            if (root.primary)
                return root.hovered || root.pressed ? Theme.primaryHover : Theme.primary;

            return root.hovered || root.pressed ? Theme.hover : Theme.surfaceRaised;
        }
        border.width: root.visualFocus ? 2 : (root.primary && root.enabled ? 0 : 1)
        border.color: root.visualFocus ? Theme.focus : Theme.border

        Behavior on color {
            ColorAnimation {
                duration: Appearance.duration(Theme.motionHover)
                easing.type: Easing.OutCubic
            }
        }
    }

    contentItem: Row {
        spacing: Theme.spaceSm

        Icon {
            visible: root.glyph !== ""
            anchors.verticalCenter: parent.verticalCenter
            name: root.glyph
            iconSize: 20
            stroke: root.primary && root.enabled ? Theme.primaryText : Theme.foreground
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.text
            textFormat: Text.PlainText
            font: root.font
            color: root.primary && root.enabled ? Theme.primaryText : Theme.foreground
        }
    }
}
