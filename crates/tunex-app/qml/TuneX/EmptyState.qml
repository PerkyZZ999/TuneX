import QtQuick

// EmptyState (S2 W-016): explicit, actionable empty surface for library
// views. Never a blank screen: always a title plus guidance. An action
// button arrives with the folders UI (W-017).
Column {
    id: root

    required property string title
    required property string note

    anchors.centerIn: parent
    width: Math.min(parent.width - Theme.spaceXl * 2, 420)
    spacing: Theme.spaceSm

    Text {
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.WordWrap
        text: root.title
        textFormat: Text.PlainText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontTitle
        font.weight: Font.DemiBold
        color: Theme.foreground
        Accessible.role: Accessible.Heading
        Accessible.name: root.title
    }

    Text {
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.WordWrap
        text: root.note
        textFormat: Text.PlainText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontBody
        color: Theme.muted
    }

}
