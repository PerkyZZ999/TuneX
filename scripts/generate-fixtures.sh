#!/usr/bin/env bash
# Regenerate tests/fixtures/ — tiny synthetic clips (440 Hz sine) for the
# codec matrix (W-006). Requires the full GStreamer plugin set (base/good/
# bad/ugly/libav); see AGENTS.md setup. Safe to re-run: outputs are
# deterministic short clips, a few KB each.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIR="$ROOT/tests/fixtures"
mkdir -p "$DIR"

# ~0.9 s of mono tone; resampled to 8 kHz to keep fixtures small.
SRC="audiotestsrc wave=sine freq=440 num-buffers=40 ! audioconvert ! audioresample ! audio/x-raw,rate=8000,channels=1"

gst-launch-1.0 -q $SRC ! wavenc ! filesink location="$DIR/sine.wav"
gst-launch-1.0 -q $SRC ! flacenc ! filesink location="$DIR/sine.flac"
gst-launch-1.0 -q $SRC ! vorbisenc ! oggmux ! filesink location="$DIR/sine.ogg"
gst-launch-1.0 -q $SRC ! opusenc ! oggmux ! filesink location="$DIR/sine.opus"
gst-launch-1.0 -q $SRC ! lamemp3enc ! filesink location="$DIR/sine.mp3"
gst-launch-1.0 -q $SRC ! avenc_aac ! mp4mux ! filesink location="$DIR/sine.m4a"

# Deterministic undecodable file: plain text with an audio extension.
printf 'TuneX fixture: this is not audio, decoding must fail loudly.\n' >"$DIR/corrupt.mp3"

ls -la "$DIR"
