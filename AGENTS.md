# Bevy Sprite Atlas Editor - Project Specification

## Overview

A Bevy-based editor for creating and editing sprite atlases. A sprite atlas is a single image file split into multiple square tiles. The editor provides pixel-level editing with a color palette, tile management, and undo/redo functionality.

**Technology Stack:**
- **Engine:** Bevy 0.19
- **UI:** Feathers (Bevy UI library)
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

### UI Layout (Feathers)

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
│       ├── ui.mml              # Feathers UI structure
│       └── file_io.mml         # Save/load flow
├── src/
│   ├── main.rs                 # Bevy app entry point
│   ├── config.rs               # App configuration
│   ├── atlas.rs                # Atlas data model
│   ├── palette.rs              # Color palette
│   ├── editor.rs               # Drawing & viewport logic
│   ├── history.rs              # Undo/redo stack
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── palette_ui.rs       # Palette sidebar
│   │   ├── viewport.rs         # Atlas viewport rendering
│   │   └── toolbar.rs          # Top/bottom UI elements
│   └── file_io.rs              # Save/load implementation
└── assets/
    └── default_palette.json    # Default color palette
```

---

## Data Models

### Atlas
```rust
struct Atlas {
    tile_size: u32,        // 8 (pixels per tile, square)
    width_in_tiles: u32,   // Number of tiles wide
    height_in_tiles: u32,  // Number of tiles tall (v1: always 1?)
    pixels: Vec<Color>,    // Flat array: [tile0_row0, tile0_row1, ... tile1_row0, ...]
}
```

### Palette
```rust
struct Palette {
    colors: Vec<Color>,    // Ordered list of RGBA colors
    selected_index: Option<usize>,  // Currently selected color
}

// Color = bevy::prelude::Color (RGBA, f32 0.0-1.0)
```

### History
```rust
struct History {
    undo_stack: VecDeque<Action>,  // Max 100 entries
    redo_stack: VecDeque<Action>,
}

enum Action {
    SetPixel { position: (u32, u32), old_color: Color, new_color: Color },
    AddTile { tile_index: u32 },
    AddColor { color: Color, index: usize },
    RemoveColor { color: Color, index: usize },
    ReorderColor { from: usize, to: usize },
}
```

---

## Input Mapping

| Input | Action |
|-------|--------|
| Mouse Left Click (viewport) | Set pixel to active palette color |
| Mouse Left Click (palette) | Select color |
| `\u00F6` (o umlaut) | Scroll viewport left |
| `#` | Scroll viewport right |
| (TBD) | Add tile to right |
| (TBD) | Add color to palette |
| (TBD) | Remove color from palette |
| (TBD) | Reorder palette |
| (TBD) | Undo |
| (TBD) | Redo |
| (TBD) | Save |
| (TBD) | Load |

---

## Rendering

### Pixel Scale
- 1 atlas pixel = 64 screen pixels
- 1 tile (8x8) = 512x512 screen pixels
- atlas pixel at (x,y) renders at screen position:
  - x_screen = x * 64 - viewport_offset_x
  - y_screen = y * 64 - viewport_offset_y

### Colors
- Transparent: `Color::rgba(0.0, 0.0, 0.0, 0.0)`
- Palette colors: Stored as `Color` (Bevy's RGBA)
- Pixel rendering: Solid color rectangles (64x64)

---

## Implementation Priority

### Phase 1: Core (MVP)
1. Bevy app scaffold with window setup
2. Atlas data structure (1 tile, 8x8)
3. Basic viewport rendering (pixels visible at 64x64)
4. Color palette with default colors
5. Click to draw (set pixel color)
6. Active color selection

### Phase 2: Navigation
1. Scrolling with \u00F6 and # keys
2. Viewport offset management
3. Add tile button/functionality

### Phase 3: Palette Management
1. Add color UI
2. Remove color UI
3. Reorder colors (drag-and-drop)
4. Palette persistence with atlas

### Phase 4: Undo/Redo
1. History stack implementation
2. Undo/redo actions
3. Integration with all mutable operations

### Phase 5: File I/O
1. Save atlas as PNG + JSON
2. Load atlas from PNG + JSON
3. File picker UI (Feathers)

### Phase 6: Polish
1. Visual feedback (active color border, hover states)
2. Status bar
3. Error handling

---

## Open Questions / Decisions

- [ ] Should atlas support multiple rows (height > 1 tile) in v1?
- [ ] Should there be a "background" color separate from transparent?
- [ ] How to handle palette color selection when drawing?
- [ ] Should clicking a pixel with the active color do nothing, or toggle?
- [ ] Feathers version compatibility with Bevy 0.19?
- [ ] Key bindings for: add tile, add color, remove color, undo, redo, save, load

---

## Dependencies

```toml
[dependencies]
bevy = "0.19"
feathers = "0.19"  # Need to verify version compatibility
# May need additional crates for:
# - PNG encoding/decoding (image crate)
# - JSON serialization (serde, serde_json)
```

---

*Last updated: 2026-07-19*
*Project lead: jbieber*
