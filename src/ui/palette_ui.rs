//! Palette UI for the sprite atlas editor

use bevy::ecs::relationship::RelationshipSourceCollection;
use bevy::feathers::controls::FeathersButton;
use bevy::feathers::dark_theme::create_dark_theme;
use bevy::feathers::focus::FocusIndicator;
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeToken, UiTheme};
use bevy::feathers::FeathersPlugins;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

/// Plugin for the palette UI
pub struct PaletteUiPlugin;

impl Plugin for PaletteUiPlugin {
    fn build(&self, app: &mut App) {
        let mut theme = create_dark_theme();
        let colors = [
            Color::srgba(1.0, 1.0, 1.0, 0.0),
            Color::srgba(0.8, 0.2, 0.2, 1.0),
            Color::srgba(0.2, 0.8, 0.2, 1.0),
            Color::srgba(0.2, 0.2, 0.8, 1.0),
            Color::srgba(0.8, 0.8, 0.2, 1.0),
        ];

        for (i, color) in colors.into_iter().enumerate() {
            theme.color.insert(palette_theme(i), color);
        }

        app.add_plugins(FeathersPlugins);
        app.insert_resource(SelectedPaletteOption {
            index: 0,
            entity: Entity::new(),
        });
        app.insert_resource(UiTheme(theme));
        app.add_systems(Startup, setup_palette_panel.spawn())
            .add_systems(Update, (populate_buttons,));
    }
}

/// Setup the palette panel
fn setup_palette_panel() -> impl Scene {
    bsn! {
        Node {
            width: percent(5),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            position_type: PositionType::Absolute,
            left: px(20),
            top: px(20),
            overflow: Overflow::scroll_y(),
            row_gap: px(6),
        }
        ScrollPosition::default()
        ColorPickerRoot
    }
}

fn populate_buttons(
    mut commands: Commands,
    root: Query<Entity, Added<ColorPickerRoot>>,
    mut selected: ResMut<SelectedPaletteOption>,
) {
    let Ok(root_ent) = root.single() else { return };

    for i in 0..5 {
        let mut e = commands.spawn_scene(colored_button(i));
        e.insert(ChildOf(root_ent))
            .remove::<Hovered>() // Hovered overrides the theme key
            .remove::<FocusIndicator>();
        if i == 0 {
            e.insert(Outline {
                width: px(2),
                color: Color::WHITE,
                ..Default::default()
            });
            selected.entity = e.id();
        }
    }
}

#[derive(Component, Default, Clone)]
struct ColorPickerRoot;

fn colored_button(i: usize) -> impl Scene {
    bsn! {
        Node{top:px(3), left:px(3), right:px(3), bottom: px(3)}
        @FeathersButton {
        }
        ThemeBackgroundColor(palette_theme(i))
        on(move |activate: On<Activate>,  commands: Commands, previous: ResMut<SelectedPaletteOption>| {
            on_palette_click(activate, i, commands, previous);
        })

    }
}

fn palette_theme(i: usize) -> ThemeToken {
    ThemeToken::new(format!("ColorPalette::{i}").into())
}

#[derive(Resource)]
struct SelectedPaletteOption {
    index: usize,
    entity: Entity,
}

fn on_palette_click(
    activate: On<Activate>,
    index: usize,
    mut commands: Commands,
    mut previous: ResMut<SelectedPaletteOption>,
) {
    commands
        .get_entity(previous.entity)
        .unwrap()
        .remove::<Outline>();

    commands
        .get_entity(activate.entity)
        .unwrap()
        .insert(Outline {
            width: px(2),
            color: Color::WHITE,
            ..Default::default()
        });

    previous.index = index;
    previous.entity = activate.entity;
}
