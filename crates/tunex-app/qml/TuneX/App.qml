import QtQuick
import QtQuick.Controls.Basic
import TuneX

// Application shell (S1 W-003): navigation rail, top bar with view history
// and the global search field, and per-section content. Home, search,
// library, and playlists are live. S4 W-026 adds the persistent player:
// docked Up Next at ≥1280px, MiniPlayer bar below that, both hidden until
// the first play.
Window {
    id: root

    // View history: reassigned (never mutated in place) so bindings update.
    property list<string> history: ["home"]
    property int historyAt: 0
    property string section: "home"
    property bool playerActive: false
    property bool nowPlayingOpen: false
    readonly property bool canGoBack: historyAt > 0
    readonly property bool canGoForward: historyAt < history.length - 1
    readonly property bool wideShell: root.width >= Theme.shellWide
    readonly property bool compactRail: root.width < Theme.shellCompact
    readonly property int railSize: root.compactRail ? Theme.railNarrow : Theme.railWidth
    readonly property bool showMiniPlayer: !root.wideShell && root.playerActive
    readonly property bool showDockedQueue: root.wideShell
    readonly property int volumeStep: 5
    // Cover of the playing track, mirrored here so the ambient wash and the
    // chrome that sits over it share one source.
    property url ambientArt
    // The rail's playlist entries are list delegates, and a delegate scope
    // resolves `root` but not the other ids in this file, so both the open
    // playlist and the call that opens one travel through the root item.
    readonly property int openPlaylistId: playlistsView.playlistId
    readonly property bool editingText: {
        const item = root.activeFocusItem;
        if (!item)
            return false;

        return item instanceof TextField || item instanceof TextInput || item instanceof TextEdit || item instanceof TextArea;
    }

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
            return;

        root.history = root.history.slice(0, root.historyAt + 1).concat([target]);
        root.historyAt = root.history.length - 1;
        root.section = target;
    }

    function goBack() {
        if (!root.canGoBack)
            return;

        root.historyAt -= 1;
        root.section = root.history[root.historyAt];
    }

    function goForward() {
        if (!root.canGoForward)
            return;

        root.historyAt += 1;
        root.section = root.history[root.historyAt];
    }

    function openPlaylist(id, name) {
        root.navigate("playlists");
        playlistsView.selectPlaylist(id, name);
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

    function nudgeVolume(delta) {
        queueModel.setVolumePct(queueModel.volumePct() + delta);
    }

    function syncPlayer() {
        queueModel.poll();
        root.ambientArt = queueModel.currentArtUrl();
        const state = queueModel.playbackState();
        const cursor = queueModel.currentIndex();
        if (cursor >= 0 || state === 1 || state === 2 || state === 3)
            root.playerActive = true;

        if (miniPlayer.visible)
            miniPlayer.sync();

        if (nowPlayingLoader.status === Loader.Ready)
            (nowPlayingLoader.item as NowPlayingView).sync();

        library.poll();
        if (library.takeFinished()) {
            homeView.refresh();
            libraryView.refresh();
        }
    }

    minimumWidth: Theme.windowMinWidth
    minimumHeight: Theme.windowMinHeight
    width: 1280
    height: 800
    visible: true
    title: qsTr("TuneX")
    color: Theme.background
    // Token palette: any Basic-style chrome that is not custom-built
    // (tooltips, scroll indicators, text selection, fallback dims) follows
    // the theme instead of stock greys.
    palette.window: Theme.background
    palette.windowText: Theme.foreground
    palette.base: Theme.chrome
    palette.text: Theme.foreground
    palette.button: Theme.surfaceRaised
    palette.buttonText: Theme.foreground
    palette.highlight: Theme.primary
    palette.highlightedText: Theme.primaryText
    palette.placeholderText: Theme.muted
    palette.toolTipBase: Theme.surfaceRaised
    palette.toolTipText: Theme.foreground
    palette.mid: Theme.border
    palette.dark: Theme.muted
    palette.shadow: Theme.background
    palette.link: Theme.accent
    Component.onCompleted: {
        // Appearance first, so the first frame already honours both
        // accessibility preferences (read once, not per poll).
        Appearance.reduceTransparency = queueModel.reduceTransparency();
        Appearance.reduceMotion = queueModel.reduceMotion();
        Appearance.canvas = canvas;
        library.startup();
        playlistModel.refresh();
    }
    onWidthChanged: {
        if (root.wideShell && queueDrawer.opened)
            queueDrawer.close();
    }

    // Global search shortcut (R-014): `/` or Ctrl+K focuses the shell field
    // from anywhere; typing navigates to the results view.
    Shortcut {
        sequences: ["/", "Ctrl+K"]
        enabled: !root.editingText
        onActivated: {
            if (root.nowPlayingOpen)
                return;

            if (!searchField.activeFocus) {
                if (root.section !== "search")
                    root.navigate("search");

                searchField.forceActiveFocus();
            }
        }
    }

    // Space toggles playback except when a text field has focus (R-014).
    Shortcut {
        sequence: "Space"
        context: Qt.ApplicationShortcut
        enabled: !root.editingText
        onActivated: queueModel.playPause()
    }

    Shortcut {
        sequences: ["Media Play", "Media Pause", "Media Toggle Play Pause"]
        context: Qt.ApplicationShortcut
        onActivated: queueModel.playPause()
    }

    Shortcut {
        sequence: "Media Next"
        context: Qt.ApplicationShortcut
        onActivated: queueModel.nextTrack()
    }

    Shortcut {
        sequence: "Media Previous"
        context: Qt.ApplicationShortcut
        onActivated: queueModel.previousTrack(queueModel.positionMs())
    }

    Shortcut {
        sequence: "Volume Up"
        context: Qt.ApplicationShortcut
        onActivated: root.nudgeVolume(root.volumeStep)
    }

    Shortcut {
        sequence: "Volume Down"
        context: Qt.ApplicationShortcut
        onActivated: root.nudgeVolume(-root.volumeStep)
    }

    Shortcut {
        sequence: "Volume Mute"
        context: Qt.ApplicationShortcut
        onActivated: queueModel.setMuted(!queueModel.isMuted())
    }

    Shortcut {
        sequences: ["Alt+Left", "Back"]
        enabled: root.canGoBack && !root.editingText
        onActivated: root.goBack()
    }

    Shortcut {
        sequences: ["Alt+Right", "Forward"]
        enabled: root.canGoForward && !root.editingText
        onActivated: root.goForward()
    }

    // Overlay Esc is the Popup closePolicy. Search field Esc is local.
    // Remaining Esc walks view history.
    Shortcut {
        sequence: "Esc"
        enabled: !root.editingText && !root.nowPlayingOpen && root.canGoBack
        onActivated: root.goBack()
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

    LibraryManager {
        id: library
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
        // Overlay budget: 200ms slide, zeroed by reduce-motion. The scrim
        // fades with it (Overlay.modal below).
        enter: Transition {
            NumberAnimation {
                property: "position"
                to: 1
                duration: Appearance.duration(Theme.overlayMs)
                easing.type: Easing.OutCubic
            }
        }
        exit: Transition {
            NumberAnimation {
                property: "position"
                to: 0
                duration: Appearance.duration(Theme.overlayMs)
                easing.type: Easing.OutCubic
            }
        }

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

        Overlay.modal: Rectangle {
            color: Qt.alpha(Theme.background, Theme.scrimOpacity)
        }

        background: GlassBackdrop {
            restingRect: Qt.rect(root.width - queueDrawer.width, 0, queueDrawer.width, queueDrawer.height)
        }
    }

    FoldersDrawer {
        id: foldersDrawer

        manager: library
    }

    // The shell canvas the glass backdrops snapshot. It paints its own
    // opaque background so a blurred snapshot always covers the sharp
    // content beneath an overlay.
    Item {
        id: canvas

        anchors.fill: parent

        Rectangle {
            anchors.fill: parent
            color: Theme.background
        }

        AmbientWash {
            anchors.fill: parent
            source: root.ambientArt
        }

        Column {
            anchors.fill: parent

            Row {
                width: parent.width
                height: parent.height - miniPlayer.height

                Rectangle {
                    width: root.railSize
                    height: parent.height
                    // Translucent so the ambient wash shows through; solid
                    // again whenever the user reduces transparency.
                    color: Appearance.reduceTransparency ? Theme.surface : Qt.rgba(Theme.surface.r, Theme.surface.g, Theme.surface.b, Theme.chromeTint)

                    Rectangle {
                        anchors.top: parent.top
                        anchors.bottom: parent.bottom
                        anchors.right: parent.right
                        width: 1
                        color: Theme.border
                        Accessible.ignored: true
                    }

                    Column {
                        anchors.fill: parent
                        anchors.margins: Theme.spaceSm
                        spacing: Theme.spaceXs

                        Item {
                            width: parent.width
                            height: 48

                            Rectangle {
                                id: mark

                                width: 28
                                height: 28
                                radius: Theme.radiusSm
                                anchors.verticalCenter: parent.verticalCenter
                                anchors.left: parent.left
                                anchors.leftMargin: root.compactRail ? (parent.width - width) / 2 : Theme.spaceXs
                                color: Theme.primary

                                Text {
                                    anchors.centerIn: parent
                                    text: "X"
                                    font.family: Theme.fontFamily
                                    font.pixelSize: Theme.fontTitle
                                    font.weight: Font.Bold
                                    color: Theme.primaryText
                                }
                            }

                            Text {
                                visible: !root.compactRail
                                anchors.verticalCenter: parent.verticalCenter
                                anchors.left: mark.right
                                anchors.leftMargin: Theme.spaceSm
                                text: qsTr("TuneX")
                                font.family: Theme.fontFamily
                                font.pixelSize: Theme.fontTitle
                                font.weight: Font.DemiBold
                                color: Theme.foreground
                            }
                        }

                        NavItem {
                            label: qsTr("Home")
                            iconName: "home"
                            compact: root.compactRail
                            selected: root.section === "home"
                            onActivated: root.navigate("home")
                        }

                        NavItem {
                            label: qsTr("Search")
                            iconName: "search"
                            compact: root.compactRail
                            selected: root.section === "search"
                            onActivated: root.navigate("search")
                        }

                        Text {
                            visible: !root.compactRail
                            width: parent.width
                            leftPadding: Theme.spaceMd
                            topPadding: Theme.spaceSm
                            text: qsTr("YOUR LIBRARY")
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontCaption
                            font.weight: Font.DemiBold
                            font.letterSpacing: 0.8
                            color: Theme.muted
                        }

                        NavItem {
                            label: qsTr("Artists")
                            iconName: "users"
                            compact: root.compactRail
                            selected: root.section === "library" && libraryView.tab === "artists"
                            onActivated: {
                                libraryView.tab = "artists";
                                root.navigate("library");
                            }
                        }

                        NavItem {
                            label: qsTr("Albums")
                            iconName: "disc"
                            compact: root.compactRail
                            selected: root.section === "library" && libraryView.tab === "albums"
                            onActivated: {
                                libraryView.tab = "albums";
                                root.navigate("library");
                            }
                        }

                        NavItem {
                            label: qsTr("Songs")
                            iconName: "music"
                            compact: root.compactRail
                            selected: root.section === "library" && libraryView.tab === "songs"
                            onActivated: {
                                libraryView.tab = "songs";
                                root.navigate("library");
                            }
                        }

                        NavItem {
                            label: qsTr("Folders")
                            iconName: "folder"
                            compact: root.compactRail
                            selected: false
                            onActivated: foldersDrawer.open()
                        }

                        Text {
                            visible: !root.compactRail
                            width: parent.width
                            leftPadding: Theme.spaceMd
                            topPadding: Theme.spaceSm
                            text: qsTr("PLAYLISTS")
                            font.family: Theme.fontFamily
                            font.pixelSize: Theme.fontCaption
                            font.weight: Font.DemiBold
                            font.letterSpacing: 0.8
                            color: Theme.muted
                        }

                        NavItem {
                            label: qsTr("Create playlist")
                            iconName: "plus"
                            compact: root.compactRail
                            selected: false
                            onActivated: {
                                root.navigate("playlists");
                                playlistsView.openCreate();
                            }
                        }

                        ListView {
                            width: parent.width
                            height: Math.min(contentHeight, parent.height * 0.28)
                            clip: true
                            model: playlistModel
                            boundsBehavior: Flickable.StopAtBounds

                            delegate: NavItem {
                                id: playlistEntry

                                required property string name
                                required property int playlistId

                                width: ListView.view.width
                                label: playlistEntry.name
                                iconName: "list-music"
                                compact: root.compactRail
                                selected: root.section === "playlists" && root.openPlaylistId === playlistEntry.playlistId
                                onActivated: root.openPlaylist(playlistEntry.playlistId, playlistEntry.name)
                            }
                        }

                        Item {
                            width: 1
                            height: Theme.spaceSm
                        }
                    }
                }

                Column {
                    width: parent.width - root.railSize
                    height: parent.height

                    Item {
                        id: topBar

                        width: parent.width
                        height: 56

                        // Translucent chrome over the ambient wash, with a
                        // hairline where it meets the content below so the
                        // bar keeps an edge even when the wash is off.
                        Rectangle {
                            anchors.fill: parent
                            color: Appearance.reduceTransparency ? Theme.surface : Qt.rgba(Theme.surface.r, Theme.surface.g, Theme.surface.b, Theme.chromeTint)

                            Rectangle {
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.bottom: parent.bottom
                                height: 1
                                color: Theme.border
                                Accessible.ignored: true
                            }
                        }

                        Row {
                            id: navRow

                            anchors.verticalCenter: parent.verticalCenter
                            anchors.left: parent.left
                            anchors.leftMargin: Theme.spaceMd
                            spacing: Theme.spaceXs

                            IconButton {
                                iconName: "chevron-left"
                                accessibleName: qsTr("Back")
                                resting: true
                                enabled: root.canGoBack
                                onActivated: root.goBack()
                            }

                            IconButton {
                                iconName: "chevron-right"
                                accessibleName: qsTr("Forward")
                                resting: true
                                enabled: root.canGoForward
                                onActivated: root.goForward()
                            }
                        }

                        // Global pill search field (S3 W-020): text persists for the
                        // session; typing navigates to the results view, Esc is
                        // scope-aware (clear text, then leave search), Down/Enter
                        // move focus into the results. Magnifier + centered pill
                        // match docs/mockup.png / DESIGN.md search-field.
                        Item {
                            id: searchWrap

                            width: Math.min(520, Math.max(240, topBar.width - navRow.width - settingsButton.width - queueButton.width - Theme.spaceXl * 3))
                            height: Theme.targetMin
                            anchors.horizontalCenter: parent.horizontalCenter
                            anchors.verticalCenter: parent.verticalCenter

                            TextField {
                                id: searchField

                                anchors.fill: parent
                                placeholderText: qsTr("Search for songs, artists, albums…")
                                Accessible.name: qsTr("Search your library")
                                color: Theme.foreground
                                placeholderTextColor: Theme.muted
                                font.family: Theme.fontFamily
                                font.pixelSize: Theme.fontBody
                                leftPadding: Theme.spaceXl + Theme.spaceSm
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
                                    color: Theme.chrome
                                    border.color: searchField.activeFocus ? Theme.focus : Theme.border
                                    border.width: searchField.activeFocus ? 2 : 1
                                }
                            }

                            Icon {
                                anchors.left: parent.left
                                anchors.leftMargin: Theme.spaceMd
                                anchors.verticalCenter: parent.verticalCenter
                                name: "search"
                                iconSize: 20
                                stroke: Theme.muted
                            }
                        }

                        IconButton {
                            id: queueButton

                            visible: !root.wideShell
                            width: visible ? Theme.targetMin : 0
                            anchors.right: settingsButton.left
                            anchors.rightMargin: Theme.spaceXs
                            anchors.verticalCenter: parent.verticalCenter
                            iconName: "list-music"
                            accessibleName: queueDrawer.opened ? qsTr("Close Up Next") : qsTr("Open Up Next")
                            checkable: true
                            checked: queueDrawer.opened
                            onActivated: root.toggleQueue()
                        }

                        IconButton {
                            id: settingsButton

                            anchors.right: parent.right
                            anchors.rightMargin: Theme.spaceMd
                            anchors.verticalCenter: parent.verticalCenter
                            iconName: "settings"
                            accessibleName: qsTr("Settings")
                            checked: root.section === "settings"
                            onActivated: root.navigate("settings")
                        }
                    }

                    Row {
                        width: parent.width
                        height: parent.height - topBar.height

                        Item {
                            width: parent.width - dockedQueue.width
                            height: parent.height

                            HomeView {
                                id: homeView

                                visible: root.section === "home"
                                queue: queueModel
                                playlists: playlistModel
                                library: library
                                canContinue: root.playerActive
                                onBrowseRequested: tab => {
                                    libraryView.tab = tab;
                                    root.navigate("library");
                                }
                                onFoldersRequested: foldersDrawer.open()
                                onPlaylistsRequested: root.navigate("playlists")
                                onPlaylistOpened: (id, name) => root.openPlaylist(id, name)
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
                                id: libraryView

                                visible: root.section === "library"
                                queue: queueModel
                                playlists: playlistModel
                                library: library
                                onFoldersRequested: foldersDrawer.open()
                                onArtistRequested: name => {
                                    queueModel.clearQueue();
                                    queueModel.enqueueArtist(name);
                                    queueModel.playAt(0);
                                }
                            }

                            PlaylistsView {
                                id: playlistsView

                                visible: root.section === "playlists"
                                queue: queueModel
                                playlists: playlistModel
                            }

                            SectionStub {
                                visible: root.section === "settings"
                                title: qsTr("Settings")
                                note: qsTr("Keyboard (R-014): Space play/pause · media next/previous · volume up/down/mute · / or Ctrl+K search · Esc closes Now Playing, then search, then back · Alt+Left/Right history. Music folders live in the Folders rail item. Playback and appearance persist in config.")
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
