#!/bin/bash
#
# Automated checks: compile, unit tests, release build, and a real scripted run
# that walks the whole editing workflow and captures screenshots.
#
# Usage: ./tests/test_application.sh [OUTPUT_DIR]

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

OUT="${1:-target/screenshots}"
mkdir -p "$OUT" "$OUT/doc"

echo "=========================================="
echo "Bevy Sprite Atlas Editor — checks"
echo "=========================================="

echo
echo "1. cargo check"
cargo check --all-targets --quiet
echo "   ok"

echo
echo "2. cargo test"
cargo test --quiet
echo "   ok"

echo
echo "3. cargo build --release"
cargo build --release --quiet
echo "   ok"

echo
echo "4. workflow run (paint, add tiles, save)"
WORK="$OUT/doc/workflow"
rm -f "$WORK.png" "$WORK.atlas.json"
./tests/shot.sh "$OUT/01-painted.png" \
    "select 3; paint 0,0; paint 1,1; paint 2,2; paint 7,7; tile+; select 5; paint 8,0; paint 15,7; save" \
    -- "$WORK"

for f in "$WORK.png" "$WORK.atlas.json"; do
    if [ -s "$f" ]; then
        echo "   wrote $f"
    else
        echo "   FAIL: $f was not written" >&2
        exit 1
    fi
done

# The sidecar must agree with what the script built: 2 tiles wide.
if grep -q '"width_in_tiles": 2' "$WORK.atlas.json"; then
    echo "   sidecar reports 2 tiles"
else
    echo "   FAIL: sidecar does not report 2 tiles" >&2
    cat "$WORK.atlas.json" >&2
    exit 1
fi

echo
echo "5. reopen the saved file, widen it, and scroll"
# Green goes at x=24,25 — the first column of tile 3 — so after scrolling 3
# tiles it lands exactly at the canvas's left edge. That is what makes the
# screenshot prove the scroll offset is exact and not merely nonzero.
./tests/shot.sh "$OUT/02-scrolled.png" \
    "tile+6; select 4; paint 24,0; paint 25,1; paint 26,2; paint 63,7; scroll+3" \
    -- "$WORK.png"

echo
echo "=========================================="
echo "All checks passed"
echo "=========================================="
echo
echo "Screenshots in $OUT:"
ls -1 "$OUT"/*.png
echo
echo "Look for:"
echo "  01-painted.png  — red marks in tile 0, blue in tile 1, orange tile"
echo "                    boundaries, no '*' in the status strip (it saved)"
echo "  02-scrolled.png — 8 tiles wide, scrolled 3 tiles right. The green"
echo "                    diagonal painted at x=24 starts exactly at the left"
echo "                    edge, and tiles 0-2 (the red/blue marks) have"
echo "                    scrolled off."
echo
echo "To drive it by hand:  cargo run -- $WORK.png"
