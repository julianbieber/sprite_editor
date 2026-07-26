//! Configuration constants and small shared helpers.

use bevy::prelude::*;

// ============================================================================
// DISPLAY CONFIGURATION
// ============================================================================

/// Screen pixels per atlas pixel, unless `--scale` overrides it.
///
/// 32 puts an 8x8 tile at 256 screen pixels, so roughly four tiles fit next to
/// the sidebar in the default window — wide enough that scrolling is useful.
pub const DEFAULT_PIXEL_SCALE: f32 = 32.0;

/// Pixels per side of a tile for a brand new document.
pub const DEFAULT_TILE_SIZE: u32 = 8;

/// Below this scale the per-pixel grid is more noise than help.
pub const PIXEL_GRID_MIN_SCALE: f32 = 12.0;

// ============================================================================
// WINDOW CONFIGURATION
// ============================================================================

pub const WINDOW_TITLE: &str = "Bevy Sprite Atlas Editor";
pub const DEFAULT_WINDOW_WIDTH: u32 = 1280;
pub const DEFAULT_WINDOW_HEIGHT: u32 = 800;

// ============================================================================
// UI CONFIGURATION
// ============================================================================

/// Width of the palette sidebar, in logical pixels.
pub const SIDEBAR_WIDTH: f32 = 200.0;

/// Height of the status strip at the bottom, in logical pixels.
pub const STATUS_BAR_HEIGHT: f32 = 28.0;

pub const SWATCH_SIZE: f32 = 44.0;
pub const SWATCH_GAP: f32 = 6.0;
pub const SWATCH_BORDER: f32 = 2.0;

// ============================================================================
// COLOURS
// ============================================================================

/// Cleared behind everything; also what shows through transparent pixels.
pub const CANVAS_BACKGROUND: Color = Color::srgb(0.11, 0.11, 0.13);
pub const SIDEBAR_BACKGROUND: Color = Color::srgb(0.16, 0.16, 0.19);
pub const STATUS_BACKGROUND: Color = Color::srgb(0.09, 0.09, 0.11);
pub const STATUS_TEXT: Color = Color::srgb(0.78, 0.78, 0.82);
pub const STATUS_FONT_SIZE: f32 = 13.0;

/// Tile boundaries: bright enough to read over an opaque pixel.
pub const TILE_GRID_COLOR: Color = Color::srgb(0.95, 0.75, 0.25);
/// Pixel boundaries: only a hint.
pub const PIXEL_GRID_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.10);

// ============================================================================
// PALETTE CONFIGURATION
// ============================================================================

/// Default palette, `(r, g, b, a)` in 0.0..=1.0. Transparent first, so erasing
/// needs no separate tool.
pub const DEFAULT_PALETTE: &[(f32, f32, f32, f32)] = &[
    (0.0, 0.0, 0.0, 0.0),    // Transparent
    (0.0, 0.0, 0.0, 1.0),    // Black
    (1.0, 1.0, 1.0, 1.0),    // White
    (1.0, 0.0, 0.0, 1.0),    // Red
    (0.0, 1.0, 0.0, 1.0),    // Green
    (0.0, 0.0, 1.0, 1.0),    // Blue
    (1.0, 1.0, 0.0, 1.0),    // Yellow
    (0.0, 1.0, 1.0, 1.0),    // Cyan
    (1.0, 0.0, 1.0, 1.0),    // Magenta
    (0.5, 0.5, 0.5, 1.0),    // Gray 50%
    (0.25, 0.25, 0.25, 1.0), // Dark gray
    (0.75, 0.75, 0.75, 1.0), // Light gray
    (0.5, 0.25, 0.0, 1.0),   // Brown
    (0.0, 0.5, 0.0, 1.0),    // Dark green
    (0.0, 0.0, 0.5, 1.0),    // Dark blue
    (0.75, 0.5, 0.25, 1.0),  // Tan
    (0.5, 0.0, 0.5, 1.0),    // Purple
];

/// The colour a new document starts drawing with: the first opaque entry.
pub const DEFAULT_SELECTED_COLOR: usize = 1;

// ============================================================================
// HISTORY CONFIGURATION
// ============================================================================

/// Retained undo depth. Only `history` reads this, which is not wired in yet.
#[allow(dead_code)]
pub const HISTORY_MAX_DEPTH: usize = 100;

// ============================================================================
// FILE CONFIGURATION
// ============================================================================

pub const FILE_FORMAT_VERSION: &str = "1.0";
pub const METADATA_SUFFIX: &str = "atlas.json";
pub const UNTITLED_BASE: &str = "untitled";

// ============================================================================
// RENDER LAYERS
// ============================================================================

/// Canvas content lives on its own render layer so the full-window UI camera
/// does not also draw it across the sidebar.
pub const CANVAS_LAYER: usize = 1;

// ============================================================================
// COLOUR UTILITIES
// ============================================================================

pub fn transparent() -> Srgba {
    Srgba::new(0.0, 0.0, 0.0, 0.0)
}

/// `#rrggbbaa`, the form the status strip reports.
pub fn color_to_hex(color: Srgba) -> String {
    let to_u8 = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        to_u8(color.red),
        to_u8(color.green),
        to_u8(color.blue),
        to_u8(color.alpha)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_the_obvious_colours() {
        assert_eq!(color_to_hex(transparent()), "#00000000");
        assert_eq!(color_to_hex(Srgba::new(1.0, 0.0, 0.0, 1.0)), "#ff0000ff");
        assert_eq!(color_to_hex(Srgba::new(1.0, 1.0, 1.0, 1.0)), "#ffffffff");
    }

    #[test]
    fn default_selected_colour_is_opaque() {
        let (_, _, _, a) = DEFAULT_PALETTE[DEFAULT_SELECTED_COLOR];
        assert_eq!(a, 1.0, "a new document must start on a visible colour");
    }
}
