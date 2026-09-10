#!/usr/bin/env bash
# TuneX Definition-of-Done rehearsal draft (W-009).
#
# Exercises everything verifiable headlessly today: the Rust gate, the CMake
# build, QML lint/format, and the MPRIS control round-trip against a real
# session bus. Interactive GUI flows (scan-while-browsing, artwork, offline
# run) stay manual until the S5 gate — see docs/project/VALIDATION.md.
#
# Usage: ./scripts/dod-demo.sh
# Exit status: 0 when every step passes, 1 otherwise (all steps still run).
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PASS=0
FAIL=0

step() {
    local name="$1"
    shift
    echo "--- $name"
    if "$@" >"/tmp/tunex-dod-$RANDOM.log" 2>&1; then
        echo "ok: $name"
        PASS=$((PASS + 1))
    else
        echo "FAIL: $name (see log above)"
        FAIL=$((FAIL + 1))
    fi
}

step "rust gate (fmt, clippy, tests)" rust-tc quick
step "cmake configure" cmake -B build -S . -G Ninja -DCMAKE_BUILD_TYPE=RelWithDebInfo
step "cmake build" cmake --build build
step "qmllint" qmllint crates/tunex-app/qml/TuneX/App.qml crates/tunex-app/qml/TuneX/Theme.qml crates/tunex-app/qml/TuneX/HomeView.qml crates/tunex-app/qml/TuneX/SectionStub.qml crates/tunex-app/qml/TuneX/NavItem.qml

mpris_smoke() {
    local service="org.mpris.MediaPlayer2.tunex"
    local path="/org/mpris/MediaPlayer2"
    QT_QPA_PLATFORM=offscreen ./build/tunex &
    local pid=$!
    # Wait for the bus name (up to 10 s), then exercise it.
    local waited=0
    while ! busctl --user list --no-legend 2>/dev/null | grep -q "$service"; do
        sleep 0.5
        waited=$((waited + 1))
        if [ "$waited" -ge 20 ]; then
            kill "$pid" 2>/dev/null
            return 1
        fi
    done
    [ "$(qdbus6 "$service" "$path" org.mpris.MediaPlayer2.Identity)" = "TuneX" ] || { kill "$pid"; return 1; }
    qdbus6 "$service" "$path" org.mpris.MediaPlayer2.Player.PlayPause >/dev/null || { kill "$pid"; return 1; }
    sleep 1
    [ "$(qdbus6 "$service" "$path" org.freedesktop.DBus.Properties.Get org.mpris.MediaPlayer2.Player PlaybackStatus)" = "Playing" ] || { kill "$pid"; return 1; }
    kill "$pid" 2>/dev/null
    return 0
}

echo "--- mpris smoke (bus name, identity, play toggle)"
if mpris_smoke >"/tmp/tunex-dod-mpris.log" 2>&1; then
    echo "ok: mpris smoke"
    PASS=$((PASS + 1))
else
    echo "FAIL: mpris smoke (see /tmp/tunex-dod-mpris.log)"
    FAIL=$((FAIL + 1))
fi
# Belt and braces: never leave a stray player behind.
pkill -f "build/tunex$" 2>/dev/null || true

echo
echo "DoD rehearsal: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
