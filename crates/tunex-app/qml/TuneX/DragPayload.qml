pragma Singleton
import QtQuick

// DragPayload: the one drag/drop contract. Rows publish ids/origin/rows
// under these mime keys; drop areas parse through these helpers; hosts
// classify moves through these predicates. renames touch only this file.
//
// Origins: "" is an external copy (library/search); "queue" and
// "playlist:<id>" mark internal moves whose source display rows ride along
// in `rows` (queue and playlist rows can repeat one track), so targets move
// rows and never duplicate ids.
QtObject {
    readonly property string mimeIds: "application/x-tunex-trackids"
    readonly property string mimeOrigin: "application/x-tunex-origin"
    readonly property string mimeRows: "application/x-tunex-rows"
    readonly property string originQueue: "queue"

    function playlistOrigin(playlistId) {
        return "playlist:" + playlistId;
    }

    // Full mime map for a row drag (object literals take no computed keys,
    // so rows cannot assemble this inline from the constants above).
    function mimeMap(ids, origin, rows) {
        const map = {};
        map["text/plain"] = ids;
        map[mimeIds] = ids;
        map[mimeOrigin] = origin;
        map[mimeRows] = rows;
        return map;
    }

    function idsOf(drag) {
        let ids = "";
        try {
            ids = drag.getDataAsString(mimeIds);
        } catch (err) {}
        if (!ids)
            ids = drag.text || "";
        return ids;
    }

    function originOf(drag) {
        let origin = "";
        try {
            origin = drag.getDataAsString(mimeOrigin);
        } catch (err) {}
        return origin || "";
    }

    function rowsOf(drag) {
        let rows = "";
        try {
            rows = drag.getDataAsString(mimeRows);
        } catch (err) {}
        return rows || "";
    }

    function countIds(ids) {
        if (!ids)
            return 0;
        return ids.split(",").filter(function (part) {
            return part.trim().length > 0;
        }).length;
    }

    function isInternalMove(origin, rows) {
        return origin !== "" && rows !== "";
    }

    function isQueueMove(origin, rows) {
        return origin === originQueue && rows !== "";
    }

    function isSamePlaylistMove(origin, rows, playlistId) {
        return origin === playlistOrigin(playlistId) && rows !== "";
    }
}
