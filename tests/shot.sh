#!/bin/bash
#
# Screenshot the editor after a scripted sequence of actions.
#
# The capture is done by the app itself (Bevy's Screenshot API), not by an
# external screenshotter — so there is no window manager, focus or sleep
# involved, and the same script always produces the same image for a build.
#
# Usage:
#   ./tests/shot.sh OUTPUT.png [SCRIPT] [-- EXTRA ARGS...]
#
# Examples:
#   ./tests/shot.sh /tmp/blank.png
#   ./tests/shot.sh /tmp/painted.png "select 3; paint 2,3; paint 3,3; tile+"
#   ./tests/shot.sh /tmp/wide.png "tile+5; scroll+3" -- art/demo --scale 16
#
# Script verbs: select N | paint X,Y | tile+[N] | scroll+[N] | scroll-[N]
#               color #rrggbb[aa] | color+ | color- | save | wait [N]

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [ $# -lt 1 ]; then
    sed -n '3,20p' "${BASH_SOURCE[0]}" | sed 's/^# \?//'
    exit 1
fi

OUTPUT="$1"
shift

SCRIPT=""
if [ $# -gt 0 ] && [ "$1" != "--" ]; then
    SCRIPT="$1"
    shift
fi

# Anything after `--` goes to the editor verbatim (document path, --scale, ...).
EXTRA=()
if [ $# -gt 0 ] && [ "$1" = "--" ]; then
    shift
    EXTRA=("$@")
fi

ARGS=("--screenshot" "$OUTPUT")
if [ -n "$SCRIPT" ]; then
    ARGS+=("--script" "$SCRIPT")
fi

# No DISPLAY: fall back to a virtual one so this works over ssh and in CI.
RUNNER=()
if [ -z "${DISPLAY:-}" ]; then
    if command -v xvfb-run >/dev/null; then
        echo "no DISPLAY — running under xvfb-run"
        RUNNER=(xvfb-run -a)
    else
        echo "error: no DISPLAY and no xvfb-run; install xvfb or set DISPLAY" >&2
        exit 1
    fi
fi

mkdir -p "$(dirname "$OUTPUT")"

echo "capturing to $OUTPUT"
[ -n "$SCRIPT" ] && echo "  script: $SCRIPT"
[ ${#EXTRA[@]} -gt 0 ] && echo "  args:   ${EXTRA[*]}"

"${RUNNER[@]}" cargo run --quiet -- "${EXTRA[@]}" "${ARGS[@]}"

if [ -s "$OUTPUT" ]; then
    echo "wrote $OUTPUT ($(du -h "$OUTPUT" | cut -f1))"
else
    echo "error: no screenshot was written" >&2
    exit 1
fi
