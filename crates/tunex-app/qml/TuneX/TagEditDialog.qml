import QtQuick
import QtQuick.Controls.Basic
import TuneX

// TagEditDialog (S12 W-063): in-app tag editor. Empty fields stay unchanged
// (never invented). Writes with lofty on a worker, then re-indexes the path.
GlassDialog {
    id: root

    required property LibraryManager library
    property int trackId: -1
    property string errorLine: ""

    signal tagsSaved

    function openFor(id) {
        root.trackId = id;
        root.errorLine = "";
        titleField.text = root.library.trackValue(id, "title");
        artistField.text = root.library.trackValue(id, "artist");
        albumField.text = root.library.trackValue(id, "album");
        trackField.text = root.library.trackValue(id, "track");
        discField.text = root.library.trackValue(id, "disc");
        yearField.text = root.library.trackValue(id, "year");
        genreField.text = root.library.trackValue(id, "genre");
        composerField.text = root.library.trackValue(id, "composer");
        root.open();
        titleField.forceActiveFocus();
        titleField.selectAll();
    }

    title: qsTr("Edit tags")
    acceptLabel: qsTr("Save")
    onAccepted: {
        const packed = [titleField.text, artistField.text, albumField.text, trackField.text, discField.text, yearField.text, genreField.text, composerField.text].join("\u001f");
        const ok = root.library.saveTags(root.trackId, packed);
        if (ok > 0)
            root.tagsSaved();
        else {
            root.errorLine = root.library.errorText();
            root.open();
        }
    }

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

        TagField {
            id: titleField

            width: parent.width
            label: qsTr("Title")
        }

        TagField {
            id: artistField

            width: parent.width
            label: qsTr("Artist")
        }

        TagField {
            id: albumField

            width: parent.width
            label: qsTr("Album")
        }

        Row {
            width: parent.width
            spacing: Theme.spaceSm

            TagField {
                id: trackField

                width: (parent.width - Theme.spaceSm * 2) / 3
                label: qsTr("Track")
                // Digits only: letters would parse to empty and drop the
                // edit silently.
                validator: IntValidator {
                    bottom: 1
                    top: 9999
                }
                inputMethodHints: Qt.ImhDigitsOnly
            }

            TagField {
                id: discField

                width: (parent.width - Theme.spaceSm * 2) / 3
                label: qsTr("Disc")
                validator: IntValidator {
                    bottom: 1
                    top: 9999
                }
                inputMethodHints: Qt.ImhDigitsOnly
            }

            TagField {
                id: yearField

                width: (parent.width - Theme.spaceSm * 2) / 3
                label: qsTr("Year")
                validator: IntValidator {
                    bottom: 0
                    top: 9999
                }
                inputMethodHints: Qt.ImhDigitsOnly
            }
        }

        TagField {
            id: genreField

            width: parent.width
            label: qsTr("Genre")
        }

        TagField {
            id: composerField

            width: parent.width
            label: qsTr("Composer")
        }
    }

    component TagField: TextField {
        id: field

        property string label: ""

        implicitHeight: Theme.targetMin
        placeholderText: field.label
        maximumLength: 200
        color: Theme.foreground
        placeholderTextColor: Theme.muted
        selectionColor: Theme.primary
        selectedTextColor: Theme.primaryText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontBody
        leftPadding: Theme.spaceMd
        rightPadding: Theme.spaceMd
        Accessible.name: field.label
        onAccepted: {
            if (root.acceptEnabled)
                root.accept();
        }

        background: Rectangle {
            radius: Theme.radiusSm
            color: Theme.chrome
            border.color: field.activeFocus ? Theme.focus : Theme.border
            border.width: field.activeFocus ? 2 : 1
        }
    }
}
