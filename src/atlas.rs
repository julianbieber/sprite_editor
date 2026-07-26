//! The atlas document: the pixel grid being edited.
//!
//! Pixels are stored **row-major over the whole atlas image**, so pixel
//! `(x, y)` lives at index `y * width_in_pixels + x`. Tiles are a *view*
//! concept only — they are where the grid lines go, and nothing more. That
//! makes the texture upload and the PNG encode straight copies.
//!
//! (The previous layout was per-tile-contiguous while the PNG encoder assumed
//! row-major, so saving anything wider than one tile scrambled it.)

use bevy::prelude::*;

use crate::config::{transparent, DEFAULT_TILE_SIZE};

// ============================================================================
// DATA STRUCTURES
// ============================================================================

/// The document being edited. The single source of truth for pixel data.
#[derive(Debug, Clone, Resource)]
pub struct AtlasDocument {
    /// Pixels per side of a tile.
    pub tile_size: u32,
    /// Tiles across.
    pub width_in_tiles: u32,
    /// Tiles down. Always 1 in this version.
    pub height_in_tiles: u32,
    /// Row-major over the whole image: `(x, y)` is at `y * W + x`.
    pub pixels: Vec<Srgba>,
    /// Bumped on every accepted mutation; the renderer compares against it.
    pub revision: u64,
    /// Set on edit, cleared on save/load.
    pub unsaved: bool,
}

impl Default for AtlasDocument {
    fn default() -> Self {
        Self::new()
    }
}

impl AtlasDocument {
    // ------------------------------------------------------------------------
    // Constructors
    // ------------------------------------------------------------------------

    pub fn new() -> Self {
        Self::with_tile_size(DEFAULT_TILE_SIZE)
    }

    /// One transparent tile at the given tile size.
    pub fn with_tile_size(tile_size: u32) -> Self {
        Self::with_dimensions(tile_size, 1, 1)
    }

    pub fn with_dimensions(tile_size: u32, width_in_tiles: u32, height_in_tiles: u32) -> Self {
        let tile_size = tile_size.max(1);
        let width_in_tiles = width_in_tiles.max(1);
        let height_in_tiles = height_in_tiles.max(1);
        let total = (tile_size * tile_size * width_in_tiles * height_in_tiles) as usize;

        Self {
            tile_size,
            width_in_tiles,
            height_in_tiles,
            pixels: vec![transparent(); total],
            revision: 0,
            unsaved: false,
        }
    }

    /// Build from decoded image data. `pixels` must be row-major and match the
    /// stated dimensions exactly.
    pub fn from_pixels(
        tile_size: u32,
        width_in_tiles: u32,
        height_in_tiles: u32,
        pixels: Vec<Srgba>,
    ) -> Result<Self, String> {
        let expected = (tile_size * tile_size * width_in_tiles * height_in_tiles) as usize;
        if pixels.len() != expected {
            return Err(format!(
                "pixel count {} does not match {}x{} tiles of {}px ({} expected)",
                pixels.len(),
                width_in_tiles,
                height_in_tiles,
                tile_size,
                expected
            ));
        }

        Ok(Self {
            tile_size,
            width_in_tiles,
            height_in_tiles,
            pixels,
            revision: 0,
            unsaved: false,
        })
    }

    // ------------------------------------------------------------------------
    // Accessors
    // ------------------------------------------------------------------------

    #[inline]
    pub fn width_in_pixels(&self) -> u32 {
        self.width_in_tiles * self.tile_size
    }

    #[inline]
    pub fn height_in_pixels(&self) -> u32 {
        self.height_in_tiles * self.tile_size
    }

    #[inline]
    pub fn contains(&self, x: u32, y: u32) -> bool {
        x < self.width_in_pixels() && y < self.height_in_pixels()
    }

    /// Flat index of `(x, y)`. Callers must have checked [`Self::contains`].
    #[inline]
    pub fn index(&self, x: u32, y: u32) -> usize {
        (y * self.width_in_pixels() + x) as usize
    }

    /// Which tile column `x` falls in — the status strip reports it.
    #[inline]
    pub fn tile_of(&self, x: u32) -> u32 {
        x / self.tile_size
    }

    // ------------------------------------------------------------------------
    // Pixel access
    // ------------------------------------------------------------------------

    /// Only the tests and `history` read pixels back today.
    #[allow(dead_code)]
    #[inline]
    pub fn get_pixel_at(&self, x: u32, y: u32) -> Srgba {
        if !self.contains(x, y) {
            return transparent();
        }
        self.pixels[self.index(x, y)]
    }

    /// Writes a pixel. Returns whether anything actually changed, so callers can
    /// avoid bumping the revision for a no-op paint.
    pub fn set_pixel_at(&mut self, x: u32, y: u32, color: Srgba) -> bool {
        if !self.contains(x, y) {
            return false;
        }
        let index = self.index(x, y);
        if self.pixels[index] == color {
            return false;
        }
        self.pixels[index] = color;
        true
    }

    /// Marks the document changed. Every accepted mutation goes through here.
    pub fn touch(&mut self) {
        self.revision += 1;
        self.unsaved = true;
    }

    // ------------------------------------------------------------------------
    // Tile management
    // ------------------------------------------------------------------------

    /// Appends one transparent tile column on the right.
    ///
    /// Row-major storage means this is a splice per row, not an append — the new
    /// pixels have to be interleaved so every row stays contiguous. Rows are
    /// walked back to front so earlier insertion points stay valid.
    pub fn add_tile_to_right(&mut self) {
        let old_width = self.width_in_pixels() as usize;
        let height = self.height_in_pixels() as usize;
        let tile_size = self.tile_size as usize;

        for row in (0..height).rev() {
            let end_of_row = row * old_width + old_width;
            self.pixels.splice(
                end_of_row..end_of_row,
                std::iter::repeat(transparent()).take(tile_size),
            );
        }

        self.width_in_tiles += 1;
    }

    /// Removes the rightmost tile column, returning its pixels row-major.
    /// Refuses to remove the last column.
    ///
    /// No key is bound to this yet — it exists for `history` and the scroll
    /// re-clamp test.
    #[allow(dead_code)]
    pub fn remove_tile_from_right(&mut self) -> Option<Vec<Srgba>> {
        if self.width_in_tiles <= 1 {
            return None;
        }

        let old_width = self.width_in_pixels() as usize;
        let height = self.height_in_pixels() as usize;
        let tile_size = self.tile_size as usize;

        let mut rows: Vec<Vec<Srgba>> = Vec::with_capacity(height);
        for row in (0..height).rev() {
            let end_of_row = row * old_width + old_width;
            let removed: Vec<Srgba> = self
                .pixels
                .splice(end_of_row - tile_size..end_of_row, [])
                .collect();
            rows.push(removed);
        }
        rows.reverse();

        self.width_in_tiles -= 1;
        Some(rows.into_iter().flatten().collect())
    }

    // ------------------------------------------------------------------------
    // Conversion
    // ------------------------------------------------------------------------

    /// RGBA8 bytes, row-major — ready for both the GPU texture and the PNG
    /// encoder, with no reordering in either direction.
    pub fn to_rgba8(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(self.pixels.len() * 4);
        for pixel in &self.pixels {
            let to_u8 = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
            data.push(to_u8(pixel.red));
            data.push(to_u8(pixel.green));
            data.push(to_u8(pixel.blue));
            data.push(to_u8(pixel.alpha));
        }
        data
    }

    /// Inverse of [`Self::to_rgba8`].
    pub fn pixels_from_rgba8(data: &[u8]) -> Vec<Srgba> {
        data.chunks_exact(4)
            .map(|px| {
                Srgba::new(
                    px[0] as f32 / 255.0,
                    px[1] as f32 / 255.0,
                    px[2] as f32 / 255.0,
                    px[3] as f32 / 255.0,
                )
            })
            .collect()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Srgba = Srgba::new(1.0, 0.0, 0.0, 1.0);
    const BLUE: Srgba = Srgba::new(0.0, 0.0, 1.0, 1.0);

    #[test]
    fn new_document_is_one_transparent_tile() {
        let doc = AtlasDocument::new();
        assert_eq!(doc.tile_size, 8);
        assert_eq!(doc.width_in_tiles, 1);
        assert_eq!(doc.pixels.len(), 64);
        assert!(doc.pixels.iter().all(|p| *p == transparent()));
        assert!(!doc.unsaved);
    }

    #[test]
    fn index_is_row_major_over_the_whole_image() {
        let doc = AtlasDocument::with_dimensions(8, 3, 1);
        assert_eq!(doc.width_in_pixels(), 24);
        assert_eq!(doc.index(0, 0), 0);
        assert_eq!(doc.index(23, 0), 23);
        // Second row starts one full image row in, not one tile in.
        assert_eq!(doc.index(0, 1), 24);
        // A pixel in the third tile, second row.
        assert_eq!(doc.index(16, 1), 40);
    }

    #[test]
    fn set_pixel_reports_whether_it_changed() {
        let mut doc = AtlasDocument::new();
        assert!(doc.set_pixel_at(2, 3, RED));
        assert_eq!(doc.get_pixel_at(2, 3), RED);
        // Repainting the same colour is a no-op the caller can skip.
        assert!(!doc.set_pixel_at(2, 3, RED));
        // Out of bounds never writes.
        assert!(!doc.set_pixel_at(99, 99, BLUE));
    }

    #[test]
    fn out_of_bounds_reads_are_transparent_not_panics() {
        let doc = AtlasDocument::new();
        assert_eq!(doc.get_pixel_at(8, 0), transparent());
        assert_eq!(doc.get_pixel_at(0, 8), transparent());
    }

    /// The layout's whole reason for existing: growing the atlas must not move
    /// any pixel that is already there.
    #[test]
    fn add_tile_preserves_existing_pixel_positions() {
        let mut doc = AtlasDocument::with_tile_size(4);
        // Mark a recognisable pattern across the full first tile.
        for y in 0..4 {
            for x in 0..4 {
                let shade = (y * 4 + x) as f32 / 16.0;
                doc.set_pixel_at(x, y, Srgba::new(shade, 0.0, 0.0, 1.0));
            }
        }
        let before: Vec<Srgba> = (0..4)
            .flat_map(|y| (0..4).map(move |x| (x, y)))
            .map(|(x, y)| doc.get_pixel_at(x, y))
            .collect();

        doc.add_tile_to_right();

        assert_eq!(doc.width_in_tiles, 2);
        assert_eq!(doc.width_in_pixels(), 8);
        assert_eq!(doc.pixels.len(), 32);

        let after: Vec<Srgba> = (0..4)
            .flat_map(|y| (0..4).map(move |x| (x, y)))
            .map(|(x, y)| doc.get_pixel_at(x, y))
            .collect();
        assert_eq!(before, after, "existing pixels moved when the atlas grew");

        // And the new column is transparent.
        for y in 0..4 {
            for x in 4..8 {
                assert_eq!(doc.get_pixel_at(x, y), transparent());
            }
        }
    }

    #[test]
    fn add_then_remove_tile_round_trips() {
        let mut doc = AtlasDocument::with_tile_size(4);
        doc.set_pixel_at(1, 1, RED);
        let before = doc.pixels.clone();

        doc.add_tile_to_right();
        let removed = doc.remove_tile_from_right();

        assert!(removed.is_some());
        assert_eq!(doc.width_in_tiles, 1);
        assert_eq!(doc.pixels, before);
    }

    #[test]
    fn remove_tile_returns_that_tiles_pixels_row_major() {
        let mut doc = AtlasDocument::with_tile_size(2);
        doc.add_tile_to_right();
        // Fill the right-hand tile with distinguishable values.
        doc.set_pixel_at(2, 0, RED);
        doc.set_pixel_at(3, 0, BLUE);
        doc.set_pixel_at(2, 1, BLUE);
        doc.set_pixel_at(3, 1, RED);

        let removed = doc.remove_tile_from_right().expect("two tiles wide");
        assert_eq!(removed, vec![RED, BLUE, BLUE, RED]);
        assert_eq!(doc.width_in_tiles, 1);
    }

    #[test]
    fn the_last_tile_cannot_be_removed() {
        let mut doc = AtlasDocument::new();
        assert!(doc.remove_tile_from_right().is_none());
        assert_eq!(doc.width_in_tiles, 1);
    }

    #[test]
    fn rgba8_round_trips_and_keeps_row_order() {
        let mut doc = AtlasDocument::with_tile_size(2);
        doc.add_tile_to_right();
        doc.set_pixel_at(0, 0, RED);
        doc.set_pixel_at(3, 1, BLUE);

        let bytes = doc.to_rgba8();
        assert_eq!(bytes.len() as u32, doc.width_in_pixels() * doc.height_in_pixels() * 4);
        // First pixel is red, and it is the first four bytes.
        assert_eq!(&bytes[0..4], &[255, 0, 0, 255]);

        let restored = AtlasDocument::pixels_from_rgba8(&bytes);
        assert_eq!(restored, doc.pixels);
    }

    #[test]
    fn from_pixels_rejects_a_mismatched_count() {
        assert!(AtlasDocument::from_pixels(8, 2, 1, vec![transparent(); 128]).is_ok());
        assert!(AtlasDocument::from_pixels(8, 2, 1, vec![transparent(); 100]).is_err());
    }

    #[test]
    fn touch_bumps_revision_and_marks_unsaved() {
        let mut doc = AtlasDocument::new();
        assert_eq!(doc.revision, 0);
        doc.touch();
        assert_eq!(doc.revision, 1);
        assert!(doc.unsaved);
    }

    #[test]
    fn tile_of_reports_the_column() {
        let mut doc = AtlasDocument::with_tile_size(8);
        doc.add_tile_to_right();
        assert_eq!(doc.tile_of(0), 0);
        assert_eq!(doc.tile_of(7), 0);
        assert_eq!(doc.tile_of(8), 1);
        assert_eq!(doc.tile_of(15), 1);
    }
}
