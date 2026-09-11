#!/usr/bin/env bash
# TuneX QML gate: Qt 6 `qmllint` in module context plus `qmlformat` verify.
#
# Why a script instead of a bare `qmllint *.qml`:
#   - On Arch, /usr/bin/qmllint and /usr/bin/qmlformat belong to
#     qt5-declarative (Qt 5.15). TuneX is Qt 6 (D-002), so the Qt 6 host
#     binaries are resolved explicitly.
#   - The source tree has no qmldir: cxx-qt-build (crates/tunex-app/build.rs)
#     generates it at build time, lists files under qml/TuneX/, and omits the
#     `depends QtQml` line the Rust models' QAbstractListModel base needs. Run
#     from the source tree, qmllint sees Theme/Glass as plain components and
#     every Rust model as unresolved. So the files are linted from a staged
#     copy with a complete qmldir plus the generated plugin.qmltypes.
#
# Unqualified access is the one category switched off: delegates read model
# roles through `model.*`/`index` context lookups by project convention (the
# W-018 gate proved `required` delegate properties lock to their defaults
# with these cxx-qt models). Every other warning fails the gate
# (--max-warnings 0).
#
# BestPractices plugin: the QQMLSA plugin in the sibling QtQmlBestPractices
# project adds eight Qt Quick best-practice categories on top of qmllint's
# own. It is a separate local project with no published remote, so it is
# discovered rather than required — present means its categories are part of
# the gate, absent (a clean CI container) means the baseline lint still runs
# and the script says which categories went unchecked. Point
# TUNEX_QMLLINT_PLUGIN_PATH at another build directory to override discovery,
# or set it to `off` to skip the plugin deliberately.
#
# Usage: scripts/qml-lint.sh [file.qml ...]   (default: every module file)
# Needs one prior cargo build of tunex-app (for plugin.qmltypes); the
# pre-commit hook and CI run the Rust gate first, which provides it.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$ROOT/crates/tunex-app/qml/TuneX"

QT_BINS="$(qmake6 -query QT_HOST_BINS 2>/dev/null || true)"
QT_BINS="${QT_BINS:-/usr/lib/qt6/bin}"
QMLLINT="$QT_BINS/qmllint"
QMLFORMAT="$QT_BINS/qmlformat"
for tool in "$QMLLINT" "$QMLFORMAT"; do
    [[ -x "$tool" ]] || { echo "qml-lint: missing Qt 6 tool $tool (install qt6-declarative)" >&2; exit 1; }
done

# Categories the plugin ships (see QtQmlBestPractices/README.md). Every one is
# enabled at `warning`, which --max-warnings 0 turns into a gate failure.
BEST_PRACTICES=(
    prefer-typed-properties
    layout-child-geometry
    versioned-imports
    no-state-in-delegates
    imperative-completed-assignment
    native-style-customization
    prefer-interaction-signals
    redundant-window-import
)

PLUGIN_LIB="libBestPracticesPlugin.so"
QT_PLUGINS="$(qmake6 -query QT_INSTALL_PLUGINS 2>/dev/null || true)"
plugin_dir="${TUNEX_QMLLINT_PLUGIN_PATH-}"
if [[ -z "$plugin_dir" ]]; then
    for candidate in "$ROOT/../QtQmlBestPractices/build" "${QT_PLUGINS:-/usr/lib/qt6/plugins}/qmllint"; do
        if [[ -f "$candidate/$PLUGIN_LIB" ]]; then
            plugin_dir="$(cd "$candidate" && pwd)"
            break
        fi
    done
fi

plugin_args=()
if [[ "$plugin_dir" == "off" ]]; then
    echo "qml-lint: BestPractices plugin skipped (TUNEX_QMLLINT_PLUGIN_PATH=off)" >&2
elif [[ -n "$plugin_dir" && -f "$plugin_dir/$PLUGIN_LIB" ]]; then
    # Already-installed plugins load on their own; -P is for a build tree.
    [[ "$plugin_dir" == "${QT_PLUGINS:-}/qmllint" ]] || plugin_args+=(-P "$plugin_dir")
    for category in "${BEST_PRACTICES[@]}"; do
        plugin_args+=("--Plugin.BestPractices.$category" warning)
    done
elif [[ -n "$plugin_dir" ]]; then
    echo "qml-lint: no $PLUGIN_LIB under $plugin_dir (build it: cmake -S . -B build && cmake --build build)" >&2
    exit 1
else
    echo "qml-lint: BestPractices plugin not found — ${#BEST_PRACTICES[@]} categories unchecked" >&2
fi

# Newest generated type info wins (cargo target dir or the CMake build).
TYPES="$(find "$ROOT/target" "$ROOT/build" -path '*qml_modules/TuneX/plugin.qmltypes' \
    -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2- || true)"
[[ -n "$TYPES" ]] || { echo "qml-lint: no plugin.qmltypes yet — build tunex-app once (rust-tc check)" >&2; exit 1; }

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
# Mirror the runtime layout exactly: the module qmldir sits in TuneX/ and
# lists files under qml/TuneX/, which has no qmldir of its own. Types from
# the Rust bridge then resolve only through an explicit `import TuneX`,
# as at runtime (a file missing that import fails to load in the app).
mkdir -p "$STAGE/TuneX/qml/TuneX"
cp "$SRC"/*.qml "$STAGE/TuneX/qml/TuneX/"
cp "$TYPES" "$STAGE/TuneX/"
{
    echo "module TuneX"
    echo "typeinfo plugin.qmltypes"
    echo "depends QtQml"
    for file in "$SRC"/*.qml; do
        name="$(basename "$file" .qml)"
        if grep -q '^pragma Singleton' "$file"; then
            echo "singleton $name 1.0 qml/TuneX/$name.qml"
        else
            echo "$name 1.0 qml/TuneX/$name.qml"
        fi
    done
} >"$STAGE/TuneX/qmldir"

targets=()
if [[ $# -gt 0 ]]; then
    for file in "$@"; do targets+=("TuneX/qml/TuneX/$(basename "$file")"); done
else
    for file in "$SRC"/*.qml; do targets+=("TuneX/qml/TuneX/$(basename "$file")"); done
fi

status=0
lint_log="$STAGE/qmllint.log"
if ! (cd "$STAGE" && "$QMLLINT" -I "$STAGE" --unqualified disable --max-warnings 0 \
    "${plugin_args[@]}" "${targets[@]}") \
    >"$lint_log" 2>&1; then
    status=1
fi
# Staged paths map back to source paths so findings stay clickable.
sed "s#TuneX/qml/TuneX/#crates/tunex-app/qml/TuneX/#g" "$lint_log" >&2
[[ $status -eq 0 ]] || echo "qml-lint: qmllint reported findings" >&2

for target in "${targets[@]}"; do
    source_file="$SRC/$(basename "$target")"
    if ! diff -q <("$QMLFORMAT" "$source_file") "$source_file" >/dev/null; then
        echo "qml-lint: not qmlformat-clean: ${source_file#"$ROOT"/} (fix: $QMLFORMAT -i <file>)" >&2
        status=1
    fi
done

[[ $status -eq 0 ]] && echo "qml-lint: ${#targets[@]} file(s) clean (Qt 6 qmllint + qmlformat)"
exit $status
