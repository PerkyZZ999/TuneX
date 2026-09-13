import QtQuick
import QtQuick.Controls.Basic
import TuneX

// GlassMenuItem (S6 W-038): one 40px body-md row in a GlassMenu. Hover or
// keyboard highlight steps an inset `hover` surface (sm radius); keyboard
// focus adds the 2px ring. Sub-menu entries show a chevron. Disabled rows
// dim to 38% with muted text — never hidden, so the menu shape is stable.
MenuItem {
    id: root

    implicitHeight: Theme.denseTarget
    rightPadding: Theme.spaceMd
    font.family: Theme.fontFamily
    font.pixelSize: Theme.fontBody

    contentItem: Text {
        rightPadding: root.subMenu ? Theme.spaceLg : 0
        text: root.text
        textFormat: Text.PlainText
        font: root.font
        color: root.enabled ? Theme.foreground : Theme.muted
        opacity: root.enabled ? 1 : 0.38
        elide: Text.ElideRight
        verticalAlignment: Text.AlignVCenter
    }

    indicator: Item {}

    arrow: Icon {
        x: root.width - width - Theme.spaceSm - Theme.spaceXs
        y: (root.height - height) / 2
        visible: root.subMenu
        name: "chevron-right"
        iconSize: 16
        stroke: Theme.muted
    }

    background: Rectangle {
        x: Theme.spaceXs
        implicitWidth: 216
        implicitHeight: Theme.denseTarget
        width: root.width - Theme.spaceXs * 2
        radius: Theme.radiusSm
        color: root.highlighted && root.enabled ? Theme.hover : "transparent"
        Behavior on color {
            ColorAnimation {
                duration: Appearance.duration(Theme.motionHover)
                easing.type: Easing.OutCubic
            }
        }
        border.width: root.visualFocus ? 2 : 0
        border.color: Theme.focus
    }
}
