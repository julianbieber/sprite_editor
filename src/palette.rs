//! The colour palette, and its serialisable mirror.
//!
//! `Srgba` is not `Serialize`, so [`ColorData`] / [`PaletteData`] exist purely to
//! carry the palette through the JSON sidecar.
//!
//! There are two ways to say what the palette holds, and [`PaletteMode`] says
//! which one is in charge — a hand-edited list, or the Oklab ramps in
//! [`crate::oklab`]. Only ever one [`Palette`] exists either way: the generated
//! mode overwrites it and parks the hand-edited one until it is switched back,
//! so nothing downstream has to know which mode produced the colour it is
//! painting with.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::config::{color_to_hex, DEFAULT_PALETTE, DEFAULT_SELECTED_COLOR};

// ============================================================================
// SERIALISABLE MIRROR
// ============================================================================

/// An RGBA colour as stored in the JSON sidecar.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColorData {
    #[serde(rename = "r")]
    pub red: f32,
    #[serde(rename = "g")]
    pub green: f32,
    #[serde(rename = "b")]
    pub blue: f32,
    #[serde(rename = "a")]
    pub alpha: f32,
}

impl ColorData {
    pub fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            red: red.clamp(0.0, 1.0),
            green: green.clamp(0.0, 1.0),
            blue: blue.clamp(0.0, 1.0),
            alpha: alpha.clamp(0.0, 1.0),
        }
    }

    pub fn to_srgba(self) -> Srgba {
        Srgba::new(self.red, self.green, self.blue, self.alpha)
    }

    pub fn from_srgba(color: Srgba) -> Self {
        Self::new(color.red, color.green, color.blue, color.alpha)
    }
}

/// The palette as stored in the JSON sidecar. `selected_index` stays an
/// `Option` for compatibility with format 1.0 files already on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaletteData {
    pub colors: Vec<ColorData>,
    #[serde(default)]
    pub selected_index: Option<usize>,
}

impl PaletteData {
    pub fn to_palette(&self) -> Palette {
        let colors: Vec<Srgba> = self.colors.iter().map(|c| c.to_srgba()).collect();
        let selected = self
            .selected_index
            .unwrap_or(0)
            .min(colors.len().saturating_sub(1));
        Palette { colors, selected }
    }

    pub fn from_palette(palette: &Palette) -> Self {
        Self {
            colors: palette
                .colors
                .iter()
                .map(|&c| ColorData::from_srgba(c))
                .collect(),
            selected_index: Some(palette.selected),
        }
    }
}

// ============================================================================
// MODE
// ============================================================================

/// Which of the two ways of specifying a palette currently owns [`Palette`].
///
/// The invariant is `stashed.is_some() == generated`: the hand-edited palette is
/// parked here exactly while the generated one is live, so switching back
/// restores the colours and the selection rather than rebuilding a default.
/// [`crate::document`] is the only place that maintains it.
#[derive(Resource, Debug, Clone, Default)]
pub struct PaletteMode {
    generated: bool,
    stashed: Option<Palette>,
}

impl PaletteMode {
    pub fn is_generated(&self) -> bool {
        self.generated
    }

    /// What the mode button and the status strip call the current mode.
    pub fn label(&self) -> &'static str {
        if self.generated {
            "oklab"
        } else {
            "list"
        }
    }

    /// The parked hand-edited palette, for the sidecar to write alongside the
    /// generated one. `None` is not "empty" — it is what says the live palette
    /// *is* the hand-edited one.
    pub fn stashed(&self) -> Option<&Palette> {
        self.stashed.as_ref()
    }

    /// Parks `live` and switches to the generated mode. The caller installs the
    /// generated colours; this only remembers what they replaced.
    pub fn stash(&mut self, live: Palette) {
        self.stashed = Some(live);
        self.generated = true;
    }

    /// Switches back, handing the parked palette over. `None` means the mode was
    /// already the hand-edited one, which the caller treats as "nothing to do"
    /// rather than as an error.
    pub fn restore(&mut self) -> Option<Palette> {
        self.generated = false;
        self.stashed.take()
    }
}

// ============================================================================
// PALETTE
// ============================================================================

/// The editor's colour palette. Exactly one colour is selected at all times, so
/// there is always something to draw with.
#[derive(Debug, Clone, Resource)]
pub struct Palette {
    pub colors: Vec<Srgba>,
    /// Always a valid index into `colors` while `colors` is non-empty.
    pub selected: usize,
}

impl Default for Palette {
    fn default() -> Self {
        Self::new()
    }
}

impl Palette {
    // ------------------------------------------------------------------------
    // Constructors
    // ------------------------------------------------------------------------

    /// The built-in palette, starting on the first opaque colour.
    pub fn new() -> Self {
        let colors: Vec<Srgba> = DEFAULT_PALETTE
            .iter()
            .map(|&(r, g, b, a)| Srgba::new(r, g, b, a))
            .collect();
        let selected = DEFAULT_SELECTED_COLOR.min(colors.len().saturating_sub(1));
        Self { colors, selected }
    }

    /// A palette of exactly these colours. Used by the tests and by a loaded
    /// sidecar via [`PaletteData::to_palette`].
    #[allow(dead_code)]
    pub fn with_colors(colors: Vec<Srgba>) -> Self {
        Self { colors, selected: 0 }
    }

    // ------------------------------------------------------------------------
    // Accessors
    // ------------------------------------------------------------------------

    pub fn len(&self) -> usize {
        self.colors.len()
    }

    /// Companion to [`Self::len`]; the editor cannot currently empty a palette.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }

    /// The colour painting uses.
    pub fn selected_color(&self) -> Srgba {
        self.colors
            .get(self.selected)
            .copied()
            .unwrap_or(crate::config::transparent())
    }

    pub fn selected_hex(&self) -> String {
        color_to_hex(self.selected_color())
    }

    #[allow(dead_code)]
    pub fn get_color(&self, index: usize) -> Option<Srgba> {
        self.colors.get(index).copied()
    }

    // ------------------------------------------------------------------------
    // Selection
    // ------------------------------------------------------------------------

    /// Selects a colour. Returns false for an out-of-range index, leaving the
    /// previous selection alone.
    pub fn select(&mut self, index: usize) -> bool {
        if index >= self.colors.len() {
            return false;
        }
        self.selected = index;
        true
    }

    // ------------------------------------------------------------------------
    // Colour management
    //
    // These are pure primitives: `remove_color` will happily empty the palette,
    // because `history` needs it to be the exact inverse of `add_color`. The
    // "at least one colour" rule is policy and lives with the editing requests
    // in `document.rs`; `insert_color` and `reorder_color` are still only used
    // by `history`, which is not wired up.
    // ------------------------------------------------------------------------

    pub fn add_color(&mut self, color: Srgba) -> usize {
        self.colors.push(color);
        self.colors.len() - 1
    }

    pub fn insert_color(&mut self, index: usize, color: Srgba) {
        if index > self.colors.len() {
            return;
        }
        self.colors.insert(index, color);
        if self.selected >= index {
            self.selected += 1;
        }
        self.clamp_selection();
    }

    pub fn remove_color(&mut self, index: usize) -> Option<Srgba> {
        if index >= self.colors.len() {
            return None;
        }
        let removed = self.colors.remove(index);
        if self.selected > index {
            self.selected -= 1;
        }
        self.clamp_selection();
        Some(removed)
    }

    pub fn replace_color(&mut self, index: usize, new_color: Srgba) -> Option<Srgba> {
        let slot = self.colors.get_mut(index)?;
        let old = *slot;
        *slot = new_color;
        Some(old)
    }

    pub fn reorder_color(&mut self, from: usize, to: usize) -> bool {
        if from >= self.colors.len() || to >= self.colors.len() || from == to {
            return false;
        }
        let color = self.colors.remove(from);
        self.colors.insert(to, color);

        if self.selected == from {
            self.selected = to;
        } else if self.selected > from && self.selected <= to {
            self.selected -= 1;
        } else if self.selected >= to && self.selected < from {
            self.selected += 1;
        }
        self.clamp_selection();
        true
    }

    fn clamp_selection(&mut self) {
        if self.colors.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.colors.len() {
            self.selected = self.colors.len() - 1;
        }
    }

    // ------------------------------------------------------------------------
    // Serialisation
    // ------------------------------------------------------------------------

    pub fn to_data(&self) -> PaletteData {
        PaletteData::from_palette(self)
    }

    #[allow(dead_code)]
    pub fn from_data(data: &PaletteData) -> Self {
        data.to_palette()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Srgba = Srgba::new(1.0, 0.0, 0.0, 1.0);
    const BLUE: Srgba = Srgba::new(0.0, 0.0, 1.0, 1.0);

    #[test]
    fn new_palette_starts_on_an_opaque_colour() {
        let palette = Palette::new();
        assert_eq!(palette.len(), DEFAULT_PALETTE.len());
        assert_eq!(palette.selected, DEFAULT_SELECTED_COLOR);
        assert_eq!(palette.selected_color().alpha, 1.0);
    }

    #[test]
    fn transparent_is_available_as_an_eraser() {
        let palette = Palette::new();
        assert_eq!(palette.get_color(0).unwrap().alpha, 0.0);
    }

    #[test]
    fn select_rejects_out_of_range_and_keeps_the_old_choice() {
        let mut palette = Palette::new();
        assert!(palette.select(3));
        assert_eq!(palette.selected, 3);
        assert!(!palette.select(999));
        assert_eq!(palette.selected, 3, "a bad index must not move the selection");
    }

    #[test]
    fn selected_hex_reports_the_draw_colour() {
        let mut palette = Palette::with_colors(vec![RED, BLUE]);
        assert_eq!(palette.selected_hex(), "#ff0000ff");
        palette.select(1);
        assert_eq!(palette.selected_hex(), "#0000ffff");
    }

    #[test]
    fn serialisation_round_trips_colours_and_selection() {
        let mut palette = Palette::new();
        palette.select(5);

        let restored = Palette::from_data(&palette.to_data());
        assert_eq!(restored.colors, palette.colors);
        assert_eq!(restored.selected, 5);
    }

    #[test]
    fn a_sidecar_without_a_selection_lands_on_a_valid_index() {
        let data = PaletteData {
            colors: vec![ColorData::from_srgba(RED), ColorData::from_srgba(BLUE)],
            selected_index: None,
        };
        assert_eq!(data.to_palette().selected, 0);
    }

    #[test]
    fn a_sidecar_with_an_overshooting_selection_is_clamped() {
        let data = PaletteData {
            colors: vec![ColorData::from_srgba(RED)],
            selected_index: Some(17),
        };
        assert_eq!(data.to_palette().selected, 0);
    }

    #[test]
    fn removing_a_colour_keeps_the_selection_on_the_same_colour() {
        let mut palette = Palette::with_colors(vec![RED, BLUE, RED]);
        palette.select(2);
        palette.remove_color(0);
        assert_eq!(palette.selected, 1, "selection should follow its colour");
    }

    #[test]
    fn removing_the_last_colour_clamps_the_selection() {
        let mut palette = Palette::with_colors(vec![RED, BLUE]);
        palette.select(1);
        palette.remove_color(1);
        assert_eq!(palette.selected, 0);
        assert!(palette.selected < palette.len());
    }

    #[test]
    fn reorder_moves_the_selection_with_its_colour() {
        let mut palette = Palette::with_colors(vec![RED, BLUE, RED]);
        palette.select(0);
        assert!(palette.reorder_color(0, 2));
        assert_eq!(palette.selected, 2);
        assert!(!palette.reorder_color(0, 0));
    }

    #[test]
    fn a_fresh_mode_is_the_hand_edited_one() {
        let mode = PaletteMode::default();
        assert!(!mode.is_generated());
        assert_eq!(mode.label(), "list");
    }

    #[test]
    fn stashing_and_restoring_returns_the_palette_untouched() {
        let mut mode = PaletteMode::default();
        let mut parked = Palette::with_colors(vec![RED, BLUE]);
        parked.select(1);

        mode.stash(parked.clone());
        assert!(mode.is_generated());
        assert_eq!(mode.label(), "oklab");

        let restored = mode.restore().expect("the parked palette must come back");
        assert_eq!(restored.colors, parked.colors);
        assert_eq!(restored.selected, 1, "the selection is parked with it");
        assert!(!mode.is_generated());
    }

    /// The stash is what says a mode switch has something to undo, so restoring
    /// twice must not hand the same palette back a second time.
    #[test]
    fn restoring_without_a_stash_yields_nothing() {
        let mut mode = PaletteMode::default();
        assert!(mode.restore().is_none());

        mode.stash(Palette::with_colors(vec![RED]));
        assert!(mode.restore().is_some());
        assert!(mode.restore().is_none());
    }

    #[test]
    fn color_data_clamps_out_of_range_components() {
        let data = ColorData::new(2.0, -1.0, 0.5, 0.5);
        assert_eq!(data.red, 1.0);
        assert_eq!(data.green, 0.0);
        assert_eq!(data.to_srgba(), Srgba::new(1.0, 0.0, 0.5, 0.5));
    }
}
