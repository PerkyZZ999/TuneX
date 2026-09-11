import QtQuick
import QtQuick.Controls.Basic
import TuneX 1.0

// PlaylistNameDialog (S3 W-024): name entry for playlist create/rename.
// Blank and duplicate names fail in the model with surfaced text (the
// dialog closes; the view error line explains). Opened via openFor().
Dialog {
    id: root

    property string initialName: ""
    required property QueueModel queue

    signal nameAccepted(string name)

    function openFor(initial) {
        root.initialName = initial;
        nameField.text = initial;
        root.open();
        nameField.forceActiveFocus();
        nameField.selectAll();
    }

    title: root.initialName === "" ? qsTr("New playlist") : qsTr("Rename playlist")
    standardButtons: Dialog.Ok | Dialog.Cancel
    modal: true
    onAccepted: root.nameAccepted(nameField.text)

    TextField {
        id: nameField

        width: 320
        placeholderText: qsTr("Playlist name")
        maximumLength: 120
        color: Theme.foreground
        placeholderTextColor: Theme.muted
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontBody
        Accessible.name: qsTr("Playlist name")
        onAccepted: root.accept()

        background: Rectangle {
            radius: Theme.radiusSm
            color: Theme.surfaceRaised
            border.color: nameField.activeFocus ? Theme.focus : Theme.border
            border.width: nameField.activeFocus ? 2 : 1
        }

    }

    background: GlassBackdrop {
        cornerRadius: Theme.radiusLg
        transparencyOff: root.queue.reduceTransparency()
    }

}
