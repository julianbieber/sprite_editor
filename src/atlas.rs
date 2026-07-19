//! Atlas data structure for sprite atlas editor
//!
//! An atlas is a grid of square tiles, each containing tile_size x tile_size pixels.
//! The atlas stores all pixel data in a flat array for efficient access.

use bevy::prelude::*;
use std::path::Path;

use crate::config::{self, transparent, DEFAULT_TILE_SIZE, INITIAL_ATLAS_HEIGHT, INITIAL_ATLAS_WIDTH};

// ============================================================================
// DATA STRUCTURES
// ============================================================================

/// A sprite atlas containing multiple square tiles
#[derive(Debug, Clone)]
pub struct Atlas {
    /// Number of pixels per side of each square tile
    pub tile_size: u32,
    /// Number of tiles wide
    pub width_in_tiles: u32,
    /// Number of tiles tall
    pub height_in_tiles: u32,
    /// Flat array of pixel colors, stored in row-major order
    /// Layout: [tile(0,0) row 0, tile(0,0) row 1..., tile(1,0) row 0...]
    pub pixels: Vec<Srgba>,
}

impl Atlas {
    // --------------------------------------------------------------------------
    // Constructors
    // --------------------------------------------------------------------------

    /// Create a new atlas with default settings (8x8 tiles, 1x1 tiles)
    pub fn new() -> Self {
        Self::with_tile_size(DEFAULT_TILE_SIZE)
    }

    /// Create a new atlas with a specific tile size
    pub fn with_tile_size(tile_size: u32) -> Self {
        Self::with_dimensions(tile_size, INITIAL_ATLAS_WIDTH, INITIAL_ATLAS_HEIGHT)
    }

    /// Create a new atlas with specific dimensions
    pub fn with_dimensions(tile_size: u32, width_in_tiles: u32, height_in_tiles: u32) -> Self {
        let total_pixels = tile_size * tile_size * width_in_tiles * height_in_tiles;
        let pixels = vec![transparent(); total_pixels as usize];

        Atlas {
            tile_size,
            width_in_tiles,
            height_in_tiles,
            pixels,
        }
    }

    // --------------------------------------------------------------------------
    // Accessors
    // --------------------------------------------------------------------------

    /// Get the total width of the atlas in pixels
    pub fn width_in_pixels(&self) -> u32 {
        self.width_in_tiles * self.tile_size
    }

    /// Get the total height of the atlas in pixels
    pub fn height_in_pixels(&self) -> u32 {
        self.height_in_tiles * self.tile_size
    }

    /// Get the total number of pixels in the atlas
    pub fn pixel_count(&self) -> usize {
        self.pixels.len()
    }

    /// Get the total number of tiles in the atlas
    pub fn tile_count(&self) -> usize {
        (self.width_in_tiles * self.height_in_tiles) as usize
    }

    // --------------------------------------------------------------------------
    // Pixel Access
    // --------------------------------------------------------------------------

    /// Calculate the flat index of a pixel in the pixels array
    #[inline]
    pub fn pixel_index(&self, tile_x: u32, tile_y: u32, pixel_x: u32, pixel_y: u32) -> usize {
        let tiles_per_row = self.width_in_tiles;
        let pixels_per_tile = self.tile_size;
        
        let tile_row = tile_y * tiles_per_row + tile_x;
        let pixel_row = tile_row * pixels_per_tile + pixel_y;
        let pixel_col = pixel_row * pixels_per_tile + pixel_x;
        
        pixel_col as usize
    }

    /// Get the color of a specific pixel
    #[inline]
    pub fn get_pixel(&self, tile_x: u32, tile_y: u32, pixel_x: u32, pixel_y: u32) -> Srgba {
        let index = self.pixel_index(tile_x, tile_y, pixel_x, pixel_y);
        self.pixels[index]
    }

    /// Get the color of a specific pixel by atlas coordinates
    #[inline]
    pub fn get_pixel_at(&self, x: u32, y: u32) -> Srgba {
        let (tile_x, tile_y) = config::pixel_to_tile(x, y, self.tile_size);
        let (pixel_x, pixel_y) = (x % self.tile_size, y % self.tile_size);
        self.get_pixel(tile_x, tile_y, pixel_x, pixel_y)
    }

    /// Set the color of a specific pixel
    #[inline]
    pub fn set_pixel(&mut self, tile_x: u32, tile_y: u32, pixel_x: u32, pixel_y: u32, color: Srgba) {
        let index = self.pixel_index(tile_x, tile_y, pixel_x, pixel_y);
        self.pixels[index] = color;
    }

    /// Set the color of a specific pixel by atlas coordinates
    #[inline]
    pub fn set_pixel_at(&mut self, x: u32, y: u32, color: Srgba) {
        let (tile_x, tile_y) = config::pixel_to_tile(x, y, self.tile_size);
        let (pixel_x, pixel_y) = (x % self.tile_size, y % self.tile_size);
        self.set_pixel(tile_x, tile_y, pixel_x, pixel_y, color);
    }

    // --------------------------------------------------------------------------
    // Tile Management
    // --------------------------------------------------------------------------

    /// Add a new transparent tile to the right of the atlas
    pub fn add_tile_to_right(&mut self) {
        let new_tile_pixels = self.tile_size * self.tile_size;
        
        self.pixels.extend(std::iter::repeat(transparent())
            .take(new_tile_pixels as usize));
        
        self.width_in_tiles += 1;
    }

    /// Add a new transparent tile below the atlas
    pub fn add_tile_below(&mut self) {
        let new_tile_pixels = self.tile_size * self.tile_size * self.width_in_tiles;
        
        self.pixels.extend(std::iter::repeat(transparent())
            .take(new_tile_pixels as usize));
        
        self.height_in_tiles += 1;
    }

    /// Remove a tile from the right edge
    /// Returns the pixels of the removed tile, or None if atlas is already 1 tile wide
    pub fn remove_tile_from_right(&mut self) -> Option<Vec<Srgba>> {
        if self.width_in_tiles <= 1 {
            return None;
        }

        let pixels_per_tile = (self.tile_size * self.tile_size) as usize;
        let start_index = self.pixels.len() - pixels_per_tile;
        let removed_pixels = self.pixels.splice(start_index.., []).collect();
        
        self.width_in_tiles -= 1;
        
        Some(removed_pixels)
    }

    // --------------------------------------------------------------------------
    // Validation
    // --------------------------------------------------------------------------

    /// Check if coordinates are within bounds
    pub fn is_valid_pixel(&self, tile_x: u32, tile_y: u32, pixel_x: u32, pixel_y: u32) -> bool {
        tile_x < self.width_in_tiles
        && tile_y < self.height_in_tiles
        && pixel_x < self.tile_size
        && pixel_y < self.tile_size
    }

    /// Check if atlas pixel coordinates are within bounds
    pub fn is_valid_atlas_pixel(&self, x: u32, y: u32) -> bool {
        x < self.width_in_pixels() && y < self.height_in_pixels()
    }

    // --------------------------------------------------------------------------
    // Conversion
    // --------------------------------------------------------------------------

    /// Convert to a Bevy Image for rendering or saving
    pub fn to_image(&self) -> Image {
        use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
        use bevy::asset::RenderAssetUsages;
        
        let width = self.width_in_pixels() as usize;
        let height = self.height_in_pixels() as usize;
        
        // Convert Srgba to RGBA8 u8
        let mut data = Vec::with_capacity(width * height * 4);
        
        for pixel in &self.pixels {
            data.push((pixel.red * 255.0) as u8);
            data.push((pixel.green * 255.0) as u8);
            data.push((pixel.blue * 255.0) as u8);
            data.push((pixel.alpha * 255.0) as u8);
        }
        
        Image::new_fill(
            Extent3d {
                width: width as u32,
                height: height as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        )
    }

    // --------------------------------------------------------------------------
    // Serialization Support
    // --------------------------------------------------------------------------

    /// Save the atlas pixels to a PNG file
    pub fn save_to_png(&self, path: &Path) -> Result<(), image::ImageError> {
        use image::{ImageBuffer, Rgba};
        
        let width = self.width_in_pixels() as u32;
        let height = self.height_in_pixels() as u32;
        
        // Create an image buffer
        let mut img = ImageBuffer::new(width, height);
        
        // Fill with pixel data
        for (i, pixel) in self.pixels.iter().enumerate() {
            let x = (i as u32) % width;
            let y = (i as u32) / width;
            
            let r = (pixel.red * 255.0) as u8;
            let g = (pixel.green * 255.0) as u8;
            let b = (pixel.blue * 255.0) as u8;
            let a = (pixel.alpha * 255.0) as u8;
            
            img.put_pixel(x, y, Rgba([r, g, b, a]));
        }
        
        // Save to file
        img.save(path)?;
        
        Ok(())
    }

    /// Load an atlas from a PNG file
    pub fn load_from_png(path: &Path, tile_size: u32) -> Result<Self, image::ImageError> {
        use image::ImageReader;
        
        // Load the image
        let img = ImageReader::open(path)?.decode()?;
        let rgba_img = img.to_rgba8();
        
        let width = rgba_img.width();
        let height = rgba_img.height();
        
        // Calculate dimensions in tiles
        let width_in_tiles = (width + tile_size - 1) / tile_size;
        let height_in_tiles = (height + tile_size - 1) / tile_size;
        
        // Create atlas
        let total_pixels = width_in_tiles * height_in_tiles * tile_size * tile_size;
        let mut pixels = Vec::with_capacity(total_pixels as usize);
        
        // Convert RGBA8 to Srgba
        for y in 0..height {
            for x in 0..width {
                let pixel = rgba_img.get_pixel(x, y);
                pixels.push(Srgba::new(
                    pixel[0] as f32 / 255.0,
                    pixel[1] as f32 / 255.0,
                    pixel[2] as f32 / 255.0,
                    pixel[3] as f32 / 255.0,
                ));
            }
        }
        
        // If image doesn't align with tile grid, pad with transparent
        let expected_pixels = (width_in_tiles * height_in_tiles * tile_size * tile_size) as usize;
        if pixels.len() < expected_pixels {
            pixels.extend(std::iter::repeat(transparent())
                .take(expected_pixels - pixels.len()));
        }
        
        Ok(Atlas {
            tile_size,
            width_in_tiles,
            height_in_tiles,
            pixels,
        })
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
    fn test_new_atlas() {
        let atlas = Atlas::new();
        assert_eq!(atlas.tile_size, 8);
        assert_eq!(atlas.width_in_tiles, 1);
        assert_eq!(atlas.height_in_tiles, 1);
        assert_eq!(atlas.pixels.len(), 64); // 8x8
    }

    #[test]
    fn test_custom_tile_size() {
        let atlas = Atlas::with_tile_size(16);
        assert_eq!(atlas.tile_size, 16);
        assert_eq!(atlas.pixels.len(), 256); // 16x16
    }

    #[test]
    fn test_pixel_index() {
        let atlas = Atlas::with_tile_size(8);
        
        assert_eq!(atlas.pixel_index(0, 0, 0, 0), 0);
        assert_eq!(atlas.pixel_index(0, 0, 7, 7), 63);
        
        let mut atlas = Atlas::with_tile_size(8);
        atlas.add_tile_to_right();
        
        assert_eq!(atlas.pixel_index(1, 0, 0, 0), 64);
    }

    #[test]
    fn test_set_get_pixel() {
        let mut atlas = Atlas::new();
        let red = RED;
        
        atlas.set_pixel(0, 0, 0, 0, red);
        assert_eq!(atlas.get_pixel(0, 0, 0, 0), red);
    }

    #[test]
    fn test_add_tile() {
        let mut atlas = Atlas::new();
        assert_eq!(atlas.width_in_tiles, 1);
        assert_eq!(atlas.pixels.len(), 64);
        
        atlas.add_tile_to_right();
        assert_eq!(atlas.width_in_tiles, 2);
        assert_eq!(atlas.pixels.len(), 128); // 2 tiles * 64 pixels
    }

    #[test]
    fn test_width_height_pixels() {
        let atlas = Atlas::with_dimensions(8, 3, 2);
        assert_eq!(atlas.width_in_pixels(), 24);  // 3 tiles * 8
        assert_eq!(atlas.height_in_pixels(), 16); // 2 tiles * 8
    }
}
