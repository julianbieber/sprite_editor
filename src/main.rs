//! Bevy Sprite Atlas Editor
//!
//! A Bevy-based editor for creating and editing sprite atlases.
//! 
//! Features:
//! - Pixel-level editing with color palette
//! - Tile-based atlas organization
//! - Undo/redo with 100-depth history
//! - PNG + JSON file format

use bevy::prelude::*;
use bevy::window::WindowResolution;

mod atlas;
mod config;
mod editor;
mod file_io;
mod history;
mod palette;
mod ui;

use crate::config::{
    DEFAULT_TILE_SIZE, DEFAULT_WINDOW_HEIGHT, DEFAULT_WINDOW_WIDTH, WINDOW_TITLE,
};
use crate::editor::EditorResource;
use crate::ui::{PaletteUiPlugin, ViewportPlugin};

// ============================================================================
// MAIN APP
// ============================================================================

fn main() {
    App::new()
        // Bevy plugins
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: WINDOW_TITLE.to_string(),
                resolution: WindowResolution::new(DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT),
                resizable: true,
                present_mode: bevy::window::PresentMode::AutoNoVsync,
                ..default()
            }),
            ..default()
        }))
        // Editor resource
        .insert_resource(EditorResource::with_tile_size(DEFAULT_TILE_SIZE))
        // Custom plugins
        .add_plugins(PaletteUiPlugin)
        .add_plugins(ViewportPlugin)
        // Systems
        .add_systems(Startup, setup_camera)
        .add_systems(Update, clamp_viewport_system)
        .add_systems(Update, handle_window_resize)
        .run();
}

// ============================================================================
// SETUP SYSTEMS
// ============================================================================

/// Setup the camera
fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

// ============================================================================
// UPDATE SYSTEMS
// ============================================================================

/// Clamp the viewport to the atlas boundaries
fn clamp_viewport_system(
    mut editor: ResMut<EditorResource>,
    windows: Query<&Window>,
) {
    if let Ok(window) = windows.single() {
        editor
            .editor
            .clamp_viewport(window.width(), window.height());
    }
}

/// Handle window resize
fn handle_window_resize(
    mut editor: ResMut<EditorResource>,
    windows: Query<&Window>,
) {
    if let Ok(window) = windows.single() {
        editor
            .editor
            .clamp_viewport(window.width(), window.height());
    }
}
