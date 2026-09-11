import QtQuick
import QtQuick.Controls.Basic
import TuneX 1.0

// PlaylistNameDialog (S3 W-024, glass in S6 W-038): name entry for playlist
// create/rename. The action names the verb and stays disabled for blank
// names; duplicates still fail in the model with surfaced text (the dialog
// closes and the view error line explains). Opened via openFor().
GlassDialog {
    id: root

    property string initialName: ""

    signal nameAccepted(string name)

    function openFor(initial: string) {
        root.initialName = initial;
        nameField.text = initial;
        root.open();
        nameField.forceActiveFocus();
        nameField.selectAll();
    }

    title: root.initialName === "" ? qsTr("New playlist") : qsTr("Rename playlist")
    acceptLabel: root.initialName === "" ? qsTr("Create") : qsTr("Rename")
    acceptEnabled: nameField.text.trim() !== ""
    onAccepted: root.nameAccepted(nameField.text)

    TextField {
        id: nameField

        width: parent.width
        implicitHeight: Theme.targetMin
        placeholderText: qsTr("Playlist name")
        maximumLength: 120
        color: Theme.foreground
        placeholderTextColor: Theme.muted
        selectionColor: Theme.primary
        selectedTextColor: Theme.primaryText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontBody
        leftPadding: Theme.spaceMd
        rightPadding: Theme.spaceMd
        Accessible.name: qsTr("Playlist name")
        onAccepted: {
            if (root.acceptEnabled)
                root.accept();
        }

        background: Rectangle {
            radius: Theme.radiusSm
            color: Theme.chrome
            border.color: nameField.activeFocus ? Theme.focus : Theme.border
            border.width: nameField.activeFocus ? 2 : 1
        }
    }
}
