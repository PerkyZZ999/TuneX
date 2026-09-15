import QtQuick
import QtQuick.Controls.Basic
import TuneX

// PlaylistNameDialog (S3 W-024, glass in S6 W-038): name entry for playlist
// create/rename. The action names the verb and stays disabled for blank
// names; duplicates still fail in the model with surfaced text (the dialog
// closes and the view error line explains). Opened via openFor().
GlassDialog {
    id: root

    property string initialName: ""
    // Rejection reason kept in the dialog so a duplicate never costs the
    // typed name (TagEditDialog works the same way).
    property string errorLine: ""

    signal nameAccepted(string name)

    function openFor(initial: string) {
        root.initialName = initial;
        root.errorLine = "";
        nameField.text = initial;
        root.open();
        nameField.forceActiveFocus();
        nameField.selectAll();
    }

    // Rejection keeps the typed name on screen with the reason, instead
    // of closing and losing the input.
    function reopenWithError(message: string) {
        root.errorLine = message;
        root.open();
        nameField.forceActiveFocus();
        nameField.selectAll();
    }

    title: root.initialName === "" ? qsTr("New playlist") : qsTr("Rename playlist")
    acceptLabel: root.initialName === "" ? qsTr("Create") : qsTr("Rename")
    acceptEnabled: nameField.text.trim() !== ""
    onAccepted: root.nameAccepted(nameField.text)

    Column {
        width: parent.width
        spacing: Theme.spaceSm

        Text {
            visible: root.errorLine !== ""
            width: parent.width
            wrapMode: Text.WordWrap
            text: root.errorLine
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontCaption
            color: Theme.error
        }

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
}
