//! Color palette system for the sprite atlas editor
//!
//! The palette stores an ordered list of colors that can be used for drawing.
//! One color can be selected as the active color.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::config::{color_to_hex, DEFAULT_PALETTE, transparent};

// ============================================================================
// DATA STRUCTURES
// ============================================================================

/// A color in RGBA format that can be serialized
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColorData {
    #[serde(rename = "r")]
    pub red: f32,
    #[serde(rename = "g")]
    pub green: f32,
    #[serde(rename = "b")]
    pub blue: f32,
    #[serde(rename = "a")]
    pub alpha: f32,
}

impl ColorData {
    /// Create a new ColorData from RGBA components
    pub fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            red: red.clamp(0.0, 1.0),
            green: green.clamp(0.0, 1.0),
            blue: blue.clamp(0.0, 1.0),
            alpha: alpha.clamp(0.0, 1.0),
        }
    }

    /// Convert to Srgba
    pub fn to_srgba(&self) -> Srgba {
        Srgba::new(self.red, self.green, self.blue, self.alpha)
    }

    /// Convert from Srgba
    pub fn from_srgba(color: Srgba) -> Self {
        Self::new(color.red, color.green, color.blue, color.alpha)
    }

    /// Convert to hex string
    pub fn to_hex(&self) -> String {
        color_to_hex(self.to_srgba())
    }
}

/// Serializable representation of a palette for saving to JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaletteData {
    pub colors: Vec<ColorData>,
    #[serde(default)]
    pub selected_index: Option<usize>,
}

impl PaletteData {
    /// Convert to a Palette
    pub fn to_palette(&self) -> Palette {
        let colors: Vec<Srgba> = self.colors.iter().map(|c| c.to_srgba()).collect();
        Palette {
            colors,
            selected_index: self.selected_index,
        }
    }

    /// Convert from a Palette
    pub fn from_palette(palette: &Palette) -> Self {
        let colors: Vec<ColorData> = palette.colors.iter()
            .map(|&c| ColorData::from_srgba(c))
            .collect();
        Self {
            colors,
            selected_index: palette.selected_index,
        }
    }
}

/// The color palette for the editor
#[derive(Debug, Clone)]
pub struct Palette {
    /// Ordered list of colors in the palette
    pub colors: Vec<Srgba>,
    /// Index of the currently selected color, if any
    pub selected_index: Option<usize>,
}

impl Default for Palette {
    fn default() -> Self {
        Self::new()
    }
}

impl Palette {
    // --------------------------------------------------------------------------
    // Constructors
    // --------------------------------------------------------------------------

    /// Create a new palette with the default colors
    pub fn new() -> Self {
        let colors: Vec<Srgba> = DEFAULT_PALETTE
            .iter()
            .map(|&(r, g, b, a)| Srgba::new(r, g, b, a))
            .collect();

        Self {
            colors,
            selected_index: Some(0), // Select first color by default
        }
    }

    /// Create an empty palette
    pub fn empty() -> Self {
        Self {
            colors: Vec::new(),
            selected_index: None,
        }
    }

    /// Create a palette with specific colors
    pub fn with_colors(colors: Vec<Srgba>) -> Self {
        let selected_index = if colors.is_empty() {
            None
        } else {
            Some(0)
        };

        Self {
            colors,
            selected_index,
        }
    }

    // --------------------------------------------------------------------------
    // Accessors
    // --------------------------------------------------------------------------

    /// Get the number of colors in the palette
    pub fn len(&self) -> usize {
        self.colors.len()
    }

    /// Check if the palette is empty
    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }

    /// Get the currently selected color, or transparent if none selected
    pub fn get_selected_color(&self) -> Srgba {
        match self.selected_index {
            Some(index) => self.colors[index],
            None => transparent(),
        }
    }

    /// Get the color at a specific index
    #[inline]
    pub fn get_color(&self, index: usize) -> Option<Srgba> {
        self.colors.get(index).copied()
    }

    /// Check if a color index is valid
    #[inline]
    pub fn is_valid_index(&self, index: usize) -> bool {
        index < self.colors.len()
    }

    // --------------------------------------------------------------------------
    // Color Selection
    // --------------------------------------------------------------------------

    /// Select a color by index
    /// Returns true if the index was valid and selection changed, false otherwise
    pub fn select_color(&mut self, index: usize) -> bool {
        if index >= self.colors.len() {
            return false;
        }

        self.selected_index = Some(index);
        true
    }

    /// Select the next color in the palette (wraps around)
    pub fn select_next(&mut self) {
        if self.colors.is_empty() {
            self.selected_index = None;
            return;
        }

        match self.selected_index {
            Some(index) => {
                self.selected_index = Some((index + 1) % self.colors.len());
            }
            None => {
                self.selected_index = Some(0);
            }
        }
    }

    /// Select the previous color in the palette (wraps around)
    pub fn select_previous(&mut self) {
        if self.colors.is_empty() {
            self.selected_index = None;
            return;
        }

        match self.selected_index {
            Some(index) => {
                let new_index = if index == 0 {
                    self.colors.len() - 1
                } else {
                    index - 1
                };
                self.selected_index = Some(new_index);
            }
            None => {
                self.selected_index = Some(self.colors.len() - 1);
            }
        }
    }

    /// Clear the selection (no color selected)
    pub fn clear_selection(&mut self) {
        self.selected_index = None;
    }

    // --------------------------------------------------------------------------
    // Color Management
    // --------------------------------------------------------------------------

    /// Add a new color to the end of the palette
    /// Returns the index of the newly added color
    pub fn add_color(&mut self, color: Srgba) -> usize {
        let index = self.colors.len();
        self.colors.push(color);

        // If nothing was selected, select the new color
        if self.selected_index.is_none() {
            self.selected_index = Some(index);
        }

        index
    }

    /// Insert a color at a specific position
    pub fn insert_color(&mut self, index: usize, color: Srgba) {
        if index <= self.colors.len() {
            self.colors.insert(index, color);

            // Adjust selection if needed
            if let Some(selected) = self.selected_index {
                if selected >= index {
                    self.selected_index = Some(selected + 1);
                }
            }
        }
    }

    /// Remove a color by index
    /// Returns the removed color, or None if index was invalid
    pub fn remove_color(&mut self, index: usize) -> Option<Srgba> {
        if index >= self.colors.len() {
            return None;
        }

        let removed = self.colors.remove(index);

        // Adjust selection
        match self.selected_index {
            Some(selected) => {
                if selected == index {
                    // Removed the selected color
                    if self.colors.is_empty() {
                        self.selected_index = None;
                    } else if selected >= self.colors.len() {
                        self.selected_index = Some(self.colors.len() - 1);
                    }
                } else if selected > index {
                    // Shift selection down
                    self.selected_index = Some(selected - 1);
                }
            }
            None => {}
        }

        Some(removed)
    }

    /// Replace a color at a specific index
    /// Returns the old color, or None if index was invalid
    pub fn replace_color(&mut self, index: usize, new_color: Srgba) -> Option<Srgba> {
        if index >= self.colors.len() {
            return None;
        }

        let old_color = self.colors[index];
        self.colors[index] = new_color;
        Some(old_color)
    }

    // --------------------------------------------------------------------------
    // Reordering
    // --------------------------------------------------------------------------

    /// Move a color from one position to another
    /// Returns true if the operation was successful, false if indices were invalid
    pub fn reorder_color(&mut self, from: usize, to: usize) -> bool {
        if from >= self.colors.len() || to >= self.colors.len() || from == to {
            return false;
        }

        let color = self.colors.remove(from);
        self.colors.insert(to, color);

        // Adjust selection
        if let Some(selected) = self.selected_index {
            if selected == from {
                self.selected_index = Some(to);
            } else if selected > from && selected <= to {
                self.selected_index = Some(selected - 1);
            } else if selected >= to && selected < from {
                self.selected_index = Some(selected + 1);
            }
        }

        true
    }

    /// Move a color up one position (toward the beginning)
    pub fn move_color_up(&mut self, index: usize) -> bool {
        if index > 0 {
            self.reorder_color(index, index - 1)
        } else {
            false
        }
    }

    /// Move a color down one position (toward the end)
    pub fn move_color_down(&mut self, index: usize) -> bool {
        if index < self.colors.len() - 1 {
            self.reorder_color(index, index + 1)
        } else {
            false
        }
    }

    /// Swap two colors
    pub fn swap_colors(&mut self, a: usize, b: usize) -> bool {
        if a >= self.colors.len() || b >= self.colors.len() {
            return false;
        }

        self.colors.swap(a, b);

        // Update selection if it was pointing to one of the swapped colors
        if let Some(selected) = self.selected_index {
            if selected == a {
                self.selected_index = Some(b);
            } else if selected == b {
                self.selected_index = Some(a);
            }
        }

        true
    }

    // --------------------------------------------------------------------------
    // Bulk Operations
    // --------------------------------------------------------------------------

    /// Clear all colors from the palette
    pub fn clear(&mut self) {
        self.colors.clear();
        self.selected_index = None;
    }

    /// Replace all colors in the palette
    pub fn replace_all(&mut self, new_colors: Vec<Srgba>) {
        self.colors = new_colors;
        if self.colors.is_empty() {
            self.selected_index = None;
        } else {
            self.selected_index = Some(0).min(self.selected_index);
        }
    }

    // --------------------------------------------------------------------------
    // Serialization
    // --------------------------------------------------------------------------

    /// Convert to serializable PaletteData
    pub fn to_data(&self) -> PaletteData {
        PaletteData::from_palette(self)
    }

    /// Load from serializable PaletteData
    pub fn from_data(data: &PaletteData) -> Self {
        data.to_palette()
    }

    /// Save to a JSON file
    pub fn save_to_json(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        let data = self.to_data();
        let json = serde_json::to_string_pretty(&data)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load from a JSON file
    pub fn load_from_json(path: &std::path::Path) -> Result<Self, std::io::Error> {
        let json = std::fs::read_to_string(path)?;
        let data: PaletteData = serde_json::from_str(&json)?;
        Ok(data.to_palette())
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::color::palettes::css::{RED, WHITE};

    #[test]
    fn test_new_palette() {
        let palette = Palette::new();
        assert!(!palette.is_empty());
        assert_eq!(palette.len(), DEFAULT_PALETTE.len());
        assert!(palette.selected_index.is_some());
    }

    #[test]
    fn test_empty_palette() {
        let palette = Palette::empty();
        assert!(palette.is_empty());
        assert!(palette.selected_index.is_none());
    }

    #[test]
    fn test_add_color() {
        let mut palette = Palette::empty();
        assert!(palette.is_empty());

        let index = palette.add_color(WHITE);
        assert_eq!(index, 0);
        assert_eq!(palette.len(), 1);
        assert_eq!(palette.get_selected_color(), WHITE);
    }

    #[test]
    fn test_select_color() {
        let mut palette = Palette::new();

        // Add a red color
        palette.add_color(RED);

        // Select it
        assert!(palette.select_color(palette.len() - 1));
        assert_eq!(palette.get_selected_color(), RED);
    }

    #[test]
    fn test_remove_color() {
        let mut palette = Palette::new();
        let initial_len = palette.len();

        // Remove a color
        let removed = palette.remove_color(0);
        assert!(removed.is_some());
        assert_eq!(palette.len(), initial_len - 1);
    }

    #[test]
    fn test_remove_selected_color() {
        let mut palette = Palette::new();
        
        // Select first color
        palette.select_color(0);
        
        // Remove it
        palette.remove_color(0);
        
        // Selection should have moved
        assert!(palette.selected_index.is_some());
    }

    #[test]
    fn test_reorder_color() {
        let mut palette = Palette::new();
        let first = palette.colors[0];
        let second = palette.colors[1];

        // Move first to second position
        assert!(palette.reorder_color(0, 1));
        
        assert_eq!(palette.colors[0], second);
        assert_eq!(palette.colors[1], first);
    }

    #[test]
    fn test_swap_colors() {
        let mut palette = Palette::new();
        let first = palette.colors[0];
        let second = palette.colors[1];

        assert!(palette.swap_colors(0, 1));
        
        assert_eq!(palette.colors[0], second);
        assert_eq!(palette.colors[1], first);
    }

    #[test]
    fn test_color_data_conversion() {
        let color = Srgba::new(0.5, 0.6, 0.7, 0.8);
        let data = ColorData::from_srgba(color);
        
        assert_eq!(data.red, 0.5);
        assert_eq!(data.green, 0.6);
        assert_eq!(data.blue, 0.7);
        assert_eq!(data.alpha, 0.8);
        
        assert_eq!(data.to_srgba(), color);
    }

    #[test]
    fn test_palette_serialization() {
        let palette = Palette::new();
        let data = palette.to_data();
        let restored = Palette::from_data(&data);
        
        assert_eq!(palette.colors, restored.colors);
        assert_eq!(palette.selected_index, restored.selected_index);
    }
}
