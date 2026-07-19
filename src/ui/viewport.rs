//! Viewport rendering for the sprite atlas editor

use crate::config::{PALETTE_PANEL_WIDTH, PIXEL_SCALE};
use crate::editor::EditorResource;
use bevy::prelude::*;

/// Marker component for the atlas viewport
#[derive(Component)]
pub struct AtlasViewport;

/// Marker component for an individual pixel sprite
#[derive(Component)]
pub struct AtlasPixel {
    pub atlas_x: u32,
    pub atlas_y: u32,
}

/// Marker component for a grid line (visual only, not saved)
#[derive(Component)]
pub struct GridLine {
    pub is_vertical: bool,
    pub position: f32,
}

/// Resource to track which pixel is under the mouse
#[derive(Resource, Default)]
pub struct HoveredPixel {
    pub atlas_x: Option<u32>,
    pub atlas_y: Option<u32>,
}

/// Plugin for the atlas viewport
pub struct ViewportPlugin;

impl Plugin for ViewportPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HoveredPixel>()
            .add_systems(Startup, setup_viewport)
            .add_systems(
                Update,
                (
                    update_viewport_rendering,
                    update_grid_lines,
                    handle_pixel_hover,
                    handle_pixel_draw,
                    handle_scroll_input,
                    handle_undo_redo_input,
                    handle_tile_input,
                    handle_save_load_input,
                ),
            )
            .add_systems(PostUpdate, cleanup_pixels);
    }
}

/// Setup the viewport
fn setup_viewport(mut commands: Commands) {
    commands.spawn((Transform::default(), AtlasViewport));
}

/// Calculate the left edge x-coordinate for the atlas
/// In Bevy Camera2d: screen center is (0,0), left edge at -window_width/2
/// Palette is at left edge with width PALETTE_PANEL_WIDTH
/// So palette covers: [-window_width/2, -window_width/2 + PALETTE_PANEL_WIDTH]
/// Atlas should start at: -window_width/2 + PALETTE_PANEL_WIDTH
fn atlas_start_x(window_width: f32) -> f32 {
    -window_width / 2.0 + PALETTE_PANEL_WIDTH
}

/// Update the viewport rendering
fn update_viewport_rendering(
    editor: Res<EditorResource>,
    windows: Query<&Window>,
    mut commands: Commands,
    pixel_query: Query<Entity, With<AtlasPixel>>,
) {
    let atlas = &editor.editor.atlas;
    let viewport = &editor.editor.viewport;
    let window = windows.single().unwrap();
    let start_x = atlas_start_x(window.width());

    for entity in pixel_query.iter() {
        commands.entity(entity).despawn();
    }

    let atlas_width = atlas.width_in_pixels();
    let atlas_height = atlas.height_in_pixels();

    for y in 0..atlas_height {
        for x in 0..atlas_width {
            let pixel_color = atlas.get_pixel_at(x, y);
            
            // Pixel center in screen coords:
            // start_x (left edge of atlas) + x * PIXEL_SCALE (left edge of pixel) + PIXEL_SCALE/2 (center of pixel)
            // Then subtract viewport offset (screen pixels)
            let screen_x = start_x + (x as f32 + 0.5) * PIXEL_SCALE as f32 - viewport.offset_x;
            
            // In Bevy Camera2d: screen top is at window_height/2, center at 0, bottom at -window_height/2
            // Atlas (0,0) is top-left, so pixel (0,0) top edge should be at screen top (window_height/2)
            // Since sprite is center-anchored: pixel (0,0) center = window_height/2 - PIXEL_SCALE/2
            // For y=n: pixel center = window_height/2 - (n + 0.5) * PIXEL_SCALE
            let screen_y = window.height() / 2.0 - (y as f32 + 0.5) * PIXEL_SCALE as f32 + viewport.offset_y;

            commands.spawn((
                Sprite::from_color(
                    pixel_color,
                    Vec2::new(PIXEL_SCALE as f32, PIXEL_SCALE as f32),
                ),
                Transform::from_xyz(screen_x, screen_y, 0.0),
                AtlasPixel {
                    atlas_x: x,
                    atlas_y: y,
                },
            ));
        }
    }
}

/// Update grid lines to show tile boundaries
fn update_grid_lines(
    editor: Res<EditorResource>,
    windows: Query<&Window>,
    mut commands: Commands,
    grid_query: Query<Entity, With<GridLine>>,
) {
    let atlas = &editor.editor.atlas;
    let viewport = &editor.editor.viewport;
    let window = windows.single().unwrap();
    let start_x = atlas_start_x(window.width());
    let tile_size = atlas.tile_size as f32;

    for entity in grid_query.iter() {
        commands.entity(entity).despawn();
    }

    let atlas_width = atlas.width_in_pixels() as f32;
    let atlas_height = atlas.height_in_pixels() as f32;
    let atlas_width_screen = atlas_width * PIXEL_SCALE as f32;
    let atlas_height_screen = atlas_height * PIXEL_SCALE as f32;

    let grid_thickness = 2.0;
    let grid_color = Color::Srgba(Srgba::new(0.3, 0.3, 0.3, 0.8));

    // Vertical grid lines - centered at atlas vertical center
    let atlas_top_y = window.height() / 2.0 - atlas_height_screen / 2.0 + viewport.offset_y;
    let center_y = atlas_top_y - atlas_height_screen / 2.0;
    let mut x_atlas = 0.0;
    while x_atlas <= atlas_width {
        let screen_x = start_x + x_atlas * PIXEL_SCALE as f32 - viewport.offset_x;
        commands.spawn((
            Sprite::from_color(grid_color, Vec2::new(grid_thickness, atlas_height_screen)),
            Transform::from_xyz(screen_x, center_y, 1.0),
            GridLine { is_vertical: true, position: x_atlas },
        ));
        x_atlas += tile_size;
    }

    // Horizontal grid lines
    let center_x = start_x + atlas_width_screen / 2.0 - viewport.offset_x;
    let mut y_atlas = 0.0;
    while y_atlas <= atlas_height {
        // Match the pixel y-coordinate system: screen_y = window_height/2 - (y + 0.5) * PIXEL_SCALE + offset_y
        let screen_y = window.height() / 2.0 - (y_atlas as f32 + 0.5) * PIXEL_SCALE as f32 + viewport.offset_y;
        commands.spawn((
            Sprite::from_color(grid_color, Vec2::new(atlas_width_screen, grid_thickness)),
            Transform::from_xyz(center_x, screen_y, 1.0),
            GridLine { is_vertical: false, position: y_atlas },
        ));
        y_atlas += tile_size;
    }
}

/// Handle mouse hover
fn handle_pixel_hover(
    editor: Res<EditorResource>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    mut hovered: ResMut<HoveredPixel>,
) {
    let viewport = &editor.editor.viewport;
    let window = windows.single().unwrap();
    let start_x = atlas_start_x(window.width());
    let atlas = &editor.editor.atlas;

    if let Some(cursor_position) = window.cursor_position() {
        if let Ok((camera, camera_transform)) = camera_q.single() {
            if let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, cursor_position) {
                // Reverse the screen_x formula:
                // screen_x = start_x + (x + 0.5) * PIXEL_SCALE - offset_x
                // => (x + 0.5) * PIXEL_SCALE = screen_x - start_x + offset_x
                // => x = (screen_x - start_x + offset_x) / PIXEL_SCALE - 0.5
                let atlas_x = ((world_position.x - start_x + viewport.offset_x) / PIXEL_SCALE as f32).floor() as u32;
                
                // Reverse the screen_y formula:
                // screen_y = window_height/2 - (y + 0.5) * PIXEL_SCALE + offset_y
                // => (y + 0.5) * PIXEL_SCALE = window_height/2 + offset_y - screen_y
                // => y + 0.5 = (window_height/2 + offset_y - screen_y) / PIXEL_SCALE
                // => y = (window_height/2 + offset_y - screen_y) / PIXEL_SCALE - 0.5
                let atlas_y = ((window.height() / 2.0 + viewport.offset_y - world_position.y) / PIXEL_SCALE as f32 - 0.5).floor() as u32;
                
                if atlas.is_valid_atlas_pixel(atlas_x, atlas_y) {
                    hovered.atlas_x = Some(atlas_x);
                    hovered.atlas_y = Some(atlas_y);
                    return;
                }
            }
        }
    }
    hovered.atlas_x = None;
    hovered.atlas_y = None;
}

/// Handle mouse clicks to draw pixels
fn handle_pixel_draw(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredPixel>,
    mut editor: ResMut<EditorResource>,
) {
    if mouse_buttons.just_pressed(MouseButton::Left) {
        if let (Some(x), Some(y)) = (hovered.atlas_x, hovered.atlas_y) {
            if editor.editor.atlas.is_valid_atlas_pixel(x, y) {
                if let Some(selected_index) = editor.editor.palette.selected_index {
                    if let Some(color) = editor.editor.palette.colors.get(selected_index).cloned() {
                        let old_color = editor.editor.atlas.get_pixel_at(x, y);
                        editor.editor.atlas.set_pixel_at(x, y, color);
                        editor.editor.history.record(crate::history::Action::SetPixel {
                            x, y, old_color, new_color: color,
                        });
                    }
                }
            }
        }
    }
}

/// Handle keyboard input for scrolling
fn handle_scroll_input(keyboard: Res<ButtonInput<KeyCode>>, mut editor: ResMut<EditorResource>) {
    if keyboard.just_pressed(KeyCode::KeyA) {
        editor.editor.viewport.scroll_tile_left();
    }
    if keyboard.just_pressed(KeyCode::KeyD) {
        editor.editor.viewport.scroll_tile_right();
    }
}

/// Handle keyboard input for undo/redo
fn handle_undo_redo_input(keyboard: Res<ButtonInput<KeyCode>>, mut editor: ResMut<EditorResource>) {
    if (keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight))
        && keyboard.just_pressed(KeyCode::KeyZ)
    {
        editor.editor.undo();
    }
    if (keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight))
        && keyboard.just_pressed(KeyCode::KeyY)
    {
        editor.editor.redo();
    }
}

/// Handle keyboard input for tile operations
fn handle_tile_input(keyboard: Res<ButtonInput<KeyCode>>, mut editor: ResMut<EditorResource>) {
    if keyboard.just_pressed(KeyCode::Space) {
        editor.editor.add_tile_to_right();
    }
    if keyboard.just_pressed(KeyCode::Backspace) {
        editor.editor.remove_tile_from_right();
    }
}

/// Cleanup pixel sprites
fn cleanup_pixels(
    editor: Res<EditorResource>,
    mut commands: Commands,
    pixel_query: Query<(Entity, &AtlasPixel)>,
) {
    let atlas_width = editor.editor.atlas.width_in_pixels();
    let atlas_height = editor.editor.atlas.height_in_pixels();

    for (entity, pixel) in pixel_query.iter() {
        if pixel.atlas_x >= atlas_width || pixel.atlas_y >= atlas_height {
            commands.entity(entity).despawn();
        }
    }
}

/// Handle keyboard input for save/load operations
fn handle_save_load_input(keyboard: Res<ButtonInput<KeyCode>>, _editor: ResMut<EditorResource>) {
    if (keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight))
        && keyboard.just_pressed(KeyCode::KeyS)
    {
        println!("Save triggered (Ctrl+S)");
    }
    if (keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight))
        && keyboard.just_pressed(KeyCode::KeyO)
    {
        println!("Load triggered (Ctrl+O)");
    }
}
