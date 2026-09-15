import QtQuick
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

        Field {
            id: titleField

            width: parent.width
            placeholderText: qsTr("Title")
        }

        Field {
            id: artistField

            width: parent.width
            placeholderText: qsTr("Artist")
        }

        Field {
            id: albumField

            width: parent.width
            placeholderText: qsTr("Album")
        }

        Row {
            width: parent.width
            spacing: Theme.spaceSm

            Field {
                id: trackField

                width: (parent.width - Theme.spaceSm * 2) / 3
                placeholderText: qsTr("Track")
                // Digits only: letters would parse to empty and drop the
                // edit silently.
                validator: IntValidator {
                    bottom: 1
                    top: 9999
                }
                inputMethodHints: Qt.ImhDigitsOnly
            }

            Field {
                id: discField

                width: (parent.width - Theme.spaceSm * 2) / 3
                placeholderText: qsTr("Disc")
                validator: IntValidator {
                    bottom: 1
                    top: 9999
                }
                inputMethodHints: Qt.ImhDigitsOnly
            }

            Field {
                id: yearField

                width: (parent.width - Theme.spaceSm * 2) / 3
                placeholderText: qsTr("Year")
                validator: IntValidator {
                    bottom: 0
                    top: 9999
                }
                inputMethodHints: Qt.ImhDigitsOnly
            }
        }

        Field {
            id: genreField

            width: parent.width
            placeholderText: qsTr("Genre")
        }

        Field {
            id: composerField

            width: parent.width
            placeholderText: qsTr("Composer")
        }
    }
}
