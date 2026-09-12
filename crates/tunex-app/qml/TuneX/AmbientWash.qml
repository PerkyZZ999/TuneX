import QtQuick
import QtQuick.Effects
import TuneX

// AmbientWash (S6 W-045): the room the translucent chrome looks into.
//
// Tinted-translucent chrome over a flat canvas reveals a flat canvas, which
// reads as a lighter grey panel rather than glass. What sells the effect is
// having something varied behind it, so the playing track's own artwork is
// blurred to abstraction and held far down under the charcoal canvas. The
// room takes the record's colour; the record stays the only picture.
//
// Cost is a blur of one small image on track change, not per frame: the
// source is the 512px cached thumbnail, and nothing here animates except the
// crossfade between two tracks. With transparency reduced it renders nothing
// at all and the shell falls back to flat charcoal.
Item {
    id: root

    // Cached cover of the playing track; empty means "no wash".
    property url source
    readonly property bool active: !Appearance.reduceTransparency && art.status === Image.Ready

    Image {
        id: art

        anchors.fill: parent
        source: root.source
        // One small decode: the wash is unrecognizable by design, so the
        // thumbnail is already more resolution than it needs.
        sourceSize.width: 256
        sourceSize.height: 256
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
        cache: true
        visible: false
        Accessible.ignored: true
    }

    MultiEffect {
        anchors.fill: parent
        source: art
        autoPaddingEnabled: false
        blurEnabled: true
        blur: 1
        blurMax: Theme.ambientBlur
        saturation: 0.2
        opacity: root.active ? Theme.ambientOpacity : 0
        visible: opacity > 0

        Behavior on opacity {
            NumberAnimation {
                duration: Appearance.duration(Theme.artCrossfadeMs)
                easing.type: Easing.OutCubic
            }
        }
    }

    // Vignette: the wash is strongest behind the chrome at the edges and
    // fades out under the content, so artwork on screen never competes with
    // a coloured haze behind it.
    Rectangle {
        anchors.fill: parent

        gradient: Gradient {
            orientation: Gradient.Vertical

            GradientStop {
                position: 0
                color: Qt.rgba(Theme.background.r, Theme.background.g, Theme.background.b, 0.35)
            }

            GradientStop {
                position: 0.55
                color: Qt.rgba(Theme.background.r, Theme.background.g, Theme.background.b, 0.86)
            }

            GradientStop {
                position: 1
                color: Qt.rgba(Theme.background.r, Theme.background.g, Theme.background.b, 0.97)
            }
        }
    }
}
