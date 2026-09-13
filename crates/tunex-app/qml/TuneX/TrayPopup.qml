import QtQuick
import QtQuick.Effects
import TuneX

// Compact tray card: now-playing plus transport. Hosts (Plasma) draw the
// SNI tooltip on hover; Activate/ContextMenu land here so the same controls
// work without opening the main window. Keyboard: tab through the buttons,
// Esc closes. 44px targets, Lucide glyphs, DESIGN.md surfaces.
Window {
    id: root

    required property QueueModel queue
    property bool mainVisible: true
    property int transportState: 0
    property string titleText: ""
    property string artistText: ""
    property url artUrl
    readonly property string shownTitle: root.titleText !== "" ? root.titleText : qsTr("Unknown Title")
    readonly property string shownArtist: root.artistText !== "" ? root.artistText : qsTr("Unknown Artist")
    readonly property bool hasCurrent: root.titleText !== "" || root.transportState > 0
    readonly property string monogram: {
        const words = root.shownTitle.split(/\s+/).filter(function (word) {
            return word.length > 0;
        });
        const letters = words.slice(0, 2).map(function (word) {
            return word[0].toUpperCase();
        });
        return letters.join("");
    }

    signal showWindowRequested
    signal quitRequested

    function sync() {
        root.transportState = root.queue.playbackState();
        root.titleText = root.queue.currentTitle();
        root.artistText = root.queue.currentArtist();
        root.artUrl = root.queue.currentArtUrl();
    }

    function openAt(x, y) {
        const scr = root.screen;
        const margin = Theme.spaceMd;
        let px = x;
        let py = y;
        if (x <= 0 && y <= 0) {
            px = scr.virtualX + scr.width - root.width - margin;
            py = scr.virtualY + scr.height - root.height - margin;
        } else {
            px = Math.min(Math.max(x, scr.virtualX + margin), scr.virtualX + scr.width - root.width - margin);
            py = y > scr.virtualY + scr.height / 2 ? y - root.height - margin : y + margin;
            py = Math.min(Math.max(py, scr.virtualY + margin), scr.virtualY + scr.height - root.height - margin);
        }
        root.x = px;
        root.y = py;
        root.sync();
        root.show();
        root.raise();
        root.requestActivate();
        playButton.forceActiveFocus();
    }

    function dismiss() {
        leaveTimer.stop();
        root.hide();
    }

    width: Theme.panelWidth
    height: card.implicitHeight
    visible: false
    flags: Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.Tool
    color: "transparent"
    title: qsTr("TuneX")
    onActiveChanged: {
        if (!root.active)
            leaveTimer.restart();
    }

    Timer {
        id: leaveTimer

        interval: 400
        onTriggered: {
            if (!root.active && !cardHover.containsMouse)
                root.dismiss();
        }
    }

    Shortcut {
        sequence: "Esc"
        enabled: root.visible
        onActivated: root.dismiss()
    }

    RectangularShadow {
        anchors.fill: card
        radius: Theme.radiusLg
        blur: Theme.shadowOverlayBlur
        offset.y: Theme.shadowOverlayY
        color: Qt.alpha(Theme.shadow, Theme.shadowOverlayOpacity)
        visible: root.visible
    }

    Rectangle {
        id: card

        implicitHeight: body.height + Theme.spaceMd * 2
        width: parent.width
        height: implicitHeight
        radius: Theme.radiusLg
        color: Theme.surfaceRaised
        border.width: 1
        border.color: Theme.border
        Accessible.role: Accessible.Pane
        Accessible.name: root.hasCurrent ? qsTr("Now playing") + ", " + root.shownTitle + ", " + root.shownArtist : qsTr("Nothing playing")

        MouseArea {
            id: cardHover

            anchors.fill: parent
            hoverEnabled: true
            onContainsMouseChanged: {
                if (cardHover.containsMouse)
                    leaveTimer.stop();
                else
                    leaveTimer.restart();
            }
        }

        Column {
            id: body

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: Theme.spaceMd
            spacing: Theme.spaceMd

            Item {
                width: parent.width
                height: Theme.artThumb

                Artwork {
                    id: art

                    width: Theme.artThumb
                    height: Theme.artThumb
                    source: root.artUrl
                    monogram: root.monogram
                    radius: Theme.radiusMd
                    monogramSize: Theme.fontTitle
                    Accessible.ignored: true
                }

                Column {
                    anchors.left: art.right
                    anchors.leftMargin: Theme.spaceSm
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 0

                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: root.hasCurrent ? root.shownTitle : qsTr("Nothing playing")
                        textFormat: Text.PlainText
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontBody
                        font.weight: Font.DemiBold
                        color: Theme.foreground
                    }

                    Text {
                        width: parent.width
                        elide: Text.ElideRight
                        text: root.hasCurrent ? root.shownArtist : qsTr("Play something from your library")
                        textFormat: Text.PlainText
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontBodySm
                        color: Theme.muted
                    }
                }
            }

            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: Theme.spaceSm

                IconButton {
                    iconName: "skip-back"
                    accessibleName: qsTr("Previous track")
                    onActivated: {
                        root.queue.previousTrack(root.queue.positionMs());
                        root.sync();
                    }
                }

                PlayButton {
                    id: playButton

                    diameter: Theme.targetMin
                    playing: root.transportState === 2
                    onActivated: {
                        root.queue.playPause();
                        root.sync();
                    }
                }

                IconButton {
                    iconName: "skip-forward"
                    accessibleName: qsTr("Next track")
                    onActivated: {
                        root.queue.nextTrack();
                        root.sync();
                    }
                }
            }

            Item {
                width: parent.width
                height: Theme.targetMin

                TextLink {
                    visible: !root.mainVisible
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    text: qsTr("Show TuneX")
                    accessibleName: qsTr("Show TuneX window")
                    onActivated: {
                        root.dismiss();
                        root.showWindowRequested();
                    }
                }

                TextLink {
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    text: qsTr("Quit")
                    accessibleName: qsTr("Quit TuneX")
                    onActivated: root.quitRequested()
                }
            }
        }
    }
}
