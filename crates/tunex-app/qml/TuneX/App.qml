import QtQuick
import QtQuick.Controls.Basic
import TuneX 1.0

// Application shell (S1 W-003): navigation rail, top bar with view history
// and the global search field, and per-section content. Home, search,
// library, and playlists are live. S4 W-026 adds the persistent player:
// docked Up Next at ≥1280px, MiniPlayer bar below that, both hidden until
// the first play.
Window {
    id: root

    // View history: reassigned (never mutated in place) so bindings update.
    property var history: ["home"]
    property int historyAt: 0
    property string section: "home"
    property bool playerActive: false
    property bool nowPlayingOpen: false
    readonly property bool canGoBack: historyAt > 0
    readonly property bool canGoForward: historyAt < history.length - 1
    readonly property bool wideShell: root.width >= Theme.shellWide
    readonly property bool showMiniPlayer: !root.wideShell && root.playerActive
    readonly property bool showDockedQueue: root.wideShell && root.playerActive

    function sectionTitle(key) {
        if (key === "search")
            return qsTr("Search");

        if (key === "library")
            return qsTr("Your Library");

        if (key === "playlists")
            return qsTr("Playlists");

        if (key === "settings")
            return qsTr("Settings");

        return qsTr("Home");
    }

    function navigate(target) {
        if (target === root.history[root.historyAt])
            return ;

        root.history = root.history.slice(0, root.historyAt + 1).concat([target]);
        root.historyAt = root.history.length - 1;
        root.section = target;
    }

    function goBack() {
        if (!root.canGoBack)
            return ;

        root.historyAt -= 1;
        root.section = root.history[root.historyAt];
    }

    function goForward() {
        if (!root.canGoForward)
            return ;

        root.historyAt += 1;
        root.section = root.history[root.historyAt];
    }

    function toggleQueue() {
        if (queueDrawer.opened)
            queueDrawer.close();
        else
            queueDrawer.open();
    }

    function openNowPlaying() {
        root.nowPlayingOpen = true;
    }

    function closeNowPlaying() {
        root.nowPlayingOpen = false;
    }

    function syncPlayer() {
        queueModel.poll();
        const state = queueModel.playbackState();
        const cursor = queueModel.currentIndex();
        if (cursor >= 0 || state === 1 || state === 2 || state === 3)
            root.playerActive = true;

        if (miniPlayer.visible)
            miniPlayer.sync();

        if (nowPlayingLoader.item)
            nowPlayingLoader.item.sync();

    }

    minimumWidth: Theme.windowMinWidth
    minimumHeight: Theme.windowMinHeight
    width: 1280
    height: 800
    visible: true
    title: qsTr("TuneX")
    color: Theme.background
    onWidthChanged: {
        if (root.wideShell && queueDrawer.opened)
            queueDrawer.close();

    }

    // Global search shortcut (R-014): `/` or Ctrl+K focuses the shell field
    // from anywhere; typing navigates to the results view.
    Shortcut {
        sequences: ["/", "Ctrl+K"]
        onActivated: {
            if (root.nowPlayingOpen)
                return ;

            if (!searchField.activeFocus) {
                if (root.section !== "search")
                    root.navigate("search");

                searchField.forceActiveFocus();
            }
        }
    }

    // Up Next queue, owned by the shell so playback and rows survive
    // navigation. Polls always (gapless advance must run with the drawer
    // closed); the panel only mirrors display state while open.
    QueueModel {
        id: queueModel
    }

    // App-scoped so library/search row menus and the Playlists section
    // share one list (adds from a ⋯ menu land without a second index).
    PlaylistModel {
        id: playlistModel
    }

    Timer {
        interval: 300
        running: true
        repeat: true
        onTriggered: root.syncPlayer()
    }

    Drawer {
        id: queueDrawer

        width: Theme.panelWidth
        height: parent.height
        edge: Qt.RightEdge
        interactive: !root.wideShell

        QueuePanel {
            id: drawerQueue

            anchors.fill: parent
            queue: queueModel
            embedded: false
            tracking: queueDrawer.opened
            onBrowseRequested: {
                queueDrawer.close();
                root.navigate("library");
            }
            onCloseRequested: queueDrawer.close()
            onExpandRequested: root.openNowPlaying()
        }

    }

    Column {
        anchors.fill: parent

        Row {
            width: parent.width
            height: parent.height - miniPlayer.height

            Rectangle {
                width: Theme.railWidth
                height: parent.height
                color: Theme.surface

                Column {
                    anchors.fill: parent
                    anchors.margins: Theme.spaceMd
                    spacing: Theme.spaceXs

                    NavItem {
                        label: qsTr("Home")
                        selected: root.section === "home"
                        onActivated: root.navigate("home")
                    }

                    NavItem {
                        label: qsTr("Search")
                        selected: root.section === "search"
                        onActivated: root.navigate("search")
                    }

                    NavItem {
                        label: qsTr("Your Library")
                        selected: root.section === "library"
                        onActivated: root.navigate("library")
                    }

                    NavItem {
                        label: qsTr("Playlists")
                        selected: root.section === "playlists"
                        onActivated: root.navigate("playlists")
                    }

                    NavItem {
                        label: qsTr("Settings")
                        selected: root.section === "settings"
                        onActivated: root.navigate("settings")
                    }

                }

            }

            Column {
                width: parent.width - Theme.railWidth
                height: parent.height

                Item {
                    id: topBar

                    width: parent.width
                    height: 56

                    Row {
                        id: navRow

                        anchors.verticalCenter: parent.verticalCenter
                        anchors.left: parent.left
                        anchors.leftMargin: Theme.spaceMd
                        spacing: Theme.spaceSm

                        Button {
                            text: qsTr("Back")
                            enabled: root.canGoBack
                            onClicked: root.goBack()
                        }

                        Button {
                            text: qsTr("Forward")
                            enabled: root.canGoForward
                            onClicked: root.goForward()
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: root.sectionTitle(root.section)
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontTitle
                            font.weight: Font.DemiBold
                            color: Theme.foreground
                        }

                    }

                    // Global pill search field (S3 W-020): text persists for the
                    // session; typing navigates to the results view, Esc is
                    // scope-aware (clear text, then leave search), Down/Enter
                    // move focus into the results.
                    TextField {
                        id: searchField

                        anchors.left: navRow.right
                        anchors.leftMargin: Theme.spaceMd
                        anchors.right: queueButton.visible ? queueButton.left : parent.right
                        anchors.rightMargin: Theme.spaceMd
                        anchors.verticalCenter: parent.verticalCenter
                        height: 44
                        placeholderText: qsTr("Search your library")
                        Accessible.name: qsTr("Search your library")
                        color: Theme.foreground
                        placeholderTextColor: Theme.muted
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontBody
                        leftPadding: Theme.spaceMd
                        rightPadding: Theme.spaceMd
                        onTextChanged: {
                            if (searchField.text !== "" && root.section !== "search")
                                root.navigate("search");

                        }
                        Keys.onEscapePressed: {
                            // Scope-aware unwind: drill first (query preserved),
                            // then clear text, then leave search.
                            if (searchField.text !== "") {
                                if (root.section === "search" && searchView.drilled)
                                    searchView.leaveDrill();
                                else
                                    searchField.text = "";
                            } else if (root.section === "search") {
                                root.goBack();
                            }
                        }
                        Keys.onDownPressed: {
                            if (root.section === "search")
                                searchView.focusResults();

                        }
                        Keys.onReturnPressed: {
                            if (root.section === "search")
                                searchView.focusResults();

                        }
                        Keys.onEnterPressed: {
                            if (root.section === "search")
                                searchView.focusResults();

                        }

                        background: Rectangle {
                            radius: Theme.radiusPill
                            color: Theme.surfaceRaised
                            border.color: searchField.activeFocus ? Theme.focus : Theme.border
                            border.width: searchField.activeFocus ? 2 : 1
                        }

                    }

                    // Up Next toggle until the persistent player owns it
                    // (MiniPlayer bar or the docked panel after first play).
                    Button {
                        id: queueButton

                        visible: !root.playerActive
                        anchors.right: parent.right
                        anchors.rightMargin: Theme.spaceMd
                        anchors.verticalCenter: parent.verticalCenter
                        text: qsTr("Up Next")
                        Accessible.name: qsTr("Open Up Next queue")
                        onClicked: root.toggleQueue()
                    }

                }

                Row {
                    width: parent.width
                    height: parent.height - topBar.height

                    Item {
                        width: parent.width - dockedQueue.width
                        height: parent.height

                        HomeView {
                            visible: root.section === "home"
                        }

                        SearchView {
                            id: searchView

                            visible: root.section === "search"
                            query: searchField.text
                            queue: queueModel
                            playlists: playlistModel
                            onFocusFieldRequested: searchField.forceActiveFocus()
                            onClearRequested: {
                                searchField.text = "";
                                searchField.forceActiveFocus();
                            }
                        }

                        LibraryView {
                            visible: root.section === "library"
                            queue: queueModel
                            playlists: playlistModel
                        }

                        PlaylistsView {
                            visible: root.section === "playlists"
                            queue: queueModel
                            playlists: playlistModel
                        }

                        SectionStub {
                            visible: root.section === "settings"
                            title: qsTr("Settings")
                            note: qsTr("Library folders, playback, and appearance land in S4.")
                        }

                    }

                    QueuePanel {
                        id: dockedQueue

                        visible: root.showDockedQueue
                        width: visible ? Theme.panelWidth : 0
                        height: parent.height
                        queue: queueModel
                        embedded: true
                        tracking: visible
                        onBrowseRequested: root.navigate("library")
                        onExpandRequested: root.openNowPlaying()
                    }

                }

            }

        }

        MiniPlayer {
            id: miniPlayer

            visible: root.showMiniPlayer
            width: parent.width
            height: visible ? Theme.miniPlayerHeight : 0
            queue: queueModel
            queueOpen: queueDrawer.opened
            onQueueToggleRequested: root.toggleQueue()
            onExpandRequested: root.openNowPlaying()
        }

    }

    Loader {
        id: nowPlayingLoader

        active: root.nowPlayingOpen
        anchors.fill: parent
        z: 20
        sourceComponent: nowPlayingComponent
    }

    Component {
        id: nowPlayingComponent

        NowPlayingView {
            queue: queueModel
            onCloseRequested: root.closeNowPlaying()
            onQueueToggleRequested: {
                root.closeNowPlaying();
                if (!root.wideShell)
                    root.toggleQueue();

            }
        }

    }

}
