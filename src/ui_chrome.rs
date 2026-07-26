//! The bevy_ui chrome: the Feathers palette sidebar and the status strip.
//!
//! Swatch colours live in the Feathers `UiTheme` as one token per palette index,
//! generated from the *loaded* palette — so the sidebar shows the colours the
//! document actually has, with no second hardcoded list. That is also why the
//! initial build must run after `DocumentLoad`: the theme is derived from the
//! palette, so the palette has to be final first.
//!
//! The palette is editable, so that derivation has to hold at runtime too:
//!
//! - **Recolouring** an entry only rewrites its token. Feathers repaints every
//!   `ThemeBackgroundColor` when `UiTheme` changes, so the swatch follows for
//!   free and no entity is touched.
//! - **Adding or removing** an entry changes how many tokens there are, so the
//!   swatch row is despawned and respawned against the new palette. That is the
//!   cost of keeping the colours in the theme, and it is only paid on the two
//!   clicks that change the palette's length.
//!
//! Under the swatches sit four Feathers colour sliders — one per RGBA channel —
//! which edit the *selected* entry. They are a view, not a store: they emit
//! [`SetPaletteColorRequested`] and read their position back from `Palette`, so
//! the palette stays the single source of truth even mid-drag.

use bevy::feathers::controls::{
    ColorChannel, FeathersButton, FeathersColorSlider, SliderBaseColor,
};
use bevy::feathers::dark_theme::create_dark_theme;
use bevy::feathers::display::label_small;
use bevy::feathers::focus::FocusIndicator;
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeToken, UiTheme};
use bevy::feathers::FeathersPlugins;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::scene::SceneList;
use bevy::text::FontSize;
use bevy::ui_widgets::{Activate, ScrollArea, SliderValue, ValueChange};

use crate::atlas::AtlasDocument;
use crate::canvas_view::HoveredPixel;
use crate::config::{
    EDITOR_FONT_SIZE, EDITOR_ROW_GAP, NEW_PALETTE_COLOR, SIDEBAR_BACKGROUND, SIDEBAR_WIDTH,
    STATUS_BACKGROUND, STATUS_BAR_HEIGHT, STATUS_FONT_SIZE, STATUS_TEXT, SWATCH_BORDER, SWATCH_GAP,
    SWATCH_SIZE,
};
use crate::document::{
    AddPaletteColorRequested, DocumentPath, RemovePaletteColorRequested, SelectColorRequested,
    SetPaletteColorRequested,
};
use crate::palette::Palette;

// ============================================================================
// COMPONENTS
// ============================================================================

/// A palette swatch, tagged with the colour index it selects.
#[derive(Component, Debug, Clone, Copy)]
pub struct PaletteSwatch {
    pub index: usize,
}

/// The wrapping row the swatches live in; respawned into when the palette's
/// length changes.
#[derive(Component)]
pub struct SwatchGrid;

/// One of the four channel sliders under the swatches.
// No `Debug`: Feathers' `ColorChannel` does not derive it.
#[derive(Component, Clone, Copy)]
pub struct PaletteChannelSlider {
    pub channel: ColorChannel,
}

#[derive(Component)]
pub struct Sidebar;

#[derive(Component)]
pub struct StatusStrip;

#[derive(Component)]
pub struct StatusLabel;

/// The channels the sidebar edits, in the order they are stacked. RGBA rather
/// than HSL because that is how a colour is stored and reported: what the
/// sliders show and what the status strip's hex says cannot drift.
const EDITED_CHANNELS: [(&str, ColorChannel); 4] = [
    ("R", ColorChannel::Red),
    ("G", ColorChannel::Green),
    ("B", ColorChannel::Blue),
    ("A", ColorChannel::Alpha),
];

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
            .add_systems(
                Update,
                (
                    // The theme has to carry the new colours before any swatch
                    // that reads a token is spawned.
                    (sync_palette_theme, rebuild_swatches).chain(),
                    sync_channel_sliders,
                    mark_selected_swatch,
                    update_status_label,
                )
                    // A palette edit applied this frame should reach the sidebar
                    // this frame, not the next one.
                    .after(crate::document::DocumentEdit),
            );
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
// CHANNELS
// ============================================================================

/// The component of `color` a slider on `channel` shows.
///
/// Only the four channels in [`EDITED_CHANNELS`] can occur; the HSL variants
/// would need a colour-space conversion, and the sidebar never builds one.
fn channel_value(color: Srgba, channel: ColorChannel) -> f32 {
    match channel {
        ColorChannel::Red => color.red,
        ColorChannel::Green => color.green,
        ColorChannel::Blue => color.blue,
        ColorChannel::Alpha => color.alpha,
        _ => 0.0,
    }
}

/// `color` with one channel moved to `value` — the whole colour a slider drag
/// asks for, so the applier stays a plain assignment.
fn with_channel(color: Srgba, channel: ColorChannel, value: f32) -> Srgba {
    let value = value.clamp(0.0, 1.0);
    match channel {
        ColorChannel::Red => Srgba { red: value, ..color },
        ColorChannel::Green => Srgba {
            green: value,
            ..color
        },
        ColorChannel::Blue => Srgba {
            blue: value,
            ..color
        },
        ColorChannel::Alpha => Srgba {
            alpha: value,
            ..color
        },
        _ => color,
    }
}

// ============================================================================
// STARTUP
// ============================================================================

/// Rewrites the palette tokens to match `palette`, dropping any left over from
/// a longer palette so a stale token cannot resurface under a later colour.
fn write_palette_tokens(palette: &Palette, theme: &mut UiTheme) {
    for (index, color) in palette.colors.iter().enumerate() {
        theme.0.color.insert(palette_token(index), Color::Srgba(*color));
    }

    let mut index = palette.len();
    while theme.0.color.remove(&palette_token(index)).is_some() {
        index += 1;
    }
}

fn install_palette_theme(palette: Res<Palette>, mut theme: ResMut<UiTheme>) {
    write_palette_tokens(&palette, &mut theme);
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
                flex_direction: FlexDirection::Column,
                padding: UiRect {
                    left: px(SWATCH_GAP),
                    right: px(SWATCH_GAP),
                    top: px(SWATCH_GAP),
                    // The status strip is a sibling drawn over the bottom of the
                    // window, so keep the editor clear of it.
                    bottom: px(STATUS_BAR_HEIGHT + SWATCH_GAP),
                },
                row_gap: px(SWATCH_GAP),
                ..default()
            },
            BackgroundColor(SIDEBAR_BACKGROUND),
            Sidebar,
        ))
        .id();

    let grid = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_content: AlignContent::FlexStart,
                // Takes whatever the editor below it leaves, and scrolls when a
                // long palette overflows it — `+` has no upper bound, so a
                // clipped swatch has to stay reachable.
                flex_grow: 1.0,
                min_height: px(0),
                row_gap: px(SWATCH_GAP),
                column_gap: px(SWATCH_GAP),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            // Wheel scrolling; `Overflow::scroll_y` alone only clips. Brings
            // `ScrollPosition` with it.
            ScrollArea,
            ChildOf(sidebar),
            SwatchGrid,
        ))
        .id();

    for index in 0..palette.len() {
        spawn_swatch(&mut commands, grid, index);
    }

    build_palette_editor(&mut commands, sidebar, palette.selected_color());

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

/// The add/remove row and the four channel sliders, in that order: the buttons
/// change *which* colour is being edited, the sliders change the colour itself.
fn build_palette_editor(commands: &mut Commands, sidebar: Entity, selected: Srgba) {
    let editor = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
                row_gap: px(EDITOR_ROW_GAP),
                ..default()
            },
            ChildOf(sidebar),
        ))
        .id();

    let buttons = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(EDITOR_ROW_GAP),
                ..default()
            },
            ChildOf(editor),
        ))
        .id();

    commands
        .spawn_scene(add_color_button())
        .insert(ChildOf(buttons));
    commands
        .spawn_scene(remove_color_button())
        .insert(ChildOf(buttons));

    for (caption, channel) in EDITED_CHANNELS {
        let row = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(EDITOR_ROW_GAP),
                    ..default()
                },
                ChildOf(editor),
            ))
            .id();

        commands.spawn_scene(label_small(caption)).insert((
            Node {
                width: px(EDITOR_FONT_SIZE),
                ..default()
            },
            ChildOf(row),
        ));

        // Seeded with the selected colour so the first frame is already right
        // rather than snapping into place once `sync_channel_sliders` runs.
        commands
            .spawn_scene(channel_slider(channel, channel_value(selected, channel)))
            .insert((
                ChildOf(row),
                PaletteChannelSlider { channel },
                SliderBaseColor(Color::Srgba(selected)),
            ));
    }
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

fn spawn_swatch(commands: &mut Commands, grid: Entity, index: usize) {
    commands
        .spawn_scene(swatch(index))
        .insert((ChildOf(grid), PaletteSwatch { index }))
        // Both of these repaint the button and would hide its colour.
        .remove::<Hovered>()
        .remove::<FocusIndicator>();
}

/// One channel slider. It reports the *whole* colour it wants, not the channel
/// that moved, so the request stays meaningful however many sliders move in a
/// frame — and it targets the selection, which is what the editor edits.
///
/// Named rather than inlined into the scene so the tests can trigger it without
/// standing up the whole UI: this is the one piece of the sidebar a screenshot
/// cannot exercise, because the harness has no pointer to drag with.
fn on_channel_dragged(
    channel: ColorChannel,
) -> impl Fn(On<ValueChange<f32>>, Res<Palette>, MessageWriter<SetPaletteColorRequested>)
       + Clone
       + Send
       + Sync
       + 'static {
    move |change, palette, mut requests| {
        requests.write(SetPaletteColorRequested {
            index: palette.selected,
            color: with_channel(palette.selected_color(), channel, change.value),
        });
    }
}

fn on_add_clicked(
    _activate: On<Activate>,
    mut requests: MessageWriter<AddPaletteColorRequested>,
) {
    requests.write(AddPaletteColorRequested {
        color: NEW_PALETTE_COLOR,
    });
}

fn on_remove_clicked(
    _activate: On<Activate>,
    palette: Res<Palette>,
    mut requests: MessageWriter<RemovePaletteColorRequested>,
) {
    requests.write(RemovePaletteColorRequested {
        index: palette.selected,
    });
}

fn channel_slider(channel: ColorChannel, value: f32) -> impl Scene {
    bsn! {
        @FeathersColorSlider {
            @channel: {channel},
            @value: {value},
        }
        on(on_channel_dragged(channel))
    }
}

fn add_color_button() -> impl Scene {
    bsn! {
        @FeathersButton {
            @caption: {Box::new(bsn_list![label_small("+")]) as Box<dyn SceneList>},
        }
        Node {
            flex_grow: 1.0,
        }
        on(on_add_clicked)
    }
}

fn remove_color_button() -> impl Scene {
    bsn! {
        @FeathersButton {
            @caption: {Box::new(bsn_list![label_small("-")]) as Box<dyn SceneList>},
        }
        Node {
            flex_grow: 1.0,
        }
        on(on_remove_clicked)
    }
}

// ============================================================================
// SYSTEMS
// ============================================================================

/// A recoloured entry reaches its swatch through the theme, so this is all an
/// edit costs: Feathers repaints every `ThemeBackgroundColor` when `UiTheme`
/// changes.
fn sync_palette_theme(palette: Res<Palette>, mut theme: ResMut<UiTheme>) {
    if !palette.is_changed() {
        return;
    }
    // Only reached on a real change: touching `UiTheme` repaints every themed
    // node in the tree, so doing it every frame would not be free.
    write_palette_tokens(&palette, &mut theme);
}

/// Adding or removing a colour changes how many tokens there are, which no
/// amount of repainting can express — so the swatches are rebuilt against the
/// new palette. Recolouring leaves the count alone and skips all of this.
fn rebuild_swatches(
    mut commands: Commands,
    palette: Res<Palette>,
    grid: Query<Entity, With<SwatchGrid>>,
    swatches: Query<Entity, With<PaletteSwatch>>,
) {
    if !palette.is_changed() {
        return;
    }
    if swatches.iter().count() == palette.len() {
        return;
    }
    let Ok(grid) = grid.single() else {
        return;
    };

    for entity in &swatches {
        commands.entity(entity).despawn();
    }
    for index in 0..palette.len() {
        spawn_swatch(&mut commands, grid, index);
    }
}

/// The sliders show the selected colour, and nothing else ever sets them —
/// including mid-drag, where the value that comes back is the one the palette
/// accepted rather than the one the pointer asked for.
fn sync_channel_sliders(
    mut commands: Commands,
    palette: Res<Palette>,
    mut sliders: Query<(Entity, &PaletteChannelSlider, &SliderValue, &mut SliderBaseColor)>,
) {
    if !palette.is_changed() {
        return;
    }

    let color = palette.selected_color();
    for (entity, slider, value, mut base) in &mut sliders {
        let target = channel_value(color, slider.channel);
        // Both writes are guarded: an unchanged one would still register as a
        // change and make Feathers move the thumb and rebuild the track
        // gradient every frame. `SliderValue` is immutable, so it is replaced
        // rather than assigned — which is also what keeps its accessibility
        // value in step.
        if value.0 != target {
            commands.entity(entity).insert(SliderValue(target));
        }
        if base.0 != Color::Srgba(color) {
            base.0 = Color::Srgba(color);
        }
    }
}

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
        "{}{}    {}    {} [{}/{}]    {}x{} tiles of {}px",
        path.display_name(),
        if document.unsaved { "*" } else { "" },
        position,
        palette.selected_hex(),
        // Which entry and how many there are, so adding and removing one is
        // readable straight off a screenshot.
        palette.selected,
        palette.len(),
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

    const RED: Srgba = Srgba::new(1.0, 0.0, 0.0, 1.0);
    const GREEN: Srgba = Srgba::new(0.0, 1.0, 0.0, 1.0);

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
            .insert_resource(Palette::with_colors(vec![RED, GREEN]))
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

    #[test]
    fn a_recoloured_entry_reaches_its_token() {
        let mut theme = UiTheme(create_dark_theme());
        let mut palette = Palette::with_colors(vec![RED, GREEN]);
        write_palette_tokens(&palette, &mut theme);

        palette.replace_color(1, Srgba::new(0.0, 0.0, 1.0, 1.0));
        write_palette_tokens(&palette, &mut theme);

        assert_eq!(
            theme.0.color.get(&palette_token(1)),
            Some(&Color::srgba(0.0, 0.0, 1.0, 1.0))
        );
    }

    /// A shrunken palette must not leave a token behind: the next colour added
    /// would otherwise inherit the removed one's colour until it was rewritten.
    #[test]
    fn shrinking_the_palette_drops_the_tokens_it_no_longer_needs() {
        let mut theme = UiTheme(create_dark_theme());
        let mut palette = Palette::new();
        write_palette_tokens(&palette, &mut theme);
        let full_length = palette.len();

        while palette.len() > 2 {
            palette.remove_color(palette.len() - 1);
        }
        write_palette_tokens(&palette, &mut theme);

        assert!(theme.0.color.get(&palette_token(1)).is_some());
        for index in 2..full_length {
            assert!(
                theme.0.color.get(&palette_token(index)).is_none(),
                "token {index} outlived its colour"
            );
        }
    }

    #[test]
    fn a_slider_reports_the_channel_it_edits() {
        let color = Srgba::new(0.1, 0.2, 0.3, 0.4);
        assert_eq!(channel_value(color, ColorChannel::Red), 0.1);
        assert_eq!(channel_value(color, ColorChannel::Green), 0.2);
        assert_eq!(channel_value(color, ColorChannel::Blue), 0.3);
        assert_eq!(channel_value(color, ColorChannel::Alpha), 0.4);
    }

    #[test]
    fn moving_one_channel_leaves_the_others_alone() {
        let color = Srgba::new(0.1, 0.2, 0.3, 0.4);
        assert_eq!(
            with_channel(color, ColorChannel::Green, 0.9),
            Srgba::new(0.1, 0.9, 0.3, 0.4)
        );
        assert_eq!(
            with_channel(color, ColorChannel::Alpha, 0.0),
            Srgba::new(0.1, 0.2, 0.3, 0.0)
        );
    }

    #[test]
    fn a_channel_setting_and_reading_round_trip() {
        for (_, channel) in EDITED_CHANNELS {
            let edited = with_channel(Srgba::new(0.5, 0.5, 0.5, 0.5), channel, 0.75);
            assert_eq!(
                channel_value(edited, channel),
                0.75,
                "a slider must read back what it wrote"
            );
        }
    }

    #[test]
    fn an_out_of_range_slider_value_is_clamped_rather_than_stored() {
        let color = Srgba::new(0.5, 0.5, 0.5, 1.0);
        assert_eq!(with_channel(color, ColorChannel::Red, 1.5).red, 1.0);
        assert_eq!(with_channel(color, ColorChannel::Red, -0.5).red, 0.0);
    }

    /// What a widget emitted, read the way a real consumer would — see the note
    /// on `Recorded` in `input.rs` for why this is not read off `Messages`.
    #[derive(Resource, Default)]
    struct Recorded {
        sets: Vec<(usize, Srgba)>,
        adds: Vec<Srgba>,
        removes: Vec<usize>,
    }

    fn record(
        mut sets: MessageReader<SetPaletteColorRequested>,
        mut adds: MessageReader<AddPaletteColorRequested>,
        mut removes: MessageReader<RemovePaletteColorRequested>,
        mut recorded: ResMut<Recorded>,
    ) {
        let seen: Vec<(usize, Srgba)> = sets.read().map(|m| (m.index, m.color)).collect();
        recorded.sets.extend(seen);
        let seen: Vec<Srgba> = adds.read().map(|m| m.color).collect();
        recorded.adds.extend(seen);
        let seen: Vec<usize> = removes.read().map(|m| m.index).collect();
        recorded.removes.extend(seen);
    }

    /// The sidebar's widgets without the sidebar: the observers are attached to
    /// a bare entity, so a trigger stands in for the drag or click the harness
    /// has no pointer to perform.
    fn widget_app(palette: Palette) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<SetPaletteColorRequested>()
            .add_message::<AddPaletteColorRequested>()
            .add_message::<RemovePaletteColorRequested>()
            .init_resource::<Recorded>()
            .insert_resource(palette)
            .add_systems(Update, record);
        app
    }

    #[test]
    fn dragging_a_channel_asks_for_the_whole_colour_it_wants() {
        let mut app = widget_app(Palette::with_colors(vec![RED, GREEN]));
        app.world_mut().resource_mut::<Palette>().select(1);

        let slider = app
            .world_mut()
            .spawn_empty()
            .observe(on_channel_dragged(ColorChannel::Red))
            .id();
        // `ValueChange` is generic, and an untyped `0.5` would infer as `f64` —
        // a different event, which the observer would silently never see.
        app.world_mut().trigger(ValueChange::<f32> {
            source: slider,
            value: 0.5,
            is_final: true,
        });
        app.update();

        assert_eq!(
            app.world().resource::<Recorded>().sets,
            vec![(1, Srgba::new(0.5, 1.0, 0.0, 1.0))],
            "the request must target the selection and keep the untouched channels"
        );
    }

    #[test]
    fn the_add_button_appends_a_visible_colour() {
        let mut app = widget_app(Palette::with_colors(vec![RED]));
        let button = app.world_mut().spawn_empty().observe(on_add_clicked).id();
        app.world_mut().trigger(Activate { entity: button });
        app.update();

        assert_eq!(app.world().resource::<Recorded>().adds, vec![NEW_PALETTE_COLOR]);
        assert_eq!(
            NEW_PALETTE_COLOR.alpha, 1.0,
            "appending a transparent colour would look like nothing happened"
        );
    }

    #[test]
    fn the_remove_button_targets_the_selection() {
        let mut app = widget_app(Palette::with_colors(vec![RED, GREEN, RED]));
        app.world_mut().resource_mut::<Palette>().select(2);

        let button = app.world_mut().spawn_empty().observe(on_remove_clicked).id();
        app.world_mut().trigger(Activate { entity: button });
        app.update();

        assert_eq!(app.world().resource::<Recorded>().removes, vec![2]);
    }
}
