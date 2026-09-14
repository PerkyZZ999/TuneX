import QtQuick
import QtQuick.Effects
import TuneX

// PageBanner: short photoreal header for content pages that are not Home.
// Each host supplies a distinct asset; the image has no type — the title
// overlays on a left readability scrim (DESIGN.md page-banner).
Item {
    id: root

    property url source
    property string title: ""

    implicitHeight: Theme.pageBannerHeight
    height: Theme.pageBannerHeight
    Accessible.role: Accessible.Heading
    Accessible.name: root.title

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

    Rectangle {
        anchors.fill: card
        radius: Theme.radiusLg
        color: "transparent"
        border.color: Theme.border
        border.width: 1
        z: 1
        Accessible.ignored: true
    }

    Rectangle {
        id: card

        anchors.fill: parent
        radius: Theme.radiusLg
        color: Theme.surface

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
                source: root.source
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                cache: true
                Accessible.ignored: true
            }

            Rectangle {
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: parent.width * 0.55
                Accessible.ignored: true

                gradient: Gradient {
                    orientation: Gradient.Horizontal

                    GradientStop {
                        position: 0
                        color: Qt.rgba(Theme.background.r, Theme.background.g, Theme.background.b, 0.78)
                    }

                    GradientStop {
                        position: 0.72
                        color: Qt.rgba(Theme.background.r, Theme.background.g, Theme.background.b, 0.22)
                    }

                    GradientStop {
                        position: 1
                        color: Qt.rgba(Theme.background.r, Theme.background.g, Theme.background.b, 0)
                    }
                }
            }
        }

        Text {
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            anchors.leftMargin: Theme.spaceXl
            anchors.right: parent.right
            anchors.rightMargin: Theme.spaceXl
            elide: Text.ElideRight
            text: root.title
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: Theme.fontHeadline
            font.weight: Font.Bold
            color: Theme.foreground
        }
    }
}
