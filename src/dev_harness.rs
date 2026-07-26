//! The dev harness: replay a scripted action list, capture the window, exit.
//!
//! It drives the editor **only** by emitting the same requests real input emits,
//! so a screenshot exercises the production path — it cannot look right while
//! the actual interaction is broken.
//!
//! ```bash
//! cargo run -- demo --script "select 3; color #ff8800; paint 2,3; tile+" \
//!                   --screenshot /tmp/shot.png
//! ```
//!
//! `color`, `color+` and `color-` stand in for the sidebar's channel sliders and
//! its two buttons, which have no pointer to drive them here — they emit the
//! same requests those widgets do, against the selected entry.
//!
//! The window manager may resize the window a frame or two after it opens, which
//! changes the canvas rect. `warmup_frames` waits that out before the first step,
//! and `settle_frames` waits for the last edit to reach the screen before the
//! capture.
//!
//! With neither `--script` nor `--screenshot` this plugin does nothing at all.

use std::num::NonZero;
use std::path::PathBuf;

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};

use crate::canvas_view::ScrollRequested;
use crate::cli::LaunchOptions;
use crate::config::{hex_to_color, NEW_PALETTE_COLOR};
use crate::document::{
    AddPaletteColorRequested, AddTileRequested, PaintPixelRequested, RemovePaletteColorRequested,
    SelectColorRequested, SetPaletteColorRequested,
};
use crate::file_io::SaveRequested;

/// How long to wait for a capture before giving up.
const SCREENSHOT_TIMEOUT_FRAMES: u32 = 600;

// ============================================================================
// SCRIPT
// ============================================================================

/// One scripted action. Each runs on its own frame.
// No `Eq`: `SetColor` carries f32 channels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DevStep {
    Select(usize),
    Paint(u32, u32),
    AddTile(u32),
    Scroll(i32),
    Save,
    Wait(u32),
    /// Recolour the selected entry — what dragging the channel sliders does.
    SetColor(Srgba),
    /// Append a colour and select it, as the `+` button does.
    AddColor,
    /// Drop the selected entry, as the `−` button does.
    RemoveColor,
}

#[derive(Resource, Debug, Default)]
pub struct DevScript {
    pub steps: Vec<DevStep>,
    pub cursor: usize,
    pub wait_frames: u32,
    /// Counts down before the first step, so the window has stopped resizing.
    pub warmup_frames: u32,
    pub settle_frames: u32,
    pub frames_since_done: u32,
    pub screenshot_path: Option<PathBuf>,
    pub shot_requested: bool,
    /// False when neither a script nor a screenshot was asked for.
    pub enabled: bool,
}

impl DevScript {
    fn finished(&self) -> bool {
        self.warmup_frames == 0 && self.cursor >= self.steps.len() && self.wait_frames == 0
    }
}

/// Parses `"select 3; paint 2,3; tile+2"`. Unknown verbs are reported and
/// skipped, so one typo does not discard the rest of the run.
pub fn parse_script(source: &str) -> Vec<DevStep> {
    source
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .filter_map(parse_step)
        .collect()
}

fn parse_step(part: &str) -> Option<DevStep> {
    let (verb, argument) = match part.split_once(char::is_whitespace) {
        Some((verb, rest)) => (verb.trim(), rest.trim()),
        None => (part, ""),
    };

    // `tile+`, `tile+3`, `scroll-2`, `color+` carry their argument in the verb.
    if verb == "color+" {
        return Some(DevStep::AddColor);
    }
    if verb == "color-" {
        return Some(DevStep::RemoveColor);
    }
    if let Some(count) = verb.strip_prefix("tile+") {
        return Some(DevStep::AddTile(count_or(count, 1)));
    }
    if let Some(count) = verb.strip_prefix("scroll+") {
        return Some(DevStep::Scroll(count_or(count, 1) as i32));
    }
    if let Some(count) = verb.strip_prefix("scroll-") {
        return Some(DevStep::Scroll(-(count_or(count, 1) as i32)));
    }

    match verb {
        "select" => match argument.parse() {
            Ok(index) => Some(DevStep::Select(index)),
            Err(_) => {
                warn!("script: `select` needs a colour index, got {argument:?}");
                None
            }
        },
        "paint" => parse_coordinate(argument),
        "color" => match hex_to_color(argument) {
            Some(color) => Some(DevStep::SetColor(color)),
            None => {
                warn!("script: `color` needs #rrggbb or #rrggbbaa, got {argument:?}");
                None
            }
        },
        "tile" => Some(DevStep::AddTile(count_or(argument, 1))),
        "save" => Some(DevStep::Save),
        "wait" => Some(DevStep::Wait(count_or(argument, 1))),
        other => {
            warn!("script: ignoring unknown verb {other:?}");
            None
        }
    }
}

fn parse_coordinate(argument: &str) -> Option<DevStep> {
    let (x, y) = argument.split_once(',')?;
    match (x.trim().parse(), y.trim().parse()) {
        (Ok(x), Ok(y)) => Some(DevStep::Paint(x, y)),
        _ => {
            warn!("script: `paint` needs X,Y, got {argument:?}");
            None
        }
    }
}

fn count_or(raw: &str, fallback: u32) -> u32 {
    if raw.trim().is_empty() {
        return fallback;
    }
    raw.trim().parse().unwrap_or(fallback)
}

// ============================================================================
// PLUGIN
// ============================================================================

pub struct DevHarnessPlugin;

impl Plugin for DevHarnessPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DevScript>()
            .add_systems(Startup, load_script)
            .add_systems(Update, (run_dev_script, capture_and_exit).chain());
    }
}

// ============================================================================
// SYSTEMS
// ============================================================================

fn load_script(options: Res<LaunchOptions>, mut script: ResMut<DevScript>) {
    let steps = options
        .script
        .as_deref()
        .map(parse_script)
        .unwrap_or_default();

    script.enabled = !steps.is_empty() || options.screenshot_path.is_some();
    script.steps = steps;
    script.warmup_frames = options.warmup_frames;
    script.settle_frames = options.settle_frames.max(1);
    script.screenshot_path = options.screenshot_path.clone();

    if script.enabled {
        info!(
            "dev harness: {} step(s), screenshot {:?}",
            script.steps.len(),
            script.screenshot_path
        );
    }
}

/// One step per frame, so the same script always yields the same screenshot.
///
/// `Palette` is read, never written: `color` and `color-` act on the selected
/// entry, exactly as the sidebar's sliders and `−` button do.
fn run_dev_script(
    mut script: ResMut<DevScript>,
    palette: Option<Res<crate::palette::Palette>>,
    mut paint: MessageWriter<PaintPixelRequested>,
    mut select: MessageWriter<SelectColorRequested>,
    mut add_tile: MessageWriter<AddTileRequested>,
    mut scroll: MessageWriter<ScrollRequested>,
    mut save: MessageWriter<SaveRequested>,
    mut set_color: MessageWriter<SetPaletteColorRequested>,
    mut add_color: MessageWriter<AddPaletteColorRequested>,
    mut remove_color: MessageWriter<RemovePaletteColorRequested>,
) {
    if !script.enabled {
        return;
    }
    // Let the window finish opening and resizing before touching anything.
    if script.warmup_frames > 0 {
        script.warmup_frames -= 1;
        return;
    }
    if script.wait_frames > 0 {
        script.wait_frames -= 1;
        return;
    }

    let Some(step) = script.steps.get(script.cursor).copied() else {
        return;
    };
    script.cursor += 1;

    match step {
        DevStep::Select(index) => {
            select.write(SelectColorRequested { index });
        }
        DevStep::Paint(x, y) => {
            paint.write(PaintPixelRequested { x, y });
        }
        DevStep::AddTile(tiles) => {
            add_tile.write(AddTileRequested { tiles });
        }
        DevStep::Scroll(tiles) => {
            scroll.write(ScrollRequested { tiles });
        }
        DevStep::Save => {
            save.write(SaveRequested);
        }
        DevStep::Wait(frames) => {
            script.wait_frames = frames.saturating_sub(1);
        }
        DevStep::SetColor(color) => {
            if let Some(index) = palette.as_deref().map(|p| p.selected) {
                set_color.write(SetPaletteColorRequested { index, color });
            }
        }
        DevStep::AddColor => {
            add_color.write(AddPaletteColorRequested {
                color: NEW_PALETTE_COLOR,
            });
        }
        DevStep::RemoveColor => {
            if let Some(index) = palette.as_deref().map(|p| p.selected) {
                remove_color.write(RemovePaletteColorRequested { index });
            }
        }
    }
}

/// Waits for the world to settle, captures, then quits.
fn capture_and_exit(
    mut commands: Commands,
    mut script: ResMut<DevScript>,
    mut exit: MessageWriter<AppExit>,
) {
    if !script.enabled || !script.finished() {
        return;
    }

    script.frames_since_done += 1;
    // Let the texture upload and the layout land before capturing.
    if script.frames_since_done < script.settle_frames {
        return;
    }

    let Some(path) = script.screenshot_path.clone() else {
        // Script-only run: leave the window open for a human.
        return;
    };

    if !script.shot_requested {
        // Removed first, so "the file exists" means "this run wrote it".
        let _ = std::fs::remove_file(&path);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path.clone()));
        script.shot_requested = true;
        return;
    }

    // The capture is asynchronous; the observer writes the file when it lands.
    let written = std::fs::metadata(&path).map(|m| m.len() > 0).unwrap_or(false);
    if written {
        info!("dev harness: captured {}", path.display());
        exit.write(AppExit::Success);
    } else if script.frames_since_done > script.settle_frames + SCREENSHOT_TIMEOUT_FRAMES {
        error!("dev harness: no screenshot after {SCREENSHOT_TIMEOUT_FRAMES} frames");
        exit.write(AppExit::Error(NonZero::new(1).unwrap()));
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_documented_script_parses() {
        assert_eq!(
            parse_script("select 3; paint 2,3; tile+; scroll+"),
            vec![
                DevStep::Select(3),
                DevStep::Paint(2, 3),
                DevStep::AddTile(1),
                DevStep::Scroll(1),
            ]
        );
    }

    #[test]
    fn counted_forms_are_understood() {
        assert_eq!(
            parse_script("tile+3; scroll-2; wait 5"),
            vec![DevStep::AddTile(3), DevStep::Scroll(-2), DevStep::Wait(5)]
        );
    }

    #[test]
    fn the_palette_editing_verbs_parse() {
        assert_eq!(
            parse_script("color+; color #ff8800; color-"),
            vec![
                DevStep::AddColor,
                DevStep::SetColor(Srgba::new(1.0, 136.0 / 255.0, 0.0, 1.0)),
                DevStep::RemoveColor,
            ]
        );
    }

    #[test]
    fn a_colour_without_a_valid_hex_string_is_skipped() {
        assert_eq!(parse_script("color; color zzz; select 1"), vec![DevStep::Select(1)]);
    }


    #[test]
    fn whitespace_and_empty_segments_are_tolerated() {
        assert_eq!(
            parse_script("  select 1 ;; paint 0 , 0 ;  "),
            vec![DevStep::Select(1), DevStep::Paint(0, 0)]
        );
    }

    #[test]
    fn save_takes_no_argument() {
        assert_eq!(parse_script("save"), vec![DevStep::Save]);
    }

    #[test]
    fn a_bad_verb_is_skipped_without_losing_the_rest() {
        assert_eq!(
            parse_script("select 2; frobnicate; paint 1,1"),
            vec![DevStep::Select(2), DevStep::Paint(1, 1)]
        );
    }

    #[test]
    fn a_malformed_argument_is_skipped() {
        assert_eq!(parse_script("paint 3; select x"), Vec::new());
    }

    #[test]
    fn an_empty_script_yields_no_steps() {
        assert!(parse_script("").is_empty());
        assert!(parse_script("   ;  ").is_empty());
    }

    /// The harness must emit the same requests real input does, one per frame.
    #[test]
    fn stepping_emits_one_request_per_frame() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<PaintPixelRequested>()
            .add_message::<SelectColorRequested>()
            .add_message::<AddTileRequested>()
            .add_message::<ScrollRequested>()
            .add_message::<SaveRequested>()
            .add_message::<SetPaletteColorRequested>()
            .add_message::<AddPaletteColorRequested>()
            .add_message::<RemovePaletteColorRequested>()
            .insert_resource(DevScript {
                steps: parse_script("select 3; paint 1,2"),
                settle_frames: 1,
                warmup_frames: 0,
                enabled: true,
                ..default()
            })
            .add_systems(Update, run_dev_script);

        app.update();
        assert_eq!(
            app.world()
                .resource::<Messages<SelectColorRequested>>()
                .iter_current_update_messages()
                .map(|m| m.index)
                .collect::<Vec<_>>(),
            vec![3]
        );
        assert_eq!(
            app.world()
                .resource::<Messages<PaintPixelRequested>>()
                .iter_current_update_messages()
                .count(),
            0,
            "the second step must wait for the next frame"
        );

        app.update();
        assert_eq!(
            app.world()
                .resource::<Messages<PaintPixelRequested>>()
                .iter_current_update_messages()
                .map(|m| (m.x, m.y))
                .collect::<Vec<_>>(),
            vec![(1, 2)]
        );

        assert!(app.world().resource::<DevScript>().finished());
    }

    #[test]
    fn wait_holds_the_cursor_for_the_requested_frames() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<PaintPixelRequested>()
            .add_message::<SelectColorRequested>()
            .add_message::<AddTileRequested>()
            .add_message::<ScrollRequested>()
            .add_message::<SaveRequested>()
            .add_message::<SetPaletteColorRequested>()
            .add_message::<AddPaletteColorRequested>()
            .add_message::<RemovePaletteColorRequested>()
            .insert_resource(DevScript {
                steps: parse_script("wait 3; paint 0,0"),
                settle_frames: 1,
                warmup_frames: 0,
                enabled: true,
                ..default()
            })
            .add_systems(Update, run_dev_script);

        // Frame 1 consumes the wait, frames 2 and 3 idle, frame 4 paints.
        for _ in 0..3 {
            app.update();
            assert_eq!(
                app.world()
                    .resource::<Messages<PaintPixelRequested>>()
                    .iter_current_update_messages()
                    .count(),
                0
            );
        }
        app.update();
        assert_eq!(
            app.world()
                .resource::<Messages<PaintPixelRequested>>()
                .iter_current_update_messages()
                .count(),
            1
        );
    }

    #[test]
    fn a_disabled_harness_does_nothing() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<PaintPixelRequested>()
            .add_message::<SelectColorRequested>()
            .add_message::<AddTileRequested>()
            .add_message::<ScrollRequested>()
            .add_message::<SaveRequested>()
            .add_message::<SetPaletteColorRequested>()
            .add_message::<AddPaletteColorRequested>()
            .add_message::<RemovePaletteColorRequested>()
            .insert_resource(DevScript {
                steps: parse_script("paint 0,0"),
                enabled: false,
                ..default()
            })
            .add_systems(Update, run_dev_script);
        app.update();

        assert_eq!(
            app.world()
                .resource::<Messages<PaintPixelRequested>>()
                .iter_current_update_messages()
                .count(),
            0
        );
        assert_eq!(app.world().resource::<DevScript>().cursor, 0);
    }
}
