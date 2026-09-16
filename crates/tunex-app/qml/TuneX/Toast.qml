import QtQuick
import TuneX

// Toast (DESIGN.md Components): one-line confirmation or error above the
// persistent player — surface-raised, 12px radius, icon + copy. Success and
// error differ by icon and copy, never colour alone. Auto-dismisses after
// 5s; errors without actions dismiss too (actions are not in V1). The copy
// announces politely for screen readers. Replacement policy: latest wins,
// except a visible error is never overwritten by an info toast, so a failed
// add cannot be hidden by the next success.
Rectangle {
    id: root

    property string message: ""
    property bool isError: false
    readonly property bool showing: root.message !== ""

    function show(text, error) {
        const isErr = error === true;
        if (root.showing && root.isError && !isErr)
            return;
        root.message = text;
        root.isError = isErr;
        hideTimer.restart();
    }

    function dismiss() {
        if (!root.showing)
            return;
        root.message = "";
        hideTimer.stop();
        // Keep the item visible for the fade-out below; the guard hides it
        // exactly when the opacity animation lands.
        fadeGuard.restart();
    }

    // Stays up through the exit fade; instant under reduce-motion, when the
    // fade duration (and this guard) collapse to zero.
    visible: root.showing || fadeGuard.running
    opacity: root.showing ? 1 : 0
    width: Math.min(row.implicitWidth + Theme.spaceLg * 2, parent ? parent.width - Theme.spaceXl * 2 : 480)
    height: row.implicitHeight + Theme.spaceMd
    radius: Theme.radiusMd
    color: Theme.surfaceRaised
    border.width: 1
    border.color: root.isError ? Theme.error : Theme.border

    Behavior on opacity {
        NumberAnimation {
            duration: Appearance.duration(Theme.motionHover)
            easing.type: Easing.OutCubic
        }
    }
    Accessible.role: Accessible.StatusBar
    Accessible.name: root.message
    Accessible.ignored: !root.showing

    Timer {
        id: hideTimer

        interval: 5000
        onTriggered: root.dismiss()
    }

    // Holds `visible` true through the exit fade: `visible` above reads
    // `fadeGuard.running`, so expiry hides the item with no imperative
    // assignment (which would destroy that binding).
    Timer {
        id: fadeGuard

        interval: Appearance.duration(Theme.motionHover)
    }

    Row {
        id: row

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        anchors.leftMargin: Theme.spaceLg
        anchors.rightMargin: Theme.spaceLg
        spacing: Theme.spaceSm

        Icon {
            anchors.verticalCenter: parent.verticalCenter
            name: root.isError ? "alert" : "check"
            iconSize: Theme.navIconSize
            stroke: root.isError ? Theme.error : Theme.accent
        }

        Text {
            width: parent.width - Theme.navIconSize - Theme.spaceSm
            anchors.verticalCenter: parent.verticalCenter
            elide: Text.ElideRight
            maximumLineCount: 2
            wrapMode: Text.WordWrap
            text: root.message
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontBodySm
            color: Theme.foreground
        }
    }
}
