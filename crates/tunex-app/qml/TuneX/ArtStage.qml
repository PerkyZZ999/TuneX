import QtQuick
import TuneX

// Artwork well for QueuePanel and the Now Playing overlay (S16 W-072).
// Lyrics still replace this on the overlay. Reduce-motion forces Artwork.
Item {
    id: root

    required property QueueModel queue
    property url source
    property string monogram: ""
    property int radius: Theme.radiusMd
    property bool circular: false
    property int monogramSize: Theme.fontHeadline
    property int mode: 0
    property string spectrumCsv: ""
    property string waveformCsv: ""
    readonly property int shownMode: Appearance.reduceMotion ? 0 : root.mode
    readonly property int bandCount: 32

    function levelAt(index) {
        const parts = root.spectrumCsv.split(",");
        if (index < 0 || index >= parts.length)
            return 0.04;
        const level = Number(parts[index]);
        if (isNaN(level))
            return 0.04;
        return Math.max(0.04, Math.min(1, level));
    }

    implicitWidth: Theme.artThumb
    implicitHeight: Theme.artThumb
    Accessible.role: Accessible.Graphic
    Accessible.name: qsTr("Now Playing artwork")

    Artwork {
        visible: root.shownMode === 0
        anchors.fill: parent
        source: root.source
        monogram: root.monogram
        radius: root.radius
        circular: root.circular
        monogramSize: root.monogramSize
    }

    Rectangle {
        visible: root.shownMode !== 0
        anchors.fill: parent
        radius: root.circular ? width / 2 : root.radius
        color: Theme.surfaceRaised
        clip: true

        Row {
            id: spectrumBars

            visible: root.shownMode === 1 || root.shownMode === 3
            anchors.fill: parent
            anchors.margins: Theme.spaceSm
            spacing: 2

            Repeater {
                model: root.bandCount

                Rectangle {
                    required property int index

                    width: Math.max(2, (spectrumBars.width - (root.bandCount - 1) * spectrumBars.spacing) / root.bandCount)
                    height: spectrumBars.height * (root.shownMode === 3 ? Math.min(1, root.levelAt(index) * 1.4) : root.levelAt(index))
                    anchors.bottom: spectrumBars.bottom
                    radius: 1
                    color: root.shownMode === 3 ? Theme.accentSecondary : Theme.accent
                    opacity: root.shownMode === 3 ? 0.85 : 1
                }
            }
        }

        Canvas {
            id: wave

            visible: root.shownMode === 2
            anchors.fill: parent
            onPaint: {
                const ctx = getContext("2d");
                ctx.reset();
                const samples = root.waveformCsv.split(",");
                if (samples.length < 2)
                    return;
                ctx.strokeStyle = Theme.accent;
                ctx.lineWidth = 2;
                ctx.beginPath();
                for (let i = 0; i < samples.length; i++) {
                    const x = (i / (samples.length - 1)) * wave.width;
                    const y = wave.height * 0.5 - Number(samples[i]) * wave.height * 0.45;
                    if (i === 0)
                        ctx.moveTo(x, y);
                    else
                        ctx.lineTo(x, y);
                }
                ctx.stroke();
            }
        }
    }

    Connections {
        target: root
        function onWaveformCsvChanged() {
            // The CSV ticks at the frame cap in every mode; repainting an
            // invisible Canvas is pure main-thread cost.
            if (root.shownMode === 2)
                wave.requestPaint();
        }
        function onShownModeChanged() {
            if (root.shownMode === 2)
                wave.requestPaint();
        }
    }

    TapHandler {
        acceptedButtons: Qt.RightButton
        onTapped: viewMenu.popup()
    }

    GlassMenu {
        id: viewMenu

        title: qsTr("View")

        GlassMenuItem {
            text: qsTr("Artwork")
            checkable: true
            checked: root.mode === 0
            onTriggered: root.queue.setVisualizerMode(0)
        }

        GlassMenuItem {
            text: qsTr("Spectrum")
            checkable: true
            checked: root.mode === 1
            onTriggered: root.queue.setVisualizerMode(1)
        }

        GlassMenuItem {
            text: qsTr("Waveform")
            checkable: true
            checked: root.mode === 2
            onTriggered: root.queue.setVisualizerMode(2)
        }

        GlassMenuItem {
            text: qsTr("Visualizer")
            checkable: true
            checked: root.mode === 3
            onTriggered: root.queue.setVisualizerMode(3)
        }
    }
}
