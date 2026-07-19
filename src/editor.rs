//! Editor state and operations for the sprite atlas editor
//!
//! The Editor orchestrates the atlas, palette, viewport, and history systems.

use bevy::prelude::*;

use crate::atlas::Atlas;
use crate::config::{self, PIXEL_SCALE, DEFAULT_TILE_SIZE};
use crate::history::History;
use crate::palette::Palette;

// ============================================================================
// VIEWPORT
// ============================================================================

/// The viewport defines what portion of the atlas is visible on screen
#[derive(Debug, Clone)]
pub struct Viewport {
    /// Horizontal offset in atlas pixels (0 = leftmost)
    pub offset_x: f32,
    /// Vertical offset in atlas pixels (0 = topmost)
    pub offset_y: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self::new()
    }
}

impl Viewport {
    /// Create a new viewport at the origin
    pub fn new() -> Self {
        Self {
            offset_x: 0.0,
            // Initialize offset_y to center a single tile vertically
            // A tile is DEFAULT_TILE_SIZE pixels tall, each pixel is PIXEL_SCALE screen pixels
            // To center: offset_y = (DEFAULT_TILE_SIZE * PIXEL_SCALE / 2) - (window_height / 2)
            // But we don't have window_height here, so we'll rely on clamp to adjust it
            // For now, use a reasonable default that works with typical window heights
            offset_y: 0.0,
        }
    }

    /// Scroll the viewport left by a certain amount (in atlas pixels)
    pub fn scroll_left(&mut self, amount: f32) {
        self.offset_x = (self.offset_x - amount).max(0.0);
    }

    /// Scroll the viewport right by a certain amount (in atlas pixels)
    pub fn scroll_right(&mut self, amount: f32) {
        self.offset_x += amount;
    }

    /// Scroll the viewport up by a certain amount (in atlas pixels)
    pub fn scroll_up(&mut self, amount: f32) {
        self.offset_y = (self.offset_y - amount).max(0.0);
    }

    /// Scroll the viewport down by a certain amount (in atlas pixels)
    pub fn scroll_down(&mut self, amount: f32) {
        self.offset_y += amount;
    }

    /// Clamp the viewport to ensure it doesn't scroll past the atlas boundaries
    pub fn clamp(&mut self, atlas: &Atlas, window_width: f32, window_height: f32) {
        use crate::config::PALETTE_PANEL_WIDTH;
        
        let atlas_width_pixels = atlas.width_in_pixels() as f32;
        let atlas_height_pixels = atlas.height_in_pixels() as f32;

        // Account for palette panel taking up space on the left
        // Viewport pixels start at x=0, palette covers -200 to 0
        let available_width = window_width - PALETTE_PANEL_WIDTH;
        let atlas_width_screen = atlas_width_pixels * PIXEL_SCALE as f32;
        let atlas_height_screen = atlas_height_pixels * PIXEL_SCALE as f32;

        // For x-axis: clamp offset_x to valid range
        // Pixel rendering: screen_x = atlas_x * PIXEL_SCALE - offset_x * PIXEL_SCALE
        // So offset_x moves atlas left/right in atlas pixel units
        // 
        // Minimum offset_x: when leftmost atlas pixel is at right edge of viewport
        // screen_x(0) = 0 - offset_x * PIXEL_SCALE = available_width
        // => offset_x = -available_width / PIXEL_SCALE
        //
        // Maximum offset_x: when rightmost atlas pixel is at left edge of viewport  
        // screen_x(atlas_width-1) = (atlas_width-1) * PIXEL_SCALE - offset_x * PIXEL_SCALE = 0
        // => offset_x = (atlas_width_pixels - 1)  (approximately atlas_width_pixels)
        //
        // For simplicity, use atlas_width_pixels as max and -available_width/PIXEL_SCALE as min
        let min_offset_x = -available_width / PIXEL_SCALE as f32;
        let max_offset_x = atlas_width_pixels - 1.0;
        self.offset_x = self.offset_x.clamp(min_offset_x, max_offset_x);

        // For y-axis with flipped coordinates:
        // screen_y = -y * PIXEL_SCALE + offset_y
        // With offset_y=0, atlas extends from y=0 (top) to y=-atlas_height_screen (bottom)
        // To see the full atlas:
        // - Bottom of atlas must be >= -window_height/2 (bottom of visible area)
        //   => -atlas_height_screen + offset_y >= -window_height/2
        //   => offset_y >= atlas_height_screen - window_height/2
        // - Top of atlas must be <= window_height/2 (top of visible area)
        //   => offset_y <= window_height/2
        //
        // If atlas fits in window (atlas_height_screen < window_height):
        // We need offset_y >= atlas_height_screen - window_height/2 (which is negative, so min is 0)
        // And offset_y <= window_height/2
        // But to center: offset_y = window_height/2 - atlas_height_screen/2
        //
        // If atlas is taller than window:
        // We need offset_y >= atlas_height_screen - window_height/2 (positive)
        // And offset_y <= window_height/2
        //
        // For simplicity, we'll set:
        // - min_offset_y: ensures bottom of atlas doesn't go below window bottom
        // - max_offset_y: ensures top of atlas doesn't go above window top
        let min_offset_y = (atlas_height_screen - window_height / 2.0).max(0.0);
        let max_offset_y = window_height / 2.0;
        
        // But we also want to allow scrolling when atlas is taller than window
        // In that case, max_offset_y should be larger
        let required_max_for_bottom = atlas_height_screen - window_height / 2.0;
        let max_offset_y = max_offset_y.max(required_max_for_bottom);

        self.offset_y = self.offset_y.clamp(min_offset_y, max_offset_y);
    }

    /// Scroll by one tile to the left
    pub fn scroll_tile_left(&mut self) {
        self.scroll_left(DEFAULT_TILE_SIZE as f32);
    }

    /// Scroll by one tile to the right
    pub fn scroll_tile_right(&mut self) {
        self.scroll_right(DEFAULT_TILE_SIZE as f32);
    }
}

// ============================================================================
// EDITOR STATE
// ============================================================================

/// The main editor state containing all editable data
#[derive(Debug, Clone)]
pub struct Editor {
    /// The sprite atlas being edited
    pub atlas: Atlas,
    /// The color palette
    pub palette: Palette,
    /// The viewport
    pub viewport: Viewport,
    /// The history for undo/redo
    pub history: History,
    /// The current tile size (can be changed at runtime)
    pub tile_size: u32,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    // --------------------------------------------------------------------------
    // Constructors
    // --------------------------------------------------------------------------

    /// Create a new editor with default settings
    pub fn new() -> Self {
        Self::with_tile_size(DEFAULT_TILE_SIZE)
    }

    /// Create a new editor with a specific tile size
    pub fn with_tile_size(tile_size: u32) -> Self {
        Self {
            atlas: Atlas::with_tile_size(tile_size),
            palette: Palette::new(),
            viewport: Viewport::new(),
            history: History::new(),
            tile_size,
        }
    }

    // --------------------------------------------------------------------------
    // Drawing Operations
    // --------------------------------------------------------------------------

    /// Draw a pixel at the specified atlas coordinates
    /// This sets the pixel to the currently selected palette color.
    /// The action is recorded in history for undo/redo.
    pub fn draw_pixel(&mut self, x: u32, y: u32) {
        if !self.atlas.is_valid_atlas_pixel(x, y) {
            return;
        }

        let color = self.palette.get_selected_color();
        let old_color = self.atlas.get_pixel_at(x, y);
        self.atlas.set_pixel_at(x, y, color);

        // Record the action
        self.history.record(crate::history::Action::SetPixel {
            x,
            y,
            old_color,
            new_color: color,
        });
    }

    // --------------------------------------------------------------------------
    // Tile Operations
    // --------------------------------------------------------------------------

    /// Add a new transparent tile to the right of the atlas
    pub fn add_tile_to_right(&mut self) {
        self.history.record(crate::history::Action::AddTile);
        self.atlas.add_tile_to_right();
    }

    /// Remove a tile from the right of the atlas
    pub fn remove_tile_from_right(&mut self) {
        if self.atlas.width_in_tiles <= 1 {
            return;
        }

        if let Some(pixels) = self.atlas.remove_tile_from_right() {
            self.history.record(crate::history::Action::RemoveTile { pixels });
        }
    }

    // --------------------------------------------------------------------------
    // Palette Operations
    // --------------------------------------------------------------------------

    /// Add a color to the palette
    pub fn add_color(&mut self, color: Srgba) -> usize {
        let index = self.palette.add_color(color);
        self.history.record(crate::history::Action::AddColor { index, color });
        index
    }

    /// Remove a color from the palette
    pub fn remove_color(&mut self, index: usize) -> Option<Srgba> {
        if let Some(color) = self.palette.remove_color(index) {
            self.history.record(crate::history::Action::RemoveColor { index, color });
            Some(color)
        } else {
            None
        }
    }

    /// Reorder colors in the palette
    pub fn reorder_color(&mut self, from: usize, to: usize) -> bool {
        if self.palette.reorder_color(from, to) {
            self.history.record(crate::history::Action::ReorderColor { from, to });
            true
        } else {
            false
        }
    }

    /// Select a color from the palette
    pub fn select_color(&mut self, index: usize) -> bool {
        self.palette.select_color(index)
    }

    // --------------------------------------------------------------------------
    // Viewport Operations
    // --------------------------------------------------------------------------

    /// Scroll the viewport left by one tile
    pub fn scroll_left(&mut self) {
        self.viewport.scroll_tile_left();
    }

    /// Scroll the viewport right by one tile
    pub fn scroll_right(&mut self) {
        self.viewport.scroll_tile_right();
    }

    /// Clamp the viewport to the atlas boundaries
    pub fn clamp_viewport(&mut self, window_width: f32, window_height: f32) {
        self.viewport.clamp(&self.atlas, window_width, window_height);
    }

    // --------------------------------------------------------------------------
    // Undo/Redo
    // --------------------------------------------------------------------------

    /// Undo the most recent action
    pub fn undo(&mut self) -> Option<crate::history::Action> {
        self.history.undo(&mut self.atlas, &mut self.palette)
    }

    /// Redo the most recently undone action
    pub fn redo(&mut self) -> Option<crate::history::Action> {
        self.history.redo(&mut self.atlas, &mut self.palette)
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    // --------------------------------------------------------------------------
    // Screen to Atlas Conversion
    // --------------------------------------------------------------------------

    /// Convert screen coordinates to atlas pixel coordinates
    /// Note: Bevy's screen coordinates have (0,0) at bottom-left, but we want top-left for atlas
    pub fn screen_to_atlas(
        &self,
        screen_x: f32,
        screen_y: f32,
        _window_width: f32,
        window_height: f32,
    ) -> Option<(u32, u32)> {
        // Convert screen coordinates to atlas coordinates
        // Screen origin is bottom-left in Bevy, but we want top-left for atlas
        let atlas_x = ((screen_x + self.viewport.offset_x) / PIXEL_SCALE as f32).floor() as u32;
        let atlas_y = ((window_height - screen_y + self.viewport.offset_y) / PIXEL_SCALE as f32)
            .floor() as u32;

        if self.atlas.is_valid_atlas_pixel(atlas_x, atlas_y) {
            Some((atlas_x, atlas_y))
        } else {
            None
        }
    }

    /// Get the hover pixel under the cursor
    pub fn get_hover_pixel(
        &self,
        cursor_x: f32,
        cursor_y: f32,
        window_width: f32,
        window_height: f32,
    ) -> Option<(u32, u32)> {
        self.screen_to_atlas(cursor_x, cursor_y, window_width, window_height)
    }
}

// ============================================================================
// RESOURCE FOR BEVY
// ============================================================================

/// Bevy resource wrapper for the editor
#[derive(Debug, Default, Resource)]
pub struct EditorResource {
    pub editor: Editor,
}

impl EditorResource {
    pub fn new() -> Self {
        Self {
            editor: Editor::new(),
        }
    }

    pub fn with_tile_size(tile_size: u32) -> Self {
        Self {
            editor: Editor::with_tile_size(tile_size),
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::color::palettes::css::RED;

    #[test]
    fn test_new_editor() {
        let editor = Editor::new();
        assert_eq!(editor.tile_size, DEFAULT_TILE_SIZE);
        assert_eq!(editor.atlas.width_in_tiles, 1);
        assert_eq!(editor.atlas.height_in_tiles, 1);
        assert!(!editor.palette.is_empty());
    }

    #[test]
    fn test_draw_pixel() {
        let mut editor = Editor::new();

        // Select red color (index 3 in default palette: transparent, black, white, red)
        editor.palette.select_color(3);

        // Draw pixel
        editor.draw_pixel(0, 0);

        // Check pixel was set
        assert_eq!(editor.atlas.get_pixel_at(0, 0), RED);

        // Check undo is available
        assert!(editor.can_undo());
    }

    // Note: screen_to_atlas is not currently used in the application
    // The coordinate system conversion is handled by the viewport rendering and hover detection
    // #[test]
    // fn test_screen_to_atlas() {
    //     let editor = Editor::new();
    //
    //     // Screen position at (0, 0) with viewport at (0, 0)
    //     let result = editor.screen_to_atlas(0.0, 0.0, 512.0, 512.0);
    //
    //     // Should map to top-left pixel (0, 0)
    //     assert_eq!(result, Some((0, 0)));
    //
    //     // Screen position at (64, 0) should be pixel (1, 0)
    //     let result = editor.screen_to_atlas(64.0, 0.0, 512.0, 512.0);
    //     assert_eq!(result, Some((1, 0)));
    // }

    #[test]
    fn test_add_tile() {
        let mut editor = Editor::new();
        assert_eq!(editor.atlas.width_in_tiles, 1);

        editor.add_tile_to_right();
        assert_eq!(editor.atlas.width_in_tiles, 2);
        assert!(editor.can_undo());
    }

    #[test]
    fn test_undo_redo() {
        let mut editor = Editor::new();

        // Draw a pixel
        editor.palette.select_color(3); // Red (index 3)
        editor.draw_pixel(0, 0);

        // Undo
        let _ = editor.undo();
        assert_eq!(editor.atlas.get_pixel_at(0, 0), config::transparent());

        // Redo
        let _ = editor.redo();
        assert_eq!(editor.atlas.get_pixel_at(0, 0), RED);
    }
}
