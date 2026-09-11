import QtQuick
import QtQuick.Effects
import TuneX

// Artwork (S6 W-040): the one place a cover is drawn. The generated monogram
// sits underneath at all times; a cover fades in over it when one decodes
// (120ms, the DESIGN.md fade-in budget), and a broken or missing file simply
// never arrives — unknown art stays a placeholder, never a guess (R-NFR-02).
//
// `source` is a cached thumbnail handed over by the lazy resolver, so nothing
// here decodes on demand, and `sourceSize` caps the decode at the size drawn
// rather than the file's own resolution.
Item {
    id: root

    // `file://` URL of a cached thumbnail; empty means "no cover (yet)".
    property url source
    // Initials shown while there is no cover (already uppercased).
    property string monogram: ""
    property int radius: Theme.radiusMd
    // Circular crest (Now Playing) instead of a rounded card.
    property bool circular: false
    property int monogramSize: Theme.fontHeadline
    readonly property bool showingArt: cover.status === Image.Ready

    implicitWidth: Theme.artThumb
    implicitHeight: Theme.artThumb

    Rectangle {
        anchors.fill: parent
        radius: root.circular ? width / 2 : root.radius
        color: Theme.surfaceRaised

        Text {
            anchors.centerIn: parent
            text: root.monogram
            textFormat: Text.PlainText
            font.family: Theme.fontFamily
            font.pixelSize: root.monogramSize
            font.weight: Font.DemiBold
            color: Theme.muted
            Accessible.ignored: true
        }
    }

    // Corner mask: the cover takes the placeholder's shape.
    Rectangle {
        id: coverMask

        anchors.fill: parent
        radius: root.circular ? width / 2 : root.radius
        visible: false
        layer.enabled: true
    }

    Item {
        anchors.fill: parent
        opacity: root.showingArt ? 1 : 0
        visible: opacity > 0
        layer.enabled: true
        layer.effect: MultiEffect {
            maskEnabled: true
            maskSource: coverMask
        }

        Image {
            id: cover

            anchors.fill: parent
            source: root.source
            sourceSize.width: Math.max(1, Math.round(root.width))
            sourceSize.height: Math.max(1, Math.round(root.height))
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            cache: true
            Accessible.ignored: true
        }

        Behavior on opacity {
            NumberAnimation {
                duration: Appearance.duration(Theme.motionHover)
                easing.type: Easing.OutCubic
            }
        }
    }
}
