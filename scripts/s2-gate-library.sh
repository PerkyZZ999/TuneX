#!/usr/bin/env bash
# S2 gate fixture builder (W-018): a 12k-file local library from the tiny
# committed tones. Real copies (never hardlinks — every file needs its own
# inode or rename matching cannot be observed).
set -euo pipefail

LIB="${1:-/tmp/tunex-gate-lib}"
COUNT="${2:-12000}"
FIXTURES="$(dirname "$0")/../tests/fixtures"

rm -rf "$LIB"
mkdir -p "$LIB"

formats=(sine.ogg sine.mp3 sine.flac sine.opus sine.m4a sine.wav)
index=0
while [ "$index" -lt "$COUNT" ]; do
  group=$((index / 1000))
  mkdir -p "$LIB/group-$group"
  tone="${formats[$((index % 6))]}"
  ext="${tone##*.}"
  # Zero-padded names sort alongside the scan's path order.
  printf -v name "track-%05d.%s" "$index" "$ext"
  cp "$FIXTURES/$tone" "$LIB/group-$group/$name"
  index=$((index + 1))
done

echo "built $COUNT files in $LIB ($(du -sh "$LIB" | cut -f1))"
