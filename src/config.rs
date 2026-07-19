//! Configuration constants for the sprite atlas editor

use bevy::prelude::*;

// ============================================================================
// DISPLAY CONFIGURATION
// ============================================================================

/// Number of screen pixels per atlas pixel
pub const PIXEL_SCALE: u32 = 64;

/// Size of each tile in atlas pixels (configurable at startup, default 8x8)
pub const DEFAULT_TILE_SIZE: u32 = 8;

/// Initial atlas dimensions in tiles
pub const INITIAL_ATLAS_WIDTH: u32 = 1;
pub const INITIAL_ATLAS_HEIGHT: u32 = 1;

// ============================================================================
// WINDOW CONFIGURATION
// ============================================================================

/// Minimum window width (enough for 1 tile + palette)
pub const MIN_WINDOW_WIDTH: f32 = 800.0;

/// Minimum window height
pub const MIN_WINDOW_HEIGHT: f32 = 600.0;

/// Recommended window width
pub const DEFAULT_WINDOW_WIDTH: u32 = 1024;

/// Recommended window height
pub const DEFAULT_WINDOW_HEIGHT: u32 = 768;

/// Window title
pub const WINDOW_TITLE: &str = "Bevy Sprite Atlas Editor";

// ============================================================================
// UI CONFIGURATION
// ============================================================================

/// Width of the palette panel on the left
pub const PALETTE_PANEL_WIDTH: f32 = 200.0;

/// Size of each color swatch in the palette
pub const COLOR_SWATCH_SIZE: f32 = 48.0;

/// Padding around color swatches
pub const COLOR_SWATCH_PADDING: f32 = 4.0;

/// Border width for selected color swatch
pub const COLOR_SWATCH_BORDER: f32 = 2.0;

/// Height of the status bar at the bottom
pub const STATUS_BAR_HEIGHT: f32 = 30.0;

/// Height of the toolbar at the top (if enabled)
pub const TOOLBAR_HEIGHT: f32 = 40.0;

// ============================================================================
// PALETTE CONFIGURATION
// ============================================================================

/// Default palette as tuple array (r, g, b, a) in 0.0-1.0 range
pub const DEFAULT_PALETTE: &[(f32, f32, f32, f32)] = &[
    // Transparent (for erasing)
    (0.0, 0.0, 0.0, 0.0),
    // Basic colors
    (0.0, 0.0, 0.0, 1.0),       // Black
    (1.0, 1.0, 1.0, 1.0),       // White
    (1.0, 0.0, 0.0, 1.0),       // Red
    (0.0, 1.0, 0.0, 1.0),       // Green
    (0.0, 0.0, 1.0, 1.0),       // Blue
    (1.0, 1.0, 0.0, 1.0),       // Yellow
    (0.0, 1.0, 1.0, 1.0),       // Cyan
    (1.0, 0.0, 1.0, 1.0),       // Magenta
    // Grays
    (0.5, 0.5, 0.5, 1.0),       // Gray 50%
    (0.25, 0.25, 0.25, 1.0),    // Dark gray
    (0.75, 0.75, 0.75, 1.0),    // Light gray
    // Common game colors
    (0.5, 0.25, 0.0, 1.0),      // Brown
    (0.0, 0.5, 0.0, 1.0),       // Dark green
    (0.0, 0.0, 0.5, 1.0),       // Dark blue
    (0.75, 0.5, 0.25, 1.0),     // Tan
    (0.5, 0.0, 0.5, 1.0),       // Purple
];

// ============================================================================
// HISTORY CONFIGURATION
// ============================================================================

/// Maximum number of actions to retain in undo/redo history
pub const HISTORY_MAX_DEPTH: usize = 100;

// ============================================================================
// KEY BINDINGS
// ============================================================================

/// Key for scrolling left
pub const KEY_SCROLL_LEFT: KeyCode = KeyCode::KeyO;

/// Key for scrolling right
pub const KEY_SCROLL_RIGHT: KeyCode = KeyCode::Digit3;

// ============================================================================
// FILE CONFIGURATION
// ============================================================================

/// File format version for saved projects
pub const FILE_FORMAT_VERSION: &str = "1.0";

/// Extension for atlas metadata files
pub const METADATA_EXTENSION: &str = ".atlas.json";

/// Extension for atlas image files
pub const IMAGE_EXTENSION: &str = ".png";

// ============================================================================
// RENDERING CONFIGURATION
// ============================================================================

/// Checkerboard background square size (in screen pixels)
pub const CHECKERBOARD_SQUARE_SIZE: f32 = 8.0;

/// Grid line width
pub const GRID_LINE_WIDTH: f32 = 1.0;

// ============================================================================
// COLOR UTILITIES
// ============================================================================

/// Create a transparent Srgba color
pub fn transparent() -> Srgba {
    Srgba::new(0.0, 0.0, 0.0, 0.0)
}

/// Convert Srgba to hex string
pub fn color_to_hex(color: Srgba) -> String {
    let r = (color.red * 255.0) as u8;
    let g = (color.green * 255.0) as u8;
    let b = (color.blue * 255.0) as u8;
    let a = (color.alpha * 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a)
}

// ============================================================================
// COORDINATE UTILITIES
// ============================================================================

/// Convert atlas pixel coordinates to screen coordinates
pub fn atlas_to_screen(
    atlas_x: u32,
    atlas_y: u32,
    viewport_offset_x: f32,
    viewport_offset_y: f32,
) -> (f32, f32) {
    (
        atlas_x as f32 * PIXEL_SCALE as f32 - viewport_offset_x,
        atlas_y as f32 * PIXEL_SCALE as f32 - viewport_offset_y,
    )
}

/// Convert screen coordinates to atlas pixel coordinates
pub fn screen_to_atlas(
    screen_x: f32,
    screen_y: f32,
    viewport_offset_x: f32,
    viewport_offset_y: f32,
    atlas_width: u32,
    atlas_height: u32,
) -> Option<(u32, u32)> {
    let atlas_x = ((screen_x + viewport_offset_x) / PIXEL_SCALE as f32).floor() as u32;
    let atlas_y = ((screen_y + viewport_offset_y) / PIXEL_SCALE as f32).floor() as u32;
    if atlas_x < atlas_width && atlas_y < atlas_height {
        Some((atlas_x, atlas_y))
    } else {
        None
    }
}

/// Convert atlas pixel coordinates to tile coordinates
pub fn pixel_to_tile(pixel_x: u32, pixel_y: u32, tile_size: u32) -> (u32, u32) {
    (pixel_x / tile_size, pixel_y / tile_size)
}

/// Convert tile coordinates to atlas pixel coordinates (top-left of tile)
pub fn tile_to_pixel(tile_x: u32, tile_y: u32, tile_size: u32) -> (u32, u32) {
    (tile_x * tile_size, tile_y * tile_size)
}
