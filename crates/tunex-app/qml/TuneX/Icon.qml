import QtQuick
import QtQuick.Shapes
import TuneX

// Icon (S6 W-038): one Lucide line family — 2px stroke at 24px, round caps
// and joins, scaled for 16/20px. Drawn on the GPU from upstream path data
// (a stroke colour change is a uniform, not a repaint). Each subpath starts
// with an absolute `M`: Lucide ships one <path> per stroke, and a leading
// relative `m` would shift once concatenated. Decorative: hosts set
// Accessible.name. `filled` paints the same geometry solid (primary
// transport glyph only, as in the mockup).
// Path data from Lucide (https://lucide.dev): ISC License, Copyright (c)
// Lucide Icons and Contributors; portions derived from Feather: MIT
// License, Copyright (c) 2013-present Cole Bemis. Full notices:
// LICENSES/Lucide.txt.
Shape {
    id: root

    property string name: "home"
    property color stroke: Theme.muted
    property int iconSize: 24
    property bool filled: false

    function pathFor(key: string): string {
        switch (key) {
        case "home":
            return "M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8 M3 10a2 2 0 0 1 .709-1.528l7-6a2 2 0 0 1 2.582 0l7 6A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z";
        case "search":
            return "M21 21l-4.34-4.34 M3 11a8 8 0 1 0 16 0a8 8 0 1 0-16 0";
        case "users":
            return "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2 M16 3.128a4 4 0 0 1 0 7.744 M22 21v-2a4 4 0 0 0-3-3.87 M5 7a4 4 0 1 0 8 0a4 4 0 1 0-8 0";
        case "disc":
            return "M2 12a10 10 0 1 0 20 0a10 10 0 1 0-20 0 M6 12c0-1.7.7-3.2 1.8-4.2 M10 12a2 2 0 1 0 4 0a2 2 0 1 0-4 0 M18 12c0 1.7-.7 3.2-1.8 4.2";
        case "music":
            return "M9 18V5l12-2v13 M3 18a3 3 0 1 0 6 0a3 3 0 1 0-6 0 M15 16a3 3 0 1 0 6 0a3 3 0 1 0-6 0";
        case "list":
            return "M3 5h.01 M3 12h.01 M3 19h.01 M8 5h13 M8 12h13 M8 19h13";
        case "list-music":
            return "M16 5H3 M11 12H3 M11 19H3 M21 16V5 M15 16a3 3 0 1 0 6 0a3 3 0 1 0-6 0";
        case "folder":
            return "M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z";
        case "folder-plus":
            return "M12 10v6 M9 13h6 M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z";
        case "plus":
            return "M5 12h14 M12 5v14";
        case "settings":
            return "M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915 M9 12a3 3 0 1 0 6 0a3 3 0 1 0-6 0";
        case "chevron-left":
            return "M15 18l-6-6 6-6";
        case "chevron-right":
            return "M9 18l6-6-6-6";
        case "play":
            return "M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z";
        case "pause":
            return "M15 3h3a1 1 0 0 1 1 1v16a1 1 0 0 1-1 1h-3a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z M6 3h3a1 1 0 0 1 1 1v16a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z";
        case "skip-back":
            return "M17.971 4.285A2 2 0 0 1 21 6v12a2 2 0 0 1-3.029 1.715l-9.997-5.998a2 2 0 0 1-.003-3.432z M3 20V4";
        case "skip-forward":
            return "M21 4v16 M6.029 4.285A2 2 0 0 0 3 6v12a2 2 0 0 0 3.029 1.715l9.997-5.998a2 2 0 0 0 .003-3.432z";
        case "shuffle":
            return "M18 14l4 4-4 4 M18 2l4 4-4 4 M2 18h1.973a4 4 0 0 0 3.3-1.7l5.454-8.6a4 4 0 0 1 3.3-1.7H22 M2 6h1.972a4 4 0 0 1 3.6 2.2 M22 18h-6.041a4 4 0 0 1-3.3-1.8l-.359-.45";
        case "repeat":
            return "M17 2l4 4-4 4 M3 11v-1a4 4 0 0 1 4-4h14 M7 22l-4-4 4-4 M21 13v1a4 4 0 0 1-4 4H3";
        case "repeat-1":
            return "M17 2l4 4-4 4 M3 11v-1a4 4 0 0 1 4-4h14 M7 22l-4-4 4-4 M21 13v1a4 4 0 0 1-4 4H3 M11 10h1v4";
        case "volume":
            return "M11 4.702a.705.705 0 0 0-1.203-.498L6.413 7.587A1.4 1.4 0 0 1 5.416 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.416a1.4 1.4 0 0 1 .997.413l3.383 3.384A.705.705 0 0 0 11 19.298z M16 9a5 5 0 0 1 0 6 M19.364 18.364a9 9 0 0 0 0-12.728";
        case "volume-x":
            return "M11 4.702a.7.7 0 0 0-1.203-.498L6.413 7.587A1.4 1.4 0 0 1 5.416 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.416a1.4 1.4 0 0 1 .997.413l3.383 3.384A.7.7 0 0 0 11 19.298z M16.5 14.5l5-5 M16.5 9.5l5 5";
        case "ellipsis":
            return "M11 12a1 1 0 1 0 2 0a1 1 0 1 0-2 0 M18 12a1 1 0 1 0 2 0a1 1 0 1 0-2 0 M4 12a1 1 0 1 0 2 0a1 1 0 1 0-2 0";
        case "refresh":
            return "M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8 M21 3v5h-5 M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16 M8 16H3v5";
        case "alert":
            return "M2 12a10 10 0 1 0 20 0a10 10 0 1 0-20 0 M12 8L12 12 M12 16L12.01 16";
        case "warning":
            return "M21.73 18l-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3 M12 9v4 M12 17h.01";
        case "check":
            return "M20 6L9 17l-5-5";
        case "keyboard":
            return "M2 8a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2Z M6 10h.01 M10 10h.01 M14 10h.01 M18 10h.01 M6 14h.01 M18 14h.01 M10 14h4";
        case "sliders-horizontal":
            return "M16 6a2 2 0 1 0-4 0a2 2 0 1 0 4 0 M4 6h8 M16 6h4 M10 12a2 2 0 1 0-4 0a2 2 0 1 0 4 0 M4 12h2 M10 12h10 M18 18a2 2 0 1 0-4 0a2 2 0 1 0 4 0 M4 18h10 M18 18h2";
        case "blend":
            return "M2 9a7 7 0 1 0 14 0a7 7 0 1 0-14 0 M8 15a7 7 0 1 0 14 0a7 7 0 1 0-14 0";
        case "log-out":
            return "M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4 M16 17l5-5-5-5 M21 12H9";
        case "app-window":
            return "M5 5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2Z M5 10h14";
        default:
            // "x": close, and the visible fallback for unknown names.
            return "M18 6L6 18 M6 6l12 12";
        }
    }

    width: root.iconSize
    height: root.iconSize
    implicitWidth: root.iconSize
    implicitHeight: root.iconSize
    preferredRendererType: Shape.CurveRenderer
    Accessible.ignored: true

    ShapePath {
        strokeColor: root.stroke
        strokeWidth: 2
        fillColor: root.filled ? root.stroke : "transparent"
        capStyle: ShapePath.RoundCap
        joinStyle: ShapePath.RoundJoin
        scale: Qt.size(root.iconSize / 24, root.iconSize / 24)

        PathSvg {
            path: root.pathFor(root.name)
        }
    }
}
