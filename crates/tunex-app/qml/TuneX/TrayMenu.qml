import QtQuick
import QtQuick.Effects
import TuneX

// TrayMenu: the tray icon's right-click menu — Open App plus Quit. A
// separate Tool window like TrayPopup: the SNI host only forwards the
// click coordinates and draws no menu itself. Opaque like its sibling —
// a detached window cannot sample the main canvas for live glass.
// Keyboard: the first row takes focus on open, Tab walks the rows,
// Return/Enter/Space activate, Esc closes. 44px rows, Lucide glyphs,
// DESIGN.md surfaces.
Window {
    id: root

    signal openAppRequested
    signal quitRequested

    function activateOpen() {
        root.dismiss();
        root.openAppRequested();
    }

    function activateQuit() {
        root.dismiss();
        root.quitRequested();
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
        root.show();
        root.raise();
        root.requestActivate();
        openRow.forceActiveFocus();
    }

    function dismiss() {
        leaveTimer.stop();
        root.hide();
    }

    width: 224
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
            if (!root.active && !menuHover.containsMouse)
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

        implicitHeight: body.height + Theme.spaceXs * 2
        width: parent.width
        height: implicitHeight
        radius: Theme.radiusLg
        color: Theme.surfaceRaised
        border.width: 1
        border.color: Theme.border
        Accessible.role: Accessible.Pane
        Accessible.name: qsTr("TuneX tray menu")

        MouseArea {
            id: menuHover

            anchors.fill: parent
            hoverEnabled: true
            onContainsMouseChanged: {
                if (menuHover.containsMouse)
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
            anchors.margins: Theme.spaceXs
            spacing: 0

            FocusScope {
                id: openRow

                width: parent.width
                height: Theme.targetMin
                activeFocusOnTab: true
                Accessible.role: Accessible.Button
                Accessible.name: qsTr("Open App")
                Accessible.onPressAction: root.activateOpen()
                Keys.onReturnPressed: root.activateOpen()
                Keys.onEnterPressed: root.activateOpen()
                Keys.onSpacePressed: root.activateOpen()

                Rectangle {
                    anchors.fill: parent
                    radius: Theme.radiusXs
                    color: rowHoverOpen.containsMouse || openRow.activeFocus ? Theme.hover : "transparent"
                    border.width: openRow.activeFocus ? 2 : 0
                    border.color: Theme.focus

                    Behavior on color {
                        ColorAnimation {
                            duration: Appearance.duration(Theme.motionHover)
                            easing.type: Easing.OutCubic
                        }
                    }
                }

                Row {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.leftMargin: Theme.spaceSm
                    anchors.rightMargin: Theme.spaceSm
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: Theme.spaceSm

                    Icon {
                        anchors.verticalCenter: parent.verticalCenter
                        name: "app-window"
                        iconSize: 20
                        stroke: Theme.foreground
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: qsTr("Open App")
                        textFormat: Text.PlainText
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontBody
                        color: Theme.foreground
                    }
                }

                MouseArea {
                    id: rowHoverOpen

                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.activateOpen()
                }
            }

            FocusScope {
                id: quitRow

                width: parent.width
                height: Theme.targetMin
                activeFocusOnTab: true
                Accessible.role: Accessible.Button
                Accessible.name: qsTr("Quit TuneX")
                Accessible.onPressAction: root.activateQuit()
                Keys.onReturnPressed: root.activateQuit()
                Keys.onEnterPressed: root.activateQuit()
                Keys.onSpacePressed: root.activateQuit()

                Rectangle {
                    anchors.fill: parent
                    radius: Theme.radiusXs
                    color: rowHoverQuit.containsMouse || quitRow.activeFocus ? Theme.hover : "transparent"
                    border.width: quitRow.activeFocus ? 2 : 0
                    border.color: Theme.focus

                    Behavior on color {
                        ColorAnimation {
                            duration: Appearance.duration(Theme.motionHover)
                            easing.type: Easing.OutCubic
                        }
                    }
                }

                Row {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.leftMargin: Theme.spaceSm
                    anchors.rightMargin: Theme.spaceSm
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: Theme.spaceSm

                    Icon {
                        anchors.verticalCenter: parent.verticalCenter
                        name: "log-out"
                        iconSize: 20
                        stroke: Theme.foreground
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: qsTr("Quit")
                        textFormat: Text.PlainText
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontBody
                        color: Theme.foreground
                    }
                }

                MouseArea {
                    id: rowHoverQuit

                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.activateQuit()
                }
            }
        }
    }
}
