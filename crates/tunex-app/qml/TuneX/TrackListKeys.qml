import QtQuick

// TrackListKeys: the shared keyboard contract for track lists. One instance
// per list owns cursor policy (cursor-or-zero, clamping, stepping with view
// follow) and routes the uniform keys — Return/Enter to activate, Menu to
// the row menu, Ctrl+A to select all, Delete to remove, Shift+Up/Down to
// extend the selection. What each action MEANS stays with the view, which
// connects the signals (play a track, open a playlist, pop a menu).
// Alt-reorder, j/k, and type-to-select stay per view: they are not uniform.
QtObject {
    id: root

    property ListView view
    property TrackListSelection selection

    signal activateRequested(int row)
    signal menuRequested(int row)
    signal deleteSelectionRequested
    signal deleteRowRequested(int row)

    // Cursor-or-zero: keyboard actions apply to the cursor row, falling back
    // to the first row when the list never took focus.
    function cursorRow() {
        const view = root.view;
        if (!view)
            return 0;
        return view.currentIndex >= 0 ? view.currentIndex : 0;
    }

    // Keep the cursor inside the list after removals; -1 when empty. Valid
    // and absent cursors are left untouched.
    function clamp() {
        const view = root.view;
        if (!view)
            return;
        if (view.count === 0)
            view.currentIndex = -1;
        else if (view.currentIndex >= view.count)
            view.currentIndex = view.count - 1;
    }

    // Move the cursor, follow it into view, and optionally extend the
    // selection (Shift+Up/Down). Always handled when the list is non-empty.
    function step(delta, extend) {
        const view = root.view;
        if (!view || view.count <= 0)
            return false;
        const next = Math.max(0, Math.min(view.count - 1, view.currentIndex + delta));
        view.currentIndex = next;
        view.positionViewAtIndex(next, ListView.Contain);
        if (extend && root.selection)
            root.selection.setRange(next);
        return true;
    }

    function activate() {
        const view = root.view;
        if (!view)
            return false;
        const at = cursorRow();
        if (at >= view.count)
            return false;
        activateRequested(at);
        return true;
    }

    function menu() {
        const view = root.view;
        if (!view)
            return false;
        const at = cursorRow();
        if (at >= view.count)
            return false;
        menuRequested(at);
        return true;
    }

    function selectAll() {
        const view = root.view;
        if (!view || !root.selection)
            return false;
        root.selection.selectAll(view.count);
        return true;
    }

    // Removal prefers the selection; with none, the cursor row goes. Views
    // without deletion simply never wire the signals.
    function remove() {
        const selection = root.selection;
        if (selection && selection.count > 0) {
            deleteSelectionRequested();
            return true;
        }
        const view = root.view;
        if (!view)
            return false;
        const at = view.currentIndex;
        if (at >= 0 && at < view.count) {
            deleteRowRequested(at);
            return true;
        }
        return false;
    }
}
