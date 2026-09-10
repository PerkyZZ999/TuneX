import QtQuick
import QtQuick.Controls.Basic
import TuneX 1.0

// Application shell (S1 W-003): navigation rail, top bar with view history,
// and per-section content. Home is real (hero + empty state); other sections
// are honest stubs until their slices land. Search bar, settings, and player
// surfaces arrive with S3/S4.
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

            }

            Item {
                width: parent.width
                height: parent.height - topBar.height

                HomeView {
                    visible: root.section === "home"
                }

                SectionStub {
                    visible: root.section === "search"
                    title: qsTr("Search")
                    note: qsTr("Instant library search lands in S3.")
                }

                LibraryView {
                    visible: root.section === "library"
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
