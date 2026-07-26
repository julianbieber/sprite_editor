//! Drawing the canvas: the atlas as a single nearest-sampled texture, with the
//! tile grid over it.
//!
//! One sprite, not one per pixel, and the texture is only reuploaded when the
//! document's revision moves — so an idle frame costs nothing and the entity
//! count does not grow with the atlas.
//!
//! Canvas content sits on [`CANVAS_LAYER`] so the full-window UI camera does not
//! also draw it across the sidebar.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::Anchor;

use crate::atlas::AtlasDocument;
use crate::canvas_view::{CanvasView, CanvasViewUpdate};
use crate::config::{
    CANVAS_LAYER, PIXEL_GRID_COLOR, PIXEL_GRID_MIN_SCALE, TILE_GRID_COLOR,
};

// ============================================================================
// COMPONENTS & RESOURCES
// ============================================================================

/// The single entity the whole atlas is drawn on.
#[derive(Component)]
pub struct CanvasSprite;

/// Tracks what is currently on the GPU, so uploads can be skipped.
#[derive(Resource, Default)]
pub struct CanvasTexture {
    pub handle: Handle<Image>,
    pub revision: u64,
    pub width: u32,
    pub height: u32,
    pub initialized: bool,
}

// ============================================================================
// PLUGIN
// ============================================================================

pub struct CanvasRenderPlugin;

impl Plugin for CanvasRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CanvasTexture>()
            .add_systems(Startup, (configure_canvas_gizmos, spawn_canvas_sprite))
            .add_systems(
                Update,
                (
                    sync_canvas_texture,
                    layout_canvas_sprite,
                    draw_tile_grid,
                )
                    .chain()
                    .after(CanvasViewUpdate),
            );
    }
}

// ============================================================================
// STARTUP
// ============================================================================

/// Confine gizmos to the canvas camera; otherwise the UI camera redraws the grid
/// across the whole window.
fn configure_canvas_gizmos(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
    config.render_layers = RenderLayers::layer(CANVAS_LAYER);
    config.line.width = 1.5;
}

fn spawn_canvas_sprite(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // A placeholder until the first sync; the real texture arrives before
    // anything renders.
    let placeholder = images.add(Image::new(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ));

    commands.spawn((
        Sprite::from_image(placeholder),
        // Anchoring top-left makes the sprite's translation *be* the atlas
        // origin, which is what the view's atlas_left/atlas_top describe.
        Anchor::TOP_LEFT,
        Transform::default(),
        RenderLayers::layer(CANVAS_LAYER),
        CanvasSprite,
    ));
}

// ============================================================================
// SYSTEMS
// ============================================================================

/// Reuploads the atlas image, but only when something actually changed.
fn sync_canvas_texture(
    document: Res<AtlasDocument>,
    mut texture: ResMut<CanvasTexture>,
    mut images: ResMut<Assets<Image>>,
    mut sprites: Query<&mut Sprite, With<CanvasSprite>>,
) {
    let width = document.width_in_pixels();
    let height = document.height_in_pixels();

    let up_to_date = texture.initialized
        && texture.revision == document.revision
        && texture.width == width
        && texture.height == height;
    if up_to_date {
        return;
    }

    let data = document.to_rgba8();
    let resized = texture.width != width || texture.height != height;

    if !texture.initialized || resized {
        // Dimensions changed, so this needs a new image asset.
        let handle = images.add(Image::new(
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        ));
        texture.handle = handle.clone();
        for mut sprite in &mut sprites {
            sprite.image = handle.clone();
        }
    } else if let Some(mut image) = images.get_mut(&texture.handle) {
        // Same size: overwrite in place.
        image.data = Some(data);
    }

    texture.revision = document.revision;
    texture.width = width;
    texture.height = height;
    texture.initialized = true;
}

/// Sizes and positions the canvas from the view.
fn layout_canvas_sprite(
    view: Res<CanvasView>,
    document: Res<AtlasDocument>,
    mut sprites: Query<(&mut Sprite, &mut Transform), With<CanvasSprite>>,
) {
    let size = Vec2::new(
        document.width_in_pixels() as f32 * view.pixel_scale,
        document.height_in_pixels() as f32 * view.pixel_scale,
    );
    let origin = Vec3::new(view.atlas_left(), view.atlas_top(), 0.0);

    for (mut sprite, mut transform) in &mut sprites {
        if sprite.custom_size != Some(size) {
            sprite.custom_size = Some(size);
        }
        if transform.translation != origin {
            transform.translation = origin;
        }
    }
}

/// Outlines every tile, so tile boundaries are visible even over opaque pixels.
fn draw_tile_grid(view: Res<CanvasView>, document: Res<AtlasDocument>, mut gizmos: Gizmos) {
    let scale = view.pixel_scale;
    let left = view.atlas_left();
    let top = view.atlas_top();
    let width_px = document.width_in_pixels();
    let height_px = document.height_in_pixels();
    let right = left + width_px as f32 * scale;
    let bottom = top - height_px as f32 * scale;
    let tile_size = document.tile_size;

    // Only bother with lines inside the viewport, plus a pixel of slack.
    let visible_min_x = -view.rect_size.x / 2.0 - scale;
    let visible_max_x = view.rect_size.x / 2.0 + scale;
    let visible = |x: f32| x >= visible_min_x && x <= visible_max_x;

    // Faint pixel grid underneath, only when a pixel is big enough to read.
    if scale >= PIXEL_GRID_MIN_SCALE {
        for x in 0..=width_px {
            if x % tile_size == 0 {
                continue; // the tile line covers it
            }
            let world_x = left + x as f32 * scale;
            if visible(world_x) {
                gizmos.line_2d(
                    Vec2::new(world_x, top),
                    Vec2::new(world_x, bottom),
                    PIXEL_GRID_COLOR,
                );
            }
        }
        for y in 0..=height_px {
            if y % tile_size == 0 {
                continue;
            }
            let world_y = top - y as f32 * scale;
            gizmos.line_2d(
                Vec2::new(left.max(visible_min_x), world_y),
                Vec2::new(right.min(visible_max_x), world_y),
                PIXEL_GRID_COLOR,
            );
        }
    }

    // Tile boundaries, including the far edge of the last tile so every tile
    // reads as closed.
    for tile_x in 0..=document.width_in_tiles {
        let world_x = left + (tile_x * tile_size) as f32 * scale;
        if visible(world_x) {
            gizmos.line_2d(
                Vec2::new(world_x, top),
                Vec2::new(world_x, bottom),
                TILE_GRID_COLOR,
            );
        }
    }
    for tile_y in 0..=document.height_in_tiles {
        let world_y = top - (tile_y * tile_size) as f32 * scale;
        gizmos.line_2d(
            Vec2::new(left.max(visible_min_x), world_y),
            Vec2::new(right.min(visible_max_x), world_y),
            TILE_GRID_COLOR,
        );
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// The sprite must cover exactly the atlas, at the scale the view states —
    /// this is the other half of the hit-testing contract.
    #[test]
    fn the_canvas_covers_exactly_the_atlas() {
        let view = CanvasView {
            scroll_x: 0.0,
            pixel_scale: 32.0,
            rect_min: Vec2::new(200.0, 0.0),
            rect_size: Vec2::new(800.0, 600.0),
        };
        let mut document = AtlasDocument::with_tile_size(8);
        document.add_tile_to_right();

        let size = Vec2::new(
            document.width_in_pixels() as f32 * view.pixel_scale,
            document.height_in_pixels() as f32 * view.pixel_scale,
        );
        assert_eq!(size, Vec2::new(512.0, 256.0));

        // Top-left anchored, so the sprite's own rect runs right and down from
        // the origin the view reports.
        let origin = Vec2::new(view.atlas_left(), view.atlas_top());
        assert_eq!(origin + Vec2::new(0.0, -size.y), view.pixel_world_min(0, 7));
    }

    #[test]
    fn texture_state_starts_uninitialised() {
        let texture = CanvasTexture::default();
        assert!(!texture.initialized);
        assert_eq!(texture.revision, 0);
    }
}
