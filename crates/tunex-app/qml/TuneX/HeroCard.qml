import QtQuick
import QtQuick.Effects
import TuneX

// Home greeting hero (DESIGN_BRIEF HeroCard). Photoreal night backdrop
// from assets/hero-charcoal.png; chrome recedes. Time-of-day line, one
// primary action, TuneX wordmark on the trailing edge (mockup chrome).
// Elevation: the hero step `0 4px 24px rgba(0,0,0,0.35)`; the photo and its
// readability scrim are masked to the lg radius as one static layer (item
// clipping is rectangular), text and the action stay outside the layer.
Item {
    id: root

    property bool libraryEmpty: true
    property bool canContinue: false
    property string statusLine: ""
    readonly property string greeting: {
        const hour = new Date().getHours();
        if (hour < 12)
            return qsTr("Good morning");

        if (hour < 18)
            return qsTr("Good afternoon");

        return qsTr("Good evening");
    }
    readonly property string actionLabel: {
        if (root.libraryEmpty)
            return qsTr("Add music folder");

        if (root.canContinue)
            return qsTr("Continue");

        return qsTr("Play Something");
    }

    signal playRequested
    signal addFolderRequested
    signal browseRequested

    implicitHeight: Theme.heroHeight
    height: Theme.heroHeight

    RectangularShadow {
        anchors.fill: card
        radius: card.radius
        offset.y: Theme.shadowHeroY
        blur: Theme.shadowHeroBlur
        color: Qt.alpha(Theme.shadow, Theme.shadowHeroOpacity)
    }

    Rectangle {
        id: cardMask

        anchors.fill: card
        radius: Theme.radiusLg
        visible: false
        layer.enabled: true
    }

    // Hairline edge over the photo, as the mockup draws the hero: the card
    // reads as a contained surface rather than a bleed of the background
    // image. Sits above the artwork so the mask never clips it away.
    Rectangle {
        anchors.fill: card
        radius: Theme.radiusLg
        color: "transparent"
        border.color: Theme.border
        border.width: 1
        z: 1
    }

    Rectangle {
        id: card

        anchors.fill: parent
        radius: Theme.radiusLg
        color: Theme.chrome

        Item {
            id: photo

            anchors.fill: parent
            layer.enabled: true
            layer.effect: MultiEffect {
                maskEnabled: true
                maskSource: cardMask
            }

            Image {
                anchors.fill: parent
                source: "qrc:/qt/qml/TuneX/hero-charcoal.png"
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                cache: true
                Accessible.ignored: true
            }

            // Left readability scrim — never raw art behind text (DESIGN.md).
            Rectangle {
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: parent.width * 0.58
                Accessible.ignored: true

                gradient: Gradient {
                    orientation: Gradient.Horizontal

                    GradientStop {
                        position: 0
                        color: Qt.rgba(Theme.background.r, Theme.background.g, Theme.background.b, 0.82)
                    }

                    GradientStop {
                        position: 0.7
                        color: Qt.rgba(Theme.background.r, Theme.background.g, Theme.background.b, 0.28)
                    }

                    GradientStop {
                        position: 1
                        color: Qt.rgba(Theme.background.r, Theme.background.g, Theme.background.b, 0)
                    }
                }
            }
        }

        Column {
            visible: parent.width >= 640
            anchors.right: parent.right
            anchors.rightMargin: Theme.spaceXl
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.spaceLg
            spacing: Theme.spaceXs

            Text {
                anchors.right: parent.right
                text: qsTr("TuneX")
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontHeadline
                font.weight: Font.Bold
                color: Theme.foreground
                opacity: 0.92
            }

            Text {
                anchors.right: parent.right
                text: qsTr("MUSIC LIVES HERE")
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontCaption
                font.weight: Font.DemiBold
                font.letterSpacing: 1.6
                color: Theme.accent
            }
        }

        Column {
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            anchors.leftMargin: Theme.spaceXl
            anchors.right: parent.right
            anchors.rightMargin: Theme.spaceXl
            spacing: Theme.spaceSm

            Text {
                text: root.greeting.toUpperCase()
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontCaption
                font.weight: Font.DemiBold
                font.letterSpacing: 1.2
                color: Theme.accent
            }

            Text {
                width: Math.min(parent.width * 0.62, 520)
                wrapMode: Text.WordWrap
                text: root.libraryEmpty ? qsTr("Your music room is empty.") : qsTr("Your music lives here.")
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontDisplay
                font.weight: Font.Bold
                color: Theme.foreground
            }

            Text {
                width: Math.min(parent.width * 0.55, 480)
                wrapMode: Text.WordWrap
                text: root.libraryEmpty ? qsTr("Add a folder of files you own. Scanning stays in the background.") : qsTr("Artwork leads. Chrome recedes. Nothing here needs the network.")
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontBody
                color: Theme.muted
            }

            // Two actions, as the mockup pairs them — but only one is
            // primary. With no library yet there is exactly one thing to do,
            // so the secondary stays out of an empty state's way.
            Row {
                spacing: Theme.spaceSm

                PrimaryButton {
                    glyph: root.libraryEmpty ? "folder" : "play"
                    text: root.actionLabel
                    Accessible.name: root.actionLabel
                    onClicked: {
                        if (root.libraryEmpty)
                            root.addFolderRequested();
                        else
                            root.playRequested();
                    }
                }

                PrimaryButton {
                    visible: !root.libraryEmpty
                    primary: false
                    glyph: "disc"
                    text: qsTr("Browse library")
                    Accessible.name: qsTr("Browse library")
                    onClicked: root.browseRequested()
                }
            }

            Text {
                visible: root.statusLine !== ""
                text: root.statusLine
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontCaption
                color: Theme.muted
            }
        }
    }
}
