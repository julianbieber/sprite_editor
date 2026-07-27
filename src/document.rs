//! Document state: the only place `AtlasDocument` and `Palette` are mutated.
//!
//! Input never writes state directly — it emits requests, and these systems
//! apply them. That is what lets the dev harness drive the editor through the
//! same code path a human does.

use std::path::PathBuf;

use bevy::prelude::*;

use crate::atlas::AtlasDocument;
use crate::config::transparent;
use crate::oklab::{OklabRamp, OklabRamps};
use crate::palette::{Palette, PaletteMode};

// ============================================================================
// REQUESTS
// ============================================================================

/// Paint the selected colour into one pixel.
#[derive(Message, Debug, Clone, Copy)]
pub struct PaintPixelRequested {
    pub x: u32,
    pub y: u32,
}

/// Make a palette entry the draw colour.
#[derive(Message, Debug, Clone, Copy)]
pub struct SelectColorRequested {
    pub index: usize,
}

/// Append transparent tile columns on the right.
#[derive(Message, Debug, Clone, Copy)]
pub struct AddTileRequested {
    pub tiles: u32,
}

/// Overwrite one palette entry. The channel sliders emit one of these per drag
/// frame, with the whole colour rather than the channel that moved — so the
/// applier stays a plain assignment and the sliders stay a view of the palette.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetPaletteColorRequested {
    pub index: usize,
    pub color: Srgba,
}

/// Append a colour and select it.
#[derive(Message, Debug, Clone, Copy)]
pub struct AddPaletteColorRequested {
    pub color: Srgba,
}

/// Drop a palette entry. Refused for the last one — something must stay
/// selected, or there would be nothing to draw with.
#[derive(Message, Debug, Clone, Copy)]
pub struct RemovePaletteColorRequested {
    pub index: usize,
}

/// Move a palette entry to another slot, carrying the selection with it.
///
/// `from == to` is the normal way to say "already at the end of the row" — the
/// arrow buttons clamp rather than disappear — so it is a no-op, not an error.
#[derive(Message, Debug, Clone, Copy)]
pub struct MovePaletteColorRequested {
    pub from: usize,
    pub to: usize,
}

/// Swap which of the two palette specifications is in charge.
#[derive(Message, Debug, Clone, Copy)]
pub struct TogglePaletteModeRequested;

/// Overwrite one generated ramp. Like [`SetPaletteColorRequested`], the whole
/// ramp comes along rather than the field that moved, so a slider drag and an
/// axis click are the same request and the applier stays an assignment.
#[derive(Message, Debug, Clone, Copy)]
pub struct SetRampRequested {
    pub index: usize,
    pub ramp: OklabRamp,
}

/// Append a copy of the ramp being edited, and select its first colour.
///
/// A copy rather than a fresh default: several ramps are wanted precisely
/// because they are variations on each other, so duplicating one and moving a
/// slider is the shortest route to the second.
#[derive(Message, Debug, Clone, Copy)]
pub struct AddRampRequested;

/// Drop a generated ramp. Refused for the last one, for the same reason the last
/// colour cannot be removed — the palette would be empty.
#[derive(Message, Debug, Clone, Copy)]
pub struct RemoveRampRequested {
    pub index: usize,
}

// ============================================================================
// RESOURCES
// ============================================================================

/// Where this document loads from and saves to.
#[derive(Resource, Debug, Clone, Default)]
pub struct DocumentPath {
    /// Base path with no extension; `None` until a save picks a default.
    pub base: Option<PathBuf>,
    pub existed_on_disk: bool,
}

impl DocumentPath {
    /// What the status strip shows.
    pub fn display_name(&self) -> String {
        match &self.base {
            Some(base) => base
                .file_name()
                .map(|n| format!("{}.png", n.to_string_lossy()))
                .unwrap_or_else(|| "<unnamed>".to_string()),
            None => "<unsaved>".to_string(),
        }
    }
}

// ============================================================================
// PLUGIN
// ============================================================================

pub struct DocumentStatePlugin;

impl Plugin for DocumentStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PaintPixelRequested>()
            .add_message::<SelectColorRequested>()
            .add_message::<AddTileRequested>()
            .add_message::<SetPaletteColorRequested>()
            .add_message::<AddPaletteColorRequested>()
            .add_message::<RemovePaletteColorRequested>()
            .add_message::<MovePaletteColorRequested>()
            .add_message::<TogglePaletteModeRequested>()
            .add_message::<SetRampRequested>()
            .add_message::<AddRampRequested>()
            .add_message::<RemoveRampRequested>()
            .add_systems(
                Update,
                // Chained, not merely grouped: painting reads the palette the
                // earlier systems write, and a script that switches mode,
                // selects, edits and paints in one frame must see its own edits.
                //
                // The two generating systems come first because they *replace*
                // the palette: a selection applied before them would be against
                // the colours of the mode being left.
                (
                    apply_palette_mode,
                    apply_ramp_edits,
                    apply_color_selection,
                    apply_palette_edits,
                    apply_paint_requests,
                    apply_add_tile,
                )
                    .chain()
                    .in_set(DocumentEdit),
            );
    }
}

/// Everything that mutates the document, so other plugins can order against it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocumentEdit;

// ============================================================================
// SYSTEMS
// ============================================================================

/// Rewrites `palette` from the ramps, keeping the selection pointing at a
/// colour. The generated palette is derived state, so this is the only way it is
/// ever written and every ramp edit ends here.
fn regenerate(palette: &mut Palette, ramps: &OklabRamps) {
    let colors = ramps.colors();
    // Clamped rather than reset: a ramp that gained a step should not throw the
    // selection back to the front of the palette.
    palette.selected = palette.selected.min(colors.len().saturating_sub(1));
    palette.colors = colors;
}

/// Switches which specification owns the palette.
///
/// Leaving the hand-edited mode parks the palette whole; coming back installs it
/// again. Nothing else in the editor knows there are two modes — the palette it
/// reads is the same resource either way.
fn apply_palette_mode(
    mut requests: MessageReader<TogglePaletteModeRequested>,
    mut mode: ResMut<PaletteMode>,
    mut palette: ResMut<Palette>,
    ramps: Res<OklabRamps>,
    mut document: ResMut<AtlasDocument>,
) {
    // Two toggles in one frame are a round trip; only the parity matters, and
    // collapsing them here keeps the stash from being written twice.
    if requests.read().count().is_multiple_of(2) {
        return;
    }

    if mode.is_generated() {
        if let Some(restored) = mode.restore() {
            *palette = restored;
        }
    } else {
        mode.stash(palette.clone());
        regenerate(&mut palette, &ramps);
    }

    // The two modes save different colours, so the switch itself is an edit —
    // but it moves no pixel, so `revision` stays put.
    document.mark_unsaved();
}

/// The ramp-editing requests, in the same overwrite-then-append-then-remove
/// order the colour ones use, and for the same reason: a duplicate-then-tweak in
/// one frame lands on the ramp the append created.
///
/// Every branch ends in a regenerate, so the palette can never disagree with the
/// ramps that produced it. They are applied even in the hand-edited mode — the
/// ramps are still there, just not on screen — but only the generated mode lets
/// the change reach the palette.
fn apply_ramp_edits(
    mut sets: MessageReader<SetRampRequested>,
    mut adds: MessageReader<AddRampRequested>,
    mut removes: MessageReader<RemoveRampRequested>,
    mut ramps: ResMut<OklabRamps>,
    mut palette: ResMut<Palette>,
    mode: Res<PaletteMode>,
    mut document: ResMut<AtlasDocument>,
) {
    let mut changed = false;
    // Set when an append needs the selection moved onto the new ramp, which can
    // only be done once the palette has been regenerated around it.
    let mut select_ramp = None;

    for request in sets.read() {
        match ramps.ramps.get_mut(request.index) {
            // A slider drag re-sends the ramp it already set; that is not an
            // edit, and regenerating on it would rebuild the swatches every
            // frame of the drag.
            Some(existing) if *existing == request.ramp => {}
            Some(existing) => {
                *existing = request.ramp;
                changed = true;
            }
            None => warn!(
                "ignoring an edit to ramp {} — there are {}",
                request.index,
                ramps.len()
            ),
        }
    }

    for _ in adds.read() {
        let source = ramps.ramp_of(palette.selected);
        let Some(&copied) = ramps.get(source) else {
            warn!("ignoring an added ramp — there is nothing to copy");
            continue;
        };
        ramps.ramps.push(copied);
        select_ramp = Some(ramps.len() - 1);
        changed = true;
    }

    for request in removes.read() {
        if ramps.len() <= 1 {
            warn!("refusing to remove the last ramp — the palette would be empty");
            continue;
        }
        if request.index >= ramps.len() {
            warn!(
                "ignoring removal of ramp {} — there are {}",
                request.index,
                ramps.len()
            );
            continue;
        }
        ramps.ramps.remove(request.index);
        // Keep the editor on a ramp that still exists, and on the one that took
        // the removed one's place rather than on whatever the clamp lands on.
        select_ramp = Some(request.index.min(ramps.len() - 1));
        changed = true;
    }

    if !changed {
        return;
    }
    // The sidecar carries the ramps whichever mode is live, so moving one is an
    // edit even when nothing on screen shows it.
    document.mark_unsaved();

    if !mode.is_generated() {
        // Off-mode the ramps still moved, but they are not what fills the
        // palette, so the hand-edited colours stay exactly as they are.
        return;
    }

    if let Some(ramp) = select_ramp {
        palette.selected = ramps.offset_of(ramp);
    }
    regenerate(&mut palette, &ramps);
}

/// Selection is applied before painting, so a script can select and paint in the
/// same frame and get the colour it just asked for.
fn apply_color_selection(
    mut requests: MessageReader<SelectColorRequested>,
    mut palette: ResMut<Palette>,
) {
    for request in requests.read() {
        if !palette.select(request.index) {
            warn!(
                "ignoring selection of colour {} — palette has {}",
                request.index,
                palette.len()
            );
        }
    }
}

/// The palette-editing requests, applied in a fixed order: overwrite, then
/// append, then remove, then move. That order is what makes "add a colour and
/// tint it in the same frame" work, since the append is what the following
/// select lands on.
///
/// A palette edit changes what a save writes but not a single pixel — pixels
/// hold colours, not palette indices — so it marks the document unsaved without
/// bumping `revision`.
///
/// All of it is refused in the generated mode: those colours are derived from the
/// ramps and the next regenerate would overwrite anything written here, so an
/// edit that appeared to work and then silently vanished would be worse than one
/// that never happened.
fn apply_palette_edits(
    mut sets: MessageReader<SetPaletteColorRequested>,
    mut adds: MessageReader<AddPaletteColorRequested>,
    mut removes: MessageReader<RemovePaletteColorRequested>,
    mut moves: MessageReader<MovePaletteColorRequested>,
    mut palette: ResMut<Palette>,
    mode: Res<PaletteMode>,
    mut document: ResMut<AtlasDocument>,
) {
    if mode.is_generated() {
        // Drained, not left buffered, or they would all land on the frame the
        // mode switches back.
        let refused = sets.read().count()
            + adds.read().count()
            + removes.read().count()
            + moves.read().count();
        if refused > 0 {
            warn!("ignoring {refused} palette edit(s) — the ramps generate these colours");
        }
        return;
    }

    let mut changed = false;

    for request in sets.read() {
        // Bound first: the borrow has to end before the arms can read `len()`.
        let previous = palette.replace_color(request.index, request.color);
        match previous {
            // A drag re-sends the colour it already set; only the first of those
            // is a real edit.
            Some(previous) if previous == request.color => {}
            Some(_) => changed = true,
            None => warn!(
                "ignoring an edit to colour {} — palette has {}",
                request.index,
                palette.len()
            ),
        }
    }

    for request in adds.read() {
        let index = palette.add_color(request.color);
        // Select it: the editor below the swatches always edits the selection,
        // so adding a colour you cannot immediately change would be useless.
        palette.select(index);
        changed = true;
    }

    for request in removes.read() {
        if palette.len() <= 1 {
            warn!("refusing to remove the last colour — something must stay selected");
            continue;
        }
        if palette.remove_color(request.index).is_some() {
            changed = true;
        } else {
            warn!(
                "ignoring removal of colour {} — palette has {}",
                request.index,
                palette.len()
            );
        }
    }

    for request in moves.read() {
        // The arrows clamp at both ends rather than vanishing, so asking to move
        // an entry onto itself is how "already first" and "already last" arrive.
        if request.from == request.to {
            continue;
        }
        if palette.reorder_color(request.from, request.to) {
            changed = true;
        } else {
            warn!(
                "ignoring a move of colour {} to {} — palette has {}",
                request.from,
                request.to,
                palette.len()
            );
        }
    }

    if changed {
        document.mark_unsaved();
    }
}

fn apply_paint_requests(
    mut requests: MessageReader<PaintPixelRequested>,
    palette: Res<Palette>,
    mut document: ResMut<AtlasDocument>,
) {
    let color = palette.selected_color();
    let mut changed = false;

    for request in requests.read() {
        if !document.contains(request.x, request.y) {
            continue;
        }
        if document.set_pixel_at(request.x, request.y, color) {
            changed = true;
        }
    }

    // One revision bump per frame however many pixels a drag covered.
    if changed {
        document.touch();
    }
}

fn apply_add_tile(
    mut requests: MessageReader<AddTileRequested>,
    mut document: ResMut<AtlasDocument>,
) {
    let mut added = 0;
    for request in requests.read() {
        for _ in 0..request.tiles {
            document.add_tile_to_right();
            added += 1;
        }
    }

    if added > 0 {
        document.touch();
    }
}

// ============================================================================
// STARTUP
// ============================================================================

/// Builds the starting document before anything is loaded from disk.
pub fn seed_document(
    mut commands: Commands,
    options: Res<crate::cli::LaunchOptions>,
) {
    let mut document = AtlasDocument::with_tile_size(options.tile_size);
    // Explicit: a fresh document is entirely transparent.
    document.pixels.fill(transparent());

    commands.insert_resource(document);
    commands.insert_resource(Palette::new());
    // The generated ramps exist from the start but do not own the palette until
    // the mode button says so, so a document always opens on what its sidecar
    // actually stored.
    commands.insert_resource(OklabRamps::default());
    commands.insert_resource(PaletteMode::default());
    commands.insert_resource(DocumentPath {
        base: options.path.clone(),
        existed_on_disk: false,
    });
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_OKLAB_RAMPS;

    const RED: Srgba = Srgba::new(1.0, 0.0, 0.0, 1.0);
    const BLUE: Srgba = Srgba::new(0.0, 0.0, 1.0, 1.0);

    /// A minimal app with just the document plugin, so the appliers can be
    /// driven by messages exactly as they are at runtime.
    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(DocumentStatePlugin)
            .insert_resource(AtlasDocument::with_tile_size(4))
            .insert_resource(Palette::with_colors(vec![transparent(), RED, BLUE]))
            .insert_resource(OklabRamps::default())
            .insert_resource(PaletteMode::default());
        app
    }

    #[test]
    fn painting_writes_the_selected_colour() {
        let mut app = test_app();
        app.world_mut().resource_mut::<Palette>().select(1);
        app.world_mut().write_message(PaintPixelRequested { x: 1, y: 2 });
        app.update();

        let document = app.world().resource::<AtlasDocument>();
        assert_eq!(document.get_pixel_at(1, 2), RED);
        assert!(document.unsaved);
        assert_eq!(document.revision, 1);
    }

    #[test]
    fn selecting_then_painting_in_one_frame_uses_the_new_colour() {
        let mut app = test_app();
        app.world_mut().write_message(SelectColorRequested { index: 1 });
        app.world_mut().write_message(PaintPixelRequested { x: 0, y: 0 });
        app.update();

        assert_eq!(app.world().resource::<AtlasDocument>().get_pixel_at(0, 0), RED);
    }

    #[test]
    fn a_paint_outside_the_atlas_changes_nothing() {
        let mut app = test_app();
        app.world_mut().resource_mut::<Palette>().select(1);
        app.world_mut().write_message(PaintPixelRequested { x: 99, y: 99 });
        app.update();

        let document = app.world().resource::<AtlasDocument>();
        assert_eq!(document.revision, 0);
        assert!(!document.unsaved);
    }

    #[test]
    fn repainting_the_same_colour_does_not_bump_the_revision() {
        let mut app = test_app();
        app.world_mut().resource_mut::<Palette>().select(1);
        app.world_mut().write_message(PaintPixelRequested { x: 0, y: 0 });
        app.update();
        let after_first = app.world().resource::<AtlasDocument>().revision;

        app.world_mut().write_message(PaintPixelRequested { x: 0, y: 0 });
        app.update();

        assert_eq!(
            app.world().resource::<AtlasDocument>().revision,
            after_first,
            "a no-op paint must not dirty the document"
        );
    }

    #[test]
    fn a_multi_pixel_drag_bumps_the_revision_once() {
        let mut app = test_app();
        app.world_mut().resource_mut::<Palette>().select(1);
        for x in 0..3 {
            app.world_mut().write_message(PaintPixelRequested { x, y: 0 });
        }
        app.update();

        assert_eq!(app.world().resource::<AtlasDocument>().revision, 1);
    }

    #[test]
    fn adding_a_tile_widens_the_document() {
        let mut app = test_app();
        app.world_mut().write_message(AddTileRequested { tiles: 2 });
        app.update();

        let document = app.world().resource::<AtlasDocument>();
        assert_eq!(document.width_in_tiles, 3);
        assert!(document.unsaved);
    }

    #[test]
    fn editing_a_colour_repaints_nothing_but_dirties_the_file() {
        let mut app = test_app();
        app.world_mut().write_message(SetPaletteColorRequested {
            index: 1,
            color: BLUE,
        });
        app.update();

        assert_eq!(app.world().resource::<Palette>().get_color(1), Some(BLUE));
        let document = app.world().resource::<AtlasDocument>();
        assert!(document.unsaved, "a save would now write a different sidecar");
        assert_eq!(
            document.revision, 0,
            "no pixel moved, so the texture must not be re-uploaded"
        );
    }

    #[test]
    fn editing_the_selected_colour_changes_what_the_next_paint_writes() {
        let mut app = test_app();
        app.world_mut().write_message(SelectColorRequested { index: 1 });
        app.world_mut().write_message(SetPaletteColorRequested {
            index: 1,
            color: BLUE,
        });
        app.world_mut().write_message(PaintPixelRequested { x: 0, y: 0 });
        app.update();

        assert_eq!(
            app.world().resource::<AtlasDocument>().get_pixel_at(0, 0),
            BLUE,
            "the edit must be visible to the paint in the same frame"
        );
    }

    #[test]
    fn re_setting_the_same_colour_does_not_dirty_the_document() {
        let mut app = test_app();
        app.world_mut().write_message(SetPaletteColorRequested {
            index: 1,
            color: RED,
        });
        app.update();

        assert!(
            !app.world().resource::<AtlasDocument>().unsaved,
            "a drag re-sends the colour it already set; that is not an edit"
        );
    }

    #[test]
    fn an_edit_to_a_missing_index_changes_nothing() {
        let mut app = test_app();
        let before = app.world().resource::<Palette>().colors.clone();
        app.world_mut().write_message(SetPaletteColorRequested {
            index: 42,
            color: BLUE,
        });
        app.update();

        assert_eq!(app.world().resource::<Palette>().colors, before);
        assert!(!app.world().resource::<AtlasDocument>().unsaved);
    }

    #[test]
    fn adding_a_colour_selects_it() {
        let mut app = test_app();
        app.world_mut()
            .write_message(AddPaletteColorRequested { color: BLUE });
        app.update();

        let palette = app.world().resource::<Palette>();
        assert_eq!(palette.len(), 4);
        assert_eq!(palette.selected, 3, "the editor edits the selection");
        assert_eq!(palette.selected_color(), BLUE);
        assert!(app.world().resource::<AtlasDocument>().unsaved);
    }

    #[test]
    fn adding_then_editing_in_one_frame_lands_on_the_new_colour() {
        let mut app = test_app();
        app.world_mut()
            .write_message(AddPaletteColorRequested { color: BLUE });
        app.update();
        app.world_mut().write_message(SetPaletteColorRequested {
            index: 3,
            color: RED,
        });
        app.update();

        assert_eq!(app.world().resource::<Palette>().get_color(3), Some(RED));
    }

    #[test]
    fn removing_a_colour_keeps_the_selection_in_range() {
        let mut app = test_app();
        app.world_mut().resource_mut::<Palette>().select(2);
        app.world_mut()
            .write_message(RemovePaletteColorRequested { index: 2 });
        app.update();

        let palette = app.world().resource::<Palette>();
        assert_eq!(palette.len(), 2);
        assert!(palette.selected < palette.len());
        assert!(app.world().resource::<AtlasDocument>().unsaved);
    }

    #[test]
    fn the_last_colour_cannot_be_removed() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(DocumentStatePlugin)
            .insert_resource(AtlasDocument::with_tile_size(4))
            .insert_resource(Palette::with_colors(vec![RED]))
            .insert_resource(OklabRamps::default())
            .insert_resource(PaletteMode::default());
        app.world_mut()
            .write_message(RemovePaletteColorRequested { index: 0 });
        app.update();

        let palette = app.world().resource::<Palette>();
        assert_eq!(palette.len(), 1, "there must always be something to draw with");
        assert_eq!(palette.selected_color(), RED);
        assert!(!app.world().resource::<AtlasDocument>().unsaved);
    }

    #[test]
    fn removing_a_missing_index_changes_nothing() {
        let mut app = test_app();
        app.world_mut()
            .write_message(RemovePaletteColorRequested { index: 42 });
        app.update();

        assert_eq!(app.world().resource::<Palette>().len(), 3);
        assert!(!app.world().resource::<AtlasDocument>().unsaved);
    }

    #[test]
    fn moving_a_colour_carries_the_selection_with_it() {
        let mut app = test_app();
        app.world_mut().resource_mut::<Palette>().select(0);
        app.world_mut()
            .write_message(MovePaletteColorRequested { from: 0, to: 2 });
        app.update();

        let palette = app.world().resource::<Palette>();
        assert_eq!(palette.colors, vec![RED, BLUE, transparent()]);
        assert_eq!(
            palette.selected, 2,
            "the selection follows its colour, not its old slot"
        );
        assert!(app.world().resource::<AtlasDocument>().unsaved);
    }

    #[test]
    fn moving_a_colour_past_a_selected_neighbour_shifts_the_selection() {
        let mut app = test_app();
        app.world_mut().resource_mut::<Palette>().select(1);
        app.world_mut()
            .write_message(MovePaletteColorRequested { from: 2, to: 0 });
        app.update();

        let palette = app.world().resource::<Palette>();
        assert_eq!(palette.colors, vec![BLUE, transparent(), RED]);
        assert_eq!(
            palette.selected, 2,
            "the selected colour moved right to make room"
        );
    }

    /// The arrows clamp at both ends rather than disappearing, so this is how
    /// "already first" arrives — it must not dirty the document.
    #[test]
    fn moving_a_colour_onto_itself_is_a_no_op() {
        let mut app = test_app();
        let before = app.world().resource::<Palette>().colors.clone();
        app.world_mut()
            .write_message(MovePaletteColorRequested { from: 0, to: 0 });
        app.update();

        assert_eq!(app.world().resource::<Palette>().colors, before);
        assert!(!app.world().resource::<AtlasDocument>().unsaved);
    }

    #[test]
    fn moving_to_a_missing_index_changes_nothing() {
        let mut app = test_app();
        let before = app.world().resource::<Palette>().colors.clone();
        app.world_mut()
            .write_message(MovePaletteColorRequested { from: 0, to: 42 });
        app.update();

        assert_eq!(app.world().resource::<Palette>().colors, before);
        assert!(!app.world().resource::<AtlasDocument>().unsaved);
    }

    /// A move reorders the sidecar but repaints nothing: pixels store colours,
    /// not palette indices.
    #[test]
    fn moving_a_colour_leaves_the_pixels_alone() {
        let mut app = test_app();
        app.world_mut().resource_mut::<Palette>().select(1);
        app.world_mut().write_message(PaintPixelRequested { x: 0, y: 0 });
        app.update();
        let revision_after_paint = app.world().resource::<AtlasDocument>().revision;

        app.world_mut()
            .write_message(MovePaletteColorRequested { from: 1, to: 2 });
        app.update();

        let document = app.world().resource::<AtlasDocument>();
        assert_eq!(document.get_pixel_at(0, 0), RED, "the painted pixel keeps its colour");
        assert_eq!(document.revision, revision_after_paint);
    }

    // ------------------------------------------------------------------------
    // The two palette specifications
    // ------------------------------------------------------------------------

    fn toggle_mode(app: &mut App) {
        app.world_mut().write_message(TogglePaletteModeRequested);
        app.update();
    }

    #[test]
    fn switching_mode_hands_the_palette_to_the_ramps() {
        let mut app = test_app();
        toggle_mode(&mut app);

        let expected = app.world().resource::<OklabRamps>().colors();
        assert!(app.world().resource::<PaletteMode>().is_generated());
        assert_eq!(app.world().resource::<Palette>().colors, expected);
        assert!(app.world().resource::<AtlasDocument>().unsaved);
    }

    /// The hand-edited palette is parked, not rebuilt: switching back has to
    /// return the colours *and* the selection that were there.
    #[test]
    fn switching_back_restores_the_hand_edited_palette() {
        let mut app = test_app();
        app.world_mut().resource_mut::<Palette>().select(2);
        let before = app.world().resource::<Palette>().clone();

        toggle_mode(&mut app);
        toggle_mode(&mut app);

        let palette = app.world().resource::<Palette>();
        assert!(!app.world().resource::<PaletteMode>().is_generated());
        assert_eq!(palette.colors, before.colors);
        assert_eq!(palette.selected, 2);
    }

    /// The button clamps nothing and the `M` key repeats, so a double toggle is
    /// a real thing to arrive in one frame. Applying both would stash the
    /// generated palette over the hand-edited one and lose it.
    #[test]
    fn two_switches_in_one_frame_cancel_out() {
        let mut app = test_app();
        let before = app.world().resource::<Palette>().colors.clone();

        app.world_mut().write_message(TogglePaletteModeRequested);
        app.world_mut().write_message(TogglePaletteModeRequested);
        app.update();

        assert!(!app.world().resource::<PaletteMode>().is_generated());
        assert_eq!(app.world().resource::<Palette>().colors, before);
    }

    /// The colours are derived, so an edit to one would be overwritten by the
    /// next regenerate. Refusing outright beats appearing to work.
    #[test]
    fn a_colour_cannot_be_hand_edited_while_the_ramps_generate_it() {
        let mut app = test_app();
        toggle_mode(&mut app);
        let generated = app.world().resource::<Palette>().colors.clone();

        app.world_mut().write_message(SetPaletteColorRequested {
            index: 0,
            color: RED,
        });
        app.world_mut()
            .write_message(AddPaletteColorRequested { color: BLUE });
        app.world_mut()
            .write_message(RemovePaletteColorRequested { index: 1 });
        app.world_mut()
            .write_message(MovePaletteColorRequested { from: 0, to: 1 });
        app.update();

        assert_eq!(app.world().resource::<Palette>().colors, generated);
    }

    /// Refused edits must be drained, not left buffered: a reader that only
    /// woke up after the switch back would apply the whole backlog at once.
    #[test]
    fn a_refused_colour_edit_does_not_wait_for_the_mode_to_change_back() {
        let mut app = test_app();
        toggle_mode(&mut app);

        app.world_mut().write_message(SetPaletteColorRequested {
            index: 0,
            color: RED,
        });
        app.update();
        toggle_mode(&mut app);

        assert_eq!(
            app.world().resource::<Palette>().get_color(0),
            Some(transparent()),
            "the refused edit must not land once the list is back"
        );
    }

    #[test]
    fn editing_a_ramp_regenerates_the_palette() {
        let mut app = test_app();
        toggle_mode(&mut app);

        let mut ramp = *app.world().resource::<OklabRamps>().get(0).unwrap();
        ramp.steps = 3;
        app.world_mut()
            .write_message(SetRampRequested { index: 0, ramp });
        app.update();

        let ramps = app.world().resource::<OklabRamps>();
        assert_eq!(ramps.step_counts()[0], 3);
        assert_eq!(app.world().resource::<Palette>().colors, ramps.colors());
    }

    /// A drag re-sends the ramp it already set. Regenerating on that would
    /// rebuild the swatch grid every frame the pointer is down.
    #[test]
    fn re_setting_the_same_ramp_does_not_dirty_the_document() {
        let mut app = test_app();
        toggle_mode(&mut app);
        app.world_mut().resource_mut::<AtlasDocument>().unsaved = false;

        let ramp = *app.world().resource::<OklabRamps>().get(0).unwrap();
        app.world_mut()
            .write_message(SetRampRequested { index: 0, ramp });
        app.update();

        assert!(!app.world().resource::<AtlasDocument>().unsaved);
    }

    #[test]
    fn adding_a_ramp_copies_the_edited_one_and_selects_it() {
        let mut app = test_app();
        toggle_mode(&mut app);

        app.world_mut().write_message(AddRampRequested);
        app.update();

        let ramps = app.world().resource::<OklabRamps>();
        assert_eq!(ramps.len(), DEFAULT_OKLAB_RAMPS.len() + 1);
        assert_eq!(
            ramps.get(ramps.len() - 1),
            ramps.get(0),
            "a copy of the ramp being edited, so tweaking it is one drag away"
        );

        let palette = app.world().resource::<Palette>();
        assert_eq!(
            palette.selected,
            ramps.offset_of(ramps.len() - 1),
            "the editor edits the selection, so it has to move to the new ramp"
        );
        assert_eq!(palette.colors, ramps.colors());
    }

    #[test]
    fn removing_a_ramp_leaves_the_selection_on_one_that_exists() {
        let mut app = test_app();
        toggle_mode(&mut app);
        // Select into the last ramp, then remove it.
        let last = app.world().resource::<OklabRamps>().len() - 1;
        let offset = app.world().resource::<OklabRamps>().offset_of(last);
        app.world_mut().resource_mut::<Palette>().select(offset);

        app.world_mut()
            .write_message(RemoveRampRequested { index: last });
        app.update();

        let ramps = app.world().resource::<OklabRamps>();
        assert_eq!(ramps.len(), DEFAULT_OKLAB_RAMPS.len() - 1);
        let palette = app.world().resource::<Palette>();
        assert!(palette.selected < palette.len());
        assert_eq!(palette.colors, ramps.colors());
    }

    #[test]
    fn the_last_ramp_cannot_be_removed() {
        let mut app = test_app();
        app.world_mut().insert_resource(OklabRamps {
            ramps: vec![DEFAULT_OKLAB_RAMPS[0]],
        });
        toggle_mode(&mut app);

        app.world_mut()
            .write_message(RemoveRampRequested { index: 0 });
        app.update();

        assert_eq!(app.world().resource::<OklabRamps>().len(), 1);
        assert!(!app.world().resource::<Palette>().is_empty());
    }

    /// The ramps exist in both modes but only fill the palette in one, so moving
    /// them while the list is in charge must leave the colours alone — and still
    /// dirty the document, because the sidecar carries the ramps either way.
    #[test]
    fn a_ramp_edit_off_mode_leaves_the_hand_edited_palette_alone() {
        let mut app = test_app();
        let before = app.world().resource::<Palette>().colors.clone();

        let mut ramp = *app.world().resource::<OklabRamps>().get(0).unwrap();
        ramp.steps = 3;
        app.world_mut()
            .write_message(SetRampRequested { index: 0, ramp });
        app.update();

        assert_eq!(app.world().resource::<OklabRamps>().step_counts()[0], 3);
        assert_eq!(app.world().resource::<Palette>().colors, before);
        assert!(
            app.world().resource::<AtlasDocument>().unsaved,
            "a save would now write a different sidecar"
        );
        assert_eq!(
            app.world().resource::<AtlasDocument>().revision,
            0,
            "no pixel moved, so the texture must not be re-uploaded"
        );
    }

    #[test]
    fn an_edit_to_a_missing_ramp_changes_nothing() {
        let mut app = test_app();
        toggle_mode(&mut app);
        let before = app.world().resource::<OklabRamps>().clone();

        app.world_mut().write_message(SetRampRequested {
            index: 42,
            ramp: DEFAULT_OKLAB_RAMPS[0],
        });
        app.world_mut()
            .write_message(RemoveRampRequested { index: 42 });
        app.update();

        assert_eq!(
            app.world().resource::<OklabRamps>().ramps,
            before.ramps
        );
    }

    /// The whole point of generating into `Palette`: painting does not know or
    /// care which specification produced the colour.
    #[test]
    fn a_generated_colour_paints_like_any_other() {
        let mut app = test_app();
        app.world_mut().write_message(TogglePaletteModeRequested);
        app.world_mut().write_message(SelectColorRequested { index: 4 });
        app.world_mut().write_message(PaintPixelRequested { x: 1, y: 1 });
        app.update();

        let expected = app.world().resource::<OklabRamps>().colors()[4];
        assert_eq!(
            app.world().resource::<AtlasDocument>().get_pixel_at(1, 1),
            expected,
            "switch, select and paint in one frame must use the new palette"
        );
    }

    #[test]
    fn an_invalid_selection_leaves_the_draw_colour_alone() {
        let mut app = test_app();
        app.world_mut().resource_mut::<Palette>().select(1);
        app.world_mut().write_message(SelectColorRequested { index: 42 });
        app.update();

        assert_eq!(app.world().resource::<Palette>().selected, 1);
    }
}
