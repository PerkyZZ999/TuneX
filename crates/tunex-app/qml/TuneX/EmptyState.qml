import QtQuick

// EmptyState (S2 W-016): explicit, actionable empty surface for library
// views. Never a blank screen: always a title plus guidance. An action
// button arrives with the folders UI (W-017).
Column {
    id: root

    // Plain (not required) properties: view delegates in this project must
    // avoid `required` — its construction-time initialization races the
    // delegate model context and locks role bindings to defaults (W-018).
    property string title: ""
    property string note: ""
    // Optional action (W-017 folders entry); hidden when unlabeled.
    property string actionLabel: ""
    property bool primaryAction: true
    property bool centerInParent: true

    signal actionRequested

    anchors.centerIn: root.centerInParent ? parent : undefined
    width: Math.min(parent.width - (root.centerInParent ? Theme.spaceXl * 2 : 0), 420)
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

    PrimaryButton {
        visible: root.actionLabel !== ""
        anchors.horizontalCenter: parent.horizontalCenter
        primary: root.primaryAction
        text: root.actionLabel
        onClicked: root.actionRequested()
    }
}
