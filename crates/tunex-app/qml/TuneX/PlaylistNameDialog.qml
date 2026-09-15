import QtQuick
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
    // of closing and losing the input. The reopen defers past accept()'s
    // close — reopening synchronously inside onAccepted loses to it
    // (proven live: the dialog still closed).
    function reopenWithError(message: string) {
        root.errorLine = message;
        reopenTimer.restart();
    }

    Timer {
        id: reopenTimer

        interval: 1
        onTriggered: {
            root.open();
            nameField.forceActiveFocus();
            nameField.selectAll();
        }
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

        Field {
            id: nameField

            width: parent.width
            placeholderText: qsTr("Playlist name")
            maximumLength: 120
            Accessible.name: qsTr("Playlist name")
            onAccepted: {
                if (root.acceptEnabled)
                    root.accept();
            }
        }
    }
}
