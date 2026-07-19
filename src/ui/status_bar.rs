//! Status bar display for the sprite atlas editor
//!
//! Shows current cursor position and selected color information.
//! 
//! Note: Text display is commented out pending Feathers UI integration.
//! The status bar background is displayed as a visual placeholder.

use bevy::prelude::*;
use crate::config::{STATUS_BAR_HEIGHT, DEFAULT_WINDOW_WIDTH};

/// Marker component for the status bar
#[derive(Component)]
pub struct StatusBar;

/// Plugin for the status bar
pub struct StatusBarPlugin;

impl Plugin for StatusBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_status_bar);
    }
}

/// Setup the status bar background
fn setup_status_bar(mut commands: Commands) {
    let window_width = DEFAULT_WINDOW_WIDTH as f32;
    let bar_height = STATUS_BAR_HEIGHT;
    
    // Create status bar background (spans bottom of window)
    commands.spawn((
        Sprite::from_color(Color::Srgba(Srgba::new(0.15, 0.15, 0.15, 0.9)), Vec2::new(window_width, bar_height)),
        Transform::from_xyz(0.0, -window_width / 2.0 + bar_height / 2.0, 100.0),
        StatusBar,
    ));
    
    // TODO: Add text display with Feathers UI
    // For now, the status bar is just a visual placeholder
}
