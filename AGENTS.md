# Bevy Sprite Atlas Editor - Project Specification

## Overview

A Bevy-based editor for creating and editing sprite atlases. A sprite atlas is a
single image file split into multiple square tiles. The editor provides
pixel-level editing with a colour palette, tile management, and PNG + JSON
persistence.

**Technology Stack:**
- **Engine:** Bevy 0.20
- **UI:** `bevy_ui` with `bevy_feathers` widgets for the palette; the canvas is a
  world-space sprite on its own camera
- **Language:** Rust

Per-plugin design specs (Mental Model Language) live in the notes repo, not here:
`~/fun_repos/hobby-mimisbrunnr/notes/bevy_sprite_editor/initial_editor/fixup/*.hmml`

---

## Core Requirements

### Atlas Structure
- **Tile dimensions:** Square, configurable at startup with `--tile-size`
  (default: 8x8 pixels)
- **Initial atlas:** 1 tile
- **Tile growth:** Adding a tile appends one fully transparent tile column on the
  right (grows atlas width by 1 tile)
- **Pixel display:** Each atlas pixel is drawn at `--scale` screen pixels
  (default 32), so an 8x8 tile is 256x256 on screen
- **Storage:** Pixels are row-major over the **whole atlas image**
  (`index = y * width_in_pixels + x`). Tiles are a view concept only.
- **Scrolling:** Horizontal only, by whole tiles, clamped to the atlas bounds

### Colour Palette
- **Capacity:** Unlimited colours
- **Visual representation:** Feathers buttons coloured from the palette, one per
  entry, in a left sidebar
- **Selection:** The active colour carries a white outline
- **Storage:** Palette saved with the atlas, in the JSON sidecar
- **Entry 0 is fully transparent,** so erasing needs no separate tool
- **Initial draw colour:** the first opaque entry (black)
- **Two ways to specify it,** switched by the button at the top of the sidebar
  (or `M`). There is still only one `Palette`; the button chooses what fills it.

#### `list` — the hand-edited palette
- **Editing:** four Feathers colour sliders under the swatches (R, G, B, A) set
  the *selected* entry; `<` and `>` move it one slot, `+` appends an opaque white
  entry and selects it, `-` drops the selected one. The last colour cannot be
  removed — something must stay selected to draw with, and the arrows clamp at
  the ends rather than disabling themselves.
- **Reordering does not repaint anything.** Pixels store colours, not palette
  indices, so a move changes the sidecar's order and nothing else.
- **Not yet implemented:** reordering by dragging a swatch

#### `oklab` — generated ramps
- **A ramp** fixes two of Oklab's three values and sweeps the third over a range
  in a fixed number of steps (2–24). Six numbers instead of a list.
- **Several are live at once,** three by default: a warm and a cool lightness
  ramp to shade with, and one hue sweep. Each gets its own row of swatches.
- **The edited ramp is the one the selected swatch came from,** so clicking a
  swatch in another ramp moves the editor to it. `L`/`a`/`b` choose which value
  sweeps (and reset the range into the new units), `+` duplicates the edited ramp
  and selects it, `-` drops it. The last ramp cannot be removed.
- **The palette is derived,** rewritten from the ramps on every change, so the
  `list` editors are refused here — an edit would be overwritten by the next
  regenerate. The hand-edited palette is parked whole and comes back with its
  selection intact when the mode is switched off.
- **Out-of-gamut colours lose chroma, not hue.** Oklab is wider than sRGB, and
  clipping each channel would collapse the far end of a chroma sweep onto one
  corner colour; the chroma is pulled toward neutral until the colour fits.
- **The ramps go in the sidecar,** so a document reopens in the mode it was
  saved in, on the same ramp, with the same sweep in the sliders. The colours
  they produced are written too, and the hand-edited palette is written parked
  behind them — nothing is lost either way round.

### Drawing
- **Granularity:** Pixel-by-pixel
- **Tools:** Single-pixel pencil. Holding the button and dragging paints a
  continuous line, at most once per pixel entered.
- **Colour source:** the selected palette colour

### Undo/Redo
- **Not wired up.** `src/history.rs` implements and tests a 100-deep action
  stack, but nothing calls it. Reattaching it is a separate task.

### File I/O
- **Format:** PNG + JSON (separate files), format version `1.0`
- **PNG:** the atlas image, same dimensions and pixel order as the document
- **JSON** (`<base>.atlas.json`): version, tile size, atlas dimensions in tiles,
  the palette that was being drawn with, and a `generated` block holding the
  Oklab ramps plus — when they were in charge — the hand-edited palette they
  displaced. `generated` is optional and the version stays `1.0`: the field is
  purely additive, so files written before it still load and builds that predate
  it still read files that have it.
- **The ramps are the specification.** On load the generated palette is rebuilt
  from them rather than read back out of the stored colours, so hand-editing the
  JSON's ramps works. Values outside the editor's own limits are clamped on the
  way in, and a file with no ramps falls back to the built-in ones.
- **Opening:** the first positional CLI argument. `demo`, `demo.png` and
  `demo.atlas.json` all name the same document.
- **A missing sidecar is fine** — the PNG loads with the `--tile-size` fallback
  and the built-in palette.
- **An image whose size is not a whole multiple of the tile size is rejected**
  rather than silently padded.
- **Saving** with no path given writes `./untitled.png` + `./untitled.atlas.json`.

### UI Layout

```
+------------------+----------------------------------------------+
|  [palette: list] |                                              |
|                  |   Atlas canvas                               |
|  Palette         |   (own camera + viewport, scrolls            |
|  swatches        |    horizontally, tile boundaries drawn)       |
|  (Feathers,      |                                              |
|   one wrapping   |                                              |
|   row per ramp)  |                                              |
|                  |                                              |
|  [<][>][+][-]    |         ... or, in `oklab` mode:  [L][a][b]  |
|  R G B A sliders |                                   [+][-]     |
|                  |                                   L a from   |
|                  |                                   to steps   |
+------------------+----------------------------------------------+
| file* | x,y tile N | #rrggbbaa [sel/count] | mode | WxH tiles   |
+----------------------------------------------------------------+
```

The mode segment is `list`, or `oklab [ramp/count] AXIS from..to xSTEPS` — so a
screenshot of a generated palette names the sweep that produced it.

The canvas camera's `viewport` is fitted each frame to the region the sidebar and
status strip leave free, so canvas world space is independent of the sidebar
width.

---

## Project Structure

```
bevy_sprite_editor/
├── AGENTS.md                # This file
├── CLAUDE.md                # Guidance for Claude Code
├── VALIDATION.md            # How to validate the build
├── Cargo.toml
├── src/
│   ├── main.rs              # App assembly, cameras, plugin + startup order
│   ├── cli.rs               # argv -> LaunchOptions
│   ├── config.rs            # Constants, colours, helpers
│   ├── atlas.rs             # AtlasDocument: the pixel grid
│   ├── palette.rs           # Palette, PaletteMode + serde mirrors
│   ├── oklab.rs             # Oklab ramps: the generated palette
│   ├── document.rs          # Edit requests and the systems applying them
│   ├── file_io.rs           # PNG + JSON codec, load and save
│   ├── canvas_view.rs       # Camera viewport, scroll, cursor -> pixel
│   ├── canvas_render.rs     # Atlas texture, sprite, tile grid
│   ├── ui_chrome.rs         # Feathers sidebar + status strip
│   ├── input.rs             # Keys and mouse -> requests
│   ├── dev_harness.rs       # Scripted actions, screenshot, exit
│   └── history.rs           # Undo/redo (compiles, tested, unwired)
├── tests/
│   ├── shot.sh              # Screenshot the editor after a scripted run
│   └── test_application.sh  # check + test + release + scripted screenshots
└── docs/
    ├── mml/                 # Superseded pre-implementation notation, kept for reference
    └── work_log/
```

---

## Data Models

```rust
struct AtlasDocument {          // Resource
    tile_size: u32,
    width_in_tiles: u32,
    height_in_tiles: u32,       // always 1 in this version
    pixels: Vec<Srgba>,         // row-major over the whole image
    revision: u64,              // bumped per accepted mutation
    unsaved: bool,
}

struct Palette {                // Resource
    colors: Vec<Srgba>,
    selected: usize,            // always valid; there is always a draw colour
}

struct PaletteMode {            // Resource
    generated: bool,            // which specification owns `Palette`
    stashed: Option<Palette>,   // the hand-edited one, parked; Some iff generated
}

struct OklabRamp {
    axis: OklabAxis,            // Lightness | A | B — which value sweeps
    lightness: f32,             // all three stored; the swept one is ignored
    a: f32,
    b: f32,
    from: f32,                  // the sweep, in the swept axis' units;
    to: f32,                    //   from > to runs the ramp backwards
    steps: usize,               // clamped to 2..=24
}

struct OklabRamps {             // Resource
    ramps: Vec<OklabRamp>,      // colours concatenated in order *are* the palette
}

struct CanvasView {             // Resource
    scroll_x: f32,              // screen pixels
    pixel_scale: f32,
    rect_min: Vec2,             // canvas region, logical pixels
    rect_size: Vec2,
}
```

### Edit requests (Bevy 0.20 "messages")

`PaintPixelRequested { x, y }`, `SelectColorRequested { index }`,
`AddTileRequested { tiles }`, `ScrollRequested { tiles }`, `SaveRequested`,
`SetPaletteColorRequested { index, color }`, `AddPaletteColorRequested { color }`,
`RemovePaletteColorRequested { index }`, `MovePaletteColorRequested { from, to }`,
`TogglePaletteModeRequested`, `SetRampRequested { index, ramp }`,
`AddRampRequested`, `RemoveRampRequested { index }`.

Input systems only emit these; `document.rs` and `file_io.rs` are the only
places that apply them. The dev harness emits the same ones, which is what makes
a screenshot proof that the real path works.

---

## Input Mapping

| Input | Action |
|-------|--------|
| Mouse Left click / drag (canvas) | Paint the hovered pixel |
| Mouse Left click (swatch) | Select that colour |
| Mouse wheel (swatches) | Scroll them when the palette overflows the sidebar |
| Mouse Left drag (R/G/B/A slider) | Set that channel of the selected colour |
| Mouse Left click (`<` / `>`) | Move the selected entry one slot; clamps at the ends |
| Mouse Left click (`+` / `-`) | Append an entry and select it / drop the selected one |
| Mouse Left click (`palette: …`) | Switch between the `list` and `oklab` specifications |
| Mouse Left click (`L` / `a` / `b`) | Sweep that value in the edited ramp |
| Mouse Left drag (ramp slider) | Set one of the edited ramp's six numbers |
| `M` | Switch palette specification |
| `Space` | Add a tile on the right |
| `Arrow Left` or `A` | Scroll one tile left |
| `Arrow Right` or `D` | Scroll one tile right |
| `Ctrl+S` | Save |

Removing a tile, undo and redo have no binding yet.

---

## Command Line

```
bevy_sprite_editor [PATH] [options]

  PATH                  document base path; .png / .atlas.json suffixes stripped
  --tile-size N         tile size for a new document (default 8)
  --scale N             screen pixels per atlas pixel (default 32)
  --script "..."        dev harness action script
  --screenshot FILE     capture to FILE once the script finishes, then exit
  --warmup-frames N     frames to wait before the script starts (default 30)
  --settle-frames N     frames to wait before capturing (default 30)
  --help
```

Script verbs: `select N`, `paint X,Y`, `tile+[N]`, `scroll+[N]`, `scroll-[N]`,
`color #rrggbb[aa]`, `color+`, `color-`, `move-`, `move+`, `mode`, `axis L|a|b`,
`ramp FIELD VALUE`, `ramp+`, `ramp-`, `save`, `wait [N]` — one step per frame,
semicolon separated. The `color`, `move`, `axis` and `ramp` verbs stand in for
the sidebar's sliders and buttons, which no script can point at; they act on the
selection (or the ramp it came from) and clamp at the ends the same way. A ramp
field is `L`, `a`, `b`, `from`, `to` or `steps` — the sidebar's own captions.

---

## Rendering

- The atlas is **one** sprite entity showing a single RGBA8 texture the size of
  the atlas, scaled up with nearest-neighbour sampling
  (`ImagePlugin::default_nearest`). Entity count does not grow with the atlas.
- The texture is reuploaded only when `AtlasDocument.revision` changes.
- The sprite is anchored top-left, so its translation is the atlas origin.
- Tile boundaries are gizmo lines drawn at every multiple of `tile_size`,
  including the far edge of the last tile. A fainter pixel grid is drawn
  underneath when `--scale` is large enough to read.
- Two cameras: the canvas camera (order 0, own render layer, viewport-fitted,
  clears) then the UI camera (order 1, no clear, full window, owns bevy_ui).
  **This order matters** — reversing it renders a black window.

### Coordinates

Atlas coordinates are top-left origin; Bevy 2D is y-up and centre-origin.
`canvas_view.rs` holds the only conversion:

```
atlas_left = -rect_size.x / 2 - scroll_x
atlas_top  =  rect_size.y / 2
pixel (x, y) covers [atlas_left + x*scale, ...] x [atlas_top - (y+1)*scale, ...]
```

`canvas_render` draws with these and `track_hovered_pixel` inverts them, so
drawing and hit testing cannot drift apart.

---

## Implementation Status

### Working
1. Open a document from a CLI argument (PNG + optional JSON sidecar)
2. Save back to that path, or to `untitled` when none was given
3. Palette sidebar built from the loaded palette; click to select, RGBA sliders
   to recolour the selection, `<` / `>` to reorder, `+` / `-` to add and remove
4. Click and drag to paint pixels
5. Add a tile on the right (`Space`)
6. Scroll horizontally by whole tiles, clamped
7. Tile boundaries and an optional pixel grid
8. Status strip: file, unsaved marker, hovered pixel and tile, selected colour,
   palette mode and edited ramp, atlas size
9. Scripted dev harness with self-capture and exit
10. A second way to specify the palette: Oklab ramps, several at once, switched
    with the sidebar's mode button or `M`

### Not implemented
1. Undo / redo — `history.rs` exists, tested, unwired, and palette edits are
   not recorded into it
2. Reordering by dragging a swatch — the arrow buttons do it instead
3. Removing a tile (`AtlasDocument::remove_tile_from_right` exists, unbound)
4. Multiple tile rows (`height_in_tiles` is always 1; the storage supports more)
5. Zoom at runtime (`--scale` is startup only)
6. A file picker; paths come from argv
7. A checkerboard behind transparent pixels — they show the flat background, and
   the transparent swatch reads as an empty slot in the sidebar
8. A transparent entry in a generated palette; erasing needs the `list` mode

---

## Decisions Made

- **Row-major pixel storage over the whole image.** Tile identity is a view
  concern. Makes texture upload and PNG encode straight copies. The previous
  per-tile-contiguous layout disagreed with the PNG encoder and corrupted any
  atlas wider than one tile.
- **All mutation flows through request messages.** Input never writes state, so
  the dev harness exercises the production path.
- **The canvas gets its own camera with a fitted viewport,** so canvas world
  space does not depend on the sidebar width.
- **One texture, not one sprite per pixel.**
- **Feathers for the palette sidebar,** with theme tokens generated per loaded
  palette colour rather than a hardcoded list. Recolouring rewrites a token and
  Feathers repaints the swatch; adding or removing changes how many tokens there
  are, so the swatches are respawned.
- **The channel sliders are RGBA, not HSL,** so what they show and what the
  status strip's hex reports cannot drift, and no colour-space round trip can
  lose a value.
- **The generated palette regenerates into `Palette`,** rather than living
  beside it. Painting, the swatches, the theme, the status strip and the save
  path all stay unaware that there are two specifications; the only code that
  knows is the applier that swaps them. The cost is that a hand edit has to be
  refused while the ramps are in charge.
- **Out-of-gamut Oklab loses chroma, not hue.** Clipping each sRGB channel is
  cheaper but makes the far end of a chroma sweep repeat one corner colour.
- **The edited ramp is derived from the selection,** not stored. Clicking a
  swatch is therefore also how you pick which ramp the sliders act on, and there
  is no second piece of state to keep in step.
- **The sidecar carries both specifications, and which one was live** — the
  presence of the parked palette says so, mirroring `PaletteMode`'s own
  invariant rather than adding a boolean that could contradict it. Saving in one
  mode does not cost you the other.
- **The sliders are a view of the palette,** never a second store: they emit a
  request and read their position back from `Palette`, even mid-drag.
- **Reordering is two arrow buttons, not drag-and-drop.** It reuses the existing
  `Palette::reorder_color`, goes through a request like everything else, and the
  dev harness can drive it — so it is screenshot-testable, which a drag is not.
- **The two chrome roots declare their `GlobalZIndex`.** `ui_stack_system`
  stable-sorts roots by z, so equal keys fall back to archetype order; leaving
  them equal let an unrelated widget flip the sidebar over the status strip.
- **Scroll keys:** Arrow Left/Right *and* A/D.
- **Coordinate system:** top-left origin for atlas coordinates.
- **Colour representation:** `Srgba`, with `ColorData`/`PaletteData` mirrors for
  serde.

---

## Open Questions

- [ ] Should the atlas support multiple tile rows in the UI? (storage already does)
- [ ] Runtime zoom, and should `--scale` then become a starting value only?
- [ ] Where should undo/redo re-enter — a `history` resource fed by the same
      request messages?
- [ ] A checkerboard behind transparent pixels, and how to show the transparent
      swatch as something other than an empty slot

---

## Dependencies

```toml
[dependencies]
bevy = { version = "0.20", features = ["2d", "bevy_ui", "bevy_feathers"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
image = "0.25"

[dev-dependencies]
tempfile = "3.10"
```

---

*Last updated: 2026-07-26*
*Rebuilt around the light main workflow: seven plugins, one source of truth, and
a scripted screenshot harness. See VALIDATION.md for how to check it.*
*Project lead: jbieber*
