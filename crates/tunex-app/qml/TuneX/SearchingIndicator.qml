import QtQuick
import QtQuick.Controls.Basic

// SearchingIndicator: a per-group loading spinner. Visible and running
// share one condition, so hosts cannot strand a spinner on — or hide a
// live one.
BusyIndicator {
    id: root

    property bool active: false
    property string accessibleName: qsTr("Searching")

    visible: root.active
    running: root.active
    Accessible.name: root.accessibleName
}
