//! Palette UI for the sprite atlas editor

use bevy::prelude::*;
use bevy::color::palettes::css;
use crate::config::{COLOR_SWATCH_SIZE, PALETTE_PANEL_WIDTH, DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT};
use crate::editor::EditorResource;

/// Marker component for the palette panel
#[derive(Component)]
pub struct PalettePanel;

/// Marker component for a color swatch
#[derive(Component)]
pub struct ColorSwatch {
    pub color_index: usize,
}

const SWATCH_MARGIN: f32 = 4.0;
const SWATCH_BORDER: f32 = 2.0;

/// Plugin for the palette UI
pub struct PaletteUiPlugin;

impl Plugin for PaletteUiPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Startup, setup_palette_panel)
            .add_systems(Update, (
                update_palette_display,
                handle_swatch_selection,
            ));
    }
}

/// Setup the palette panel
fn setup_palette_panel(mut commands: Commands, windows: Query<&Window>) {
    // Position palette panel on the left border of the screen, aligned to top
    // With Camera2d, (0,0) is center, left edge is at -window_width/2, top at window_height/2
    // Panel width is PALETTE_PANEL_WIDTH, so its center x should be at:
    // x = -window_width/2 + PALETTE_PANEL_WIDTH/2
    // Panel should be at top, so y = window_height/2 - PALETTE_PANEL_HEIGHT/2
    // But we don't have a fixed height, so we position at top and let swatches flow down
    let window_width = if let Ok(window) = windows.single() {
        window.width()
    } else {
        DEFAULT_WINDOW_WIDTH as f32
    };
    let window_height = if let Ok(window) = windows.single() {
        window.height()
    } else {
        DEFAULT_WINDOW_HEIGHT as f32
    };
    let panel_x = -window_width / 2.0 + PALETTE_PANEL_WIDTH / 2.0;
    // Position at top: y = window_height/2 - some margin
    let panel_y = window_height / 2.0 - 20.0; // 20px margin from top
    
    commands.spawn((
        Transform::from_xyz(panel_x, panel_y, 0.0),
        PalettePanel,
    ));
}

/// Update the palette display
fn update_palette_display(
    editor: Res<EditorResource>,
    mut commands: Commands,
    windows: Query<&Window>,
    panel_query: Query<Entity, With<PalettePanel>>,
    swatch_query: Query<Entity, With<ColorSwatch>>,
) {
    let palette = &editor.editor.palette;
    
    // Get or create the palette panel
    let panel_entity = if let Ok(entity) = panel_query.single() {
        entity
    } else {
        // Create panel if it doesn't exist at the left border, top-aligned
        let window_width = if let Ok(window) = windows.single() {
            window.width()
        } else {
            DEFAULT_WINDOW_WIDTH as f32
        };
        let window_height = if let Ok(window) = windows.single() {
            window.height()
        } else {
            DEFAULT_WINDOW_HEIGHT as f32
        };
        let panel_x = -window_width / 2.0 + PALETTE_PANEL_WIDTH / 2.0;
        let panel_y = window_height / 2.0 - 20.0;
        commands.spawn((
            Transform::from_xyz(panel_x, panel_y, 0.0),
            PalettePanel,
        )).id()
    };
    
    // Despawn old swatches
    for entity in swatch_query.iter() {
        commands.entity(entity).despawn();
    }
    
    let mut y_offset = 20.0;
    
    for (index, color) in palette.colors.iter().enumerate() {
        let is_selected = palette.selected_index == Some(index);
        let swatch_size = COLOR_SWATCH_SIZE + SWATCH_BORDER * 2.0;
        let border_color = if is_selected {
            Color::Srgba(css::WHITE)
        } else {
            Color::Srgba(Srgba::new(0.3, 0.3, 0.3, 0.8))
        };
        
        // Position swatches relative to panel center
        let border_entity = commands.spawn((
            Sprite::from_color(border_color, Vec2::new(swatch_size, swatch_size)),
            Transform::from_xyz(
                0.0,
                -y_offset + swatch_size / 2.0,
                0.0,
            ),
            ColorSwatch { color_index: index },
        )).id();
        
        let inner_color_entity = commands.spawn((
            Sprite::from_color(*color, Vec2::new(COLOR_SWATCH_SIZE, COLOR_SWATCH_SIZE)),
            Transform::from_xyz(0.0, 0.0, 0.1),
        )).id();
        
        // Make swatches children of the panel
        // Use add_child so the Transform is relative to parent
        commands.entity(panel_entity).add_child(border_entity);
        commands.entity(border_entity).add_child(inner_color_entity);
        
        y_offset += swatch_size + SWATCH_MARGIN * 2.0;
    }
}

/// Handle clicks on color swatches
fn handle_swatch_selection(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    swatch_query: Query<(&ColorSwatch, &GlobalTransform, &Sprite)>,
    mut editor: ResMut<EditorResource>,
) {
    if mouse_buttons.just_pressed(MouseButton::Left) {
        if let Ok(window) = windows.single() {
            if let Some(cursor_position) = window.cursor_position() {
                if let Ok((camera, camera_transform)) = camera_q.single() {
                    if let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, cursor_position) {
                        for (swatch, global_transform, sprite) in swatch_query.iter() {
                            let size = sprite.custom_size.unwrap_or(Vec2::new(COLOR_SWATCH_SIZE + SWATCH_BORDER * 2.0, COLOR_SWATCH_SIZE + SWATCH_BORDER * 2.0));
                            if world_position.x >= global_transform.translation().x - size.x / 2.0
                                && world_position.x < global_transform.translation().x + size.x / 2.0
                                && world_position.y >= global_transform.translation().y - size.y / 2.0
                                && world_position.y < global_transform.translation().y + size.y / 2.0 {
                                editor.editor.palette.selected_index = Some(swatch.color_index);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
}
