//! The canvas view: the one and only mapping between screen and atlas space.
//!
//! The canvas has its own camera whose `viewport` covers just the region the
//! sidebar and status strip leave free. That is what keeps this mapping honest:
//! `viewport_to_world_2d` divides by the *viewport* rect, so canvas world space
//! owes nothing to the sidebar width, and no placement formula anywhere needs to
//! know about it.
//!
//! `atlas_left()` / `atlas_top()` / `pixel_scale` are the whole interface.
//! `canvas_render` draws with them and `track_hovered_pixel` inverts them, so
//! drawing and hit testing cannot drift apart.

use bevy::camera::Viewport;
use bevy::prelude::*;

use crate::atlas::AtlasDocument;
use crate::config::{SIDEBAR_WIDTH, STATUS_BAR_HEIGHT};

// ============================================================================
// REQUESTS
// ============================================================================

/// Scroll the canvas by whole tiles. Negative is left.
#[derive(Message, Debug, Clone, Copy)]
pub struct ScrollRequested {
    pub tiles: i32,
}

// ============================================================================
// RESOURCES
// ============================================================================

/// Where the canvas is and how big its pixels are.
#[derive(Resource, Debug, Clone)]
pub struct CanvasView {
    /// Horizontal scroll in screen pixels; 0 is the atlas's left edge.
    pub scroll_x: f32,
    /// Screen pixels per atlas pixel.
    pub pixel_scale: f32,
    /// Top-left of the canvas region, window-relative logical pixels.
    pub rect_min: Vec2,
    /// Size of the canvas region in logical pixels.
    pub rect_size: Vec2,
}

impl Default for CanvasView {
    fn default() -> Self {
        Self {
            scroll_x: 0.0,
            pixel_scale: crate::config::DEFAULT_PIXEL_SCALE,
            rect_min: Vec2::new(SIDEBAR_WIDTH, 0.0),
            rect_size: Vec2::new(640.0, 480.0),
        }
    }
}

impl CanvasView {
    /// World x of the atlas's left edge. Canvas world space is centred on the
    /// viewport, so the left edge is at `-width/2`, shifted by the scroll.
    #[inline]
    pub fn atlas_left(&self) -> f32 {
        -self.rect_size.x / 2.0 - self.scroll_x
    }

    /// World y of the atlas's top edge — pinned to the top of the viewport.
    #[inline]
    pub fn atlas_top(&self) -> f32 {
        self.rect_size.y / 2.0
    }

    /// How far the atlas can scroll before its right edge reaches the viewport's.
    pub fn max_scroll(&self, document: &AtlasDocument) -> f32 {
        let atlas_width = document.width_in_pixels() as f32 * self.pixel_scale;
        (atlas_width - self.rect_size.x).max(0.0)
    }

    /// World rect a given atlas pixel covers. Shared by the renderer and the
    /// tests, so "what is drawn" has exactly one definition.
    #[allow(dead_code)]
    pub fn pixel_world_min(&self, x: u32, y: u32) -> Vec2 {
        Vec2::new(
            self.atlas_left() + x as f32 * self.pixel_scale,
            self.atlas_top() - (y + 1) as f32 * self.pixel_scale,
        )
    }

    /// Inverse of the placement: which atlas cell a world point falls in.
    /// `None` for anything above or left of the atlas origin.
    pub fn world_to_atlas(&self, world: Vec2) -> Option<(u32, u32)> {
        let x = (world.x - self.atlas_left()) / self.pixel_scale;
        let y = (self.atlas_top() - world.y) / self.pixel_scale;
        if x < 0.0 || y < 0.0 {
            return None;
        }
        Some((x.floor() as u32, y.floor() as u32))
    }

    /// Whether a window-relative logical cursor position is over the canvas.
    pub fn contains_cursor(&self, cursor: Vec2) -> bool {
        cursor.x >= self.rect_min.x
            && cursor.y >= self.rect_min.y
            && cursor.x < self.rect_min.x + self.rect_size.x
            && cursor.y < self.rect_min.y + self.rect_size.y
    }
}

/// The atlas pixel under the cursor, if any.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct HoveredPixel {
    pub x: u32,
    pub y: u32,
    pub valid: bool,
}

// ============================================================================
// PLUGIN
// ============================================================================

pub struct CanvasViewPlugin;

impl Plugin for CanvasViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ScrollRequested>()
            .init_resource::<HoveredPixel>()
            .add_systems(
                Update,
                (fit_canvas_camera, apply_scroll_requests, track_hovered_pixel)
                    .chain()
                    .in_set(CanvasViewUpdate),
            );
    }
}

/// So the renderer can run after the view settles.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanvasViewUpdate;

// ============================================================================
// SYSTEMS
// ============================================================================

/// Parks the canvas camera in the window region the chrome leaves free.
fn fit_canvas_camera(
    windows: Query<&Window>,
    mut cameras: Query<&mut Camera, With<crate::CanvasCamera>>,
    mut view: ResMut<CanvasView>,
) {
    let Ok(window) = windows.single() else { return };
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };

    let width = window.width() - SIDEBAR_WIDTH;
    let height = window.height() - STATUS_BAR_HEIGHT;
    // A window too small to hold the chrome keeps the previous viewport rather
    // than getting a degenerate one.
    if width < 1.0 || height < 1.0 {
        return;
    }

    let rect_min = Vec2::new(SIDEBAR_WIDTH, 0.0);
    let rect_size = Vec2::new(width, height);
    if view.rect_min != rect_min || view.rect_size != rect_size {
        view.rect_min = rect_min;
        view.rect_size = rect_size;
    }

    // The viewport itself is physical pixels.
    let scale = window.scale_factor();
    let position = (rect_min * scale).round().as_uvec2();
    let size = (rect_size * scale).round().as_uvec2().max(UVec2::ONE);

    let needs_update = match &camera.viewport {
        Some(current) => current.physical_position != position || current.physical_size != size,
        None => true,
    };
    if needs_update {
        camera.viewport = Some(Viewport {
            physical_position: position,
            physical_size: size,
            depth: 0.0..1.0,
        });
    }
}

/// Applies scroll requests, then clamps every frame — a window resize or a tile
/// removal can strand the offset past the end without any request at all.
fn apply_scroll_requests(
    mut requests: MessageReader<ScrollRequested>,
    document: Res<AtlasDocument>,
    mut view: ResMut<CanvasView>,
) {
    let step = document.tile_size as f32 * view.pixel_scale;
    let mut scroll_x = view.scroll_x;
    for request in requests.read() {
        scroll_x += request.tiles as f32 * step;
    }

    let clamped = scroll_x.clamp(0.0, view.max_scroll(&document));
    if clamped != view.scroll_x {
        view.scroll_x = clamped;
    }
}

/// The single cursor-to-atlas conversion in the editor.
fn track_hovered_pixel(
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<crate::CanvasCamera>>,
    view: Res<CanvasView>,
    document: Res<AtlasDocument>,
    mut hovered: ResMut<HoveredPixel>,
) {
    let mut resolved = HoveredPixel::default();

    if let Ok(window) = windows.single() {
        if let Some(cursor) = window.cursor_position() {
            if view.contains_cursor(cursor) {
                if let Ok((camera, camera_transform)) = cameras.single() {
                    if let Ok(world) = camera.viewport_to_world_2d(camera_transform, cursor) {
                        if let Some((x, y)) = view.world_to_atlas(world) {
                            if document.contains(x, y) {
                                resolved = HoveredPixel { x, y, valid: true };
                            }
                        }
                    }
                }
            }
        }
    }

    if resolved.valid != hovered.valid || resolved.x != hovered.x || resolved.y != hovered.y {
        *hovered = resolved;
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> CanvasView {
        CanvasView {
            scroll_x: 0.0,
            pixel_scale: 32.0,
            rect_min: Vec2::new(200.0, 0.0),
            rect_size: Vec2::new(800.0, 600.0),
        }
    }

    #[test]
    fn the_atlas_starts_at_the_top_left_of_the_viewport() {
        let view = view();
        assert_eq!(view.atlas_left(), -400.0);
        assert_eq!(view.atlas_top(), 300.0);
    }

    /// The invariant the whole unit exists for: what is drawn and what is hit
    /// tested must be inverses.
    #[test]
    fn placement_and_hit_testing_are_inverses() {
        let view = view();
        for (x, y) in [(0, 0), (1, 0), (0, 1), (7, 7), (23, 3)] {
            let min = view.pixel_world_min(x, y);
            // Sample the middle of the drawn cell.
            let centre = min + Vec2::splat(view.pixel_scale / 2.0);
            assert_eq!(
                view.world_to_atlas(centre),
                Some((x, y)),
                "pixel ({x},{y}) did not round trip"
            );
        }
    }

    #[test]
    fn inverses_still_hold_once_scrolled() {
        let mut view = view();
        view.scroll_x = 256.0;
        let min = view.pixel_world_min(9, 2);
        let centre = min + Vec2::splat(view.pixel_scale / 2.0);
        assert_eq!(view.world_to_atlas(centre), Some((9, 2)));
    }

    #[test]
    fn pixel_corners_belong_to_the_pixel_they_open() {
        let view = view();
        // Bottom-left corner of (3, 4) is inclusive; the cell above/left is not.
        let min = view.pixel_world_min(3, 4);
        let just_inside = Vec2::new(min.x + 0.5, min.y + 0.5);
        assert_eq!(view.world_to_atlas(just_inside), Some((3, 4)));

        let just_left = Vec2::new(min.x - 0.5, min.y + 0.5);
        assert_eq!(view.world_to_atlas(just_left), Some((2, 4)));
    }

    #[test]
    fn points_above_or_left_of_the_atlas_are_outside() {
        let view = view();
        assert_eq!(view.world_to_atlas(Vec2::new(-500.0, 0.0)), None);
        assert_eq!(view.world_to_atlas(Vec2::new(0.0, 400.0)), None);
    }

    #[test]
    fn scrolling_is_bounded_by_the_atlas_width() {
        let view = view();
        // 8 tiles of 8px at scale 32 = 2048 screen px, viewport is 800 wide.
        let mut document = AtlasDocument::with_tile_size(8);
        for _ in 0..7 {
            document.add_tile_to_right();
        }
        assert_eq!(view.max_scroll(&document), 2048.0 - 800.0);
    }

    #[test]
    fn an_atlas_narrower_than_the_viewport_cannot_scroll() {
        let view = view();
        let document = AtlasDocument::with_tile_size(8);
        assert_eq!(
            view.max_scroll(&document),
            0.0,
            "a single tile must stay pinned at the left"
        );
    }

    #[test]
    fn the_cursor_is_only_over_the_canvas_inside_its_rect() {
        let view = view();
        assert!(view.contains_cursor(Vec2::new(200.0, 0.0)));
        assert!(view.contains_cursor(Vec2::new(999.0, 599.0)));
        // Over the sidebar.
        assert!(!view.contains_cursor(Vec2::new(199.0, 300.0)));
        // Over the status strip.
        assert!(!view.contains_cursor(Vec2::new(500.0, 600.0)));
    }

    #[test]
    fn scroll_requests_move_by_whole_tiles_and_clamp_at_zero() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(CanvasViewPlugin)
            .insert_resource(AtlasDocument::with_tile_size(8))
            .insert_resource(view());

        // Nothing to scroll: one tile is narrower than the viewport.
        app.world_mut().write_message(ScrollRequested { tiles: 1 });
        app.update();
        assert_eq!(app.world().resource::<CanvasView>().scroll_x, 0.0);

        // Widen the atlas, then a tile step is 8 * 32 = 256.
        {
            let mut document = app.world_mut().resource_mut::<AtlasDocument>();
            for _ in 0..7 {
                document.add_tile_to_right();
            }
        }
        app.world_mut().write_message(ScrollRequested { tiles: 2 });
        app.update();
        assert_eq!(app.world().resource::<CanvasView>().scroll_x, 512.0);

        app.world_mut().write_message(ScrollRequested { tiles: -9 });
        app.update();
        assert_eq!(
            app.world().resource::<CanvasView>().scroll_x,
            0.0,
            "scrolling left past the origin must clamp"
        );
    }

    #[test]
    fn shrinking_the_atlas_re_clamps_a_stranded_scroll() {
        let mut app = App::new();
        let mut document = AtlasDocument::with_tile_size(8);
        for _ in 0..7 {
            document.add_tile_to_right();
        }
        app.add_plugins(MinimalPlugins)
            .add_plugins(CanvasViewPlugin)
            .insert_resource(document)
            .insert_resource(view());

        app.world_mut().write_message(ScrollRequested { tiles: 4 });
        app.update();
        assert_eq!(app.world().resource::<CanvasView>().scroll_x, 1024.0);

        // Drop back to one tile; the offset is now past the end.
        {
            let mut document = app.world_mut().resource_mut::<AtlasDocument>();
            while document.width_in_tiles > 1 {
                document.remove_tile_from_right();
            }
        }
        app.update();
        assert_eq!(app.world().resource::<CanvasView>().scroll_x, 0.0);
    }
}
