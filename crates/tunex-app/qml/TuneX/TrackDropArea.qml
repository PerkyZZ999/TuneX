import QtQuick
import TuneX

// Drop target for TrackRow mime (`application/x-tunex-trackids`).
// Wash + accent edge + caption so a drop is never colour-only (DESIGN.md).
DropArea {
    id: root

    property string dropHint: qsTr("Drop to add")
    property bool showHint: true

    signal tracksDropped(string ids)

    keys: ["application/x-tunex-trackids", "text/plain"]
    Accessible.ignored: true

    onDropped: drop => {
        let ids = drop.getDataAsString("application/x-tunex-trackids");
        if (!ids)
            ids = drop.text;
        if (ids && ids.length > 0) {
            root.tracksDropped(ids);
            drop.acceptProposedAction();
        }
    }

    Rectangle {
        z: 1
        enabled: false
        anchors.fill: parent
        radius: Theme.radiusSm
        color: Theme.selected
        opacity: root.containsDrag ? 0.45 : 0

        Behavior on opacity {
            NumberAnimation {
                duration: Appearance.duration(Theme.motionHover)
                easing.type: Easing.OutCubic
            }
        }
    }

    Rectangle {
        z: 1
        enabled: false
        anchors.fill: parent
        radius: Theme.radiusSm
        color: "transparent"
        border.width: root.containsDrag ? 1 : 0
        border.color: Theme.accent
    }

    Text {
        z: 2
        enabled: false
        visible: root.showHint && root.containsDrag
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        anchors.topMargin: Theme.spaceSm
        text: root.dropHint
        textFormat: Text.PlainText
        font.family: Theme.fontFamily
        font.pixelSize: Theme.fontCaption
        font.weight: Font.DemiBold
        color: Theme.foreground
    }
}
