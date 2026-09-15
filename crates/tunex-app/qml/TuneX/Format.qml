pragma Singleton
import QtQuick
import TuneX

// Format: shared text shaping, sibling to the Theme/Appearance singletons.
// One copy of the monogram, duration, and singular/plural rules behind a
// three-function interface — every card, row, and player surface shapes
// text through here, so a fix lands everywhere at once.
QtObject {
    // First letters of the first two words, uppercase ("Blue Hour" -> "BH").
    function monogram(text: string): string {
        const words = text.split(/\s+/).filter(function (word) {
            return word.length > 0;
        });
        const letters = words.slice(0, 2).map(function (word) {
            return word[0].toUpperCase();
        });
        return letters.join("");
    }

    // h:mm:ss past the hour, m:ss below it. Numbers need no translation.
    function duration(ms: int): string {
        const total = Math.max(0, Math.floor(ms / 1000));
        const hours = Math.floor(total / 3600);
        const minutes = Math.floor(total / 60) % 60;
        const seconds = String(total % 60).padStart(2, "0");
        if (hours > 0)
            return hours + ":" + String(minutes).padStart(2, "0") + ":" + seconds;
        return minutes + ":" + seconds;
    }

    // Singular/plural pair: pass both qsTr forms, the count picks
    // ("1 song" / "%1 songs"). `%n` would render literally — no translation
    // catalogs ship in V1, so translators get the explicit pair.
    function plural(n: int, one: string, many: string): string {
        return n === 1 ? one : many.arg(n);
    }

    // Empty text falls back to the label ("Unknown Artist"). Centralizes
    // the wording so a dozen call sites cannot drift apart.
    function fallback(text: string, label: string): string {
        return text !== "" ? text : label;
    }

    // Durations with no length read as an em dash, never 0:00.
    function durationOrDash(ms: int): string {
        return ms > 0 ? duration(ms) : "—";
    }

    // Transport repeat vocabulary (queue rail and overlay share it; the
    // Settings value reads "All tracks" instead and stays put).
    function repeatLabel(mode: int): string {
        if (mode === 1)
            return qsTr("Repeat: All");

        if (mode === 2)
            return qsTr("Repeat: One");

        return qsTr("Repeat: Off");
    }

    // Artwork grid cell for an available width: whole cards fill the row
    // instead of clipping one mid-card at the panel edge.
    function gridCell(available: int): int {
        return Math.max(Theme.gridMin, Math.floor(available / Math.max(1, Math.floor(available / Theme.gridTarget))));
    }

    // Transport icon selection (repeat + mute appear on several
    // surfaces; the glyph choice is one rule, not five copies).
    function repeatIcon(mode: int): string {
        return mode === 2 ? "repeat-1" : "repeat";
    }

    function muteIcon(muted: bool): string {
        return muted ? "volume-x" : "volume";
    }

    // Meta-line join: dot-separated parts, empties dropped
    // ("Ash Pilot • 2024 • 3 songs").
    function meta(parts) {
        return parts.filter(function (part) {
            return part !== "";
        }).join(" • ");
    }
}
