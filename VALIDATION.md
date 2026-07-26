# Bevy Sprite Atlas Editor - Validation

How to check the editor works. Most of it is automated, including the visual
part — the editor screenshots itself, so verifying the UI does not need a human
at the keyboard.

## Automated

```bash
./tests/test_application.sh          # everything below, plus screenshots
```

That runs `cargo check --all-targets`, `cargo test`, `cargo build --release`, and
then two real scripted runs of the editor:

1. **Workflow run** — paints in tile 0, adds a tile, paints in tile 1 with a
   second colour, and saves. Asserts that both `workflow.png` and
   `workflow.atlas.json` were written and that the sidecar reports 2 tiles.
2. **Reopen run** — opens the file the first run saved, widens it to 8 tiles,
   paints a green diagonal at x=24, and scrolls 3 tiles right.

Screenshots land in `target/screenshots/`. Individually:

```bash
cargo check --all-targets
cargo test                           # 89 unit tests
cargo test a_wide_atlas              # one test
cargo build --release
```

### What the unit tests cover

- `atlas.rs` — row-major indexing, and that growing the atlas does not move any
  existing pixel (the one place a silent corruption could hide)
- `palette.rs` — selection stays valid through add/remove/reorder; sidecar
  round trip; a sidecar with a missing or overshooting selection
- `file_io.rs` — save/load round trip including a **wide** atlas pixel-for-pixel
  and alpha survival; rejection of a ragged image and of a future format version
- `canvas_view.rs` — that drawing and hit testing are exact inverses, scrolled or
  not; scroll clamping at both ends, including re-clamping after the atlas shrinks
- `document.rs` — requests applied in order; out-of-bounds and no-op paints leave
  the revision alone; a multi-pixel drag is one revision bump
- `input.rs` — click, drag-once-per-pixel, release ends the stroke, key bindings
- `dev_harness.rs` — script parsing and one-step-per-frame execution
- `cli.rs` — argument parsing, suffix stripping, bad input falling back
- `ui_chrome.rs` — theme tokens generated from the loaded palette

## Taking a screenshot

```bash
./tests/shot.sh /tmp/shot.png "select 3; paint 2,3; paint 3,3; tile+; scroll+"
./tests/shot.sh /tmp/open.png "scroll+2" -- art/demo.png --scale 16
```

Anything after `--` goes to the editor verbatim. Script verbs:

| Verb | Effect |
|---|---|
| `select N` | Select palette colour N |
| `paint X,Y` | Paint atlas pixel (X, Y) |
| `tile+[N]` | Add N tiles on the right (default 1) |
| `scroll+[N]` / `scroll-[N]` | Scroll N tiles right / left (default 1) |
| `save` | Save to the current path |
| `wait [N]` | Idle for N frames |

One step per frame, so the same script always yields the same image for a build.
The capture uses Bevy's own screenshot API, so no window manager, focus or
`sleep` is involved. `--warmup-frames` (default 30) waits out the window
manager's initial resize before the first step; `--settle-frames` (default 30)
waits for the last edit to reach the screen. With no `DISPLAY`, `shot.sh` falls
back to `xvfb-run`.

Because the harness emits the same request messages real input does, a passing
screenshot means the production path works — it is not a mock. The exception is
the raw input layer itself (mouse position → hovered pixel, and the Feathers
button activation): those are covered by unit tests, not by the screenshots.

## Reading a screenshot

The status strip makes a capture self-describing:

```
demo.png*    12,3   tile 1    #ff0000ff    3x1 tiles of 8px
    |        |      |         |            |
    |        |      |         |            atlas size and tile size
    |        |      |         selected colour
    |        |      which tile the cursor is in
    |        hovered pixel, or "off canvas"
    file name, with * when there are unsaved changes
```

## Manual check

```bash
cargo run                            # new one-tile document
cargo run -- art/demo                # open art/demo.png
cargo run -- --help
```

Expected:

- Window titled "Bevy Sprite Atlas Editor"
- **Left:** palette sidebar, one swatch per colour, wrapping. The first entry is
  fully transparent so it reads as an empty slot; the selected swatch has a white
  outline (black, the first opaque colour, by default).
- **Right:** the atlas at the top-left of the canvas area, tile boundaries in
  orange including the far edge, a faint pixel grid inside them.
- **Bottom:** the status strip.

| Action | Expected |
|---|---|
| Click a swatch | The outline moves; the hex in the status strip changes |
| Click the canvas | That pixel takes the selected colour; `*` appears |
| Hold and drag | A continuous line, one paint per pixel |
| Click palette entry 0, then a painted pixel | It is erased back to transparent |
| Move the cursor over the canvas | The coordinate and tile update |
| Move the cursor off the canvas | "off canvas" |
| `Space` | The atlas widens by one tile; the tile count updates |
| `Arrow Right` / `D` | Scrolls one tile right, if the atlas is wider than the canvas |
| `Arrow Left` / `A` | Scrolls back, stopping at the left edge |
| `Ctrl+S` | Logs the path; `*` clears |
| Resize the window | The canvas region refits; the sidebar and strip stay put |

A one-tile atlas cannot scroll — it is narrower than the canvas, so the scroll
offset is pinned at 0. Add several tiles first.

## Known Limitations

1. **No undo/redo.** `src/history.rs` is tested but unwired.
2. **No palette editing** — colours can be selected, not added, removed or reordered.
3. **No tile removal binding.**
4. **Single tile row.** Storage supports more; nothing creates them.
5. **No runtime zoom** — `--scale` is set at startup.
6. **No file picker** — paths come from the command line.
7. **Transparent pixels show the flat background,** not a checkerboard, so
   transparent-on-transparent is not visually distinguishable.

## Last Validated

- Date: 2026-07-26
- 89 unit tests passing; `./tests/test_application.sh` green
- Verified visually on X11 with an AMD Radeon 780M (radv): open, save, reopen,
  select, paint, add tile, scroll, tile boundaries
