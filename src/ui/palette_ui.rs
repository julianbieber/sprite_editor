//! Palette UI for the sprite atlas editor
//!
//! Displays the color palette on the left side of the screen.
//!
//! This is a placeholder module. The actual UI implementation
//! will be added once we have the core systems working.

use bevy::prelude::*;

use crate::editor::EditorResource;

// ============================================================================
// COMPONENTS
// ============================================================================

/// Marker component for the palette panel
#[derive(Component)]
pub struct PalettePanel;

// ============================================================================
// PLUGIN
// ============================================================================

/// Plugin for the palette UI
pub struct PaletteUiPlugin;

impl Plugin for PaletteUiPlugin {
    fn build(&self, _app: &mut App) {
        // Placeholder - will implement UI rendering
    }
}
