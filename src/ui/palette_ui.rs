//! Palette UI for the sprite atlas editor

use bevy::prelude::*;
use bevy::color::palettes::css;
use crate::config::{COLOR_SWATCH_SIZE, PALETTE_PANEL_WIDTH};
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
fn setup_palette_panel(mut commands: Commands) {
    commands.spawn((
        Transform::default(),
        PalettePanel,
    ));
}

/// Update the palette display
fn update_palette_display(
    editor: Res<EditorResource>,
    mut commands: Commands,
    swatch_query: Query<Entity, With<ColorSwatch>>,
) {
    let palette = &editor.editor.palette;
    
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
        
        let border_entity = commands.spawn((
            Sprite::from_color(border_color, Vec2::new(swatch_size, swatch_size)),
            Transform::from_xyz(
                -PALETTE_PANEL_WIDTH / 2.0 + swatch_size / 2.0,
                -y_offset + swatch_size / 2.0,
                0.0,
            ),
            ColorSwatch { color_index: index },
        )).id();
        
        let inner_color_entity = commands.spawn((
            Sprite::from_color(*color, Vec2::new(COLOR_SWATCH_SIZE, COLOR_SWATCH_SIZE)),
            Transform::from_xyz(0.0, 0.0, 0.1),
        )).id();
        
        commands.entity(inner_color_entity).set_parent_in_place(border_entity);
        
        y_offset += swatch_size + SWATCH_MARGIN * 2.0;
    }
}

/// Handle clicks on color swatches
fn handle_swatch_selection(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    swatch_query: Query<(&ColorSwatch, &Transform, &Sprite)>,
    mut editor: ResMut<EditorResource>,
) {
    if mouse_buttons.just_pressed(MouseButton::Left) {
        if let Ok(window) = windows.single() {
            if let Some(cursor_position) = window.cursor_position() {
                if let Ok((camera, camera_transform)) = camera_q.single() {
                    if let Ok(world_position) = camera.viewport_to_world_2d(camera_transform, cursor_position) {
                        for (swatch, transform, sprite) in swatch_query.iter() {
                            let size = sprite.custom_size.unwrap_or(Vec2::new(COLOR_SWATCH_SIZE, COLOR_SWATCH_SIZE));
                            if world_position.x >= transform.translation.x - size.x / 2.0
                                && world_position.x < transform.translation.x + size.x / 2.0
                                && world_position.y >= transform.translation.y - size.y / 2.0
                                && world_position.y < transform.translation.y + size.y / 2.0 {
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
