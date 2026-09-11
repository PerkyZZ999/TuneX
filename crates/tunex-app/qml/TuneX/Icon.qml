import QtQuick

// Lucide-style line icon (2px stroke at 24px, round caps). Names match
// the rail/transport metaphors. Decorative; hosts set Accessible.name.
Item {
    id: root

    property string name: "home"
    property color stroke: Theme.muted
    property int iconSize: 24

    width: root.iconSize
    height: root.iconSize
    implicitWidth: root.iconSize
    implicitHeight: root.iconSize
    Accessible.ignored: true
    onNameChanged: canvas.requestPaint()
    onStrokeChanged: canvas.requestPaint()
    onIconSizeChanged: canvas.requestPaint()
    Component.onCompleted: canvas.requestPaint()

    Canvas {
        id: canvas

        anchors.fill: parent
        antialiasing: true
        onPaint: {
            const ctx = canvas.getContext("2d");
            const s = root.iconSize / 24;
            ctx.reset();
            ctx.strokeStyle = root.stroke;
            ctx.fillStyle = "transparent";
            ctx.lineWidth = 2 * s;
            ctx.lineCap = "round";
            ctx.lineJoin = "round";
            ctx.translate(0.5, 0.5);
            ctx.beginPath();
            switch (root.name) {
            case "home":
                ctx.moveTo(3 * s, 10 * s);
                ctx.lineTo(12 * s, 3 * s);
                ctx.lineTo(21 * s, 10 * s);
                ctx.lineTo(21 * s, 20 * s);
                ctx.quadraticCurveTo(21 * s, 21 * s, 20 * s, 21 * s);
                ctx.lineTo(4 * s, 21 * s);
                ctx.quadraticCurveTo(3 * s, 21 * s, 3 * s, 20 * s);
                ctx.closePath();
                ctx.stroke();
                ctx.beginPath();
                ctx.moveTo(9 * s, 21 * s);
                ctx.lineTo(9 * s, 14 * s);
                ctx.lineTo(15 * s, 14 * s);
                ctx.lineTo(15 * s, 21 * s);
                break;
            case "search":
                ctx.arc(11 * s, 11 * s, 7 * s, 0, Math.PI * 2);
                ctx.stroke();
                ctx.beginPath();
                ctx.moveTo(16.5 * s, 16.5 * s);
                ctx.lineTo(21 * s, 21 * s);
                break;
            case "library":
                ctx.moveTo(4 * s, 19 * s);
                ctx.lineTo(4 * s, 5 * s);
                ctx.stroke();
                ctx.beginPath();
                ctx.moveTo(9 * s, 19 * s);
                ctx.lineTo(9 * s, 5 * s);
                ctx.stroke();
                ctx.beginPath();
                ctx.moveTo(14 * s, 5 * s);
                ctx.lineTo(20 * s, 19 * s);
                ctx.stroke();
                ctx.beginPath();
                ctx.moveTo(20 * s, 5 * s);
                ctx.lineTo(14 * s, 19 * s);
                break;
            case "disc":
                ctx.arc(12 * s, 12 * s, 9 * s, 0, Math.PI * 2);
                ctx.stroke();
                ctx.beginPath();
                ctx.arc(12 * s, 12 * s, 3 * s, 0, Math.PI * 2);
                break;
            case "list":
                ctx.moveTo(8 * s, 6 * s);
                ctx.lineTo(21 * s, 6 * s);
                ctx.moveTo(8 * s, 12 * s);
                ctx.lineTo(21 * s, 12 * s);
                ctx.moveTo(8 * s, 18 * s);
                ctx.lineTo(21 * s, 18 * s);
                ctx.moveTo(3 * s, 6 * s);
                ctx.lineTo(3.1 * s, 6 * s);
                ctx.moveTo(3 * s, 12 * s);
                ctx.lineTo(3.1 * s, 12 * s);
                ctx.moveTo(3 * s, 18 * s);
                ctx.lineTo(3.1 * s, 18 * s);
                break;
            case "users":
                ctx.arc(9 * s, 8 * s, 3 * s, 0, Math.PI * 2);
                ctx.stroke();
                ctx.beginPath();
                ctx.arc(16 * s, 9 * s, 2.5 * s, 0, Math.PI * 2);
                ctx.stroke();
                ctx.beginPath();
                ctx.moveTo(3 * s, 20 * s);
                ctx.quadraticCurveTo(3 * s, 14 * s, 9 * s, 14 * s);
                ctx.quadraticCurveTo(15 * s, 14 * s, 15 * s, 20 * s);
                ctx.stroke();
                ctx.beginPath();
                ctx.moveTo(15 * s, 14.5 * s);
                ctx.quadraticCurveTo(18 * s, 14.5 * s, 21 * s, 18 * s);
                break;
            case "folder":
                ctx.moveTo(3 * s, 7 * s);
                ctx.lineTo(3 * s, 19 * s);
                ctx.quadraticCurveTo(3 * s, 20 * s, 4 * s, 20 * s);
                ctx.lineTo(20 * s, 20 * s);
                ctx.quadraticCurveTo(21 * s, 20 * s, 21 * s, 19 * s);
                ctx.lineTo(21 * s, 9 * s);
                ctx.quadraticCurveTo(21 * s, 8 * s, 20 * s, 8 * s);
                ctx.lineTo(12 * s, 8 * s);
                ctx.lineTo(10 * s, 5 * s);
                ctx.lineTo(4 * s, 5 * s);
                ctx.quadraticCurveTo(3 * s, 5 * s, 3 * s, 6 * s);
                ctx.closePath();
                break;
            case "plus":
                ctx.moveTo(12 * s, 5 * s);
                ctx.lineTo(12 * s, 19 * s);
                ctx.moveTo(5 * s, 12 * s);
                ctx.lineTo(19 * s, 12 * s);
                break;
            case "settings":
                ctx.arc(12 * s, 12 * s, 3 * s, 0, Math.PI * 2);
                ctx.stroke();
                ctx.beginPath();
                ctx.moveTo(12 * s, 3 * s);
                ctx.lineTo(13.5 * s, 6.2 * s);
                ctx.lineTo(17.2 * s, 5.2 * s);
                ctx.lineTo(18.8 * s, 8.8 * s);
                ctx.lineTo(21 * s, 12 * s);
                ctx.lineTo(18.8 * s, 15.2 * s);
                ctx.lineTo(17.2 * s, 18.8 * s);
                ctx.lineTo(13.5 * s, 17.8 * s);
                ctx.lineTo(12 * s, 21 * s);
                ctx.lineTo(10.5 * s, 17.8 * s);
                ctx.lineTo(6.8 * s, 18.8 * s);
                ctx.lineTo(5.2 * s, 15.2 * s);
                ctx.lineTo(3 * s, 12 * s);
                ctx.lineTo(5.2 * s, 8.8 * s);
                ctx.lineTo(6.8 * s, 5.2 * s);
                ctx.lineTo(10.5 * s, 6.2 * s);
                ctx.closePath();
                break;
            case "chevron-left":
                ctx.moveTo(15 * s, 6 * s);
                ctx.lineTo(9 * s, 12 * s);
                ctx.lineTo(15 * s, 18 * s);
                break;
            case "chevron-right":
                ctx.moveTo(9 * s, 6 * s);
                ctx.lineTo(15 * s, 12 * s);
                ctx.lineTo(9 * s, 18 * s);
                break;
            case "play":
                ctx.moveTo(8 * s, 5 * s);
                ctx.lineTo(19 * s, 12 * s);
                ctx.lineTo(8 * s, 19 * s);
                ctx.closePath();
                break;
            case "pause":
                ctx.moveTo(8 * s, 5 * s);
                ctx.lineTo(8 * s, 19 * s);
                ctx.moveTo(16 * s, 5 * s);
                ctx.lineTo(16 * s, 19 * s);
                break;
            case "skip-back":
                ctx.moveTo(19 * s, 5 * s);
                ctx.lineTo(9 * s, 12 * s);
                ctx.lineTo(19 * s, 19 * s);
                ctx.closePath();
                ctx.stroke();
                ctx.beginPath();
                ctx.moveTo(6 * s, 5 * s);
                ctx.lineTo(6 * s, 19 * s);
                break;
            case "skip-forward":
                ctx.moveTo(5 * s, 5 * s);
                ctx.lineTo(15 * s, 12 * s);
                ctx.lineTo(5 * s, 19 * s);
                ctx.closePath();
                ctx.stroke();
                ctx.beginPath();
                ctx.moveTo(18 * s, 5 * s);
                ctx.lineTo(18 * s, 19 * s);
                break;
            case "shuffle":
                ctx.moveTo(3 * s, 7 * s);
                ctx.lineTo(8 * s, 7 * s);
                ctx.lineTo(16 * s, 17 * s);
                ctx.lineTo(21 * s, 17 * s);
                ctx.moveTo(18 * s, 14 * s);
                ctx.lineTo(21 * s, 17 * s);
                ctx.lineTo(18 * s, 20 * s);
                ctx.moveTo(3 * s, 17 * s);
                ctx.lineTo(8 * s, 17 * s);
                ctx.lineTo(11 * s, 13.5 * s);
                ctx.moveTo(13 * s, 10.5 * s);
                ctx.lineTo(16 * s, 7 * s);
                ctx.lineTo(21 * s, 7 * s);
                ctx.moveTo(18 * s, 4 * s);
                ctx.lineTo(21 * s, 7 * s);
                ctx.lineTo(18 * s, 10 * s);
                break;
            case "repeat":
                ctx.moveTo(17 * s, 2 * s);
                ctx.lineTo(21 * s, 6 * s);
                ctx.lineTo(17 * s, 10 * s);
                ctx.moveTo(21 * s, 6 * s);
                ctx.lineTo(7 * s, 6 * s);
                ctx.quadraticCurveTo(4 * s, 6 * s, 4 * s, 9 * s);
                ctx.lineTo(4 * s, 12 * s);
                ctx.moveTo(7 * s, 22 * s);
                ctx.lineTo(3 * s, 18 * s);
                ctx.lineTo(7 * s, 14 * s);
                ctx.moveTo(3 * s, 18 * s);
                ctx.lineTo(17 * s, 18 * s);
                ctx.quadraticCurveTo(20 * s, 18 * s, 20 * s, 15 * s);
                ctx.lineTo(20 * s, 12 * s);
                break;
            case "volume":
                ctx.moveTo(4 * s, 9 * s);
                ctx.lineTo(8 * s, 9 * s);
                ctx.lineTo(13 * s, 5 * s);
                ctx.lineTo(13 * s, 19 * s);
                ctx.lineTo(8 * s, 15 * s);
                ctx.lineTo(4 * s, 15 * s);
                ctx.closePath();
                ctx.stroke();
                ctx.beginPath();
                ctx.arc(13 * s, 12 * s, 5 * s, -0.7, 0.7);
                break;
            case "queue":
                ctx.moveTo(8 * s, 6 * s);
                ctx.lineTo(21 * s, 6 * s);
                ctx.moveTo(8 * s, 12 * s);
                ctx.lineTo(21 * s, 12 * s);
                ctx.moveTo(8 * s, 18 * s);
                ctx.lineTo(16 * s, 18 * s);
                ctx.moveTo(3 * s, 6 * s);
                ctx.lineTo(5 * s, 8 * s);
                ctx.lineTo(3 * s, 10 * s);
                break;
            default:
                ctx.moveTo(5 * s, 5 * s);
                ctx.lineTo(19 * s, 19 * s);
                ctx.moveTo(19 * s, 5 * s);
                ctx.lineTo(5 * s, 19 * s);
                break;
            }
            ctx.stroke();
        }
    }
}
