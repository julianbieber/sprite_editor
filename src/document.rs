//! Document state: the only place `AtlasDocument` and `Palette` are mutated.
//!
//! Input never writes state directly — it emits requests, and these systems
//! apply them. That is what lets the dev harness drive the editor through the
//! same code path a human does.

use std::path::PathBuf;

use bevy::prelude::*;

use crate::atlas::AtlasDocument;
use crate::config::transparent;
use crate::palette::Palette;

// ============================================================================
// REQUESTS
// ============================================================================

/// Paint the selected colour into one pixel.
#[derive(Message, Debug, Clone, Copy)]
pub struct PaintPixelRequested {
    pub x: u32,
    pub y: u32,
}

/// Make a palette entry the draw colour.
#[derive(Message, Debug, Clone, Copy)]
pub struct SelectColorRequested {
    pub index: usize,
}

/// Append transparent tile columns on the right.
#[derive(Message, Debug, Clone, Copy)]
pub struct AddTileRequested {
    pub tiles: u32,
}

// ============================================================================
// RESOURCES
// ============================================================================

/// Where this document loads from and saves to.
#[derive(Resource, Debug, Clone, Default)]
pub struct DocumentPath {
    /// Base path with no extension; `None` until a save picks a default.
    pub base: Option<PathBuf>,
    pub existed_on_disk: bool,
}

impl DocumentPath {
    /// What the status strip shows.
    pub fn display_name(&self) -> String {
        match &self.base {
            Some(base) => base
                .file_name()
                .map(|n| format!("{}.png", n.to_string_lossy()))
                .unwrap_or_else(|| "<unnamed>".to_string()),
            None => "<unsaved>".to_string(),
        }
    }
}

// ============================================================================
// PLUGIN
// ============================================================================

pub struct DocumentStatePlugin;

impl Plugin for DocumentStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PaintPixelRequested>()
            .add_message::<SelectColorRequested>()
            .add_message::<AddTileRequested>()
            .add_systems(
                Update,
                (
                    apply_color_selection,
                    apply_paint_requests,
                    apply_add_tile,
                )
                    .in_set(DocumentEdit),
            );
    }
}

/// Everything that mutates the document, so other plugins can order against it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocumentEdit;

// ============================================================================
// SYSTEMS
// ============================================================================

/// Selection is applied before painting, so a script can select and paint in the
/// same frame and get the colour it just asked for.
fn apply_color_selection(
    mut requests: MessageReader<SelectColorRequested>,
    mut palette: ResMut<Palette>,
) {
    for request in requests.read() {
        if !palette.select(request.index) {
            warn!(
                "ignoring selection of colour {} — palette has {}",
                request.index,
                palette.len()
            );
        }
    }
}

fn apply_paint_requests(
    mut requests: MessageReader<PaintPixelRequested>,
    palette: Res<Palette>,
    mut document: ResMut<AtlasDocument>,
) {
    let color = palette.selected_color();
    let mut changed = false;

    for request in requests.read() {
        if !document.contains(request.x, request.y) {
            continue;
        }
        if document.set_pixel_at(request.x, request.y, color) {
            changed = true;
        }
    }

    // One revision bump per frame however many pixels a drag covered.
    if changed {
        document.touch();
    }
}

fn apply_add_tile(
    mut requests: MessageReader<AddTileRequested>,
    mut document: ResMut<AtlasDocument>,
) {
    let mut added = 0;
    for request in requests.read() {
        for _ in 0..request.tiles {
            document.add_tile_to_right();
            added += 1;
        }
    }

    if added > 0 {
        document.touch();
    }
}

// ============================================================================
// STARTUP
// ============================================================================

/// Builds the starting document before anything is loaded from disk.
pub fn seed_document(
    mut commands: Commands,
    options: Res<crate::cli::LaunchOptions>,
) {
    let mut document = AtlasDocument::with_tile_size(options.tile_size);
    // Explicit: a fresh document is entirely transparent.
    document.pixels.fill(transparent());

    commands.insert_resource(document);
    commands.insert_resource(Palette::new());
    commands.insert_resource(DocumentPath {
        base: options.path.clone(),
        existed_on_disk: false,
    });
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Srgba = Srgba::new(1.0, 0.0, 0.0, 1.0);

    /// A minimal app with just the document plugin, so the appliers can be
    /// driven by messages exactly as they are at runtime.
    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(DocumentStatePlugin)
            .insert_resource(AtlasDocument::with_tile_size(4))
            .insert_resource(Palette::with_colors(vec![
                transparent(),
                RED,
                Srgba::new(0.0, 0.0, 1.0, 1.0),
            ]));
        app
    }

    #[test]
    fn painting_writes_the_selected_colour() {
        let mut app = test_app();
        app.world_mut().resource_mut::<Palette>().select(1);
        app.world_mut().write_message(PaintPixelRequested { x: 1, y: 2 });
        app.update();

        let document = app.world().resource::<AtlasDocument>();
        assert_eq!(document.get_pixel_at(1, 2), RED);
        assert!(document.unsaved);
        assert_eq!(document.revision, 1);
    }

    #[test]
    fn selecting_then_painting_in_one_frame_uses_the_new_colour() {
        let mut app = test_app();
        app.world_mut().write_message(SelectColorRequested { index: 1 });
        app.world_mut().write_message(PaintPixelRequested { x: 0, y: 0 });
        app.update();

        assert_eq!(app.world().resource::<AtlasDocument>().get_pixel_at(0, 0), RED);
    }

    #[test]
    fn a_paint_outside_the_atlas_changes_nothing() {
        let mut app = test_app();
        app.world_mut().resource_mut::<Palette>().select(1);
        app.world_mut().write_message(PaintPixelRequested { x: 99, y: 99 });
        app.update();

        let document = app.world().resource::<AtlasDocument>();
        assert_eq!(document.revision, 0);
        assert!(!document.unsaved);
    }

    #[test]
    fn repainting_the_same_colour_does_not_bump_the_revision() {
        let mut app = test_app();
        app.world_mut().resource_mut::<Palette>().select(1);
        app.world_mut().write_message(PaintPixelRequested { x: 0, y: 0 });
        app.update();
        let after_first = app.world().resource::<AtlasDocument>().revision;

        app.world_mut().write_message(PaintPixelRequested { x: 0, y: 0 });
        app.update();

        assert_eq!(
            app.world().resource::<AtlasDocument>().revision,
            after_first,
            "a no-op paint must not dirty the document"
        );
    }

    #[test]
    fn a_multi_pixel_drag_bumps_the_revision_once() {
        let mut app = test_app();
        app.world_mut().resource_mut::<Palette>().select(1);
        for x in 0..3 {
            app.world_mut().write_message(PaintPixelRequested { x, y: 0 });
        }
        app.update();

        assert_eq!(app.world().resource::<AtlasDocument>().revision, 1);
    }

    #[test]
    fn adding_a_tile_widens_the_document() {
        let mut app = test_app();
        app.world_mut().write_message(AddTileRequested { tiles: 2 });
        app.update();

        let document = app.world().resource::<AtlasDocument>();
        assert_eq!(document.width_in_tiles, 3);
        assert!(document.unsaved);
    }

    #[test]
    fn an_invalid_selection_leaves_the_draw_colour_alone() {
        let mut app = test_app();
        app.world_mut().resource_mut::<Palette>().select(1);
        app.world_mut().write_message(SelectColorRequested { index: 42 });
        app.update();

        assert_eq!(app.world().resource::<Palette>().selected, 1);
    }
}
