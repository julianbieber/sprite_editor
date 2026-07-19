# Bevy Sprite Atlas Editor - Project Specification

## Overview

A Bevy-based editor for creating and editing sprite atlases. A sprite atlas is a single image file split into multiple square tiles. The editor provides pixel-level editing with a color palette, tile management, and undo/redo functionality.

**Technology Stack:**
- **Engine:** Bevy 0.19
- **UI:** Bevy sprites (Feathers planned for future)
- **Language:** Rust

---

## Core Requirements

### Atlas Structure
- **Tile dimensions:** Square, configurable at startup (default: 8x8 pixels)
- **Initial atlas:** 1 tile (8x8 pixels)
- **Tile growth:** Adding a tile appends one fully transparent tile to the right (grows atlas width by 1 tile)
- **Pixel display:** Each atlas pixel rendered as 64x64 screen pixels
  - Therefore: each 8x8 tile = 512x512 screen pixels
- **Scrolling:** Horizontal only, via keyboard keys
  - `o umlaut` (\u00F6) key: scroll left
  - `#` key: scroll right

### Color Palette
- **Capacity:** Unlimited colors
- **Visual representation:** Color swatches (clickable)
- **Selection:** Active color has a visible border
- **Operations required:**
  - Add color
  - Remove color
  - Reorder colors (drag-and-drop or buttons)
  - Select color (click)
- **Default palette:** Pre-loaded with standard colors
- **Storage:** Palette saved with atlas (per-project)
- **Initial draw color:** Fully transparent (rgba(0,0,0,0))

### Drawing
- **Granularity:** Pixel-by-pixel (click to set pixel)
- **Tools (v1):** Single-pixel pencil only
- **Color source:** Selected palette color (active with border)
- **Eraser:** Not explicitly required; transparent can be a palette color

### Undo/Redo
- **Required:** Yes
- **History depth:** 100 actions
- **Actions to track:**
  - Pixel color changes
  - Tile additions
  - Palette modifications (add/remove/reorder)

### File I/O
- **Format:** PNG + JSON (separate files)
- **PNG:** The atlas image data
- **JSON:** Metadata including:
  - Tile size (width)
  - Atlas dimensions (tiles wide, tiles tall)
  - Color palette (list of RGBA colors)
  - Optional: undo history? (TBD)

### UI Layout

```
+------------------+-----------------------------+
|                  |                             |
|  Color Palette   |   Atlas Viewport            |
|  (left sidebar)  |   (scrollable)              |
|                  |                             |
|  +------------+   |   +----+----+----+        |
|  | [COLOR]    |   |   |    |    |    | ...   |
|  +------------+   |   |    |    |    |        |
|  | [COLOR]    |   |   +----+----+----+        |
|  +------------+   |                             |
|  | ...         |   |   [Atlas renders here]     |
|  +------------+   |                             |
|  | [+ Add]    |   |                             |
|  +------------+   |                             |
+------------------+-----------------------------+
                      [Status: X,Y | Color: #RRGGBBAA]
```

---

## Project Structure

```
bevy_sprite_editor/
├── AGENTS.md                    # This file
├── Cargo.toml                   # Dependencies
├── docs/
│   └── mml/                     # Mental Model Language specs
│       ├── atlas.mml           # Atlas data & operations
│       ├── palette.mml         # Color palette system
│       ├── editor.mml          # Pixel editing & viewport
│       ├── history.mml         # Undo/redo system
│       ├── ui.mml              # UI structure
│       └── file_io.mml         # Save/load flow
├── src/
│   ├── main.rs                 # Bevy app entry point
│   ├── config.rs               # App configuration
│   ├── atlas.rs                # Atlas data model
│   ├── palette.rs              # Color palette
│   ├── editor.rs               # Drawing & viewport logic
│   ├── history.rs              # Undo/redo stack
│   ├── file_io.rs              # Save/load implementation
│   └── ui/
│       ├── mod.rs              # UI module
│       ├── palette_ui.rs       # Palette sidebar
│       └── viewport.rs         # Atlas viewport rendering
└── assets/
    └── default_palette.json    # Default color palette (planned)
```

---

## Data Models

### Atlas
```rust
struct Atlas {
    tile_size: u32,        // 8 (pixels per tile, square)
    width_in_tiles: u32,   // Number of tiles wide
    height_in_tiles: u32,  // Number of tiles tall
    pixels: Vec<Srgba>,    // Flat array: [tile0_row0, tile0_row1, ... tile1_row0, ...]
}
```

### Palette
```rust
struct Palette {
    colors: Vec<Srgba>,    // Ordered list of RGBA colors
    selected_index: Option<usize>,  // Currently selected color
}

// Color = bevy::prelude::Srgba (RGBA, f32 0.0-1.0)
```

### History
```rust
struct History {
    undo_stack: VecDeque<Action>,  // Max 100 entries
    redo_stack: VecDeque<Action>,
}

enum Action {
    SetPixel { x: u32, y: u32, old_color: Srgba, new_color: Srgba },
    AddTile,
    RemoveTile { pixels: Vec<Srgba> },
    AddColor { color: Srgba, index: usize },
    RemoveColor { color: Srgba, index: usize },
    ReorderColor { from: usize, to: usize },
    SetColor { index: usize, old_color: Srgba, new_color: Srgba },
}
```

---

## Input Mapping

| Input | Action |
|-------|--------|
| Mouse Left Click (viewport) | Set pixel to active palette color |
| Mouse Left Click (palette) | Select color |
| `\u00F6` (o umlaut) | Scroll viewport left |
| `#` / `Digit3` | Scroll viewport right |
| `Space` | Add tile to right |
| `Backspace` | Remove tile from right |
| `Ctrl+Z` | Undo last action |
| `Ctrl+Y` | Redo last undone action |
| `Ctrl+S` | Save project (logs to console, file picker TBD) |
| `Ctrl+O` | Load project (logs to console, file picker TBD) |
| (TBD) | Add color to palette |
| (TBD) | Remove color from palette |
| (TBD) | Reorder palette |

---

## Rendering

### Pixel Scale
- 1 atlas pixel = 64 screen pixels
- 1 tile (8x8) = 512x512 screen pixels
- atlas pixel at (x,y) renders at screen position:
  - x_screen = x * 64 - viewport_offset_x
  - y_screen = -y * 64 + viewport_offset_y (flipped because atlas uses top-left origin, Bevy Camera2d has y-up)

### Colors
- Transparent: `Srgba::new(0.0, 0.0, 0.0, 0.0)`
- Palette colors: Stored as `Srgba` (Bevy's RGBA)
- Pixel rendering: Solid color sprites (64x64)

---

## Implementation Status

### ✅ Phase 1: Core (MVP) - COMPLETED
1. ✅ Bevy app scaffold with window setup
2. ✅ Atlas data structure (1 tile, 8x8)
3. ✅ Basic viewport rendering (pixels visible at 64x64)
4. ✅ Color palette with default colors
5. ✅ Click to draw (set pixel color)
6. ✅ Active color selection

### ✅ Phase 2: Navigation - COMPLETED
1. ✅ Scrolling with \u00F6 and # keys
2. ✅ Viewport offset management
3. ✅ Add tile functionality (Space key)
4. ✅ Remove tile functionality (Backspace key)

### ✅ Phase 4: Undo/Redo - COMPLETED
1. ✅ History stack implementation
2. ✅ Undo/redo actions (Ctrl+Z, Ctrl+Y)
3. ✅ Integration with pixel drawing
4. ✅ Integration with tile operations

### ✅ Phase 7: Layout - COMPLETED
1. ✅ Palette panel positioned on left side
2. ✅ Viewport offset to avoid overlap with palette
3. ✅ Swatches as children of panel entity
4. ✅ Viewport clamping accounts for palette width
5. ✅ Viewport y-axis clamping fixed to ensure atlas is fully visible

### ⚠️ Phase 3: Palette Management - PARTIAL
1. ❌ Add color UI
2. ❌ Remove color UI
3. ❌ Reorder colors (drag-and-drop)
4. ✅ Palette logic implementation
5. ✅ Palette persistence with atlas (code ready)

### ⚠️ Phase 5: File I/O - PARTIAL
1. ✅ Save atlas as PNG + JSON (code ready)
2. ✅ Load atlas from PNG + JSON (code ready)
3. ❌ File picker UI (Feathers)
4. ✅ Save/Load keyboard shortcuts (Ctrl+S, Ctrl+O) - logs to console, needs file picker
5. ⚠️ Default save/load directory utility (code ready in file_io.rs)

### ✅ Phase 7: Status Bar - COMPLETED
1. ✅ Status bar background display
2. ⚠️ Status text (pending Feathers UI integration)

### ⚠️ Phase 6: Polish - PARTIAL
1. ✅ Visual feedback (active color border, hover states)
2. ✅ Status bar background
3. ✅ Error handling (basic implementation done)
4. ❌ Grid overlay
5. ❌ Zoom functionality

### ⚠️ Phase 6: Polish - PARTIAL
1. ✅ Visual feedback (active color border, hover states)
2. ❌ Status bar
3. ⚠️ Error handling (basic implementation done)

---

## Decisions Made

✅ **Feathers version:** Using Bevy 0.19 with built-in `bevy_feathers` feature (currently using Bevy sprites for rendering)
✅ **Coordinate system:** Top-left origin for atlas coordinates (more intuitive for image editing)
✅ **Color representation:** Using `Srgba` variant with direct field access (red, green, blue, alpha)
✅ **Rendering:** Using Bevy 0.19's `Sprite::from_color()` for pixel rendering
✅ **Key bindings:**
   - Scroll: O (ö) for left, #/Digit3 for right
   - Undo/Redo: Ctrl+Z / Ctrl+Y
   - Tiles: Space to add, Backspace to remove
   - Drawing: Left click on viewport
   - Color selection: Left click on palette

---

## Open Questions / Decisions

- [ ] Should atlas support multiple rows (height > 1 tile) in v1? (Currently: yes, code supports it)
- [ ] Should there be a "background" color separate from transparent? (Currently: transparent is a palette color)
- [ ] How to handle palette color selection when drawing? (Currently: first color selected by default)
- [ ] Should clicking a pixel with the active color do nothing, or toggle? (Currently: sets to that color regardless)
- [ ] Key bindings for: add color to palette, remove color from palette, save, load

---

## Dependencies

```toml
[dependencies]
bevy = { version = "0.19", features = ["2d", "bevy_ui", "bevy_feathers"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
image = "0.25"
```

---

*Last updated: 2026-07-19*
*Recent fix: Window positioning and viewport y-axis clamping*
*Project lead: jbieber*
*Status: MVP functional - core editing features working*
