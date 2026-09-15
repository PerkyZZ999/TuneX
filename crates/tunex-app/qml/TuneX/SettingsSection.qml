import QtQuick
import TuneX

// SettingsSection: one settings page — heading, optional lede, content.
// Five pages shared one hand-rolled copy each. Spacing matches the old
// flat columns exactly (one scale between every row, fields grouped in
// the body), so wrapping changes no pixels.
Column {
    id: root

    property string title: ""
    property string description: ""
    default property alias content: body.children

    spacing: Theme.spaceMd

    Text {
        text: root.title
        textFormat: Text.PlainText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontTitle
        font.weight: Font.DemiBold
        color: Theme.foreground
        Accessible.role: Accessible.Heading
        Accessible.name: text
    }

    Text {
        visible: root.description !== ""
        width: parent.width
        wrapMode: Text.WordWrap
        text: root.description
        textFormat: Text.PlainText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontBody
        color: Theme.muted
    }

    Column {
        id: body

        width: parent.width
        spacing: Theme.spaceMd
    }
}
