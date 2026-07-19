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
const TOP_MARGIN: f32 = 10.0;

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

/// Calculate palette panel x position
/// Panel should be at left edge of screen
/// In Bevy Camera2d: screen center is (0,0), left edge at -window_width/2
/// Panel width is PALETTE_PANEL_WIDTH, so center at: -window_width/2 + PALETTE_PANEL_WIDTH/2
fn panel_x(window_width: f32) -> f32 {
    -window_width / 2.0 + PALETTE_PANEL_WIDTH / 2.0
}

/// Calculate palette panel y position
/// Panel height = window_height - TOP_MARGIN (leaving margin at bottom)
/// Panel top edge should be at screen top: window_height/2
/// So center_y = window_height/2 - panel_height/2
///          = window_height/2 - (window_height - TOP_MARGIN)/2
///          = TOP_MARGIN/2
fn panel_y() -> f32 {
    TOP_MARGIN / 2.0
}

/// Calculate palette panel height
fn panel_height(window_height: f32) -> f32 {
    // Leave TOP_MARGIN at top and bottom
    window_height - TOP_MARGIN
}

/// Setup the palette panel
fn setup_palette_panel(mut commands: Commands, windows: Query<&Window>) {
    let window = windows.single().unwrap();
    let x = panel_x(window.width());
    let y = panel_y();
    let height = panel_height(window.height());
    
    commands.spawn((
        Sprite::from_color(Color::Srgba(Srgba::new(0.15, 0.15, 0.15, 0.8)), Vec2::new(PALETTE_PANEL_WIDTH, height)),
        Transform::from_xyz(x, y, 0.0),
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
    let window = windows.single().unwrap();

    // Get or create the palette panel
    let panel_entity = if let Ok(entity) = panel_query.single() {
        entity
    } else {
        let x = panel_x(window.width());
        let y = panel_y();
        let height = panel_height(window.height());
        commands.spawn((
            Sprite::from_color(Color::Srgba(Srgba::new(0.15, 0.15, 0.15, 0.8)), Vec2::new(PALETTE_PANEL_WIDTH, height)),
            Transform::from_xyz(x, y, 0.0),
            PalettePanel,
        )).id()
    };

    // Despawn old swatches
    for entity in swatch_query.iter() {
        commands.entity(entity).despawn();
    }

    let mut y_offset = 0.0;

    for (index, color) in palette.colors.iter().enumerate() {
        let is_selected = palette.selected_index == Some(index);
        let swatch_size = COLOR_SWATCH_SIZE + SWATCH_BORDER * 2.0;
        let border_color = if is_selected {
            Color::Srgba(css::WHITE)
        } else {
            Color::Srgba(Srgba::new(0.4, 0.4, 0.4, 0.8))
        };

        // Position swatches relative to panel center (which is at panel_y)
        // First swatch at top: y = panel_y + panel_height/2 - swatch_size/2 - margin
        // Then each subsequent swatch below it
        let border_entity = commands.spawn((
            Sprite::from_color(border_color, Vec2::new(swatch_size, swatch_size)),
            Transform::from_xyz(
                0.0,
                -y_offset - swatch_size / 2.0,
                0.0,
            ),
            ColorSwatch { color_index: index },
        )).id();

        let inner_color_entity = commands.spawn((
            Sprite::from_color(*color, Vec2::new(COLOR_SWATCH_SIZE, COLOR_SWATCH_SIZE)),
            Transform::from_xyz(0.0, 0.0, 0.1),
        )).id();

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
                            let size = sprite.custom_size.unwrap_or(Vec2::new(
                                COLOR_SWATCH_SIZE + SWATCH_BORDER * 2.0,
                                COLOR_SWATCH_SIZE + SWATCH_BORDER * 2.0
                            ));
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
