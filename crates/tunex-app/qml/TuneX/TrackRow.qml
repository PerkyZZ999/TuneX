import QtQuick

// TrackRow (S2 W-016): one song row — number, title/artist, duration,
// missing badge. Solid text on the opaque view background (never glass).
// Display-only in S2; playback actions arrive with S4, so rows carry no
// pointer handling (no dead controls). Edge-anchored layout: the middle
// column fills whatever the fixed edges leave, so no spacing is
// hand-counted and nothing depends on sibling creation order.
Item {
    id: root

    // Plain (not required) properties, set from model roles at instantiation:
    // `required` construction-time initialization races the delegate context
    // in this setup and locks role bindings to their defaults (W-018 gate).
    property string title: ""
    property string artist: ""
    property int trackNumber: 0
    property int durationMs: 0
    property bool missing: false
    // m:ss, em dash when unknown. Numbers need no translation.
    readonly property string durationText: root.durationMs > 0 ? Math.floor(root.durationMs / 60000) + ":" + String(Math.floor(root.durationMs / 1000) % 60).padStart(2, "0") : "—"
    readonly property string numberText: root.trackNumber > 0 ? String(root.trackNumber) : "—"

    width: ListView.view.width
    height: 56
    Accessible.role: Accessible.ListItem
    Accessible.name: root.title + ", " + root.artist + (root.missing ? ", " + qsTr("missing") : "")

    Text {
        anchors.left: parent.left
        anchors.leftMargin: Theme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        width: 32
        horizontalAlignment: Text.AlignRight
        text: root.numberText
        textFormat: Text.PlainText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontBodySm
        color: Theme.muted
    }

    Text {
        id: durationLabel

        anchors.right: parent.right
        anchors.rightMargin: Theme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        text: root.durationText
        textFormat: Text.PlainText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontBodySm
        color: Theme.muted
    }

    Rectangle {
        id: missingBadge

        anchors.right: durationLabel.left
        anchors.rightMargin: root.missing ? Theme.spaceSm : 0
        anchors.verticalCenter: parent.verticalCenter
        width: root.missing ? missingLabel.width + Theme.spaceSm * 2 : 0
        height: missingLabel.height + Theme.spaceXs
        visible: root.missing
        radius: Theme.radiusPill
        color: Theme.surfaceRaised
        border.color: Theme.warning
        border.width: 1

        Text {
            id: missingLabel

            anchors.centerIn: parent
            text: qsTr("Missing")
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontCaption
            color: Theme.warning
        }

    }

    Column {
        anchors.left: parent.left
        anchors.leftMargin: Theme.spaceMd + 32 + Theme.spaceMd
        anchors.right: missingBadge.left
        anchors.rightMargin: root.missing ? Theme.spaceSm : 0
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2

        Text {
            width: parent.width
            elide: Text.ElideRight
            text: root.title
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBody
            color: Theme.foreground
        }

        Text {
            width: parent.width
            elide: Text.ElideRight
            text: root.artist
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBodySm
            color: Theme.muted
        }

    }

}
