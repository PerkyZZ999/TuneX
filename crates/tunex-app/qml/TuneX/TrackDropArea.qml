import QtQuick

// Drop target for TrackRow mime (`application/x-tunex-trackids`).
DropArea {
    id: root

    signal tracksDropped(string ids)

    keys: ["application/x-tunex-trackids", "text/plain"]

    onDropped: drop => {
        let ids = drop.getDataAsString("application/x-tunex-trackids");
        if (!ids)
            ids = drop.text;
        if (ids && ids.length > 0) {
            root.tracksDropped(ids);
            drop.acceptProposedAction();
        }
    }
}
