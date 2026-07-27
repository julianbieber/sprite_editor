# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A Bevy 0.19 desktop editor for sprite atlases: one image split into square tiles,
edited pixel by pixel with a colour palette. Single binary crate, no workspace.

The supported workflow is: open a file from a CLI argument, select a palette
colour, click to paint pixels, edit the palette (as a hand-edited list, or as a
set of generated Oklab ramps — a button switches), add tiles on the right, scroll
a wide atlas, save.
**Undo/redo is deliberately not wired up** — `src/history.rs` is tested and
compiles but nothing calls it (`#[allow(dead_code)]` on the module declaration).
Note that palette edits are therefore not recorded into it either. Zoom and a
file picker are out of scope so far, as is reordering by dragging a swatch —
the arrow buttons do it instead.

Design specs live outside the repo, one `.hmml` per plugin:
`~/fun_repos/hobby-mimisbrunnr/notes/bevy_sprite_editor/initial_editor/fixup/`.
`AGENTS.md` is the original project spec and `VALIDATION.md` the manual test
sheet; both are kept current, but `src/` wins if they disagree.

## Commands

```bash
cargo check --all-targets        # fast feedback
cargo test                       # 176 unit tests, in #[cfg(test)] mods inside src/*.rs
cargo test a_wide_atlas          # single test by name
cargo test palette::             # one module
cargo run -- art/demo            # open art/demo.png + art/demo.atlas.json
cargo run -- --help              # flags and script verbs

./tests/test_application.sh      # check + test + release build + four scripted screenshot runs
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
`scroll+[N]`, `scroll-[N]`, `color #rrggbb[aa]`, `color+`, `color-`, `move-`,
`move+`, `mode`, `axis L|a|b`, `ramp FIELD VALUE`, `ramp+`, `ramp-`, `save`,
`wait [N]`; one step per frame. The `color`, `move`, `axis` and `ramp` verbs
stand in for the sidebar's sliders and buttons — a script has no pointer to drag
with — and act on the selected entry (or the ramp it came from), clamping at the
ends exactly as those widgets do. A ramp `FIELD` is `L`, `a`, `b`, `from`, `to`
or `steps`, spelled exactly as the sidebar captions it. The capture
uses Bevy's own `Screenshot` API, so no window manager, focus or `sleep` is
involved and the result is deterministic for a build. `--warmup-frames` (default
30) waits out the window manager's initial resize before the first step, and
`--settle-frames` (default 30) waits for the last edit to reach the screen.
`tests/shot.sh` falls back to `xvfb-run` when `DISPLAY` is unset.

The status strip is deliberately informative so a screenshot is
self-describing — file name, `*` when unsaved, hovered pixel, its tile, the
selected colour as hex, `[selected/count]` for the palette, the palette mode
(and, in `oklab`, which ramp of how many and what it sweeps), and the atlas size.
Read values off it rather than guessing.

## Architecture

Seven plugins, composed in `main.rs`. Two rules explain most of the layout:

**1. Input never mutates state.** `input.rs` and `dev_harness.rs` only emit
request messages (`PaintPixelRequested`, `SelectColorRequested`,
`AddTileRequested`, `ScrollRequested`, `SaveRequested`,
`SetPaletteColorRequested`, `AddPaletteColorRequested`,
`RemovePaletteColorRequested`, `MovePaletteColorRequested`,
`TogglePaletteModeRequested`, `SetRampRequested`, `AddRampRequested`,
`RemoveRampRequested`); `document.rs` and
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
| `palette.rs` | `Palette` and `PaletteMode`, plus `ColorData`/`PaletteData` serde mirrors |
| `oklab.rs` | `OklabRamp`/`OklabRamps`/`RampField` — the generated palette, and the gamut fit |
| `document.rs` | request messages and the systems that apply them; `DocumentPath` |
| `file_io.rs` | PNG + JSON codec (both palette specifications), startup load, save on request |
| `canvas_view.rs` | camera viewport fitting, scroll offset, cursor → pixel |
| `canvas_render.rs` | the atlas texture, sprite placement, tile grid |
| `ui_chrome.rs` | Feathers palette sidebar (mode button, swatches, both editors) and status strip |
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
so a no-op paint must not bump it. A palette edit changes what a save writes but
no pixel, so it calls `mark_unsaved()` instead — bumping `revision` there would
re-upload the texture on every frame of a slider drag.

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

### Two ways to specify the palette, one `Palette`

The mode button at the top of the sidebar switches between a hand-edited list and
Oklab ramps. It does **not** introduce a second palette. `oklab` mode rewrites
`Palette.colors` from `OklabRamps` and parks the hand-edited palette whole in
`PaletteMode::stash`; switching back puts it straight back, selection and all.
Everything downstream — painting, the theme tokens, the swatches, the status
strip, `file_io` — is unaware there are two specifications.

That is what makes the feature cheap, and it buys exactly one obligation:
`apply_palette_edits` **refuses** in `oklab` mode, and drains the requests rather
than leaving them buffered. The colours are derived, so a hand edit would survive
until the next regenerate and then vanish — worse than never applying.

Two things about ramps are worth knowing before touching `oklab.rs`:

- **Out-of-gamut colours lose chroma, not channels.** Oklab is wider than sRGB
  and a chroma sweep runs off the end of it. Clipping each channel would make the
  last few swatches of a ramp identical; `to_srgba_in_gamut` binary-searches the
  chroma down to the gamut boundary instead, keeping hue and lightness.
- **The edited ramp is derived from the selection** (`ramps.ramp_of(selected)`),
  never stored. Clicking a swatch is therefore also how you choose which ramp the
  sliders act on, and there is no second index to keep in step. Every generated
  widget goes through `edited_ramp`.

The sidebar rebuilds on *shape*, not on change, and there are two shapes:
`BuiltSwatchRows` (a row length per ramp — the total is not enough, since two
ramps can trade steps) and `BuiltEditor` (mode, ramp index, and the ramp's axis,
because the axis decides which two values get sliders). Both are resources rather
than `Local`s so `build_chrome` can seed them from `Startup`.

**The sync systems run before the rebuild systems**, and that ordering is load
bearing. Both sides queue commands; a mode switch retunes the sliders it is about
to replace, so a rebuild-then-sync order writes to a despawned entity and panics.

The sidecar carries **both** specifications and which one was live: `palette` is
whatever was being drawn with, and the `generated` block holds the ramps plus,
when they were in charge, the parked hand-edited palette. The parked one's
presence is what records the mode — the same invariant `PaletteMode` keeps, so
the codec cannot encode a state the resource could not hold. The block is
optional and the format version stays `1.0`, because the field is additive in
both directions. On load the generated palette is **rebuilt from the ramps**
rather than read back out of the stored colours, so editing the JSON's ramps by
hand works and `Palette` matches `OklabRamps` from the first frame; anything
outside the editor's limits is clamped by `OklabRamps::sanitised`.

One consequence worth remembering: a ramp edit dirties the document *even in
`list` mode*, because the sidecar stores the ramps whichever mode is live.

### The palette sidebar goes through the Feathers theme

Swatch colours live in `UiTheme` as one `ThemeToken` per palette index
(`ColorPalette::{i}`), generated at startup from the *loaded* palette by
`install_palette_theme`. There is no hardcoded colour list. The consequence is a
startup ordering constraint, declared in `main.rs` because no single plugin owns
it:

```
seed_document (PreStartup) → DocumentLoad (file_io) → ChromeBuild (ui_chrome)
```

A file's own palette must reach the theme before the swatches spawn. The same
derivation holds at runtime, and the palette is editable, so it costs two
different things:

- **Recolouring** rewrites that entry's token. Feathers repaints every
  `ThemeBackgroundColor` whenever `UiTheme` changes, so the swatch follows and no
  entity is touched. `sync_palette_theme` does this, guarded on
  `palette.is_changed()` — touching `UiTheme` every frame would repaint the whole
  tree every frame.
- **Adding or removing** changes how many tokens exist, which no repaint can
  express, so `rebuild_swatches` despawns the swatches and respawns them. It runs
  *after* the theme sync, since a new swatch reads its token on insert. Both are
  guarded on the palette's length actually changing.

Keeping Feathers here was a deliberate choice by the repo owner over plain
`Node` + `BackgroundColor`. Don't "simplify" it away.

### The two chrome roots declare their z-order

The sidebar is full-height and the status strip is full-width, so they overlap in
the bottom-left corner. The strip wins — which is why the sidebar reserves
`STATUS_BAR_HEIGHT` of bottom padding — and both carry an explicit
`GlobalZIndex` (`SIDEBAR_Z` / `STATUS_STRIP_Z`) to say so.

Leave them equal and it still *looks* right, until it doesn't: `ui_stack_system`
stable-sorts roots by `(GlobalZIndex, ZIndex)`, so equal keys fall back to query
iteration order, which is archetype order rather than spawn order. Adding two
buttons inside the sidebar flipped it once and hid the file name behind the
sidebar. If you add a third chrome root, give it a z.

### The channel sliders are a view, not a store

The four `FeathersColorSlider`s under the swatches edit the *selected* entry.
They never hold the colour: dragging one emits `SetPaletteColorRequested` with
the **whole** colour it wants, and `sync_channel_sliders` writes the position
back from `Palette` — so even mid-drag the thumb shows what the palette accepted,
not what the pointer asked for. `SliderValue` is an immutable component, so that
write goes through `Commands::insert`, and both it and `SliderBaseColor` are only
written when they actually differ; an unconditional write would count as a change
and make Feathers rebuild every track gradient every frame.

They are RGBA rather than HSL deliberately: that is how a colour is stored and
how the status strip reports it, so the sliders and the hex cannot drift.

`on_channel_dragged`, `on_add_clicked` and `on_remove_clicked` are named
functions rather than closures inside the `bsn!` blocks so the tests can trigger
them on a bare entity — the harness has no pointer, so this is the only part of
the sidebar a screenshot cannot exercise. Note that `ValueChange<T>` is generic:
triggering one in a test with an untyped `0.5` infers `f64` and the `f32`
observer silently never fires.

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
| Wheel over the swatches | Scroll them, when the palette overflows the sidebar |
| Click `<` / `>` | Move the selected entry one slot; clamps at the ends |
| Drag an R/G/B/A slider | Set that channel of the selected colour |
| Click `+` | Append an opaque white entry and select it |
| Click `-` | Drop the selected entry (refused for the last one) |
| Click `palette: …` / `M` | Switch between the `list` and `oklab` specifications |
| Click `L` / `a` / `b` | Sweep that value in the edited ramp; resets its range |
| Drag a ramp slider | Set one of the edited ramp's six numbers |
| Click `+` / `-` (oklab) | Duplicate the edited ramp and select it / drop it |
| `Space` | Add a tile on the right |
| `Arrow Left` / `A` | Scroll one tile left |
| `Arrow Right` / `D` | Scroll one tile right |
| `Ctrl+S` | Save (to the CLI path, else `./untitled.png`) |

Dragging emits at most one paint per pixel entered, and a whole drag is one
`revision` bump per frame.
