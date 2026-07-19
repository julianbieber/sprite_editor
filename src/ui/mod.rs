//! UI module for the sprite atlas editor
//!
//! This module contains UI components for the editor.

pub mod palette_ui;
pub mod viewport;

use bevy::prelude::*;

// Re-export the UI systems
pub use palette_ui::*;
pub use viewport::*;

// ============================================================================
// UI PLUGIN
// ============================================================================

/// Plugin for setting up the editor UI
pub struct EditorUiPlugin;

impl Plugin for EditorUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_ui);
    }
}

// ============================================================================
// UI SETUP
// ============================================================================

/// UI state resource
#[derive(Resource)]
pub struct UiState {
    pub palette_panel_width: f32,
    pub status_bar_height: f32,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            palette_panel_width: 200.0,
            status_bar_height: 30.0,
        }
    }
}

/// Setup the main UI containers
fn setup_ui(mut commands: Commands) {
    // Insert UI state
    commands.insert_resource(UiState::default());
}
