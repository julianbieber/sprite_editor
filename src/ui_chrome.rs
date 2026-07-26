//! The bevy_ui chrome: the Feathers palette sidebar and the status strip.
//!
//! Swatch colours live in the Feathers `UiTheme` as one token per palette index,
//! generated from the *loaded* palette — so the sidebar shows the colours the
//! document actually has, with no second hardcoded list. That is also why this
//! must run after `DocumentLoad`: the theme is derived from the palette, so the
//! palette has to be final first.
//!
//! Because the colours are in the theme, adding or removing a palette colour
//! later means rebuilding the theme and respawning the sidebar. Acceptable while
//! palette editing is out of scope.

use bevy::feathers::controls::FeathersButton;
use bevy::feathers::dark_theme::create_dark_theme;
use bevy::feathers::focus::FocusIndicator;
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeToken, UiTheme};
use bevy::feathers::FeathersPlugins;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::ui_widgets::Activate;

use crate::atlas::AtlasDocument;
use crate::canvas_view::HoveredPixel;
use crate::config::{
    SIDEBAR_BACKGROUND, SIDEBAR_WIDTH, STATUS_BACKGROUND, STATUS_BAR_HEIGHT, STATUS_FONT_SIZE,
    STATUS_TEXT, SWATCH_BORDER, SWATCH_GAP, SWATCH_SIZE,
};
use crate::document::{DocumentPath, SelectColorRequested};
use crate::palette::Palette;

// ============================================================================
// COMPONENTS
// ============================================================================

/// A palette swatch, tagged with the colour index it selects.
#[derive(Component, Debug, Clone, Copy)]
pub struct PaletteSwatch {
    pub index: usize,
}

#[derive(Component)]
pub struct Sidebar;

#[derive(Component)]
pub struct StatusStrip;

#[derive(Component)]
pub struct StatusLabel;

// ============================================================================
// PLUGIN
// ============================================================================

pub struct UiChromePlugin;

impl Plugin for UiChromePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FeathersPlugins)
            // A baseline theme; the palette tokens get folded in at startup.
            .insert_resource(UiTheme(create_dark_theme()))
            .add_systems(
                Startup,
                (install_palette_theme, build_chrome)
                    .chain()
                    .in_set(ChromeBuild),
            )
            .add_systems(Update, (mark_selected_swatch, update_status_label));
    }
}

/// Ordered after `DocumentLoad` by the app assembly.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChromeBuild;

/// The theme key carrying palette colour `index`.
fn palette_token(index: usize) -> ThemeToken {
    ThemeToken::new(format!("ColorPalette::{index}").into())
}

// ============================================================================
// STARTUP
// ============================================================================

fn install_palette_theme(palette: Res<Palette>, mut theme: ResMut<UiTheme>) {
    for (index, color) in palette.colors.iter().enumerate() {
        theme.0.color.insert(palette_token(index), Color::Srgba(*color));
    }
}

fn build_chrome(mut commands: Commands, palette: Res<Palette>) {
    let sidebar = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: px(SIDEBAR_WIDTH),
                height: percent(100),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_content: AlignContent::FlexStart,
                padding: UiRect::all(px(SWATCH_GAP)),
                row_gap: px(SWATCH_GAP),
                column_gap: px(SWATCH_GAP),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(SIDEBAR_BACKGROUND),
            ScrollPosition::default(),
            Sidebar,
        ))
        .id();

    for index in 0..palette.len() {
        commands
            .spawn_scene(swatch(index))
            .insert((ChildOf(sidebar), PaletteSwatch { index }))
            // Both of these repaint the button and would hide its colour.
            .remove::<Hovered>()
            .remove::<FocusIndicator>();
    }

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                bottom: px(0),
                width: percent(100),
                height: px(STATUS_BAR_HEIGHT),
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(px(10)),
                ..default()
            },
            BackgroundColor(STATUS_BACKGROUND),
            StatusStrip,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(String::new()),
                TextFont {
                    font_size: FontSize::Px(STATUS_FONT_SIZE),
                    ..default()
                },
                TextColor(STATUS_TEXT),
                StatusLabel,
            ));
        });
}

/// One swatch: a Feathers button filled from its theme token, which emits a
/// selection request when activated.
fn swatch(index: usize) -> impl Scene {
    bsn! {
        Node {
            width: px(SWATCH_SIZE),
            height: px(SWATCH_SIZE),
        }
        @FeathersButton {
        }
        ThemeBackgroundColor(palette_token(index))
        on(move |_activate: On<Activate>, mut requests: MessageWriter<SelectColorRequested>| {
            requests.write(SelectColorRequested { index });
        })
    }
}

// ============================================================================
// SYSTEMS
// ============================================================================

/// Keeps the outline on whichever swatch the palette says is selected.
fn mark_selected_swatch(
    mut commands: Commands,
    palette: Res<Palette>,
    swatches: Query<(Entity, &PaletteSwatch, Option<&Outline>)>,
) {
    for (entity, swatch, outline) in &swatches {
        match (swatch.index == palette.selected, outline.is_some()) {
            (true, false) => {
                commands.entity(entity).insert(Outline {
                    width: px(SWATCH_BORDER),
                    color: Color::WHITE,
                    ..default()
                });
            }
            (false, true) => {
                commands.entity(entity).remove::<Outline>();
            }
            // Already correct — don't touch it, so change detection stays quiet.
            _ => {}
        }
    }
}

/// Derived state only. Doubles as the screenshot's caption: it makes a capture
/// self-describing, so a coordinate can be read off rather than guessed.
fn update_status_label(
    document: Res<AtlasDocument>,
    palette: Res<Palette>,
    path: Res<DocumentPath>,
    hovered: Res<HoveredPixel>,
    mut labels: Query<&mut Text, With<StatusLabel>>,
) {
    let position = if hovered.valid {
        format!(
            "{},{}   tile {}",
            hovered.x,
            hovered.y,
            document.tile_of(hovered.x)
        )
    } else {
        "off canvas".to_string()
    };

    let status = format!(
        "{}{}    {}    {}    {}x{} tiles of {}px",
        path.display_name(),
        if document.unsaved { "*" } else { "" },
        position,
        palette.selected_hex(),
        document.width_in_tiles,
        document.height_in_tiles,
        document.tile_size,
    );

    for mut label in &mut labels {
        if label.0 != status {
            label.0 = status.clone();
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_palette_index_gets_its_own_theme_token() {
        let palette = Palette::new();
        let tokens: std::collections::HashSet<ThemeToken> =
            (0..palette.len()).map(palette_token).collect();
        assert_eq!(
            tokens.len(),
            palette.len(),
            "tokens must be unique per colour or swatches would share a colour"
        );
    }

    #[test]
    fn the_theme_is_populated_from_the_loaded_palette() {
        let mut app = App::new();
        app.insert_resource(UiTheme(create_dark_theme()))
            // A short, non-default palette: the theme must follow *this*, not
            // the built-in list.
            .insert_resource(Palette::with_colors(vec![
                Srgba::new(1.0, 0.0, 0.0, 1.0),
                Srgba::new(0.0, 1.0, 0.0, 1.0),
            ]))
            .add_systems(Startup, install_palette_theme);
        app.update();

        let theme = app.world().resource::<UiTheme>();
        assert_eq!(
            theme.0.color.get(&palette_token(0)),
            Some(&Color::srgba(1.0, 0.0, 0.0, 1.0))
        );
        assert_eq!(
            theme.0.color.get(&palette_token(1)),
            Some(&Color::srgba(0.0, 1.0, 0.0, 1.0))
        );
        assert!(
            theme.0.color.get(&palette_token(2)).is_none(),
            "no token should exist for a colour the palette does not have"
        );
    }
}
