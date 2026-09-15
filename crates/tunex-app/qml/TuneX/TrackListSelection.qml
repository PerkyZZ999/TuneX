import QtQuick

// View-local multi-select (S14 W-073). Rows are indices, not SQL ids, so
// duplicate playlist/queue entries stay independent. Every mutation
// reassigns `rows`, so host bindings stay subscribed by reading it —
// no version counter needed.
QtObject {
    id: root

    property int anchor: -1
    property list<int> rows

    readonly property int count: root.rows.length

    function contains(row) {
        const rows = root.rows;
        for (let i = 0; i < rows.length; i++) {
            if (rows[i] === row)
                return true;
        }
        return false;
    }

    function clear() {
        if (root.rows.length === 0 && root.anchor < 0)
            return false;
        root.rows = [];
        root.anchor = -1;
        return true;
    }

    function toggle(row) {
        const next = [];
        let found = false;
        const rows = root.rows;
        for (let i = 0; i < rows.length; i++) {
            if (rows[i] === row)
                found = true;
            else
                next.push(rows[i]);
        }
        if (!found)
            next.push(row);
        root.rows = next;
        root.anchor = row;
    }

    function setRange(to) {
        const from = root.anchor < 0 ? to : root.anchor;
        const lo = Math.min(from, to);
        const hi = Math.max(from, to);
        const next = [];
        for (let i = lo; i <= hi; i++)
            next.push(i);
        root.rows = next;
    }

    function selectAll(count) {
        const next = [];
        for (let i = 0; i < count; i++)
            next.push(i);
        root.rows = next;
        if (count > 0)
            root.anchor = 0;
    }

    function sorted() {
        return root.rows.slice().sort(function (a, b) {
            return a - b;
        });
    }

    // Comma ids for the selection through one model's trackIdAt — the
    // drag/drop/clipboard wire format every track list shares. Reading
    // `rows` keeps host bindings subscribed.
    function mimeIds(model) {
        const rows = root.sorted();
        const ids = [];
        for (let i = 0; i < rows.length; i++) {
            const id = model.trackIdAt(rows[i]);
            if (id >= 0)
                ids.push(id);
        }
        return ids.join(",");
    }
}
