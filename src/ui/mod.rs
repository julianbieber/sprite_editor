//! UI module for the sprite atlas editor

pub mod palette_ui;
pub mod viewport;

use bevy::prelude::*;

pub use palette_ui::*;
pub use viewport::*;

/// Plugin for setting up the editor UI
pub struct EditorUiPlugin;

impl Plugin for EditorUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PaletteUiPlugin)
           .add_plugins(ViewportPlugin);
    }
}
