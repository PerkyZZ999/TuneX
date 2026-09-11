import QtQuick
import QtQuick.Controls.Basic

// Pill primary/secondary action. One primary per view (DESIGN.md).
Button {
    id: root

    property bool primary: true
    property string glyph: ""

    implicitHeight: Theme.targetMin
    leftPadding: Theme.spaceLg
    rightPadding: Theme.spaceLg
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
        opacity: root.enabled ? 1 : 0.38
        border.width: root.activeFocus ? 2 : (root.primary ? 0 : 1)
        border.color: root.activeFocus ? Theme.focus : Theme.border
    }

    contentItem: Row {
        spacing: Theme.spaceSm

        Icon {
            visible: root.glyph !== ""
            anchors.verticalCenter: parent.verticalCenter
            name: root.glyph
            iconSize: 20
            stroke: root.primary ? Theme.primaryText : Theme.foreground
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.text
            font: root.font
            color: root.primary ? Theme.primaryText : Theme.foreground
        }

    }

}
