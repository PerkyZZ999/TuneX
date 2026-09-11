import QtQuick
import QtQuick.Controls.Basic
import TuneX 1.0

// GlassMenuSeparator (S6 W-038): the 1px `border` hairline between
// GlassMenu groups (DESIGN.md divider token).
MenuSeparator {
    topPadding: Theme.spaceXs
    bottomPadding: Theme.spaceXs
    leftPadding: Theme.spaceMd
    rightPadding: Theme.spaceMd

    contentItem: Rectangle {
        implicitWidth: 184
        implicitHeight: 1
        color: Theme.border
    }
}
