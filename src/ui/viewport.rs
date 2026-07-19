//! Viewport rendering for the sprite atlas editor

use bevy::prelude::*;
use crate::config::{PIXEL_SCALE, DEFAULT_TILE_SIZE, PALETTE_PANEL_WIDTH};
use crate::editor::EditorResource;
use crate::file_io;

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
    pub position: f32, // position in atlas pixels
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
        app
            .init_resource::<HoveredPixel>()
            .add_systems(Startup, setup_viewport)
            .add_systems(Update, (
                update_viewport_rendering,
                update_grid_lines,
                handle_pixel_hover,
                handle_pixel_draw,
                handle_scroll_input,
                handle_undo_redo_input,
                handle_tile_input,
                handle_save_load_input,
            ))
            .add_systems(PostUpdate, cleanup_pixels);
    }
}

/// Setup the viewport
fn setup_viewport(mut commands: Commands) {
    commands.spawn((
        Transform::default(),
        AtlasViewport,
    ));
}

/// Update the viewport rendering
fn update_viewport_rendering(
    editor: Res<EditorResource>,
    mut commands: Commands,
    pixel_query: Query<Entity, With<AtlasPixel>>,
) {
    let atlas = &editor.editor.atlas;
    let viewport = &editor.editor.viewport;
    
    for entity in pixel_query.iter() {
        commands.entity(entity).despawn();
    }
    
    let atlas_width = atlas.width_in_pixels();
    let atlas_height = atlas.height_in_pixels();
    
    for y in 0..atlas_height {
        for x in 0..atlas_width {
            let pixel_color = atlas.get_pixel_at(x, y);
            // Position viewport to the right of the palette panel
            // Palette panel is at -PALETTE_PANEL_WIDTH/2 and is 200 wide, so it covers -200 to 0
            // Viewport starts at 0 (right edge of palette)
            let screen_x = x as f32 * PIXEL_SCALE as f32 - viewport.offset_x;
            // Flip y-coordinate: atlas uses top-left origin, Bevy Camera2d has y-up
            // So we negate y to make atlas (0,0) appear at top
            let screen_y = -(y as f32) * PIXEL_SCALE as f32 + viewport.offset_y;
            
            commands.spawn((
                Sprite::from_color(pixel_color, Vec2::new(PIXEL_SCALE as f32, PIXEL_SCALE as f32)),
                Transform::from_xyz(screen_x, screen_y, 0.0),
                AtlasPixel { atlas_x: x, atlas_y: y },
            ));
        }
    }
}

/// Update grid lines to show tile boundaries
fn update_grid_lines(
    editor: Res<EditorResource>,
    mut commands: Commands,
    grid_query: Query<Entity, With<GridLine>>,
) {
    let atlas = &editor.editor.atlas;
    let viewport = &editor.editor.viewport;
    let tile_size = atlas.tile_size as f32;
    
    // Despawn old grid lines
    for entity in grid_query.iter() {
        commands.entity(entity).despawn();
    }
    
    // Calculate grid line positions
    let atlas_width = atlas.width_in_pixels() as f32;
    let atlas_height = atlas.height_in_pixels() as f32;
    
    // Vertical grid lines (between tiles, at tile boundaries)
    // Each vertical line is at x = tile_size, 2*tile_size, etc.
    // Line spans from y=0 to y=atlas_height (in atlas coords)
    // In screen coords: y_screen = -y_atlas * 64 + offset_y
    // So the line goes from (-0*64 + offset_y) to (-atlas_height*64 + offset_y)
    // Center of line in screen y: offset_y - atlas_height*32
    let mut x = tile_size;
    while x < atlas_width {
        let screen_x = x * PIXEL_SCALE as f32 - viewport.offset_x;
        let line_height = atlas_height * PIXEL_SCALE as f32;
        // Center the line vertically with the atlas
        // Top of atlas at screen_y = offset_y, bottom at screen_y = -atlas_height*64 + offset_y
        // Middle: (offset_y + (-atlas_height*64 + offset_y)) / 2 = offset_y - atlas_height*32
        let center_y = viewport.offset_y - atlas_height * PIXEL_SCALE as f32 / 2.0;
        
        // Spawn a vertical line sprite (tall and thin)
        commands.spawn((
            Sprite::from_color(Color::Srgba(Srgba::new(0.5, 0.5, 0.5, 0.5)), Vec2::new(1.0, line_height)),
            Transform::from_xyz(screen_x, center_y, 0.1),
            GridLine { is_vertical: true, position: x },
        ));
        x += tile_size;
    }
    
    // Horizontal grid lines (between tiles, at tile boundaries)
    // Each horizontal line is at y = tile_size, 2*tile_size, etc.
    // Line spans from x=0 to x=atlas_width (in atlas coords)
    // In screen coords: x_screen = x_atlas * 64 - offset_x
    // So the line goes from (-offset_x) to (atlas_width*64 - offset_x)
    // Center of line in screen x: (atlas_width*64 - offset_x) / 2 - offset_x/2... no
    // Actually center is: atlas_width*32 - offset_x
    let mut y = tile_size;
    while y < atlas_height {
        let screen_y = -(y as f32) * PIXEL_SCALE as f32 + viewport.offset_y;
        let line_width = atlas_width * PIXEL_SCALE as f32;
        // Center the line horizontally with the atlas
        // Left of atlas at screen_x = -offset_x, right at screen_x = atlas_width*64 - offset_x
        // Middle: (-offset_x + atlas_width*64 - offset_x) / 2 = atlas_width*32 - offset_x
        let center_x = atlas_width * PIXEL_SCALE as f32 / 2.0 - viewport.offset_x;
        
        // Spawn a horizontal line sprite (wide and thin)
        commands.spawn((
            Sprite::from_color(Color::Srgba(Srgba::new(0.5, 0.5, 0.5, 0.5)), Vec2::new(line_width, 1.0)),
            Transform::from_xyz(center_x, screen_y, 0.1),
            GridLine { is_vertical: false, position: y },
        ));
        y += tile_size;
    }
}

/// Handle mouse hover
fn handle_pixel_hover(
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    pixel_query: Query<(&AtlasPixel, &Transform)>,
    mut hovered: ResMut<HoveredPixel>,
) {
    if let Ok(window) = windows.single() {
        if let Some(cursor_position) = window.cursor_position() {
            if let Ok((camera, camera_transform)) = camera_q.single() {
                if let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, cursor_position) {
                    for (pixel, transform) in pixel_query.iter() {
                        let pixel_size = PIXEL_SCALE as f32;
                        if world_position.x >= transform.translation.x
                            && world_position.x < transform.translation.x + pixel_size
                            && world_position.y >= transform.translation.y
                            && world_position.y < transform.translation.y + pixel_size {
                            hovered.atlas_x = Some(pixel.atlas_x);
                            hovered.atlas_y = Some(pixel.atlas_y);
                            return;
                        }
                    }
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
                // Clone the selected color before mutating
                if let Some(selected_index) = editor.editor.palette.selected_index {
                    if let Some(color) = editor.editor.palette.colors.get(selected_index).cloned() {
                        let old_color = editor.editor.atlas.get_pixel_at(x, y);
                        editor.editor.atlas.set_pixel_at(x, y, color);
                        editor.editor.history.record(crate::history::Action::SetPixel {
                            x,
                            y,
                            old_color,
                            new_color: color,
                        });
                    }
                }
            }
        }
    }
}

/// Handle keyboard input for scrolling
fn handle_scroll_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut editor: ResMut<EditorResource>,
) {
    // Check for scroll left (o umlaut key: \u00F6)
    if keyboard.just_pressed(KeyCode::KeyO) {
        editor.editor.viewport.scroll_tile_left();
    }
    
    // Check for scroll right (# key)
    // Note: In Bevy, the '#' character is typically KeyCode::Digit3 (shift+3 produces #)
    if keyboard.just_pressed(KeyCode::Digit3) {
        editor.editor.viewport.scroll_tile_right();
    }
}

/// Handle keyboard input for undo/redo
fn handle_undo_redo_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut editor: ResMut<EditorResource>,
) {
    // Undo: Ctrl+Z
    if (keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight))
        && keyboard.just_pressed(KeyCode::KeyZ) {
        editor.editor.undo();
    }
    
    // Redo: Ctrl+Y
    if (keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight))
        && keyboard.just_pressed(KeyCode::KeyY) {
        editor.editor.redo();
    }
}

/// Handle keyboard input for tile operations
fn handle_tile_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut editor: ResMut<EditorResource>,
) {
    // Add tile to the right: Space bar
    if keyboard.just_pressed(KeyCode::Space) {
        editor.editor.add_tile_to_right();
    }
    
    // Remove tile from the right: Backspace
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
fn handle_save_load_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    _editor: ResMut<EditorResource>,
) {
    // Save: Ctrl+S
    if (keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight))
        && keyboard.just_pressed(KeyCode::KeyS) {
        // For now, just log - actual save will need a file path
        println!("Save triggered (Ctrl+S) - file picker needed for actual implementation");
        // TODO: Open file dialog and save
        // file_io::save_project(&editor.editor, &path).unwrap_or_else(|e| {
        //     eprintln!("Failed to save: {}", e);
        // });
    }
    
    // Load: Ctrl+O
    if (keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight))
        && keyboard.just_pressed(KeyCode::KeyO) {
        // For now, just log - actual load will need a file path
        println!("Load triggered (Ctrl+O) - file picker needed for actual implementation");
        // TODO: Open file dialog and load
        // match file_io::load_project(&path) {
        //     Ok(loaded_editor) => {
        //         *editor = EditorResource { editor: loaded_editor };
        //     }
        //     Err(e) => {
        //         eprintln!("Failed to load: {}", e);
        //     }
        // }
    }
}
