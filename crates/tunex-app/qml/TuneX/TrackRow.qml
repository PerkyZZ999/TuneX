import QtQuick
import TuneX

// TrackRow (S2 W-016, actions in S3 W-022): one song row — number,
// title/artist, duration, missing badge, now-playing marker. Solid text on
// the opaque view background (never glass). Click (or keyboard press) plays
// the row now; the always-visible ⋯ button opens the container-owned row
// menu (Up Next rows get move/remove, library rows get queue actions).
// Edge-anchored layout: the middle column fills whatever the fixed edges
// leave, so no spacing is hand-counted and nothing depends on sibling
// creation order.
Item {
    id: root

    // Plain (not required) properties, set from model roles at instantiation:
    // `required` construction-time initialization races the delegate context
    // in this setup and locks role bindings to their defaults (W-018 gate).
    property int trackId: -1
    property int rowIndex: -1
    property string title: ""
    property string artist: ""
    property int trackNumber: 0
    property int durationMs: 0
    property bool missing: false
    property bool dangling: false
    property bool isCurrent: false
    // m:ss, em dash when unknown. Numbers need no translation.
    readonly property string durationText: root.durationMs > 0 ? Math.floor(root.durationMs / 60000) + ":" + String(Math.floor(root.durationMs / 1000) % 60).padStart(2, "0") : "—"
    readonly property string numberText: root.trackNumber > 0 ? String(root.trackNumber) : "—"

    signal playRequested(int trackId, int rowIndex, bool dangling)
    signal menuRequested(int trackId, int rowIndex, bool dangling)

    width: ListView.view.width
    height: 56
    Accessible.role: Accessible.ListItem
    Accessible.name: root.title + ", " + root.artist + (root.isCurrent ? ", " + qsTr("now playing") : "") + (root.missing ? ", " + qsTr("missing") : "") + (root.dangling ? ", " + qsTr("unavailable") : "")
    Accessible.onPressAction: {
        if (!root.dangling && !root.missing)
            root.playRequested(root.trackId, root.rowIndex, root.dangling);
    }

    // Hover/focus surface (DESIGN.md `track-row` → `track-row-hover`): the
    // row itself stays transparent over the view, and the tint fades within
    // the 120ms colour-only budget. Unavailable rows never light up — their
    // click area is disabled, so they report no hover.
    Rectangle {
        anchors.fill: parent
        radius: Theme.radiusSm
        color: rowArea.containsMouse || root.activeFocus ? Theme.hover : "transparent"

        Behavior on color {
            ColorAnimation {
                duration: Appearance.duration(Theme.motionHover)
                easing.type: Easing.OutCubic
            }
        }
    }

    // Now-playing marker: accent bar plus bold title (never color alone).
    Rectangle {
        visible: root.isCurrent
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.topMargin: Theme.spaceSm
        anchors.bottomMargin: Theme.spaceSm
        width: 3
        radius: 2
        color: Theme.accent
    }

    // Whole-row click plays now; the ⋯ button sits above in z-order.
    MouseArea {
        id: rowArea

        anchors.fill: parent
        enabled: !root.dangling && !root.missing
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.playRequested(root.trackId, root.rowIndex, root.dangling)
    }

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

    Rectangle {
        id: danglingBadge

        anchors.right: missingBadge.left
        anchors.rightMargin: root.dangling ? Theme.spaceSm : 0
        anchors.verticalCenter: parent.verticalCenter
        width: root.dangling ? danglingLabel.width + Theme.spaceSm * 2 : 0
        height: danglingLabel.height + Theme.spaceXs
        visible: root.dangling
        radius: Theme.radiusPill
        color: Theme.surfaceRaised
        border.color: Theme.error
        border.width: 1

        Text {
            id: danglingLabel

            anchors.centerIn: parent
            text: qsTr("Unavailable")
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontCaption
            color: Theme.error
        }
    }

    // Always visible (touch and keyboard targets are never hover-only);
    // quiet muted glyph at the 40px dense-list exception size.
    IconButton {
        id: menuButton

        anchors.right: danglingBadge.left
        anchors.rightMargin: Theme.spaceSm
        anchors.verticalCenter: parent.verticalCenter
        size: 40
        iconName: "ellipsis"
        glyphColor: Theme.muted
        accessibleName: qsTr("More actions for %1").arg(root.title)
        onActivated: root.menuRequested(root.trackId, root.rowIndex, root.dangling)
    }

    Column {
        anchors.left: parent.left
        anchors.leftMargin: Theme.spaceMd + 32 + Theme.spaceMd
        anchors.right: menuButton.left
        anchors.rightMargin: Theme.spaceSm
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2

        Text {
            width: parent.width
            elide: Text.ElideRight
            text: root.title
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBody
            font.weight: root.isCurrent ? Font.DemiBold : Font.Normal
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
