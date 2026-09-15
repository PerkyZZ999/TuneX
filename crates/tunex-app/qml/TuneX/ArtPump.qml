import QtQuick
import TuneX

// ArtPump: drains lazy cover resolvers until dry, then stops itself.
// Hosts pass their album models and kick the pump from card delegates as
// rows appear; it idles otherwise. Covers resolve one row at a time so
// the grid never stalls on art.
Timer {
    id: root

    property list<AlbumListModel> models

    interval: 120
    repeat: true
    onTriggered: {
        let busy = false;
        for (let i = 0; i < root.models.length; i++)
            busy = root.models[i].pollArt() || busy;
        if (!busy)
            root.stop();
    }
}
