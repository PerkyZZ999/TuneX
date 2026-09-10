import QtQuick
import QtQuick.Controls.Basic
import TuneX 1.0

// Application shell (S1 W-003): navigation rail, top bar with view history
// and the global search field, and per-section content. Home is real (hero
// + empty state); search is live in S3; other sections are honest stubs
// until their slices land. Settings and player surfaces arrive with S4.
Window {
    id: root

    // View history: reassigned (never mutated in place) so bindings update.
    property var history: ["home"]
    property int historyAt: 0
    property string section: "home"
    readonly property bool canGoBack: historyAt > 0
    readonly property bool canGoForward: historyAt < history.length - 1

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

    minimumWidth: Theme.windowMinWidth
    minimumHeight: Theme.windowMinHeight
    width: 1280
    height: 800
    visible: true
    title: qsTr("TuneX")
    color: Theme.background

    // Global search shortcut (R-014): `/` or Ctrl+K focuses the shell field
    // from anywhere; typing navigates to the results view.
    Shortcut {
        sequences: ["/", "Ctrl+K"]
        onActivated: {
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

    Timer {
        interval: 300
        running: true
        repeat: true
        onTriggered: queueModel.poll()
    }

    QueuePanel {
        id: queuePanel

        queue: queueModel
        onBrowseRequested: {
            queuePanel.close();
            root.navigate("library");
        }
    }

    Row {
        anchors.fill: parent

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
                    anchors.right: queueButton.left
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

                // Up Next toggle (interim home until the S4 persistent
                // player owns queue access).
                Button {
                    id: queueButton

                    anchors.right: parent.right
                    anchors.rightMargin: Theme.spaceMd
                    anchors.verticalCenter: parent.verticalCenter
                    text: qsTr("Up Next")
                    Accessible.name: qsTr("Open Up Next queue")
                    onClicked: {
                        if (queuePanel.opened)
                            queuePanel.close();
                        else
                            queuePanel.open();
                    }
                }

            }

            Item {
                width: parent.width
                height: parent.height - topBar.height

                HomeView {
                    visible: root.section === "home"
                }

                SearchView {
                    id: searchView

                    visible: root.section === "search"
                    query: searchField.text
                    queue: queueModel
                    onFocusFieldRequested: searchField.forceActiveFocus()
                    onClearRequested: {
                        searchField.text = "";
                        searchField.forceActiveFocus();
                    }
                }

                LibraryView {
                    visible: root.section === "library"
                    queue: queueModel
                }

                SectionStub {
                    visible: root.section === "playlists"
                    title: qsTr("Playlists")
                    note: qsTr("Playlists and favorites land in S3.")
                }

                SectionStub {
                    visible: root.section === "settings"
                    title: qsTr("Settings")
                    note: qsTr("Library folders, playback, and appearance land in S4.")
                }

            }

        }

    }

}
