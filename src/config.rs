//! Configuration constants and small shared helpers.

use bevy::prelude::*;

use crate::oklab::{OklabAxis, OklabRamp};

// ============================================================================
// DISPLAY CONFIGURATION
// ============================================================================

/// Screen pixels per atlas pixel, unless `--scale` overrides it.
///
/// 32 puts an 8x8 tile at 256 screen pixels, so roughly four tiles fit next to
/// the sidebar in the default window — wide enough that scrolling is useful.
pub const DEFAULT_PIXEL_SCALE: f32 = 32.0;

/// Pixels per side of a tile for a brand new document.
pub const DEFAULT_TILE_SIZE: u32 = 8;

/// Below this scale the per-pixel grid is more noise than help.
pub const PIXEL_GRID_MIN_SCALE: f32 = 12.0;

// ============================================================================
// WINDOW CONFIGURATION
// ============================================================================

pub const WINDOW_TITLE: &str = "Bevy Sprite Atlas Editor";
pub const DEFAULT_WINDOW_WIDTH: u32 = 1280;
pub const DEFAULT_WINDOW_HEIGHT: u32 = 800;

// ============================================================================
// UI CONFIGURATION
// ============================================================================

/// Width of the palette sidebar, in logical pixels.
pub const SIDEBAR_WIDTH: f32 = 200.0;

/// Height of the status strip at the bottom, in logical pixels.
pub const STATUS_BAR_HEIGHT: f32 = 28.0;

pub const SWATCH_SIZE: f32 = 44.0;
pub const SWATCH_GAP: f32 = 6.0;
pub const SWATCH_BORDER: f32 = 2.0;

/// Gap between two rows of the swatch grid. Wider than the gap between swatches
/// so that one generated ramp reads as one run: the sidebar only fits three
/// swatches across, so every ramp wraps and the grouping is carried entirely by
/// this being the larger of the two gaps.
pub const SWATCH_ROW_GAP: f32 = 16.0;

/// Vertical gap between the rows of the palette editor under the swatches.
pub const EDITOR_ROW_GAP: f32 = 5.0;

/// Width of the caption column beside a slider. Wide enough for the generated
/// editor's `from` and `to`, so both editors line their sliders up the same.
pub const EDITOR_LABEL_WIDTH: f32 = 26.0;

// ============================================================================
// COLOURS
// ============================================================================

/// Cleared behind everything; also what shows through transparent pixels.
pub const CANVAS_BACKGROUND: Color = Color::srgb(0.11, 0.11, 0.13);
pub const SIDEBAR_BACKGROUND: Color = Color::srgb(0.16, 0.16, 0.19);
pub const STATUS_BACKGROUND: Color = Color::srgb(0.09, 0.09, 0.11);
pub const STATUS_TEXT: Color = Color::srgb(0.78, 0.78, 0.82);
pub const STATUS_FONT_SIZE: f32 = 13.0;

/// Tile boundaries: bright enough to read over an opaque pixel.
pub const TILE_GRID_COLOR: Color = Color::srgb(0.95, 0.75, 0.25);
/// Pixel boundaries: only a hint.
pub const PIXEL_GRID_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.10);

// ============================================================================
// PALETTE CONFIGURATION
// ============================================================================

/// Default palette, `(r, g, b, a)` in 0.0..=1.0. Transparent first, so erasing
/// needs no separate tool.
pub const DEFAULT_PALETTE: &[(f32, f32, f32, f32)] = &[
    (0.0, 0.0, 0.0, 0.0),    // Transparent
    (0.0, 0.0, 0.0, 1.0),    // Black
    (1.0, 1.0, 1.0, 1.0),    // White
    (1.0, 0.0, 0.0, 1.0),    // Red
    (0.0, 1.0, 0.0, 1.0),    // Green
    (0.0, 0.0, 1.0, 1.0),    // Blue
    (1.0, 1.0, 0.0, 1.0),    // Yellow
    (0.0, 1.0, 1.0, 1.0),    // Cyan
    (1.0, 0.0, 1.0, 1.0),    // Magenta
    (0.5, 0.5, 0.5, 1.0),    // Gray 50%
    (0.25, 0.25, 0.25, 1.0), // Dark gray
    (0.75, 0.75, 0.75, 1.0), // Light gray
    (0.5, 0.25, 0.0, 1.0),   // Brown
    (0.0, 0.5, 0.0, 1.0),    // Dark green
    (0.0, 0.0, 0.5, 1.0),    // Dark blue
    (0.75, 0.5, 0.25, 1.0),  // Tan
    (0.5, 0.0, 0.5, 1.0),    // Purple
];

/// The colour a new document starts drawing with: the first opaque entry.
pub const DEFAULT_SELECTED_COLOR: usize = 1;

/// What the `+` button appends. Opaque white reads as "a new colour to set"
/// rather than looking like nothing happened, which appending transparent would.
pub const NEW_PALETTE_COLOR: Srgba = Srgba::new(1.0, 1.0, 1.0, 1.0);

// ============================================================================
// GENERATED (OKLAB) PALETTE CONFIGURATION
// ============================================================================

/// How far the `a` and `b` axes may be pushed in either direction.
///
/// Oklab's own range is ±1, but sRGB runs out of gamut long before ±0.4 at any
/// lightness, so a wider slider would spend most of its travel on colours the
/// screen cannot tell apart.
pub const OKLAB_AB_LIMIT: f32 = 0.4;

/// A generated ramp is at least a pair — there is nothing to interpolate with
/// one colour — and at most this, which is what the sidebar shows before the
/// swatch grid crowds out the ramps below it.
pub const OKLAB_MIN_STEPS: usize = 2;
pub const OKLAB_MAX_STEPS: usize = 24;
/// What the `N` slider starts a duplicated ramp on. Also the default ramps'.
pub const OKLAB_DEFAULT_STEPS: usize = 8;

/// The sweep a ramp adopts when it changes axis. The previous one was in the
/// old axis' units, where it means nothing: `0.15..0.95` reads as "shadow to
/// highlight" on lightness and as "well past the gamut both ways" on chroma.
pub const OKLAB_LIGHTNESS_SWEEP: (f32, f32) = (0.15, 0.95);
pub const OKLAB_CHROMA_SWEEP: (f32, f32) = (-0.15, 0.15);

/// Halvings the gamut search takes. 12 puts the chroma within 1/4096 of the
/// boundary, which is finer than an 8-bit channel can show.
pub const OKLAB_GAMUT_SEARCH_STEPS: u32 = 12;

/// The ramps the generated mode starts with — a warm and a cool value ramp to
/// shade with, and one hue sweep to pick from.
pub const DEFAULT_OKLAB_RAMPS: &[OklabRamp] = &[
    // Warm: the usual sprite shading run, shadow through highlight.
    OklabRamp {
        axis: OklabAxis::Lightness,
        lightness: 0.60,
        a: 0.06,
        b: 0.10,
        from: OKLAB_LIGHTNESS_SWEEP.0,
        to: OKLAB_LIGHTNESS_SWEEP.1,
        steps: OKLAB_DEFAULT_STEPS,
    },
    // Cool: the same sweep on the other side of neutral.
    OklabRamp {
        axis: OklabAxis::Lightness,
        lightness: 0.60,
        a: -0.05,
        b: -0.10,
        from: OKLAB_LIGHTNESS_SWEEP.0,
        to: OKLAB_LIGHTNESS_SWEEP.1,
        steps: OKLAB_DEFAULT_STEPS,
    },
    // Green through red at one lightness: picks a hue rather than a value.
    OklabRamp {
        axis: OklabAxis::A,
        lightness: 0.65,
        a: 0.0,
        b: 0.08,
        from: OKLAB_CHROMA_SWEEP.0,
        to: OKLAB_CHROMA_SWEEP.1,
        steps: OKLAB_DEFAULT_STEPS,
    },
];

// ============================================================================
// HISTORY CONFIGURATION
// ============================================================================

/// Retained undo depth. Only `history` reads this, which is not wired in yet.
#[allow(dead_code)]
pub const HISTORY_MAX_DEPTH: usize = 100;

// ============================================================================
// FILE CONFIGURATION
// ============================================================================

pub const FILE_FORMAT_VERSION: &str = "1.0";
pub const METADATA_SUFFIX: &str = "atlas.json";
pub const UNTITLED_BASE: &str = "untitled";

// ============================================================================
// RENDER LAYERS
// ============================================================================

/// Canvas content lives on its own render layer so the full-window UI camera
/// does not also draw it across the sidebar.
pub const CANVAS_LAYER: usize = 1;

// ============================================================================
// COLOUR UTILITIES
// ============================================================================

pub fn transparent() -> Srgba {
    Srgba::new(0.0, 0.0, 0.0, 0.0)
}

/// `#rrggbbaa`, the form the status strip reports.
pub fn color_to_hex(color: Srgba) -> String {
    let to_u8 = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}{:02x}",
        to_u8(color.red),
        to_u8(color.green),
        to_u8(color.blue),
        to_u8(color.alpha)
    )
}

/// The inverse of [`color_to_hex`]. Accepts `#rrggbbaa` and `#rrggbb` (alpha
/// defaults to opaque), with or without the leading `#`.
///
/// Only the dev harness's `color` verb parses colours from text; the sidebar
/// edits channels numerically.
pub fn hex_to_color(text: &str) -> Option<Srgba> {
    let digits = text.trim().trim_start_matches('#');
    if digits.len() != 6 && digits.len() != 8 {
        return None;
    }

    let channel = |offset: usize| u8::from_str_radix(&digits[offset..offset + 2], 16).ok();
    Some(Srgba::new(
        channel(0)? as f32 / 255.0,
        channel(2)? as f32 / 255.0,
        channel(4)? as f32 / 255.0,
        if digits.len() == 8 {
            channel(6)? as f32 / 255.0
        } else {
            1.0
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_the_obvious_colours() {
        assert_eq!(color_to_hex(transparent()), "#00000000");
        assert_eq!(color_to_hex(Srgba::new(1.0, 0.0, 0.0, 1.0)), "#ff0000ff");
        assert_eq!(color_to_hex(Srgba::new(1.0, 1.0, 1.0, 1.0)), "#ffffffff");
    }

    #[test]
    fn default_selected_colour_is_opaque() {
        let (_, _, _, a) = DEFAULT_PALETTE[DEFAULT_SELECTED_COLOR];
        assert_eq!(a, 1.0, "a new document must start on a visible colour");
    }

    #[test]
    fn hex_parsing_inverts_hex_formatting() {
        for color in [
            transparent(),
            Srgba::new(1.0, 0.0, 0.0, 1.0),
            Srgba::new(0.2, 0.4, 0.6, 0.8),
        ] {
            let round_tripped = hex_to_color(&color_to_hex(color)).unwrap();
            assert_eq!(color_to_hex(round_tripped), color_to_hex(color));
        }
    }

    #[test]
    fn six_digit_hex_is_opaque() {
        assert_eq!(hex_to_color("#00ff00").unwrap(), Srgba::new(0.0, 1.0, 0.0, 1.0));
        assert_eq!(hex_to_color("00ff00").unwrap(), Srgba::new(0.0, 1.0, 0.0, 1.0));
    }

    #[test]
    fn a_malformed_hex_string_is_rejected_rather_than_guessed_at() {
        assert!(hex_to_color("#fff").is_none());
        assert!(hex_to_color("#gggggg").is_none());
        assert!(hex_to_color("").is_none());
    }
}
