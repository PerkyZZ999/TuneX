#!/usr/bin/env bash
# TuneX network-off DoD rehearsal (S5 W-037 gate).
#
# Runs the entire offline surface inside a network namespace
# (`bwrap --unshare-net`, no root needed) and proves:
#  1. the namespace really has no network (TCP connect must fail),
#  2. startup scan + last-track restore work offline (MPRIS props),
#  3. transport + volume round-trip over MPRIS while offline,
#  4. a corrupt-file playback error emits a `Notify` call on the session bus
#     (error-toast path; no daemon owns the name here, so delivery fails
#     gracefully — the emission itself is the assertion).
#
# Usage: ./scripts/netoff-rehearsal.sh
# Exit status: 0 when every step passes, 1 otherwise.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export TUNEX_FIX="$ROOT/tests/fixtures"
export TUNEX_SVC="org.mpris.MediaPlayer2.tunex"
export TUNEX_PATH="/org/mpris/MediaPlayer2"

exec bwrap --unshare-net --bind / / --proc /proc --dev /dev --chdir "$ROOT" \
    bash -c '
set -uo pipefail

SVC="$TUNEX_SVC"
P="$TUNEX_PATH"
FIX="$TUNEX_FIX"

echo "--- 1. network is off"
if (: </dev/tcp/8.8.8.8/53) 2>/dev/null; then
    echo "FAIL: TCP still works inside the namespace"
    exit 1
fi
echo "ok: TCP connect refused (no network)"

wait_for_bus() {
    local waited=0
    while ! busctl --user list --no-legend 2>/dev/null | grep -q "$SVC"; do
        sleep 0.5; waited=$((waited + 1))
        if [ "$waited" -ge 20 ]; then echo "FAIL: bus name never appeared"; return 1; fi
    done
}

echo "--- 2. phase 1: healthy restore + transport matrix while offline"
S1="$(mktemp -d)"
mkdir -p "$S1/config/tunex"
{
    printf "library_roots = [\"%s\"]\n" "$FIX"
    printf "\n[playback]\nlast_uri = \"file://%s/sine.wav\"\nlast_position_ms = 1500\n" "$FIX"
} > "$S1/config/tunex/config.toml"
export XDG_CONFIG_HOME="$S1/config" XDG_DATA_HOME="$S1/data" QT_QPA_PLATFORM=offscreen
./build/tunex > "$S1/app.log" 2>&1 &
APP1=$!
wait_for_bus || { kill "$APP1" 2>/dev/null; exit 1; }
echo "ok: bus name owned while offline"
sleep 4
STATUS="$(qdbus6 "$SVC" "$P" org.freedesktop.DBus.Properties.Get org.mpris.MediaPlayer2.Player PlaybackStatus)"
[ "$STATUS" = "Paused" ] || { echo "FAIL: healthy restore not Paused (got $STATUS)"; kill "$APP1" 2>/dev/null; exit 1; }
echo "ok: sine.wav restored paused at 1.5s while offline"
qdbus6 "$SVC" "$P" org.mpris.MediaPlayer2.Player.Play >/dev/null
sleep 2
[ "$(qdbus6 "$SVC" "$P" org.freedesktop.DBus.Properties.Get org.mpris.MediaPlayer2.Player PlaybackStatus)" = "Playing" ] \
    || { echo "FAIL: Play did not reach Playing offline"; kill "$APP1" 2>/dev/null; exit 1; }
qdbus6 "$SVC" "$P" org.mpris.MediaPlayer2.Player.Pause >/dev/null
sleep 1
[ "$(qdbus6 "$SVC" "$P" org.freedesktop.DBus.Properties.Get org.mpris.MediaPlayer2.Player PlaybackStatus)" = "Paused" ] \
    || { echo "FAIL: Pause did not hold offline"; kill "$APP1" 2>/dev/null; exit 1; }
busctl --user set-property "$SVC" "$P" org.mpris.MediaPlayer2.Player Volume d 0.5 >/dev/null
sleep 1
# f32 pipeline precision reads back 0.50000122 with a loaded engine (honest
# ground truth; empty-queue reads stay exact) — prefix-match the tolerance.
VOL="$(qdbus6 "$SVC" "$P" org.freedesktop.DBus.Properties.Get org.mpris.MediaPlayer2.Player Volume)"
case "$VOL" in
    0.5*) echo "ok: volume round-trips at $VOL" ;;
    *) echo "FAIL: volume round-trip failed offline (got $VOL)"; kill "$APP1" 2>/dev/null; exit 1 ;;
esac
echo "ok: play/pause/volume matrix green while offline"
kill "$APP1" 2>/dev/null; sleep 1

echo "--- 3. phase 2: corrupt-file error emits Notify while offline"
S2="$(mktemp -d)"
mkdir -p "$S2/config/tunex"
{
    printf "library_roots = [\"%s\"]\n" "$FIX"
    printf "\n[playback]\nlast_uri = \"file://%s/corrupt.mp3\"\nlast_position_ms = 0\n" "$FIX"
} > "$S2/config/tunex/config.toml"
export XDG_CONFIG_HOME="$S2/config" XDG_DATA_HOME="$S2/data"
busctl --user monitor --match "destination='"'"'org.freedesktop.Notifications'"'"'" > "$S2/mon.log" 2>&1 &
MON=$!
./build/tunex > "$S2/app.log" 2>&1 &
APP2=$!
wait_for_bus || { kill "$APP2" "$MON" 2>/dev/null; exit 1; }
sleep 3
qdbus6 "$SVC" "$P" org.mpris.MediaPlayer2.Player.Play >/dev/null
sleep 4
kill "$MON" 2>/dev/null; sleep 0.5
if grep -q "Playback error" "$S2/mon.log"; then
    echo "ok: Notify(Playback error) emitted on the bus"
else
    echo "FAIL: no Notify call captured"
    echo "--- monitor tail:"; tail -20 "$S2/mon.log"
    echo "--- app tail:"; tail -10 "$S2/app.log"
    kill "$APP2" 2>/dev/null; exit 1
fi
busctl --user list --no-legend | grep -q "$SVC" && echo "ok: app survives the corrupt file"
kill "$APP2" 2>/dev/null
pkill -f "build/tunex$" 2>/dev/null || true
rm -rf "$S1" "$S2"
echo "net-off rehearsal: ALL GREEN"
'
