import QtQuick
import TuneX

// Drop target for TrackRow mime (`application/x-tunex-trackids`).
// Two modes, one rule: drops land where the feedback showed.
//
// Plain mode (no `targetView`, e.g. a playlist sidebar row): whole-area
// wash + caption, and `tracksDropped` appends.
//
// Positional mode (`targetView` set: Now Playing, playlist detail): the
// hover computes the insertion index from the drag position and draws the
// accent line where the rows will land, with a `#N of M` caption so the
// feedback is never colour-only (DESIGN.md). Near the top/bottom edges the
// list auto-scrolls so long lists can receive positional drops too
// (functional scrolling, not motion chrome — it stays on with
// reduce-motion). `tracksDroppedAt` carries the index plus the drag
// origin/rows, so an internal drag moves rows while an external one
// inserts — never a silent duplicate append.
DropArea {
    id: root

    property string dropHint: qsTr("Drop to add")
    property bool showHint: true
    property ListView targetView
    // Insertion index while a drag hovers (0..count), -1 when idle.
    property int dropIndex: -1
    // Tracks under the cursor, for the line caption.
    property int dropCount: 0
    // Insertion-line position in this area's coordinates.
    property real lineY: 0
    // Last drag position in area coordinates (drives the line + autoscroll).
    property real dragX: 0
    property real dragY: 0
    // Edge zone + tick for auto-scroll while aiming.
    readonly property int scrollZone: 48
    readonly property int scrollTickMs: 32
    readonly property bool positional: root.targetView !== null
    readonly property string lineText: {
        if (!root.positional || root.dropIndex < 0)
            return "";
        const count = root.targetView.count;
        if (count <= 0)
            return qsTr("Drop here");
        let where = root.dropIndex >= count ? qsTr("at the end") : qsTr("#%1 of %2").arg(root.dropIndex + 1).arg(count);
        if (root.dropCount > 1)
            return qsTr("%n song(s)", "", root.dropCount) + " · " + where;
        return where;
    }

    signal tracksDropped(string ids)
    signal tracksDroppedAt(string ids, int index, string origin, string rows)

    keys: ["application/x-tunex-trackids", "application/x-tunex-origin", "application/x-tunex-rows", "text/plain"]
    Accessible.ignored: true

    onEntered: drag => root.updateHover(drag)
    onPositionChanged: drag => root.updateHover(drag)
    onExited: root.clearHover()
    onDropped: drop => {
        root.updateHover(drop);
        const ids = root.idsOf(drop);
        if (ids.length > 0) {
            if (root.positional) {
                const origin = drop.getDataAsString("application/x-tunex-origin");
                const rows = drop.getDataAsString("application/x-tunex-rows");
                root.tracksDroppedAt(ids, Math.max(0, root.dropIndex), origin || "", rows || "");
            } else {
                root.tracksDropped(ids);
            }
            drop.acceptProposedAction();
        }
        root.clearHover();
    }

    function idsOf(drag) {
        let ids = "";
        try {
            ids = drag.getDataAsString("application/x-tunex-trackids");
        } catch (err) {}
        if (!ids)
            ids = drag.text || "";
        return ids;
    }

    function countIds(ids) {
        if (!ids)
            return 0;
        return ids.split(",").filter(function (part) {
            return part.trim().length > 0;
        }).length;
    }

    function updateHover(drag) {
        if (!root.positional || drag === undefined || drag.x === undefined)
            return;
        root.dragX = drag.x;
        root.dragY = drag.y;
        root.dropCount = root.countIds(root.idsOf(drag));
        root.updateIndex();
    }

    // Recompute the insertion line for the stored cursor position (hover
    // moves and post-scroll refreshes share it).
    function updateIndex() {
        const view = root.targetView;
        if (!view)
            return;
        if (view.count <= 0) {
            root.dropIndex = 0;
            root.lineY = 0;
            return;
        }
        const probe = root.mapToItem(view.contentItem, root.dragX, root.dragY);
        const idx = view.indexAt(probe.x, probe.y);
        if (idx < 0) {
            if (probe.y < 0) {
                root.dropIndex = 0;
                root.lineY = root.boundaryY(0);
            } else {
                root.dropIndex = view.count;
                root.lineY = root.boundaryY(view.count);
            }
            return;
        }
        let item = view.itemAt(probe.x, probe.y);
        if (!item)
            item = view.itemAt(probe.x, probe.y - 2) || view.itemAt(probe.x, probe.y + 2);
        if (!item) {
            root.dropIndex = idx;
            root.lineY = root.boundaryY(idx);
            return;
        }
        const after = (probe.y - item.y) > (item.height / 2);
        const index = after ? Math.min(idx + 1, view.count) : idx;
        root.dropIndex = index;
        root.lineY = view.contentItem.mapToItem(root, 0, after ? item.y + item.height : item.y).y;
    }

    // Content y of the boundary before row `index`, in area coordinates
    // (estimated from the theme row stride when no delegate is handy).
    function boundaryY(index) {
        const view = root.targetView;
        if (!view || view.count <= 0)
            return 0;
        const stride = Theme.trackRowHeight + Theme.listRowGap;
        const header = view.headerItem ? view.headerItem.height : 0;
        const cy = header + index * stride;
        const y = view.contentItem.mapToItem(root, 0, cy).y;
        return Math.max(0, Math.min(y, root.height - 2));
    }

    function clearHover() {
        root.dropIndex = -1;
        root.dropCount = 0;
    }

    // Pixels per auto-scroll tick: faster the deeper into the edge zone.
    function scrollStep(depth) {
        return Math.round(8 + 20 * Math.min(1, depth / root.scrollZone));
    }

    function autoScroll() {
        const view = root.targetView;
        if (!view || view.count <= 0)
            return;
        let delta = 0;
        if (root.dragY < root.scrollZone)
            delta = -root.scrollStep(root.scrollZone - root.dragY);
        else if (root.dragY > root.height - root.scrollZone)
            delta = root.scrollStep(root.dragY - (root.height - root.scrollZone));
        if (delta === 0)
            return;
        view.contentY += delta;
        root.updateIndex();
    }

    Timer {
        interval: root.scrollTickMs
        repeat: true
        running: root.positional && root.containsDrag
        onTriggered: root.autoScroll()
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
        visible: root.showHint && root.containsDrag && !root.positional
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

    // Insertion line: the exact landing row boundary while aiming a
    // positional drop, with a `#N of M` caption so it is never colour-only.
    Rectangle {
        z: 3
        enabled: false
        visible: root.positional && root.containsDrag && root.dropIndex >= 0
        anchors.left: parent.left
        anchors.right: parent.right
        y: root.lineY - 1
        height: 2
        radius: 1
        color: Theme.accent
    }

    Rectangle {
        z: 3
        enabled: false
        visible: root.positional && root.containsDrag && root.dropIndex >= 0 && root.lineText !== ""
        anchors.right: parent.right
        anchors.rightMargin: Theme.spaceSm
        y: Math.max(0, Math.min(root.lineY - height - Theme.spaceXs, root.height - height))
        width: lineCaption.implicitWidth + Theme.spaceSm * 2
        height: lineCaption.implicitHeight + Theme.spaceXs
        radius: Theme.radiusXs
        color: Theme.surfaceRaised
        border.width: 1
        border.color: Theme.accent

        Text {
            id: lineCaption

            anchors.centerIn: parent
            text: root.lineText
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontCaption
            font.weight: Font.DemiBold
            color: Theme.foreground
        }
    }
}
