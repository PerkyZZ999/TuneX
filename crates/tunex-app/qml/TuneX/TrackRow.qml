import QtQuick
import TuneX

// TrackRow: one song row — number, title/artist, duration, missing badge,
// now-playing marker. Solid text on the canvas behind the list (never glass). Left-click (or keyboard press)
// plays the row now; right-click, the Menu key, Shift+F10, or the always-
// visible ⋯ button opens the container-owned row menu (queueing is
// "Add to Now Playing", never the click). Now Playing sets `compact` so
// the leading index and ⋯ drop out — the rail is too narrow for both,
// and the same menu is on the right-click.
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
    property bool isPlaying: false
    property bool reorderable: false
    property bool selected: false
    // Now Playing rail/drawer: no track number, no ⋯, title/artist one step down.
    property bool compact: false
    readonly property int indexSlotWidth: root.compact ? 14 : 32
    readonly property int titleSize: root.compact ? Theme.fontBodySm : Theme.fontBody
    readonly property int artistSize: root.compact ? Theme.fontCaption : Theme.fontBodySm
    property string dragTrackIds: root.trackId >= 0 ? String(root.trackId) : ""
    // Drag origin for drop routing: "" means an external copy (library /
    // search); "queue" and "playlist:<id>" mark an internal move, with the
    // source display rows in `dragRows` so the target moves rows, never ids
    // (queue and playlist rows can repeat the same track).
    property string dragOrigin: ""
    property string dragRows: ""
    // m:ss, em dash when unknown. Numbers need no translation.
    readonly property string durationText: root.durationMs > 0 ? Math.floor(root.durationMs / 60000) + ":" + String(Math.floor(root.durationMs / 1000) % 60).padStart(2, "0") : "—"
    readonly property string numberText: root.trackNumber > 0 ? String(root.trackNumber) : "—"

    signal playRequested(int trackId, int rowIndex, bool dangling)
    signal menuRequested(int trackId, int rowIndex, bool dangling)
    signal reorderRequested(int from, int to)
    signal toggleSelectRequested(int rowIndex)
    signal rangeSelectRequested(int rowIndex)

    readonly property int dragCount: {
        if (root.dragTrackIds.length === 0)
            return 0;
        return root.dragTrackIds.split(",").length;
    }
    readonly property bool dragging: rowArea.drag.active

    Drag.keys: ["application/x-tunex-trackids", "application/x-tunex-origin", "application/x-tunex-rows"]
    Drag.mimeData: {
        "text/plain": root.dragTrackIds,
        "application/x-tunex-trackids": root.dragTrackIds,
        "application/x-tunex-origin": root.dragOrigin,
        "application/x-tunex-rows": root.dragRows
    }
    Drag.dragType: Drag.Automatic
    Drag.active: rowArea.drag.active
    Drag.hotSpot.x: Theme.spaceMd
    Drag.hotSpot.y: Theme.trackRowHeight / 2
    opacity: root.dragging ? 0.5 : 1
    width: ListView.view.width
    height: Theme.trackRowHeight

    Behavior on opacity {
        NumberAnimation {
            duration: Appearance.duration(Theme.motionHover)
            easing.type: Easing.OutCubic
        }
    }
    Accessible.role: Accessible.ListItem
    Accessible.name: root.title + ", " + root.artist + (root.isCurrent ? ", " + qsTr("now playing") : "") + (root.missing ? ", " + qsTr("missing") : "") + (root.dangling ? ", " + qsTr("unavailable") : "")
    Accessible.onPressAction: {
        if (!root.dangling && !root.missing)
            root.playRequested(root.trackId, root.rowIndex, root.dangling);
    }
    Keys.onMenuPressed: root.menuRequested(root.trackId, root.rowIndex, root.dangling)
    Keys.onPressed: event => {
        if (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier)) {
            root.menuRequested(root.trackId, root.rowIndex, root.dangling);
            event.accepted = true;
        }
    }

    // Hover/focus surface (DESIGN.md `track-row` → `track-row-hover`): the
    // row itself stays transparent over the view, and the tint fades within
    // the 120ms colour-only budget. Unavailable rows never light up — their
    // click area is disabled, so they report no hover.
    Rectangle {
        anchors.fill: parent
        radius: Theme.radiusSm
        color: {
            if (root.selected)
                return Theme.selected;
            if (rowArea.containsMouse || root.activeFocus)
                return Theme.hover;
            return "transparent";
        }

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
        width: 3
        radius: 2
        color: Theme.accent
    }

    Row {
        id: equalizer

        visible: root.isCurrent
        anchors.left: parent.left
        anchors.leftMargin: Theme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2
        width: 14
        height: 12

        Repeater {
            model: 3

            Rectangle {
                required property int index

                width: 3
                radius: 1
                color: Theme.accent
                height: {
                    if (Appearance.reduceMotion || !root.isPlaying)
                        return index === 1 ? 12 : 6;
                    return 6;
                }

                SequentialAnimation on height {
                    running: root.isPlaying && !Appearance.reduceMotion
                    loops: Animation.Infinite

                    NumberAnimation {
                        to: equalizer.height
                        duration: Appearance.duration(Theme.motionHover) + index * 40
                        easing.type: Easing.InOutQuad
                    }

                    NumberAnimation {
                        to: 4
                        duration: Appearance.duration(Theme.motionHover) + 80 - index * 20
                        easing.type: Easing.InOutQuad
                    }
                }
            }
        }
    }

    // Whole-row click plays now; the ⋯ button sits above in z-order.
    MouseArea {
        id: rowArea

        anchors.fill: parent
        enabled: true
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: {
            if (root.dangling || root.missing)
                return Qt.ArrowCursor;
            if (root.dragging)
                return Qt.ClosedHandCursor;
            return Qt.PointingHandCursor;
        }
        drag.target: dragGhost
        drag.axis: Drag.XAndYAxis
        drag.threshold: 12
        onReleased: mouse => {
            if (mouse.button === Qt.RightButton) {
                root.menuRequested(root.trackId, root.rowIndex, root.dangling);
                return;
            }
            if (mouse.modifiers & Qt.ControlModifier) {
                root.toggleSelectRequested(root.rowIndex);
                return;
            }
            if (mouse.modifiers & Qt.ShiftModifier) {
                root.rangeSelectRequested(root.rowIndex);
                return;
            }
            if (rowArea.drag.active) {
                const action = root.Drag.drop();
                if (action !== Qt.IgnoreAction)
                    return;
                if (root.reorderable) {
                    const view = root.ListView.view;
                    if (view) {
                        const point = root.mapToItem(view.contentItem, rowArea.mouseX, rowArea.mouseY);
                        const to = view.indexAt(point.x, point.y);
                        if (to >= 0 && to !== root.rowIndex)
                            root.reorderRequested(root.rowIndex, to);
                    }
                }
                return;
            }
            if (root.dangling || root.missing)
                return;
            root.playRequested(root.trackId, root.rowIndex, root.dangling);
        }
    }

    Item {
        id: dragGhost

        width: 1
        height: 1
        visible: false
    }

    // In-window drag chip. Wayland often has no compositor pixmap for
    // Drag.Automatic, so the chip is the visible grab (surface-raised,
    // 1px border, never glow). It rides up and right of the cursor so the
    // tip — and the list's insertion line under it — stays visible while
    // aiming a positional drop.
    Rectangle {
        id: dragProxy

        parent: root.dragging && Window.window ? Window.window.contentItem : root
        visible: root.dragging
        width: Math.min(Theme.panelWidth - Theme.spaceMd, Math.max(Theme.railNarrow * 2, root.width))
        height: Theme.trackRowHeight
        x: {
            if (!root.dragging || !parent)
                return 0;
            return root.mapToItem(parent, rowArea.mouseX, rowArea.mouseY).x + Theme.spaceMd;
        }
        y: {
            if (!root.dragging || !parent)
                return 0;
            return root.mapToItem(parent, rowArea.mouseX, rowArea.mouseY).y - height - Theme.spaceSm;
        }
        z: 10000
        radius: Theme.radiusSm
        color: Theme.surfaceRaised
        border.width: 1
        border.color: Theme.accent
        opacity: 0.96

        Rectangle {
            visible: root.dragCount > 1
            anchors.right: parent.right
            anchors.rightMargin: Theme.spaceXs
            anchors.verticalCenter: parent.verticalCenter
            width: countLabel.implicitWidth + Theme.spaceSm
            height: Theme.fontCaption + Theme.spaceXs
            radius: Theme.radiusXs
            color: Theme.hover

            Text {
                id: countLabel

                anchors.centerIn: parent
                text: String(root.dragCount)
                textFormat: Text.PlainText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontCaption
                color: Theme.foreground
            }
        }

        Column {
            anchors.left: parent.left
            anchors.leftMargin: Theme.spaceSm
            anchors.right: parent.right
            anchors.rightMargin: root.dragCount > 1 ? Theme.spaceXl : Theme.spaceSm
            anchors.verticalCenter: parent.verticalCenter
            spacing: 0

            Text {
                width: parent.width
                elide: Text.ElideRight
                text: root.dragCount > 1 ? qsTr("%n songs", "", root.dragCount) : root.title
                textFormat: Text.PlainText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontBodySm
                font.weight: Font.DemiBold
                color: Theme.foreground
            }

            Text {
                width: parent.width
                elide: Text.ElideRight
                visible: root.dragCount <= 1
                text: root.artist
                textFormat: Text.PlainText
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontCaption
                color: Theme.muted
            }
        }
    }

    Text {
        anchors.left: parent.left
        anchors.leftMargin: Theme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        width: 32
        horizontalAlignment: Text.AlignRight
        text: root.numberText
        visible: !root.isCurrent && !root.compact
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
        radius: Theme.radiusXs
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
        radius: Theme.radiusXs
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
    // quiet muted glyph, 32px to match the compact list row.
    IconButton {
        id: menuButton

        anchors.right: danglingBadge.left
        anchors.rightMargin: root.compact ? 0 : Theme.spaceSm
        anchors.verticalCenter: parent.verticalCenter
        visible: !root.compact
        enabled: !root.compact
        width: root.compact ? 0 : Theme.buttonHeight
        size: Theme.buttonHeight
        glyphSize: Theme.navIconSize
        iconName: "ellipsis"
        glyphColor: Theme.muted
        accessibleName: qsTr("More actions for %1").arg(root.title)
        Accessible.ignored: root.compact
        onActivated: root.menuRequested(root.trackId, root.rowIndex, root.dangling)
    }

    Column {
        anchors.left: parent.left
        anchors.leftMargin: Theme.spaceMd + root.indexSlotWidth + Theme.spaceMd
        anchors.right: menuButton.left
        anchors.rightMargin: root.compact ? 0 : Theme.spaceSm
        anchors.verticalCenter: parent.verticalCenter
        spacing: 0

        Text {
            width: parent.width
            elide: Text.ElideRight
            text: root.title
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: root.titleSize
            font.weight: root.isCurrent ? Font.DemiBold : Font.Normal
            lineHeight: Theme.listLineHeight
            color: Theme.foreground
        }

        Text {
            width: parent.width
            elide: Text.ElideRight
            text: root.artist
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: root.artistSize
            lineHeight: Theme.listLineHeight
            color: Theme.muted
        }
    }
}
