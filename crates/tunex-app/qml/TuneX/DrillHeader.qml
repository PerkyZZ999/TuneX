import QtQuick
import TuneX

// DrillHeader: a drilled-in list header — way back, play it now, queue it,
// plus the drilled title. The album, folder, and result drills shared one
// hand-rolled row each.
Row {
    id: root

    property string backText: ""
    property string playText: ""
    property string playAccessibleName: ""
    property string queueText: ""
    property string queueAccessibleName: ""
    property string titleText: ""

    signal backRequested
    signal playRequested
    signal queueRequested

    spacing: Theme.spaceSm

    PrimaryButton {
        id: backButton

        primary: false
        text: root.backText
        onClicked: root.backRequested()
    }

    PrimaryButton {
        id: playButton

        text: root.playText
        Accessible.name: root.playAccessibleName
        onClicked: root.playRequested()
    }

    PrimaryButton {
        id: queueButton

        primary: false
        text: root.queueText
        Accessible.name: root.queueAccessibleName
        onClicked: root.queueRequested()
    }

    Text {
        width: parent.width - backButton.width - playButton.width - queueButton.width - Theme.spaceSm * 3
        anchors.verticalCenter: parent.verticalCenter
        elide: Text.ElideRight
        text: root.titleText
        textFormat: Text.PlainText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontTitle
        font.weight: Font.DemiBold
        color: Theme.foreground
    }
}
