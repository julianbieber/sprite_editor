//! Raw input to edit requests. This module never touches document state — that
//! separation is what lets the dev harness exercise the same path.

use bevy::prelude::*;

use crate::canvas_view::{HoveredPixel, ScrollRequested};
use crate::document::{AddTileRequested, PaintPixelRequested, TogglePaletteModeRequested};
use crate::file_io::SaveRequested;

// ============================================================================
// RESOURCES
// ============================================================================

/// Tracks the in-progress drag, so entering a pixel emits once rather than every
/// frame the button is held over it.
#[derive(Resource, Debug, Default)]
pub struct PaintStroke {
    pub active: bool,
    pub last: Option<(u32, u32)>,
}

// ============================================================================
// PLUGIN
// ============================================================================

pub struct EditorInputPlugin;

impl Plugin for EditorInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PaintStroke>().add_systems(
            Update,
            (paint_on_click, keyboard_commands)
                // Requests are emitted after the hover resolves, so a click
                // lands on the pixel under the cursor this frame.
                .after(crate::canvas_view::CanvasViewUpdate),
        );
    }
}

// ============================================================================
// SYSTEMS
// ============================================================================

/// Holding the left button paints; dragging paints a continuous line.
fn paint_on_click(
    mouse: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredPixel>,
    mut stroke: ResMut<PaintStroke>,
    mut requests: MessageWriter<PaintPixelRequested>,
) {
    if !mouse.pressed(MouseButton::Left) {
        if stroke.active || stroke.last.is_some() {
            stroke.active = false;
            stroke.last = None;
        }
        return;
    }

    // Off-canvas mid-drag: hold the stroke open so dragging back resumes it,
    // but forget the last pixel so re-entry repaints.
    if !hovered.valid {
        return;
    }

    let pixel = (hovered.x, hovered.y);
    if stroke.last != Some(pixel) {
        requests.write(PaintPixelRequested {
            x: hovered.x,
            y: hovered.y,
        });
        stroke.last = Some(pixel);
        stroke.active = true;
    }
}

fn keyboard_commands(
    keys: Res<ButtonInput<KeyCode>>,
    mut add_tile: MessageWriter<AddTileRequested>,
    mut scroll: MessageWriter<ScrollRequested>,
    mut save: MessageWriter<SaveRequested>,
    mut palette_mode: MessageWriter<TogglePaletteModeRequested>,
) {
    if keys.just_pressed(KeyCode::Space) {
        add_tile.write(AddTileRequested { tiles: 1 });
    }

    // The same thing the sidebar's mode button does; `M` for mode.
    if keys.just_pressed(KeyCode::KeyM) {
        palette_mode.write(TogglePaletteModeRequested);
    }

    if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::KeyA) {
        scroll.write(ScrollRequested { tiles: -1 });
    }
    if keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::KeyD) {
        scroll.write(ScrollRequested { tiles: 1 });
    }

    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if ctrl && keys.just_pressed(KeyCode::KeyS) {
        save.write(SaveRequested);
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atlas::AtlasDocument;
    use crate::canvas_view::CanvasView;

    /// What the input systems emitted, collected the way a real consumer would:
    /// through `MessageReader`, whose per-reader cursor means these assertions do
    /// not depend on when message buffers get swapped.
    #[derive(Resource, Default)]
    struct Recorded {
        paints: Vec<(u32, u32)>,
        tiles: Vec<u32>,
        scrolls: Vec<i32>,
        saves: usize,
        mode_toggles: usize,
    }

    fn record(
        mut paints: MessageReader<PaintPixelRequested>,
        mut tiles: MessageReader<AddTileRequested>,
        mut scrolls: MessageReader<ScrollRequested>,
        mut saves: MessageReader<SaveRequested>,
        mut modes: MessageReader<TogglePaletteModeRequested>,
        mut recorded: ResMut<Recorded>,
    ) {
        recorded.mode_toggles += modes.read().count();
        let seen: Vec<(u32, u32)> = paints.read().map(|m| (m.x, m.y)).collect();
        recorded.paints.extend(seen);
        let seen: Vec<u32> = tiles.read().map(|m| m.tiles).collect();
        recorded.tiles.extend(seen);
        let seen: Vec<i32> = scrolls.read().map(|m| m.tiles).collect();
        recorded.scrolls.extend(seen);
        recorded.saves += saves.read().count();
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<HoveredPixel>()
            .init_resource::<PaintStroke>()
            .init_resource::<Recorded>()
            .insert_resource(AtlasDocument::with_tile_size(8))
            .insert_resource(CanvasView::default())
            .add_message::<PaintPixelRequested>()
            .add_message::<AddTileRequested>()
            .add_message::<ScrollRequested>()
            .add_message::<SaveRequested>()
            .add_message::<TogglePaletteModeRequested>()
            .add_systems(Update, (paint_on_click, keyboard_commands, record).chain());
        app
    }

    /// Drains what was emitted since the last call.
    fn paints(app: &mut App) -> Vec<(u32, u32)> {
        std::mem::take(&mut app.world_mut().resource_mut::<Recorded>().paints)
    }

    fn hover(app: &mut App, x: u32, y: u32) {
        *app.world_mut().resource_mut::<HoveredPixel>() = HoveredPixel { x, y, valid: true };
    }

    #[test]
    fn a_click_paints_the_hovered_pixel() {
        let mut app = test_app();
        hover(&mut app, 2, 3);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        assert_eq!(paints(&mut app), vec![(2, 3)]);
    }

    #[test]
    fn holding_still_does_not_repaint_the_same_pixel() {
        let mut app = test_app();
        hover(&mut app, 2, 3);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert_eq!(paints(&mut app), vec![(2, 3)]);

        app.update();
        assert!(
            paints(&mut app).is_empty(),
            "a held button over one pixel must emit once"
        );
    }

    #[test]
    fn dragging_across_pixels_paints_each_once() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);

        hover(&mut app, 0, 0);
        app.update();
        assert_eq!(paints(&mut app), vec![(0, 0)]);
        hover(&mut app, 1, 0);
        app.update();
        assert_eq!(paints(&mut app), vec![(1, 0)]);
        hover(&mut app, 2, 0);
        app.update();
        assert_eq!(paints(&mut app), vec![(2, 0)]);
    }

    #[test]
    fn a_click_off_canvas_paints_nothing() {
        let mut app = test_app();
        *app.world_mut().resource_mut::<HoveredPixel>() = HoveredPixel::default();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        assert!(paints(&mut app).is_empty());
    }

    #[test]
    fn releasing_the_button_ends_the_stroke() {
        let mut app = test_app();
        hover(&mut app, 4, 4);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert_eq!(paints(&mut app), vec![(4, 4)]);

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        assert!(!app.world().resource::<PaintStroke>().active);
        assert_eq!(app.world().resource::<PaintStroke>().last, None);

        // Pressing again over the same pixel paints it afresh.
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert_eq!(paints(&mut app), vec![(4, 4)]);
    }

    #[test]
    fn space_adds_a_tile() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.update();

        let tiles = std::mem::take(&mut app.world_mut().resource_mut::<Recorded>().tiles);
        assert_eq!(tiles, vec![1]);
    }

    /// `M` is the keyboard's half of the sidebar's mode button, and like every
    /// other binding it only asks — `document` decides.
    #[test]
    fn m_asks_to_switch_palette_mode() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyM);
        app.update();
        assert_eq!(app.world().resource::<Recorded>().mode_toggles, 1);
    }

    #[test]
    fn both_arrow_and_letter_keys_scroll() {
        for (key, want) in [
            (KeyCode::ArrowLeft, -1),
            (KeyCode::KeyA, -1),
            (KeyCode::ArrowRight, 1),
            (KeyCode::KeyD, 1),
        ] {
            let mut app = test_app();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(key);
            app.update();

            let scrolls =
                std::mem::take(&mut app.world_mut().resource_mut::<Recorded>().scrolls);
            assert_eq!(scrolls, vec![want], "{key:?} scrolled the wrong way");
        }
    }

    #[test]
    fn ctrl_s_saves_but_s_alone_does_not() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyS);
        app.update();
        assert_eq!(app.world().resource::<Recorded>().saves, 0);

        let mut app = test_app();
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyS);
        }
        app.update();
        assert_eq!(app.world().resource::<Recorded>().saves, 1);
    }
}
