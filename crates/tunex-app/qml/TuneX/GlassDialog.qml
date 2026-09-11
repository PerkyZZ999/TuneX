import QtQuick
import QtQuick.Controls.Basic
import TuneX

// GlassDialog (S6 W-038): dialogs on the glass-panel contract (DESIGN.md
// Components) — title-md header, body content, and a footer pairing one
// primary action with a secondary cancel (never two primaries). Subtle glass
// with the overlay shadow and lg radius over a dark scrim, centred on the
// window overlay at a fixed measure (content fills the available width).
// Esc and Cancel reject; hosts accept from fields on Enter.
Dialog {
    id: root

    property string acceptLabel: qsTr("OK")
    property string rejectLabel: qsTr("Cancel")
    property bool acceptEnabled: true

    parent: Overlay.overlay
    anchors.centerIn: Overlay.overlay
    // Fixed measure; content fills `availableWidth` (width: parent.width).
    width: Math.min(Theme.dialogWidth, Overlay.overlay.width - Theme.spaceXl * 2)
    modal: true
    focus: true
    closePolicy: Popup.CloseOnEscape
    padding: Theme.spaceLg
    topPadding: Theme.spaceMd
    bottomPadding: Theme.spaceMd

    Overlay.modal: Rectangle {
        color: Qt.alpha(Theme.background, Theme.scrimOpacity)
    }

    enter: Transition {
        ParallelAnimation {
            NumberAnimation {
                property: "opacity"
                from: 0
                to: 1
                duration: Appearance.duration(Theme.overlayMs)
                easing.type: Easing.OutCubic
            }

            NumberAnimation {
                property: "scale"
                from: 0.98
                to: 1
                duration: Appearance.duration(Theme.overlayMs)
                easing.type: Easing.OutCubic
            }
        }
    }

    exit: Transition {
        NumberAnimation {
            property: "opacity"
            from: 1
            to: 0
            duration: Appearance.duration(Theme.motionHover)
            easing.type: Easing.OutCubic
        }
    }

    header: Text {
        leftPadding: Theme.spaceLg
        rightPadding: Theme.spaceLg
        topPadding: Theme.spaceLg
        text: root.title
        textFormat: Text.PlainText
        elide: Text.ElideRight
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontTitle
        font.weight: Font.DemiBold
        color: Theme.foreground
        Accessible.role: Accessible.Heading
        Accessible.name: root.title
    }

    footer: Item {
        implicitWidth: actions.implicitWidth + Theme.spaceLg * 2
        implicitHeight: actions.implicitHeight + Theme.spaceLg

        Row {
            id: actions

            anchors.right: parent.right
            anchors.rightMargin: Theme.spaceLg
            anchors.top: parent.top
            spacing: Theme.spaceSm

            PrimaryButton {
                primary: false
                text: root.rejectLabel
                onClicked: root.reject()
            }

            PrimaryButton {
                text: root.acceptLabel
                enabled: root.acceptEnabled
                onClicked: root.accept()
            }
        }
    }

    background: GlassBackdrop {
        cornerRadius: Theme.radiusLg
        restingRect: Qt.rect(root.x, root.y, root.width, root.height)
    }
}
