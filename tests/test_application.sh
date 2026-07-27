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
echo "6. edit the palette, save, and reopen"
# The palette is only half of a document: the sidecar has to carry an edited one
# back, or every recolour would be lost on the next open.
PAL="$OUT/doc/palette"
rm -f "$PAL.png" "$PAL.atlas.json"
# Recolour entry 2 to orange, add a blue one and paint with it, drop it again,
# then reselect the orange entry and walk it down to slot 0. The reselect matters:
# dropping an entry clamps the selection to the end of the palette, not back to
# where it was. Ends selected on slot 0, so the reopened run proves both the
# colour and the order came back.
./tests/shot.sh "$OUT/03-palette-edited.png" \
    "select 2; color #ff8800ff; color+; color #22aaffff; paint 1,1; paint 2,2; color-; select 2; move-; move-; save" \
    -- "$PAL"

# Started from the 17-colour built-in palette: one added, one removed.
COLORS=$(grep -c '"r":' "$PAL.atlas.json")
if [ "$COLORS" != "17" ]; then
    echo "   FAIL: sidecar has $COLORS colours, expected 17" >&2
    exit 1
fi
echo "   sidecar carries $COLORS colours"

# Entry 2 was recoloured to #ff8800: red 1.0, green 0.53..., blue 0.0.
if grep -q '"g": 0.5333' "$PAL.atlas.json"; then
    echo "   sidecar carries the recoloured entry"
else
    echo "   FAIL: the recoloured entry is not in the sidecar" >&2
    exit 1
fi

# It was then moved to slot 0, so it is the first colour in the sidecar and the
# selection followed it. Order is part of the format, not a UI detail.
if grep -q '"selected_index": 0' "$PAL.atlas.json"; then
    echo "   sidecar carries the moved selection"
else
    echo "   FAIL: the selection did not follow the moved entry" >&2
    grep '"selected_index"' "$PAL.atlas.json" >&2
    exit 1
fi
if [ "$(grep -n '"g": 0.5333' "$PAL.atlas.json" | cut -d: -f1)" -lt \
     "$(grep -n '"g": 0.0' "$PAL.atlas.json" | head -1 | cut -d: -f1)" ]; then
    echo "   the moved entry is first in the sidecar"
else
    echo "   FAIL: the moved entry is not first in the sidecar" >&2
    exit 1
fi

./tests/shot.sh "$OUT/04-palette-reopened.png" "" -- "$PAL.png"

echo
echo "7. the generated palette, painted with and saved"
# The point of generating into `Palette` is that nothing downstream knows: a
# generated colour has to paint and save exactly like a listed one. This run
# switches, edits a second ramp, paints with a colour from a third, and saves —
# so the sidecar proves both the generated colours and the ramps behind them
# reached the file.
GEN="$OUT/doc/generated"
rm -f "$GEN.png" "$GEN.atlas.json"
./tests/shot.sh "$OUT/05-generated.png" \
    "mode; select 10; axis b; ramp steps 6; ramp+; select 3; paint 1,1; paint 2,2; save" \
    -- "$GEN"

# `serde_json::to_string_pretty` writes fields in declaration order, so the live
# palette is everything before the "generated" key and the ramps and the parked
# palette everything after it. Splitting there keeps this to grep and sed.
LIVE=$(sed '/"generated"/,$d' "$GEN.atlas.json" | grep -c '"r":')
BLOCK=$(sed -n '/"generated"/,$p' "$GEN.atlas.json")
RAMPS=$(printf '%s\n' "$BLOCK" | grep -c '"axis"')
PARKED=$(printf '%s\n' "$BLOCK" | grep -c '"r":')

# Three default ramps of 8, minus the two the shortened one lost, plus the
# 6-step copy the `ramp+` made: 8 + 6 + 8 + 6.
if [ "$LIVE" != "28" ]; then
    echo "   FAIL: sidecar has $LIVE generated colours, expected 28" >&2
    exit 1
fi
echo "   sidecar carries $LIVE generated colours"

if [ "$RAMPS" != "4" ]; then
    echo "   FAIL: sidecar has $RAMPS ramps, expected 4" >&2
    exit 1
fi
if ! printf '%s\n' "$BLOCK" | grep -q '"axis": "b"'; then
    echo "   FAIL: the ramp switched to sweeping b is not in the sidecar" >&2
    exit 1
fi
echo "   sidecar carries $RAMPS ramps, including the one switched to b"

# The hand-edited palette was parked, not discarded, so the built-in 17 have to
# be in there too — otherwise switching back after a reopen would lose them.
if [ "$PARKED" != "17" ]; then
    echo "   FAIL: sidecar parked $PARKED hand-edited colours, expected 17" >&2
    exit 1
fi
echo "   sidecar parks $PARKED hand-edited colours behind them"

# Reopen and save straight back out. A lossless sidecar reproduces itself byte
# for byte — which is the strongest statement available that nothing about
# either palette was dropped, reordered or rounded on the way through.
cp "$GEN.atlas.json" "$OUT/doc/generated.expected.json"
./tests/shot.sh "$OUT/06-generated-reopened.png" "save" -- "$GEN.png"
if diff -q "$OUT/doc/generated.expected.json" "$GEN.atlas.json" >/dev/null; then
    echo "   reopening and re-saving reproduces the sidecar exactly"
else
    echo "   FAIL: the sidecar changed across a reopen" >&2
    diff "$OUT/doc/generated.expected.json" "$GEN.atlas.json" >&2 || true
    exit 1
fi

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
echo "  03-palette-edited.png"
echo "                  — the orange entry (was white) has been walked to the"
echo "                    first slot and is still selected, the added blue entry"
echo "                    was removed again, the two painted pixels stayed blue,"
echo "                    and the sliders show the selected colour."
echo "  04-palette-reopened.png"
echo "                  — the same file reopened: the orange swatch is still"
echo "                    first and still selected, so both the colour and the"
echo "                    order survived the sidecar. No '*'."
echo "  05-generated.png"
echo "                  — 'palette: oklab'. Four ramps, each on its own run of"
echo "                    rows: warm 8, cool 6 sweeping b (blue to olive), hue 8,"
echo "                    and a 6-step copy of the cool one. Two pixels painted"
echo "                    with a generated colour. No '*' (it saved)."
echo "  06-generated-reopened.png"
echo "                  — the same file reopened: identical to 05. It comes back"
echo "                    in 'oklab' mode, on the same ramp, with the same sweep"
echo "                    in the sliders — the ramps are in the sidecar, not"
echo "                    just the colours they made."
echo
echo "To drive it by hand:  cargo run -- $WORK.png"
