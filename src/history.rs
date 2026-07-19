//! Undo/Redo history system for the sprite atlas editor
//!
//! Tracks all mutable operations to enable undo and redo functionality.

use bevy::prelude::*;
use std::collections::VecDeque;

use crate::atlas::Atlas;
use crate::palette::Palette;
use crate::config::HISTORY_MAX_DEPTH;

// ============================================================================
// ACTION TYPES
// ============================================================================

/// An action that can be undone and redone
#[derive(Debug, Clone)]
pub enum Action {
    /// Set a pixel to a new color
    SetPixel {
        x: u32,
        y: u32,
        old_color: Srgba,
        new_color: Srgba,
    },
    /// Add a tile to the right of the atlas
    AddTile,
    /// Remove a tile from the right of the atlas
    RemoveTile {
        pixels: Vec<Srgba>,
    },
    /// Add a tile with specific pixel data (used for undoing RemoveTile)
    AddTileWithData {
        pixels: Vec<Srgba>,
    },
    /// Add a color to the palette
    AddColor {
        index: usize,
        color: Srgba,
    },
    /// Add a color at a specific index (used for undoing RemoveColor)
    AddColorAt {
        index: usize,
        color: Srgba,
    },
    /// Remove a color from the palette
    RemoveColor {
        index: usize,
        color: Srgba,
    },
    /// Reorder colors in the palette
    ReorderColor {
        from: usize,
        to: usize,
    },
    /// Replace a color in the palette
    SetColor {
        index: usize,
        old_color: Srgba,
        new_color: Srgba,
    },
}

impl Action {
    /// Get a human-readable description of the action
    pub fn description(&self) -> String {
        match self {
            Action::SetPixel { x, y, .. } => format!("Set pixel ({}, {})", x, y),
            Action::AddTile => "Add tile".to_string(),
            Action::RemoveTile { .. } => "Remove tile".to_string(),
            Action::AddTileWithData { .. } => "Add tile with data".to_string(),
            Action::AddColor { index, .. } => format!("Add color at {}", index),
            Action::AddColorAt { index, .. } => format!("Add color at {}", index),
            Action::RemoveColor { index, .. } => format!("Remove color at {}", index),
            Action::ReorderColor { from, to } => format!("Move color {} to {}", from, to),
            Action::SetColor { index, .. } => format!("Set color at {}", index),
        }
    }

    /// Get the reverse action that undoes this one
    pub fn reverse(&self) -> Action {
        match self {
            Action::SetPixel {
                x,
                y,
                old_color,
                new_color,
            } => Action::SetPixel {
                x: *x,
                y: *y,
                old_color: *new_color,
                new_color: *old_color,
            },
            Action::AddTile => Action::RemoveTile {
                pixels: vec![],
            },
            Action::RemoveTile { pixels } => Action::AddTileWithData {
                pixels: pixels.clone(),
            },
            Action::AddTileWithData { pixels } => Action::RemoveTile {
                pixels: pixels.clone(),
            },
            Action::AddColor { index, color } => Action::RemoveColor {
                index: *index,
                color: *color,
            },
            Action::AddColorAt { index, color } => Action::RemoveColor {
                index: *index,
                color: *color,
            },
            Action::RemoveColor { index, color } => Action::AddColorAt {
                index: *index,
                color: *color,
            },
            Action::ReorderColor { from, to } => Action::ReorderColor {
                from: *to,
                to: *from,
            },
            Action::SetColor {
                index,
                old_color,
                new_color,
            } => Action::SetColor {
                index: *index,
                old_color: *new_color,
                new_color: *old_color,
            },
        }
    }
}

// ============================================================================
// HISTORY STRUCTURE
// ============================================================================

/// Tracks all user actions for undo/redo functionality
#[derive(Debug, Clone)]
pub struct History {
    /// Actions that can be undone (most recent at back)
    undo_stack: VecDeque<Action>,
    /// Actions that can be redone (most recent at back)
    redo_stack: VecDeque<Action>,
    /// Maximum number of actions to retain
    max_depth: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    // --------------------------------------------------------------------------
    // Constructors
    // --------------------------------------------------------------------------

    /// Create a new history with default settings
    pub fn new() -> Self {
        Self::with_max_depth(HISTORY_MAX_DEPTH)
    }

    /// Create a new history with a specific maximum depth
    pub fn with_max_depth(max_depth: usize) -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            max_depth,
        }
    }

    // --------------------------------------------------------------------------
    // Stack Access
    // --------------------------------------------------------------------------

    /// Get the number of undoable actions
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Get the number of redoable actions
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    // --------------------------------------------------------------------------
    // Recording Actions
    // --------------------------------------------------------------------------

    /// Record a new action, clearing the redo stack
    pub fn record(&mut self, action: Action) {
        self.undo_stack.push_back(action);
        self.redo_stack.clear();

        if self.undo_stack.len() > self.max_depth {
            self.undo_stack.pop_front();
        }
    }

    // --------------------------------------------------------------------------
    // Undo/Redo Operations
    // --------------------------------------------------------------------------

    /// Undo the most recent action
    /// Returns the action that was undone, or None if nothing to undo
    pub fn undo(&mut self, atlas: &mut Atlas, palette: &mut Palette) -> Option<Action> {
        if self.undo_stack.is_empty() {
            return None;
        }

        let action = self.undo_stack.pop_back().unwrap();
        self.apply_action(atlas, palette, &action.reverse());
        self.redo_stack.push_back(action.clone());

        if self.redo_stack.len() > self.max_depth {
            self.redo_stack.pop_front();
        }

        Some(action)
    }

    /// Redo the most recently undone action
    /// Returns the action that was redone, or None if nothing to redo
    pub fn redo(&mut self, atlas: &mut Atlas, palette: &mut Palette) -> Option<Action> {
        if self.redo_stack.is_empty() {
            return None;
        }

        let action = self.redo_stack.pop_back().unwrap();
        self.apply_action(atlas, palette, &action);
        self.undo_stack.push_back(action.clone());

        if self.undo_stack.len() > self.max_depth {
            self.undo_stack.pop_front();
        }

        Some(action)
    }

    // --------------------------------------------------------------------------
    // Applying Actions
    // --------------------------------------------------------------------------

    /// Apply an action to the editor state
    pub fn apply_action(&mut self, atlas: &mut Atlas, palette: &mut Palette, action: &Action) {
        match action {
            Action::SetPixel {
                x,
                y,
                new_color,
                ..
            } => {
                atlas.set_pixel_at(*x, *y, *new_color);
            }
            Action::AddTile => {
                atlas.add_tile_to_right();
            }
            Action::RemoveTile { .. } => {
                let _ = atlas.remove_tile_from_right();
            }
            Action::AddTileWithData { pixels: _ } => {
                atlas.add_tile_to_right();
            }
            Action::AddColor { index: _, color } => {
                palette.add_color(*color);
            }
            Action::AddColorAt { index, color } => {
                palette.insert_color(*index, *color);
            }
            Action::RemoveColor { index, .. } => {
                let _ = palette.remove_color(*index);
            }
            Action::ReorderColor { from, to } => {
                let _ = palette.reorder_color(*from, *to);
            }
            Action::SetColor {
                index,
                new_color,
                ..
            } => {
                let _ = palette.replace_color(*index, *new_color);
            }
        }
    }

    // --------------------------------------------------------------------------
    // Utility Methods
    // --------------------------------------------------------------------------

    /// Clear all actions from both stacks
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Get the description of the next undo action
    pub fn next_undo_description(&self) -> Option<String> {
        self.undo_stack.back().map(|a| a.description())
    }

    /// Get the description of the next redo action
    pub fn next_redo_description(&self) -> Option<String> {
        self.redo_stack.back().map(|a| a.description())
    }
}

// ============================================================================
// HELPER FUNCTIONS FOR RECORDING ACTIONS
// ============================================================================

/// Record a pixel set action
pub fn record_set_pixel(
    history: &mut History,
    atlas: &mut Atlas,
    x: u32,
    y: u32,
    new_color: Srgba,
) {
    let old_color = atlas.get_pixel_at(x, y);
    atlas.set_pixel_at(x, y, new_color);
    history.record(Action::SetPixel {
        x,
        y,
        old_color,
        new_color,
    });
}

/// Record a tile addition action
pub fn record_add_tile(history: &mut History, atlas: &mut Atlas) {
    atlas.add_tile_to_right();
    history.record(Action::AddTile);
}

/// Record a tile removal action
pub fn record_remove_tile(history: &mut History, atlas: &mut Atlas) {
    if let Some(pixels) = atlas.remove_tile_from_right() {
        history.record(Action::RemoveTile { pixels });
    }
}

/// Record a color addition action
pub fn record_add_color(history: &mut History, palette: &mut Palette, color: Srgba) {
    let index = palette.add_color(color);
    history.record(Action::AddColor { index, color });
}

/// Record a color removal action
pub fn record_remove_color(history: &mut History, palette: &mut Palette, index: usize) {
    if let Some(color) = palette.remove_color(index) {
        history.record(Action::RemoveColor { index, color });
    }
}

/// Record a color reorder action
pub fn record_reorder_color(
    history: &mut History,
    palette: &mut Palette,
    from: usize,
    to: usize,
) {
    if palette.reorder_color(from, to) {
        history.record(Action::ReorderColor { from, to });
    }
}

/// Record a color set action
pub fn record_set_color(
    history: &mut History,
    palette: &mut Palette,
    index: usize,
    new_color: Srgba,
) {
    if let Some(old_color) = palette.replace_color(index, new_color) {
        history.record(Action::SetColor {
            index,
            old_color,
            new_color,
        });
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::transparent;

    #[test]
    fn test_new_history() {
        let history = History::new();
        assert!(!history.can_undo());
        assert!(!history.can_redo());
        assert_eq!(history.undo_count(), 0);
        assert_eq!(history.redo_count(), 0);
    }

    #[test]
    fn test_record_and_undo_set_pixel() {
        let mut history = History::new();
        let mut atlas = Atlas::new();
        let mut palette = Palette::new();

        let red = Srgba::RED;
        record_set_pixel(&mut history, &mut atlas, 0, 0, red);

        assert!(history.can_undo());
        assert!(!history.can_redo());

        assert_eq!(atlas.get_pixel_at(0, 0), red);

        let undone = history.undo(&mut atlas, &mut palette);
        assert!(undone.is_some());

        assert_eq!(atlas.get_pixel_at(0, 0), transparent());
        assert!(history.can_redo());
    }

    #[test]
    fn test_undo_then_redo() {
        let mut history = History::new();
        let mut atlas = Atlas::new();
        let mut palette = Palette::new();

        let red = Srgba::RED;
        record_set_pixel(&mut history, &mut atlas, 0, 0, red);

        let _ = history.undo(&mut atlas, &mut palette);
        assert_eq!(atlas.get_pixel_at(0, 0), transparent());

        let _ = history.redo(&mut atlas, &mut palette);
        assert_eq!(atlas.get_pixel_at(0, 0), red);
    }

    #[test]
    fn test_new_action_clears_redo() {
        let mut history = History::new();
        let mut atlas = Atlas::new();
        let mut palette = Palette::new();

        let red = Srgba::RED;
        let blue = Srgba::BLUE;

        record_set_pixel(&mut history, &mut atlas, 0, 0, red);
        let _ = history.undo(&mut atlas, &mut palette);
        assert!(history.can_redo());

        record_set_pixel(&mut history, &mut atlas, 0, 0, blue);
        assert!(!history.can_redo());
    }

    #[test]
    fn test_max_depth() {
        let mut history = History::with_max_depth(3);
        let mut atlas = Atlas::new();
        let mut palette = Palette::new();

        for i in 0..5 {
            record_set_pixel(&mut history, &mut atlas, i as u32, 0, Srgba::RED);
        }

        assert_eq!(history.undo_count(), 3);
    }

    #[test]
    fn test_action_reverse() {
        let action = Action::SetPixel {
            x: 1,
            y: 2,
            old_color: Srgba::RED,
            new_color: Srgba::BLUE,
        };

        let reverse = action.reverse();

        match reverse {
            Action::SetPixel {
                x,
                y,
                old_color,
                new_color,
            } => {
                assert_eq!(x, 1);
                assert_eq!(y, 2);
                assert_eq!(old_color, Srgba::BLUE);
                assert_eq!(new_color, Srgba::RED);
            }
            _ => panic!("Expected SetPixel action"),
        }
    }
}
