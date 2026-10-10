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
//! Under the swatches sit the palette editor, and *which* editor depends on the
//! mode the button at the top of the sidebar selects:
//!
//! - **`list`** — four Feathers colour sliders, one per RGBA channel, plus the
//!   reorder and add/remove buttons. They edit the selected entry.
//! - **`oklab`** — the axis buttons and the sliders of one [`OklabRamp`]: the two
//!   values it holds fixed, the range it sweeps the third over, and how many
//!   steps that takes. Which ramp is the one the selected swatch came from, so
//!   clicking a swatch in another ramp moves the editor to it.
//!
//! Both are views, not stores: every widget emits a request carrying the whole
//! value it wants and reads its position back from the resource the applier
//! wrote, so the palette and the ramps stay the single source of truth even
//! mid-drag.
//!
//! Neither editor knows about the other. The panel is despawned and rebuilt
//! whenever the thing it is editing changes shape — the mode, the ramp, or the
//! ramp's axis, since which two values are fixed decides which sliders exist.

use bevy::feathers::controls::{
    ButtonVariant, ColorChannel, FeathersButton, FeathersColorSlider, FeathersSlider,
    SliderBaseColor,
};
use bevy::feathers::dark_theme::create_dark_theme;
use bevy::feathers::display::label_small;
use bevy::feathers::focus::FocusIndicator;
use bevy::feathers::theme::{SemanticToken, ThemeBackgroundColor, ThemeToken, UiTheme};
use bevy::feathers::FeathersPlugins;
use bevy::ecs::system::IntoObserverSystem;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::ui_widgets::{Activate, ScrollArea, SliderPrecision, SliderValue, ValueChange};

use crate::atlas::AtlasDocument;
use crate::canvas_view::HoveredPixel;
use crate::config::{
    EDITOR_LABEL_WIDTH, EDITOR_ROW_GAP, NEW_PALETTE_COLOR, SIDEBAR_BACKGROUND, SIDEBAR_WIDTH,
    STATUS_BACKGROUND, STATUS_BAR_HEIGHT, STATUS_FONT_SIZE, STATUS_TEXT, SWATCH_BORDER, SWATCH_GAP,
    SWATCH_ROW_GAP, SWATCH_SIZE,
};
use crate::document::{
    AddPaletteColorRequested, AddRampRequested, DocumentPath, MovePaletteColorRequested,
    RemovePaletteColorRequested, RemoveRampRequested, SelectColorRequested,
    SetPaletteColorRequested, SetRampRequested, TogglePaletteModeRequested,
};
use crate::oklab::{OklabAxis, OklabRamp, OklabRamps, RampField};
use crate::palette::{Palette, PaletteMode};

// ============================================================================
// COMPONENTS
// ============================================================================

/// A palette swatch, tagged with the colour index it selects.
#[derive(Component, Debug, Clone, Copy)]
pub struct PaletteSwatch {
    pub index: usize,
}

/// The scrolling column the swatches live in; respawned into whenever the shape
/// of the palette changes.
#[derive(Component)]
pub struct SwatchGrid;

/// One wrapping row of swatches inside the grid — the whole palette in the
/// hand-edited mode, one ramp in the generated one.
///
/// Ramps get a row each rather than being poured into one wrap, because a run of
/// swatches only reads as a ramp when it starts on its own line.
#[derive(Component)]
pub struct SwatchRow;

/// The editor under the swatches, whichever mode built it. Rebuilt wholesale
/// rather than reconfigured, since the two modes share no widget.
#[derive(Component)]
pub struct PaletteEditor;

/// The strip at the top of the sidebar holding the mode button.
///
/// It stays put and its *children* are replaced, because the button's caption
/// names the mode and bevy_ui lays a sidebar out in child order — respawning the
/// button itself would drop it below the swatches.
#[derive(Component)]
pub struct ModeBar;


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

/// Draw order for the two chrome roots. They overlap in the bottom-left corner
/// — the sidebar is full-height, the status strip full-width — and the strip
/// wins, which is why the sidebar reserves `STATUS_BAR_HEIGHT` of bottom
/// padding.
///
/// These are declared rather than left to chance. `ui_stack_system` sorts roots
/// by `(GlobalZIndex, ZIndex)` with a *stable* sort, so equal keys fall back to
/// query iteration order — which is archetype order, not spawn order. Adding a
/// widget elsewhere in the sidebar silently flipped this once and hid the file
/// name behind the sidebar.
const SIDEBAR_Z: i32 = 0;
const STATUS_STRIP_Z: i32 = 1;

// ============================================================================
// WHAT IS CURRENTLY BUILT
// ============================================================================

/// The shape of the editor panel on screen, so a rebuild only happens when the
/// thing being edited actually changed shape rather than every frame a slider
/// moves.
///
/// The axis is part of it because it decides *which* two values are fixed, and
/// therefore which sliders exist at all.
#[derive(Debug, Clone, Copy, PartialEq)]
enum EditorShape {
    /// The RGBA sliders and the colour buttons.
    Manual,
    Generated { ramp: usize, axis: OklabAxis },
}

/// Which editor `build_chrome` or the last rebuild left standing.
///
/// A resource rather than a `Local` so that `build_chrome`, which runs in
/// `Startup`, can seed it — otherwise the first `Update` would tear the panel
/// down and build the identical one again.
#[derive(Resource, Debug)]
struct BuiltEditor(EditorShape);

impl Default for BuiltEditor {
    /// Matches what a fresh [`PaletteMode`] asks for, so the two agree even if
    /// something reads this before `build_chrome` has run.
    fn default() -> Self {
        Self(EditorShape::Manual)
    }
}

/// How many swatches each row of the grid currently holds. The palette's own
/// length is not enough: two ramps can trade steps without changing the total.
#[derive(Resource, Debug, Default)]
struct BuiltSwatchRows(Vec<usize>);

/// The row lengths the swatch grid should have — one row for the whole
/// hand-edited palette, one per ramp when they are generating it.
fn swatch_rows(palette: &Palette, mode: &PaletteMode, ramps: &OklabRamps) -> Vec<usize> {
    if mode.is_generated() {
        ramps.step_counts()
    } else {
        vec![palette.len()]
    }
}

/// The editor the current mode calls for, and the ramp it would be editing.
fn editor_shape(palette: &Palette, mode: &PaletteMode, ramps: &OklabRamps) -> EditorShape {
    if !mode.is_generated() {
        return EditorShape::Manual;
    }
    // The edited ramp is the one the selected swatch came from, so clicking a
    // swatch in another ramp is also how you switch to editing it.
    let ramp = ramps.ramp_of(palette.selected);
    EditorShape::Generated {
        ramp,
        axis: ramps.get(ramp).map(|r| r.axis).unwrap_or(OklabAxis::Lightness),
    }
}

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
            // Seeded properly by `build_chrome`; initialised here so the rebuild
            // systems can take them unconditionally.
            .init_resource::<BuiltEditor>()
            .init_resource::<BuiltSwatchRows>()
            .add_systems(
                Startup,
                (install_palette_theme, build_chrome)
                    .chain()
                    .in_set(ChromeBuild),
            )
            .add_systems(
                Update,
                (
                    // Everything that writes to a widget already on screen runs
                    // before the rebuilds that can despawn one. Both sides are
                    // deferred commands, and a despawn followed by a write to
                    // the same entity panics — which is exactly what a mode
                    // switch would do, since it retunes the sliders it is about
                    // to replace.
                    (sync_channel_sliders, sync_ramp_sliders, mark_selected_swatch),
                    // The theme has to carry the new colours before any swatch
                    // that reads a token is spawned.
                    (sync_palette_theme, rebuild_swatches, rebuild_palette_editor).chain(),
                    update_status_label,
                )
                    .chain()
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

fn palette_semantic_token(index: usize) -> SemanticToken {
    SemanticToken::new(format!("ColorPalette::{index}").into())
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
// RAMP FIELDS
// ============================================================================

/// Decimal places a drag on `field` rounds to. The step count is a count, so a
/// drag on it lands on a whole number of steps.
///
/// The only part of a ramp field that is purely presentational, which is why it
/// lives here and the rest is in [`crate::oklab`].
fn ramp_precision(field: RampField) -> i32 {
    match field {
        RampField::Steps => 0,
        _ => 2,
    }
}

// ============================================================================
// STARTUP
// ============================================================================

/// Rewrites the palette tokens to match `palette`, dropping any left over from
/// a longer palette so a stale token cannot resurface under a later colour.
fn write_palette_tokens(palette: &Palette, theme: &mut UiTheme) {
    for (index, color) in palette.colors.iter().enumerate() {
        theme
            .0
            .token_assignments
            .insert(palette_token(index), palette_semantic_token(index));
        theme
            .0
            .semantic_base
            .insert(palette_semantic_token(index), Color::Srgba(*color));
    }

    let mut index = palette.len();
    while theme.0.token_assignments.remove(&palette_token(index)).is_some() {
        theme.0.semantic_base.remove(&palette_semantic_token(index));
        index += 1;
    }
}

fn install_palette_theme(palette: Res<Palette>, mut theme: ResMut<UiTheme>) {
    write_palette_tokens(&palette, &mut theme);
}

fn build_chrome(
    mut commands: Commands,
    palette: Res<Palette>,
    mode: Res<PaletteMode>,
    ramps: Res<OklabRamps>,
) {
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
            GlobalZIndex(SIDEBAR_Z),
            Sidebar,
        ))
        .id();

    // First child, so the mode is the first thing in the sidebar and the switch
    // is where you look for it.
    let mode_bar = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                flex_shrink: 0.0,
                ..default()
            },
            ChildOf(sidebar),
            ModeBar,
        ))
        .id();
    spawn_mode_button(&mut commands, mode_bar, &mode);

    let grid = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                // Takes whatever the editor below it leaves, and scrolls when a
                // long palette overflows it — neither `+` nor a ramp's step
                // count has an upper bound tight enough to guarantee a fit, so a
                // clipped swatch has to stay reachable.
                flex_grow: 1.0,
                min_height: px(0),
                row_gap: px(SWATCH_ROW_GAP),
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

    let rows = swatch_rows(&palette, &mode, &ramps);
    spawn_swatch_rows(&mut commands, grid, palette.len(), &rows);
    commands.insert_resource(BuiltSwatchRows(rows));

    let shape = editor_shape(&palette, &mode, &ramps);
    spawn_palette_editor(&mut commands, sidebar, shape, &palette, &ramps);
    commands.insert_resource(BuiltEditor(shape));

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
            GlobalZIndex(STATUS_STRIP_Z),
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

/// The button that switches the two ways of specifying a palette. Its caption is
/// the mode it is *in*, not the one it would move to, so the sidebar always says
/// what you are looking at.
fn spawn_mode_button(commands: &mut Commands, mode_bar: Entity, mode: &PaletteMode) {
    commands
        .spawn_scene(editor_button(
            match mode.is_generated() {
                false => "palette: list",
                true => "palette: oklab",
            },
            ButtonVariant::Normal,
            on_mode_clicked,
        ))
        .insert(ChildOf(mode_bar));
}

/// The panel under the swatches, whichever editor `shape` calls for. Both start
/// with a row of buttons that change *what* is being edited and continue with
/// the sliders that change it.
fn spawn_palette_editor(
    commands: &mut Commands,
    sidebar: Entity,
    shape: EditorShape,
    palette: &Palette,
    ramps: &OklabRamps,
) {
    let editor = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
                row_gap: px(EDITOR_ROW_GAP),
                ..default()
            },
            ChildOf(sidebar),
            PaletteEditor,
        ))
        .id();

    match shape {
        EditorShape::Manual => build_color_editor(commands, editor, palette.selected_color()),
        EditorShape::Generated { ramp, .. } => {
            // Nothing else can be edited if the ramp is gone, and the panel is
            // rebuilt the moment the selection lands on one that exists.
            if let Some(ramp) = ramps.get(ramp) {
                build_ramp_editor(commands, editor, ramp);
            }
        }
    }
}

/// A row of equal-width buttons.
fn button_row(commands: &mut Commands, editor: Entity) -> Entity {
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(EDITOR_ROW_GAP),
                ..default()
            },
            ChildOf(editor),
        ))
        .id()
}

/// A captioned slider row. The caption column is a fixed width so the sliders of
/// both editors line up down the sidebar.
fn slider_row(commands: &mut Commands, editor: Entity, caption: &str) -> Entity {
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
            width: px(EDITOR_LABEL_WIDTH),
            ..default()
        },
        ChildOf(row),
    ));
    row
}

/// The hand-edited editor: reorder, add and remove, then the four RGBA channels
/// of the selected entry.
fn build_color_editor(commands: &mut Commands, editor: Entity, selected: Srgba) {
    let buttons = button_row(commands, editor);

    // Reorder first, then grow and shrink: the two arrows move the entry the
    // sliders below are editing, so they belong next to it.
    commands
        .spawn_scene(editor_button("<", ButtonVariant::Normal, on_move_left_clicked))
        .insert(ChildOf(buttons));
    commands
        .spawn_scene(editor_button(">", ButtonVariant::Normal, on_move_right_clicked))
        .insert(ChildOf(buttons));
    commands
        .spawn_scene(editor_button("+", ButtonVariant::Normal, on_add_clicked))
        .insert(ChildOf(buttons));
    commands
        .spawn_scene(editor_button("-", ButtonVariant::Normal, on_remove_clicked))
        .insert(ChildOf(buttons));

    for (caption, channel) in EDITED_CHANNELS {
        let row = slider_row(commands, editor, caption);

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

/// The generated editor: which value sweeps, how many ramps there are, and then
/// the six numbers that are the ramp.
///
/// The swept axis' button is the primary one, so the panel says which of the
/// three it is without needing a caption to explain the layout — the two sliders
/// that follow are the other two, in L, a, b order.
fn build_ramp_editor(commands: &mut Commands, editor: Entity, ramp: &OklabRamp) {
    let axes = button_row(commands, editor);
    for axis in OklabAxis::ALL {
        let variant = if axis == ramp.axis {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Normal
        };
        commands
            .spawn_scene(editor_button(axis.label(), variant, on_axis_clicked(axis)))
            .insert(ChildOf(axes));
    }

    let ramps = button_row(commands, editor);
    commands
        .spawn_scene(editor_button("+", ButtonVariant::Normal, on_add_ramp_clicked))
        .insert(ChildOf(ramps));
    commands
        .spawn_scene(editor_button(
            "-",
            ButtonVariant::Normal,
            on_remove_ramp_clicked,
        ))
        .insert(ChildOf(ramps));

    for field in RampField::of(ramp) {
        let row = slider_row(commands, editor, field.label());
        let (min, max) = field.limits(ramp);
        commands
            .spawn_scene(ramp_slider(field, field.read(ramp), min, max))
            .insert((ChildOf(row), field));
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

/// Shifts the selected entry one slot, clamping at both ends rather than
/// disabling the button — a clamped move arrives as `from == to`, which the
/// applier treats as the no-op it is.
fn on_move_left_clicked(
    _activate: On<Activate>,
    palette: Res<Palette>,
    mut requests: MessageWriter<MovePaletteColorRequested>,
) {
    requests.write(MovePaletteColorRequested {
        from: palette.selected,
        to: palette.selected.saturating_sub(1),
    });
}

fn on_move_right_clicked(
    _activate: On<Activate>,
    palette: Res<Palette>,
    mut requests: MessageWriter<MovePaletteColorRequested>,
) {
    requests.write(MovePaletteColorRequested {
        from: palette.selected,
        to: (palette.selected + 1).min(palette.len().saturating_sub(1)),
    });
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

fn on_mode_clicked(
    _activate: On<Activate>,
    mut requests: MessageWriter<TogglePaletteModeRequested>,
) {
    requests.write(TogglePaletteModeRequested);
}

/// The ramp every generated-mode widget acts on: the one that produced the
/// selected swatch.
///
/// `None` only while the palette and the ramps disagree, which lasts until the
/// applier's regenerate — so the widget declines rather than editing a ramp
/// picked by a stale index.
fn edited_ramp(palette: &Palette, ramps: &OklabRamps) -> Option<(usize, OklabRamp)> {
    let index = ramps.ramp_of(palette.selected);
    ramps.get(index).map(|ramp| (index, *ramp))
}

/// One of the three axis buttons. Switching axis resets the sweep into the new
/// axis' units — see [`OklabRamp::with_axis`].
fn on_axis_clicked(
    axis: OklabAxis,
) -> impl Fn(On<Activate>, Res<Palette>, Res<OklabRamps>, MessageWriter<SetRampRequested>)
       + Clone
       + Send
       + Sync
       + 'static {
    move |_activate, palette, ramps, mut requests| {
        if let Some((index, ramp)) = edited_ramp(&palette, &ramps) {
            requests.write(SetRampRequested {
                index,
                ramp: ramp.with_axis(axis),
            });
        }
    }
}

/// One of the ramp sliders. Like the channel sliders it reports the whole ramp
/// it wants rather than the value that moved, and it targets the edited ramp
/// rather than remembering one — so a rebuild cannot leave it pointing at a ramp
/// that has since been removed.
///
/// Named rather than inlined for the same reason the channel one is: the tests
/// have no pointer to drag with.
// The return type is the observer's signature spelled out; there is no shorter
// way to say it until `impl Trait` in type aliases lands.
#[allow(clippy::type_complexity)]
fn on_ramp_slider_dragged(
    field: RampField,
) -> impl Fn(On<ValueChange<f32>>, Res<Palette>, Res<OklabRamps>, MessageWriter<SetRampRequested>)
       + Clone
       + Send
       + Sync
       + 'static {
    move |change, palette, ramps, mut requests| {
        if let Some((index, ramp)) = edited_ramp(&palette, &ramps) {
            requests.write(SetRampRequested {
                index,
                ramp: field.apply(ramp, change.value),
            });
        }
    }
}

fn on_add_ramp_clicked(_activate: On<Activate>, mut requests: MessageWriter<AddRampRequested>) {
    requests.write(AddRampRequested);
}

fn on_remove_ramp_clicked(
    _activate: On<Activate>,
    palette: Res<Palette>,
    ramps: Res<OklabRamps>,
    mut requests: MessageWriter<RemoveRampRequested>,
) {
    requests.write(RemoveRampRequested {
        index: ramps.ramp_of(palette.selected),
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

/// One ramp slider. Unlike the channel sliders these have different ranges from
/// each other, and the step count is a count — hence the precision, which is
/// what makes a drag land on a whole number of steps.
fn ramp_slider(field: RampField, value: f32, min: f32, max: f32) -> impl Scene {
    bsn! {
        @FeathersSlider {
            @min: {min},
            @max: {max},
        }
        SliderValue({value})
        SliderPrecision({ramp_precision(field)})
        on(on_ramp_slider_dragged(field))
    }
}

/// One of the editor's buttons: a captioned Feathers button that runs `action`
/// when activated. They share a width, so a row divides evenly however many
/// there are.
fn editor_button<I, M>(caption: &str, variant: ButtonVariant, action: I) -> impl Scene
where
    I: IntoObserverSystem<Activate, M> + Clone + Send + Sync + 'static,
    M: Send + Sync + 'static,
{
    let caption = caption.to_string();
    bsn! {
        @FeathersButton {
            @caption: bsn! { @label_small(caption) },
            @variant: {variant},
        }
        Node {
            flex_grow: 1.0,
            flex_basis: px(0),
        }
        on(action)
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

/// Fills the grid with `rows.len()` rows holding `rows[i]` swatches each,
/// numbering them straight through the palette.
///
/// `count` is the palette's own length and bounds the walk, so a signature that
/// disagreed with it could only produce a short row — never a swatch with no
/// colour behind it.
fn spawn_swatch_rows(commands: &mut Commands, grid: Entity, count: usize, rows: &[usize]) {
    let mut index = 0;
    for length in rows {
        let row = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    align_content: AlignContent::FlexStart,
                    flex_shrink: 0.0,
                    row_gap: px(SWATCH_GAP),
                    column_gap: px(SWATCH_GAP),
                    ..default()
                },
                ChildOf(grid),
                SwatchRow,
            ))
            .id();

        for _ in 0..*length {
            if index >= count {
                return;
            }
            spawn_swatch(commands, row, index);
            index += 1;
        }
    }
}

/// Adding or removing a colour changes how many tokens there are, which no
/// amount of repainting can express — so the swatches are rebuilt against the
/// new palette. Recolouring leaves the shape alone and skips all of this.
///
/// The shape is the row lengths rather than the total, because two ramps can
/// trade steps without the palette changing length, and because a mode switch
/// changes how the same colours are grouped.
fn rebuild_swatches(
    mut commands: Commands,
    palette: Res<Palette>,
    mode: Res<PaletteMode>,
    ramps: Res<OklabRamps>,
    mut built: ResMut<BuiltSwatchRows>,
    grid: Query<Entity, With<SwatchGrid>>,
    rows: Query<Entity, With<SwatchRow>>,
) {
    if !palette.is_changed() && !mode.is_changed() && !ramps.is_changed() {
        return;
    }
    let wanted = swatch_rows(&palette, &mode, &ramps);
    if wanted == built.0 {
        return;
    }
    let Ok(grid) = grid.single() else {
        return;
    };

    // Despawning a row takes its swatches with it.
    for entity in &rows {
        commands.entity(entity).despawn();
    }
    spawn_swatch_rows(&mut commands, grid, palette.len(), &wanted);
    built.0 = wanted;
}

/// The panel under the swatches edits one thing, and which thing it is changes:
/// the mode, the ramp the selection lands in, and that ramp's axis all decide
/// what widgets belong there. None of that can be expressed by moving a slider,
/// so the panel is despawned and rebuilt — but only when the shape really moved,
/// or a drag would rebuild it under the pointer every frame.
fn rebuild_palette_editor(
    mut commands: Commands,
    palette: Res<Palette>,
    mode: Res<PaletteMode>,
    ramps: Res<OklabRamps>,
    mut built: ResMut<BuiltEditor>,
    sidebar: Query<Entity, With<Sidebar>>,
    editor: Query<Entity, With<PaletteEditor>>,
    mode_bar: Query<(Entity, Option<&Children>), With<ModeBar>>,
) {
    if !palette.is_changed() && !mode.is_changed() && !ramps.is_changed() {
        return;
    }
    let wanted = editor_shape(&palette, &mode, &ramps);
    if wanted == built.0 {
        return;
    }
    let Ok(sidebar) = sidebar.single() else {
        return;
    };

    // The caption names the mode, so it only moves when the mode does.
    if matches!(built.0, EditorShape::Manual) != matches!(wanted, EditorShape::Manual) {
        if let Ok((bar, children)) = mode_bar.single() {
            for child in children.into_iter().flatten() {
                commands.entity(*child).despawn();
            }
            spawn_mode_button(&mut commands, bar, &mode);
        }
    }

    for entity in &editor {
        commands.entity(entity).despawn();
    }
    // Respawned as the sidebar's last child, which is where it was: the mode bar
    // and the swatch grid are both still there and both came first.
    spawn_palette_editor(&mut commands, sidebar, wanted, &palette, &ramps);
    built.0 = wanted;
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

/// The ramp sliders show the edited ramp, and nothing else ever sets them —
/// including mid-drag, where what comes back is the value the ramp accepted
/// rather than the one the pointer asked for.
///
/// Only the values are written here. A slider's *range* depends on the axis, and
/// changing the axis rebuilds the panel, so a range can never be stale.
fn sync_ramp_sliders(
    mut commands: Commands,
    palette: Res<Palette>,
    ramps: Res<OklabRamps>,
    sliders: Query<(Entity, &RampField, &SliderValue)>,
) {
    if !palette.is_changed() && !ramps.is_changed() {
        return;
    }
    let Some((_, ramp)) = edited_ramp(&palette, &ramps) else {
        return;
    };

    for (entity, field, value) in &sliders {
        let target = field.read(&ramp);
        // Guarded for the same reason the channel sliders are: an unchanged
        // write still counts as a change and would have Feathers rebuild the
        // track every frame. `SliderValue` is immutable, hence the insert.
        if value.0 != target {
            commands.entity(entity).insert(SliderValue(target));
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
/// What the status strip says about the palette's second half: which mode, and
/// in the generated one which ramp of how many and what it sweeps.
///
/// The ramp spec is there so a screenshot of a generated palette is as
/// self-describing as the rest of the strip — the swatches show the colours, but
/// only this says what produced them.
fn describe_palette_mode(palette: &Palette, mode: &PaletteMode, ramps: &OklabRamps) -> String {
    if !mode.is_generated() {
        return mode.label().to_string();
    }
    let index = ramps.ramp_of(palette.selected);
    match ramps.get(index) {
        Some(ramp) => format!(
            "{} [{}/{}] {}",
            mode.label(),
            index,
            ramps.len(),
            ramp.describe()
        ),
        None => mode.label().to_string(),
    }
}

fn update_status_label(
    document: Res<AtlasDocument>,
    palette: Res<Palette>,
    mode: Res<PaletteMode>,
    ramps: Res<OklabRamps>,
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
        "{}{}    {}    {} [{}/{}]    {}    {}x{} tiles of {}px",
        path.display_name(),
        if document.unsaved { "*" } else { "" },
        position,
        palette.selected_hex(),
        // Which entry and how many there are, so adding and removing one is
        // readable straight off a screenshot.
        palette.selected,
        palette.len(),
        describe_palette_mode(&palette, &mode, &ramps),
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

    fn palette_token_color(theme: &UiTheme, index: usize) -> Option<&Color> {
        let semantic = theme.0.token_assignments.get(&palette_token(index))?;
        theme.0.semantic_base.get(semantic)
    }

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
            palette_token_color(theme, 0),
            Some(&Color::srgba(1.0, 0.0, 0.0, 1.0))
        );
        assert_eq!(
            palette_token_color(theme, 1),
            Some(&Color::srgba(0.0, 1.0, 0.0, 1.0))
        );
        assert!(
            palette_token_color(theme, 2).is_none(),
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
            palette_token_color(&theme, 1),
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

        assert!(palette_token_color(&theme, 1).is_some());
        for index in 2..full_length {
            assert!(
                palette_token_color(&theme, index).is_none(),
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
        moves: Vec<(usize, usize)>,
        modes: usize,
        ramp_sets: Vec<(usize, OklabRamp)>,
        ramp_adds: usize,
        ramp_removes: Vec<usize>,
    }

    fn record(
        mut sets: MessageReader<SetPaletteColorRequested>,
        mut adds: MessageReader<AddPaletteColorRequested>,
        mut removes: MessageReader<RemovePaletteColorRequested>,
        mut moves: MessageReader<MovePaletteColorRequested>,
        mut modes: MessageReader<TogglePaletteModeRequested>,
        mut ramp_sets: MessageReader<SetRampRequested>,
        mut ramp_adds: MessageReader<AddRampRequested>,
        mut ramp_removes: MessageReader<RemoveRampRequested>,
        mut recorded: ResMut<Recorded>,
    ) {
        let seen: Vec<(usize, Srgba)> = sets.read().map(|m| (m.index, m.color)).collect();
        recorded.sets.extend(seen);
        let seen: Vec<Srgba> = adds.read().map(|m| m.color).collect();
        recorded.adds.extend(seen);
        let seen: Vec<usize> = removes.read().map(|m| m.index).collect();
        recorded.removes.extend(seen);
        let seen: Vec<(usize, usize)> = moves.read().map(|m| (m.from, m.to)).collect();
        recorded.moves.extend(seen);
        recorded.modes += modes.read().count();
        let seen: Vec<(usize, OklabRamp)> =
            ramp_sets.read().map(|m| (m.index, m.ramp)).collect();
        recorded.ramp_sets.extend(seen);
        recorded.ramp_adds += ramp_adds.read().count();
        let seen: Vec<usize> = ramp_removes.read().map(|m| m.index).collect();
        recorded.ramp_removes.extend(seen);
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
            .add_message::<MovePaletteColorRequested>()
            .add_message::<TogglePaletteModeRequested>()
            .add_message::<SetRampRequested>()
            .add_message::<AddRampRequested>()
            .add_message::<RemoveRampRequested>()
            .init_resource::<Recorded>()
            .insert_resource(OklabRamps::default())
            .insert_resource(palette)
            .add_systems(Update, record);
        app
    }

    /// A widget app whose palette is what the ramps generate, which is the state
    /// the generated editor is only ever shown in.
    fn generated_widget_app() -> App {
        widget_app(Palette::with_colors(OklabRamps::default().colors()))
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

    /// Drives both arrows from one selection, so the pair cannot silently be
    /// wired up the same way round.
    #[test]
    fn the_arrows_shift_the_selection_in_opposite_directions() {
        let mut app = widget_app(Palette::with_colors(vec![RED, GREEN, RED]));
        app.world_mut().resource_mut::<Palette>().select(1);

        let left = app
            .world_mut()
            .spawn_empty()
            .observe(on_move_left_clicked)
            .id();
        let right = app
            .world_mut()
            .spawn_empty()
            .observe(on_move_right_clicked)
            .id();
        app.world_mut().trigger(Activate { entity: left });
        app.world_mut().trigger(Activate { entity: right });
        app.update();

        assert_eq!(app.world().resource::<Recorded>().moves, vec![(1, 0), (1, 2)]);
    }

    /// The arrows clamp instead of disabling themselves, so the ends ask to move
    /// an entry onto itself — which the applier treats as a no-op.
    #[test]
    fn the_arrows_clamp_at_both_ends() {
        let mut app = widget_app(Palette::with_colors(vec![RED, GREEN]));

        let left = app
            .world_mut()
            .spawn_empty()
            .observe(on_move_left_clicked)
            .id();
        let right = app
            .world_mut()
            .spawn_empty()
            .observe(on_move_right_clicked)
            .id();

        // Selected 0: left is already as far as it goes.
        app.world_mut().trigger(Activate { entity: left });
        app.world_mut().resource_mut::<Palette>().select(1);
        // Selected 1 of 2: right is already as far as it goes.
        app.world_mut().trigger(Activate { entity: right });
        app.update();

        assert_eq!(app.world().resource::<Recorded>().moves, vec![(0, 0), (1, 1)]);
    }

    // ------------------------------------------------------------------------
    // The generated editor
    // ------------------------------------------------------------------------

    #[test]
    fn the_mode_button_only_asks_to_switch() {
        let mut app = widget_app(Palette::with_colors(vec![RED]));
        let button = app.world_mut().spawn_empty().observe(on_mode_clicked).id();
        app.world_mut().trigger(Activate { entity: button });
        app.update();

        assert_eq!(app.world().resource::<Recorded>().modes, 1);
    }

    /// The generated widgets act on the ramp the selection came from, so a
    /// selection two ramps along has to reach the second one.
    #[test]
    fn the_generated_widgets_target_the_ramp_the_selection_came_from() {
        let mut app = generated_widget_app();
        let offset = app.world().resource::<OklabRamps>().offset_of(2);
        app.world_mut().resource_mut::<Palette>().select(offset);

        let button = app
            .world_mut()
            .spawn_empty()
            .observe(on_remove_ramp_clicked)
            .id();
        app.world_mut().trigger(Activate { entity: button });
        app.update();

        assert_eq!(app.world().resource::<Recorded>().ramp_removes, vec![2]);
    }

    #[test]
    fn an_axis_button_asks_for_the_whole_ramp_sweeping_that_axis() {
        let mut app = generated_widget_app();
        let button = app
            .world_mut()
            .spawn_empty()
            .observe(on_axis_clicked(OklabAxis::B))
            .id();
        app.world_mut().trigger(Activate { entity: button });
        app.update();

        let before = *app.world().resource::<OklabRamps>().get(0).unwrap();
        assert_eq!(
            app.world().resource::<Recorded>().ramp_sets,
            vec![(0, before.with_axis(OklabAxis::B))]
        );
    }

    #[test]
    fn dragging_a_ramp_slider_asks_for_the_whole_ramp_it_wants() {
        let mut app = generated_widget_app();
        let slider = app
            .world_mut()
            .spawn_empty()
            .observe(on_ramp_slider_dragged(RampField::To))
            .id();
        // `ValueChange` is generic; an untyped literal would infer `f64` and the
        // `f32` observer would silently never fire.
        app.world_mut().trigger(ValueChange::<f32> {
            source: slider,
            value: 0.5,
            is_final: true,
        });
        app.update();

        let before = *app.world().resource::<OklabRamps>().get(0).unwrap();
        assert_eq!(
            app.world().resource::<Recorded>().ramp_sets,
            vec![(0, OklabRamp { to: 0.5, ..before })],
            "one field moves and the rest of the ramp comes along unchanged"
        );
    }

    #[test]
    fn the_ramp_buttons_add_and_remove() {
        let mut app = generated_widget_app();
        let add = app.world_mut().spawn_empty().observe(on_add_ramp_clicked).id();
        let remove = app
            .world_mut()
            .spawn_empty()
            .observe(on_remove_ramp_clicked)
            .id();
        app.world_mut().trigger(Activate { entity: add });
        app.world_mut().trigger(Activate { entity: remove });
        app.update();

        let recorded = app.world().resource::<Recorded>();
        assert_eq!(recorded.ramp_adds, 1);
        assert_eq!(recorded.ramp_removes, vec![0]);
    }

    // ------------------------------------------------------------------------
    // What gets rebuilt
    // ------------------------------------------------------------------------

    /// One row for the hand-edited palette, one per ramp when they generate it —
    /// which is what makes a run of swatches read as a ramp.
    #[test]
    fn the_swatch_rows_follow_the_mode() {
        let ramps = OklabRamps::default();
        let mut mode = PaletteMode::default();
        let manual = Palette::new();
        assert_eq!(swatch_rows(&manual, &mode, &ramps), vec![manual.len()]);

        mode.stash(manual.clone());
        let generated = Palette::with_colors(ramps.colors());
        assert_eq!(swatch_rows(&generated, &mode, &ramps), ramps.step_counts());
    }

    /// The signature has to notice two ramps trading steps, because the total —
    /// and so the palette's length — does not move.
    #[test]
    fn the_swatch_rows_notice_a_reshuffle_that_keeps_the_total() {
        let mut mode = PaletteMode::default();
        mode.stash(Palette::new());
        let before = OklabRamps::default();
        let mut after = before.clone();
        after.ramps[0].steps -= 2;
        after.ramps[1].steps += 2;

        let palette = Palette::with_colors(before.colors());
        assert_eq!(before.colors().len(), after.colors().len());
        assert_ne!(
            swatch_rows(&palette, &mode, &before),
            swatch_rows(&palette, &mode, &after)
        );
    }

    /// The panel is rebuilt on this, so it has to move for every change that
    /// alters which widgets belong there — and stay put for the ones that do
    /// not, or a drag would rebuild the panel under the pointer.
    #[test]
    fn the_editor_shape_tracks_the_mode_the_ramp_and_the_axis() {
        let ramps = OklabRamps::default();
        let mut mode = PaletteMode::default();
        let manual = Palette::new();
        assert_eq!(editor_shape(&manual, &mode, &ramps), EditorShape::Manual);

        mode.stash(manual);
        let mut palette = Palette::with_colors(ramps.colors());
        let first = editor_shape(&palette, &mode, &ramps);

        // A different colour of the same ramp edits the same thing.
        palette.select(1);
        assert_eq!(editor_shape(&palette, &mode, &ramps), first);

        // A colour of another ramp does not.
        palette.select(ramps.offset_of(1));
        assert_ne!(editor_shape(&palette, &mode, &ramps), first);

        // Nor does the same ramp sweeping a different axis, since that changes
        // which two values have sliders.
        let mut swept = ramps.clone();
        swept.ramps[0] = swept.ramps[0].with_axis(OklabAxis::B);
        palette.select(0);
        assert_ne!(editor_shape(&palette, &mode, &swept), first);
    }

    #[test]
    fn only_the_step_count_slider_snaps_to_whole_numbers() {
        assert_eq!(ramp_precision(RampField::Steps), 0);
        for field in [
            RampField::From,
            RampField::To,
            RampField::Fixed(OklabAxis::Lightness),
        ] {
            assert!(ramp_precision(field) > 0, "{field:?} is a continuous value");
        }
    }
}
