import QtQuick
import QtQuick.Controls.Basic
import TuneX

// Field: the one themed text field for dialogs and settings. Standard
// height, paddings, type, selection colors, and focus ring live here;
// hosts own placeholder, length, validators, hints, accessible name, and
// what Enter does.
TextField {
    id: root

    implicitHeight: Theme.targetMin
    maximumLength: 200
    color: Theme.foreground
    placeholderTextColor: Theme.muted
    selectionColor: Theme.primary
    selectedTextColor: Theme.primaryText
    font.family: Theme.fontFamily
    font.pixelSize: Theme.fontBody
    leftPadding: Theme.spaceMd
    rightPadding: Theme.spaceMd

    background: Rectangle {
        radius: Theme.radiusSm
        color: Theme.chrome
        border.color: root.activeFocus ? Theme.focus : Theme.border
        border.width: root.activeFocus ? 2 : 1
    }
}
