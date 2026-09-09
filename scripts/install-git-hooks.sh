#!/usr/bin/env bash
# Install the repo's git hooks into .git/hooks. Re-run after cloning.
set -euo pipefail

HOOK_SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")/git-hooks" && pwd)/pre-commit"
HOOK_DST="$(git rev-parse --git-dir)/hooks/pre-commit"

cp "$HOOK_SRC" "$HOOK_DST"
chmod +x "$HOOK_DST"
echo "Installed pre-commit hook -> $HOOK_DST"
