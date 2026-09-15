import QtQuick
import QtQuick.Controls.Basic
import TuneX

// SmartPlaylistDialog (S12 W-062): rule builder for local smart playlists.
// Name + kind + optional value + exclude-missing. Evaluated in SQL on open.
GlassDialog {
    id: root

    property string kind: "never_played"
    property bool excludeMissing: true

    signal ruleAccepted(string name, string kind, string value, bool excludeMissing)

    function openBlank() {
        nameField.text = "";
        root.kind = "never_played";
        valueField.text = "";
        root.excludeMissing = true;
        root.open();
        nameField.forceActiveFocus();
    }

    function kindLabel(key) {
        if (key === "added_days")
            return qsTr("Added recently");
        if (key === "never_played")
            return qsTr("Never played");
        if (key === "artist")
            return qsTr("Artist");
        if (key === "genre")
            return qsTr("Genre");
        if (key === "composer")
            return qsTr("Composer");
        return key;
    }

    function valueNeeded() {
        return root.kind !== "never_played";
    }

    title: qsTr("New smart playlist")
    acceptLabel: qsTr("Create")
    acceptEnabled: nameField.text.trim() !== "" && (!root.valueNeeded() || valueField.text.trim() !== "")
    onAccepted: root.ruleAccepted(nameField.text, root.kind, valueField.text, root.excludeMissing)

    // Days is digits: letters would store a rule that matches nothing.
    IntValidator {
        id: daysValidator

        bottom: 1
        top: 36500
    }

    Column {
        width: parent.width
        spacing: Theme.spaceMd

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

        Flow {
            width: parent.width
            spacing: Theme.spaceSm

            Chip {
                label: root.kindLabel("never_played")
                selected: root.kind === "never_played"
                onActivated: root.kind = "never_played"
            }

            Chip {
                label: root.kindLabel("added_days")
                selected: root.kind === "added_days"
                onActivated: root.kind = "added_days"
            }

            Chip {
                label: root.kindLabel("artist")
                selected: root.kind === "artist"
                onActivated: root.kind = "artist"
            }

            Chip {
                label: root.kindLabel("genre")
                selected: root.kind === "genre"
                onActivated: root.kind = "genre"
            }

            Chip {
                label: root.kindLabel("composer")
                selected: root.kind === "composer"
                onActivated: root.kind = "composer"
            }
        }

        Field {
            id: valueField

            visible: root.valueNeeded()
            width: parent.width
            implicitHeight: visible ? Theme.targetMin : 0
            placeholderText: root.kind === "added_days" ? qsTr("Days") : root.kindLabel(root.kind)
            maximumLength: 120
            validator: root.kind === "added_days" ? daysValidator : null
            inputMethodHints: root.kind === "added_days" ? Qt.ImhDigitsOnly : Qt.ImhNone
            Accessible.name: placeholderText
            onAccepted: {
                if (root.acceptEnabled)
                    root.accept();
            }
        }

        SettingsToggle {
            width: parent.width
            title: qsTr("Hide missing files")
            description: qsTr("Skip tracks whose files are gone from disk.")
            checked: root.excludeMissing
            onToggled: root.excludeMissing = !root.excludeMissing
        }
    }
}
