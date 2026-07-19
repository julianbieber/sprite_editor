//! Viewport rendering for the sprite atlas editor
//!
//! Renders the atlas pixels and handles drawing input.
//!
//! This is a placeholder module. The actual rendering implementation
//! will be added once we have the core systems working.

use bevy::prelude::*;

use crate::editor::EditorResource;

// ============================================================================
// COMPONENTS
// ============================================================================

/// Marker component for the atlas viewport
#[derive(Component)]
pub struct AtlasViewport;

// ============================================================================
// PLUGIN
// ============================================================================

/// Plugin for the atlas viewport
pub struct ViewportPlugin;

impl Plugin for ViewportPlugin {
    fn build(&self, _app: &mut App) {
        // Placeholder - will implement rendering
    }
}
