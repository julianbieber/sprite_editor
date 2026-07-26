# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A Bevy 0.19 desktop editor for sprite atlases: one image split into square tiles,
edited pixel by pixel with a colour palette. Single binary crate, no workspace.

The supported workflow is: open a file from a CLI argument, select a palette
colour, click to paint pixels, add tiles on the right, scroll a wide atlas, save.
**Undo/redo is deliberately not wired up** — `src/history.rs` is tested and
compiles but nothing calls it (`#[allow(dead_code)]` on the module declaration).
Palette editing (add/remove/reorder), zoom and a file picker are also out of
scope so far.

Design specs live outside the repo, one `.hmml` per plugin:
`~/fun_repos/hobby-mimisbrunnr/notes/bevy_sprite_editor/initial_editor/fixup/`.
`AGENTS.md` is the original project spec and `VALIDATION.md` the manual test
sheet; both are kept current, but `src/` wins if they disagree.

## Commands

```bash
cargo check --all-targets        # fast feedback
cargo test                       # 89 unit tests, in #[cfg(test)] mods inside src/*.rs
cargo test a_wide_atlas          # single test by name
cargo test palette::             # one module
cargo run -- art/demo            # open art/demo.png + art/demo.atlas.json
cargo run -- --help              # flags and script verbs

./tests/test_application.sh      # check + test + release build + two scripted screenshot runs
./tests/shot.sh OUT.png "SCRIPT" [-- EDITOR ARGS]
```

The first build compiles all of Bevy and takes several minutes.

### Seeing a change

Do not ask the user to run the app to find out what a change looks like. The
editor screenshots itself:

```bash
./tests/shot.sh /tmp/shot.png "select 3; paint 2,3; paint 3,3; tile+; scroll+"
```

then `Read` the PNG. Script verbs: `select N`, `paint X,Y`, `tile+[N]`,
`scroll+[N]`, `scroll-[N]`, `save`, `wait [N]`; one step per frame. The capture
uses Bevy's own `Screenshot` API, so no window manager, focus or `sleep` is
involved and the result is deterministic for a build. `--warmup-frames` (default
30) waits out the window manager's initial resize before the first step, and
`--settle-frames` (default 30) waits for the last edit to reach the screen.
`tests/shot.sh` falls back to `xvfb-run` when `DISPLAY` is unset.

The status strip is deliberately informative so a screenshot is
self-describing — file name, `*` when unsaved, hovered pixel, its tile, the
selected colour as hex, and the atlas size. Read values off it rather than
guessing.

## Architecture

Seven plugins, composed in `main.rs`. Two rules explain most of the layout:

**1. Input never mutates state.** `input.rs` and `dev_harness.rs` only emit
request messages (`PaintPixelRequested`, `SelectColorRequested`,
`AddTileRequested`, `ScrollRequested`, `SaveRequested`); `document.rs` and
`file_io.rs` are the only places that apply them. This is what makes the
screenshot harness trustworthy — it drives the same messages a human does, so a
passing screenshot cannot hide broken interaction. If you add an interaction, add
a message; do not reach for `ResMut<AtlasDocument>` from an input system.

**2. One coordinate authority.** `canvas_view.rs` owns the only mapping between
screen and atlas space. Everything else asks it.

| Module | Owns |
|---|---|
| `main.rs` | plugin + startup order, the two cameras, `CanvasCamera`/`UiCamera` markers |
| `cli.rs` | argv → `LaunchOptions` (hand-rolled; no clap) |
| `atlas.rs` | `AtlasDocument` — the pixel grid |
| `palette.rs` | `Palette`, plus `ColorData`/`PaletteData` serde mirrors |
| `document.rs` | request messages and the systems that apply them; `DocumentPath` |
| `file_io.rs` | PNG + JSON codec, startup load, save on request |
| `canvas_view.rs` | camera viewport fitting, scroll offset, cursor → pixel |
| `canvas_render.rs` | the atlas texture, sprite placement, tile grid |
| `ui_chrome.rs` | Feathers palette sidebar and status strip |
| `input.rs` | keys and mouse → requests |
| `dev_harness.rs` | script → requests → screenshot → exit |
| `history.rs` | undo/redo. Compiles, tested, **unwired** |

### Pixel storage is row-major over the whole image

`AtlasDocument.pixels` is indexed `y * width_in_pixels + x`. Tiles are purely a
*view* concept — they decide where grid lines go and nothing else. This makes the
texture upload and the PNG encode straight copies in both directions.

Consequences worth knowing: `add_tile_to_right` is a splice per row (not an
append), walking rows back to front so earlier insertion points stay valid;
`atlas.rs`'s tests cover exactly that, because it is the one place a silent
corruption could hide. An earlier layout was per-tile-contiguous while the PNG
encoder assumed row-major, which corrupted every atlas wider than one tile.

`set_pixel_at` returns whether anything changed, and `touch()` is the single
place `revision`/`unsaved` move. `revision` is what the renderer diffs against,
so a no-op paint must not bump it.

### Two cameras, and their order matters

- **Canvas camera** — `order: 0`, `RenderLayers::layer(CANVAS_LAYER)`, clears to
  the background, and its `viewport` is refitted every frame to the window region
  the chrome leaves free.
- **UI camera** — `order: 1`, `ClearColorConfig::None`, `IsDefaultUiCamera`, full
  window, draws bevy_ui over the top.

**Do not flip this order.** Putting the UI camera first and the canvas second
produces an entirely black window — that cost a debugging cycle. The canvas must
be on its own render layer or the full-window UI camera also draws the atlas
across the sidebar. Gizmos are confined to the same layer via
`GizmoConfig::render_layers`.

The viewport is the point of the whole arrangement: `viewport_to_world_2d`
divides by the viewport rect, so canvas world space owes nothing to
`SIDEBAR_WIDTH`. No placement formula outside `canvas_view.rs` should ever
mention it. `Camera.viewport` is in **physical** pixels (multiply by
`window.scale_factor()`); `CanvasView.rect_*` and everything bevy_ui are
**logical**.

### The coordinate contract

`CanvasView` exposes `atlas_left()`, `atlas_top()`, `pixel_scale`, plus
`pixel_world_min()` and `world_to_atlas()`. `canvas_render` places the sprite with
the first three; `track_hovered_pixel` inverts them. `placement_and_hit_testing_are_inverses`
in `canvas_view.rs` is the test that pins this — keep it passing and clicking
cannot drift from drawing. Atlas coordinates are top-left origin; Bevy 2D is
y-up and centre-origin, hence the flip in `atlas_top()`.

The canvas sprite is one entity with `Anchor::TOP_LEFT`, so its translation *is*
the atlas origin. Its texture is reuploaded only when `AtlasDocument.revision`
differs from `CanvasTexture.revision`.

### The palette sidebar goes through the Feathers theme

Swatch colours live in `UiTheme` as one `ThemeToken` per palette index
(`ColorPalette::{i}`), generated at startup from the *loaded* palette by
`install_palette_theme`. There is no hardcoded colour list. The consequence is a
startup ordering constraint, declared in `main.rs` because no single plugin owns
it:

```
seed_document (PreStartup) → DocumentLoad (file_io) → ChromeBuild (ui_chrome)
```

A file's own palette must reach the theme before the swatches spawn. Adding or
removing a colour at runtime would need the theme rebuilt *and* the sidebar
respawned — which is why palette editing is out of scope for now.

Keeping Feathers here was a deliberate choice by the repo owner over plain
`Node` + `BackgroundColor`. Don't "simplify" it away.

## Conventions

- Module layout is uniform: `//!` docs, `// ===` banner comments separating
  Data Structures / Constructors / Systems / Tests, then `#[cfg(test)] mod tests`
  at the bottom. Follow it.
- `config.rs` is the only home for constants and colours. Everything in it is
  used; `#[allow(dead_code)]` marks the few items that exist only for `history`
  or the tests, each with a reason. Keep it that way rather than adding a
  crate-level allow.
- Colours are `Srgba` end to end (f32 0.0–1.0); transparent is
  `config::transparent()`, and it is palette entry 0 so erasing needs no tool.
- Bevy 0.19 calls buffered events **messages**: `#[derive(Message)]`,
  `MessageWriter`/`MessageReader`, `app.add_message::<T>()`. `Event` is for
  observers (`On<Activate>`).

### Testing systems

`MinimalPlugins` does not flush message buffers, so
`Messages::<T>::iter_current_update_messages()` accumulates across
`app.update()` calls and assertions on it are misleading. Read messages with a
`MessageReader` in a recording system instead — see `Recorded`/`record` in
`input.rs`'s tests. That is also closer to how a real consumer reads them.

## Key bindings

| Input | Action |
|---|---|
| Left click / drag on canvas | Paint the hovered pixel with the selected colour |
| Left click a swatch | Select that colour |
| `Space` | Add a tile on the right |
| `Arrow Left` / `A` | Scroll one tile left |
| `Arrow Right` / `D` | Scroll one tile right |
| `Ctrl+S` | Save (to the CLI path, else `./untitled.png`) |

Dragging emits at most one paint per pixel entered, and a whole drag is one
`revision` bump per frame.
