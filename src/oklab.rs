//! The generated palette: colour ramps described in Oklab rather than listed.
//!
//! A [`OklabRamp`] fixes two of Oklab's three values and sweeps the third over a
//! range in a fixed number of steps. Two fixed values plus a swept one is enough
//! to say the things a sprite palette actually wants — "this hue from shadow to
//! highlight", "these hues at one lightness" — in six numbers instead of a list.
//!
//! Several ramps are live at once ([`OklabRamps`]), and their colours are
//! concatenated into the ordinary [`Palette`](crate::palette::Palette) so that
//! everything downstream — painting, the swatches, the status strip, saving —
//! keeps working unchanged. The ramps are a *generator* for the palette, not a
//! second palette.
//!
//! Oklab is wider than sRGB, so a swept value routinely lands outside what the
//! screen can show; [`to_srgba_in_gamut`] is where that is dealt with.

use bevy::color::Oklaba;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::config::{
    DEFAULT_OKLAB_RAMPS, OKLAB_AB_LIMIT, OKLAB_CHROMA_SWEEP, OKLAB_GAMUT_SEARCH_STEPS,
    OKLAB_LIGHTNESS_SWEEP, OKLAB_MAX_STEPS, OKLAB_MIN_STEPS,
};

// ============================================================================
// DATA STRUCTURES
// ============================================================================

/// Which of Oklab's three values a ramp sweeps. The other two are held fixed.
///
/// `Serialize`/`Deserialize` directly rather than through a mirror the way
/// [`ColorData`](crate::palette::ColorData) does: this is our own type, so there
/// is no foreign one to work around. The names are lowercased for the sidecar,
/// which is hand-editable and reads better without Rust's casing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OklabAxis {
    Lightness,
    A,
    B,
}

impl OklabAxis {
    /// In the order the axis buttons are laid out, which is also the order the
    /// values are conventionally written.
    pub const ALL: [OklabAxis; 3] = [Self::Lightness, Self::A, Self::B];

    /// The single character the sidebar and the scripts call this axis.
    pub fn label(self) -> &'static str {
        match self {
            Self::Lightness => "L",
            Self::A => "a",
            Self::B => "b",
        }
    }

    /// Parses [`Self::label`], for the `axis` and `ramp` script verbs.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "L" | "l" => Some(Self::Lightness),
            "a" | "A" => Some(Self::A),
            "b" | "B" => Some(Self::B),
            _ => None,
        }
    }

    /// How far this axis may be pushed, in either direction. Lightness is a
    /// 0..1 quantity; the two chroma axes are signed and symmetric.
    pub fn limits(self) -> (f32, f32) {
        match self {
            Self::Lightness => (0.0, 1.0),
            Self::A | Self::B => (-OKLAB_AB_LIMIT, OKLAB_AB_LIMIT),
        }
    }

    /// The sweep a ramp adopts when it starts varying this axis. The old sweep
    /// was in the previous axis' units, where it means nothing.
    pub fn default_sweep(self) -> (f32, f32) {
        match self {
            Self::Lightness => OKLAB_LIGHTNESS_SWEEP,
            Self::A | Self::B => OKLAB_CHROMA_SWEEP,
        }
    }
}

/// One generated ramp.
///
/// All three of `lightness`, `a` and `b` are always stored, including the one
/// `axis` is sweeping — its value is ignored while it sweeps, but keeping it
/// means switching the axis back does not lose what was set there.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OklabRamp {
    pub axis: OklabAxis,
    pub lightness: f32,
    pub a: f32,
    pub b: f32,
    /// The sweep, in `axis`' own units. `from > to` runs the ramp backwards,
    /// which is a legitimate thing to want and needs no special case.
    pub from: f32,
    pub to: f32,
    pub steps: usize,
}

/// The live ramps. Their colours, concatenated in order, *are* the palette
/// while the generated mode is in charge.
#[derive(Resource, Debug, Clone)]
pub struct OklabRamps {
    pub ramps: Vec<OklabRamp>,
}

/// One editable number of a ramp.
///
/// A `Component` because the sidebar tags each of its sliders with the field it
/// drives; it is defined here rather than there because the scripts name the
/// same fields, and one spelling for both is what keeps a script and a click
/// doing the same thing.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RampField {
    /// One of the two values the ramp holds fixed. Naming the axis rather than
    /// the slot means the field survives the ramp changing which axis it sweeps.
    Fixed(OklabAxis),
    /// An end of the swept range, in the swept axis' units.
    From,
    To,
    Steps,
}

// ============================================================================
// GAMUT
// ============================================================================

/// Whether every sRGB channel of `color` is displayable.
fn fits_in_srgb(color: Oklaba) -> bool {
    // The conversion is a matrix multiply and a cube root; a hair over 1.0 is
    // rounding, not an out-of-gamut colour.
    const SLACK: f32 = 1.0e-4;
    let rgb = Srgba::from(color);
    [rgb.red, rgb.green, rgb.blue]
        .iter()
        .all(|c| *c >= -SLACK && *c <= 1.0 + SLACK)
}

/// `color` with its chroma scaled toward the neutral axis, keeping lightness.
fn with_chroma_scale(color: Oklaba, scale: f32) -> Oklaba {
    Oklaba::new(color.lightness, color.a * scale, color.b * scale, color.alpha)
}

/// Oklab covers more than sRGB can show, so a swept value can land outside the
/// display gamut.
///
/// Clipping each channel is the obvious answer and the wrong one: it slides the
/// hue and collapses the far end of a chroma sweep onto a single corner colour,
/// so the last few swatches of a ramp come out identical. Instead the chroma is
/// pulled toward the neutral axis until the colour fits, which keeps the hue and
/// the lightness the ramp asked for and leaves consecutive steps distinct.
pub fn to_srgba_in_gamut(color: Oklaba) -> Srgba {
    // Chroma reduction cannot rescue a lightness outside 0..1 — nothing is
    // darker than black — so pin that first.
    let color = Oklaba {
        lightness: color.lightness.clamp(0.0, 1.0),
        ..color
    };

    let scale = if fits_in_srgb(color) {
        1.0
    } else {
        // `lo` always fits (a neutral grey always does) and `hi` never does, so
        // the interval closes on the gamut boundary.
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..OKLAB_GAMUT_SEARCH_STEPS {
            let mid = 0.5 * (lo + hi);
            if fits_in_srgb(with_chroma_scale(color, mid)) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    };

    let rgb = Srgba::from(with_chroma_scale(color, scale));
    // The search stops just inside the boundary, but the conversion's own
    // rounding can still put a channel a fraction out.
    Srgba::new(
        rgb.red.clamp(0.0, 1.0),
        rgb.green.clamp(0.0, 1.0),
        rgb.blue.clamp(0.0, 1.0),
        rgb.alpha.clamp(0.0, 1.0),
    )
}

// ============================================================================
// CONSTRUCTORS AND ACCESSORS
// ============================================================================

impl OklabRamp {
    /// This ramp's value on `axis`, whether or not that is the swept one.
    pub fn value_of(&self, axis: OklabAxis) -> f32 {
        match axis {
            OklabAxis::Lightness => self.lightness,
            OklabAxis::A => self.a,
            OklabAxis::B => self.b,
        }
    }

    pub fn with_value(self, axis: OklabAxis, value: f32) -> Self {
        let (min, max) = axis.limits();
        let value = value.clamp(min, max);
        match axis {
            OklabAxis::Lightness => Self {
                lightness: value,
                ..self
            },
            OklabAxis::A => Self { a: value, ..self },
            OklabAxis::B => Self { b: value, ..self },
        }
    }

    /// Sweeps a different axis. The sweep is reset rather than carried over:
    /// `0.15..0.95` means "shadow to highlight" on lightness and "far past the
    /// gamut in both directions" on chroma.
    pub fn with_axis(self, axis: OklabAxis) -> Self {
        if axis == self.axis {
            return self;
        }
        let (from, to) = axis.default_sweep();
        Self {
            axis,
            from,
            to,
            ..self
        }
    }

    /// The two values this ramp holds fixed, in L, a, b order.
    pub fn fixed_axes(&self) -> Vec<OklabAxis> {
        OklabAxis::ALL
            .into_iter()
            .filter(|axis| *axis != self.axis)
            .collect()
    }

    /// How many colours this ramp actually produces, which is `steps` brought
    /// inside the bounds the editor enforces.
    pub fn step_count(&self) -> usize {
        self.steps.clamp(OKLAB_MIN_STEPS, OKLAB_MAX_STEPS)
    }

    /// The colour at one point of the sweep.
    fn color_at(&self, value: f32) -> Srgba {
        let (lightness, a, b) = match self.axis {
            OklabAxis::Lightness => (value, self.a, self.b),
            OklabAxis::A => (self.lightness, value, self.b),
            OklabAxis::B => (self.lightness, self.a, value),
        };
        to_srgba_in_gamut(Oklaba::lab(lightness, a, b))
    }

    /// The ramp, as the palette sees it: `step_count()` colours with both ends
    /// of the sweep included.
    pub fn colors(&self) -> Vec<Srgba> {
        let steps = self.step_count();
        (0..steps)
            .map(|step| {
                // `steps >= 2`, so the divisor is never zero and the last step
                // lands exactly on `to`.
                let t = step as f32 / (steps - 1) as f32;
                self.color_at(self.from + (self.to - self.from) * t)
            })
            .collect()
    }

    /// The ramp with every value inside the bounds the editor enforces.
    ///
    /// Applied to whatever a sidecar hands over: the file is plain JSON and can
    /// be edited by hand, and a value outside its axis' limits would give the
    /// sliders a position they cannot represent. `colors()` would still cope —
    /// it clamps lightness and fits the chroma — but the editor would then
    /// disagree with the swatches, which is worse than clamping on the way in.
    pub fn sanitised(self) -> Self {
        let (min, max) = self.axis.limits();
        Self {
            from: self.from.clamp(min, max),
            to: self.to.clamp(min, max),
            steps: self.step_count(),
            ..self
        }
        // `with_value` clamps to each axis' own limits, so the three fixed
        // values need no separate arithmetic here.
        .with_value(OklabAxis::Lightness, self.lightness)
        .with_value(OklabAxis::A, self.a)
        .with_value(OklabAxis::B, self.b)
    }

    /// What the status strip says about this ramp, so a screenshot names the
    /// sweep it is showing rather than leaving it to be guessed at.
    pub fn describe(&self) -> String {
        format!(
            "{} {:.2}..{:.2} x{}",
            self.axis.label(),
            self.from,
            self.to,
            self.step_count()
        )
    }
}

impl RampField {
    /// The fields of `ramp`, in the order the sidebar stacks them: what is held
    /// fixed, then where the third value goes, then how many stops it makes.
    ///
    /// Which two are fixed is a property of the ramp, so this is also what says
    /// the sidebar has to be rebuilt when the axis changes.
    pub fn of(ramp: &OklabRamp) -> Vec<RampField> {
        ramp.fixed_axes()
            .into_iter()
            .map(Self::Fixed)
            .chain([Self::From, Self::To, Self::Steps])
            .collect()
    }

    /// One name per field, used both as the sidebar's caption and as the word a
    /// script says — so a click and a scripted step are visibly the same edit.
    pub fn label(self) -> &'static str {
        match self {
            Self::Fixed(axis) => axis.label(),
            Self::From => "from",
            Self::To => "to",
            Self::Steps => "steps",
        }
    }

    /// Parses [`Self::label`]. An axis name always yields [`Self::Fixed`], which
    /// is the right answer even for the axis currently being swept: the value is
    /// stored either way and simply has no effect until the sweep moves on.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "from" => Some(Self::From),
            "to" => Some(Self::To),
            "steps" => Some(Self::Steps),
            other => OklabAxis::parse(other).map(Self::Fixed),
        }
    }

    /// What this field may be set to. The range ends move over the swept axis'
    /// own limits, and neither is pinned to the other — `from` above `to` is
    /// simply the ramp run backwards.
    pub fn limits(self, ramp: &OklabRamp) -> (f32, f32) {
        match self {
            Self::Fixed(axis) => axis.limits(),
            Self::From | Self::To => ramp.axis.limits(),
            Self::Steps => (OKLAB_MIN_STEPS as f32, OKLAB_MAX_STEPS as f32),
        }
    }

    pub fn read(self, ramp: &OklabRamp) -> f32 {
        match self {
            Self::Fixed(axis) => ramp.value_of(axis),
            Self::From => ramp.from,
            Self::To => ramp.to,
            Self::Steps => ramp.step_count() as f32,
        }
    }

    /// `ramp` with this one field moved — the whole ramp an edit asks for, so
    /// the applier stays a plain assignment.
    pub fn apply(self, ramp: OklabRamp, value: f32) -> OklabRamp {
        let (min, max) = self.limits(&ramp);
        let value = value.clamp(min, max);
        match self {
            Self::Fixed(axis) => ramp.with_value(axis, value),
            Self::From => OklabRamp { from: value, ..ramp },
            Self::To => OklabRamp { to: value, ..ramp },
            Self::Steps => OklabRamp {
                steps: value.round() as usize,
                ..ramp
            },
        }
    }
}

impl Default for OklabRamps {
    fn default() -> Self {
        Self {
            ramps: DEFAULT_OKLAB_RAMPS.to_vec(),
        }
    }
}

impl OklabRamps {
    /// Every ramp's colours, concatenated. This is what the palette becomes.
    pub fn colors(&self) -> Vec<Srgba> {
        self.ramps.iter().flat_map(|ramp| ramp.colors()).collect()
    }

    /// One entry per ramp, saying how many swatches it contributes. The sidebar
    /// uses this as the layout signature it rebuilds the swatch rows on.
    pub fn step_counts(&self) -> Vec<usize> {
        self.ramps.iter().map(OklabRamp::step_count).collect()
    }

    pub fn len(&self) -> usize {
        self.ramps.len()
    }

    /// Which ramp a flattened palette index came from.
    ///
    /// This is how the editor knows what to edit: clicking a swatch selects a
    /// colour, and the ramp that produced it is the one the sliders act on. Out
    /// of range falls back to the last ramp, since an index past the end can
    /// only be a selection the palette has not clamped yet.
    pub fn ramp_of(&self, index: usize) -> usize {
        let mut remaining = index;
        for (ramp_index, ramp) in self.ramps.iter().enumerate() {
            let steps = ramp.step_count();
            if remaining < steps {
                return ramp_index;
            }
            remaining -= steps;
        }
        self.ramps.len().saturating_sub(1)
    }

    /// The flattened index of `ramp`'s first colour.
    pub fn offset_of(&self, ramp: usize) -> usize {
        self.ramps
            .iter()
            .take(ramp)
            .map(OklabRamp::step_count)
            .sum()
    }

    pub fn get(&self, index: usize) -> Option<&OklabRamp> {
        self.ramps.get(index)
    }

    /// The ramps as loaded from a sidecar, brought inside the editor's rules:
    /// each one clamped, and an empty list replaced by the defaults.
    ///
    /// "At least one ramp" is a load-time rule as much as an editing one — the
    /// `-` button refuses to remove the last, and a file with none would leave
    /// the generated palette empty and nothing to draw with.
    pub fn sanitised(self) -> Self {
        if self.ramps.is_empty() {
            warn!("sidecar has no ramps — falling back to the built-in ones");
            return Self::default();
        }
        Self {
            ramps: self.ramps.into_iter().map(OklabRamp::sanitised).collect(),
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::color_to_hex;

    fn lightness_ramp(steps: usize) -> OklabRamp {
        OklabRamp {
            axis: OklabAxis::Lightness,
            lightness: 0.5,
            a: 0.0,
            b: 0.0,
            from: 0.0,
            to: 1.0,
            steps,
        }
    }

    #[test]
    fn a_ramp_produces_one_colour_per_step() {
        assert_eq!(lightness_ramp(5).colors().len(), 5);
        assert_eq!(lightness_ramp(2).colors().len(), 2);
    }

    #[test]
    fn a_ramp_includes_both_ends_of_its_sweep() {
        let colors = lightness_ramp(5).colors();
        // A neutral sweep from L=0 to L=1 is black to white.
        assert_eq!(colors.first().unwrap().red, 0.0);
        assert!(colors.last().unwrap().red > 0.99);
    }

    #[test]
    fn a_sweep_is_monotonic_in_the_axis_it_sweeps() {
        let colors = lightness_ramp(8).colors();
        for pair in colors.windows(2) {
            assert!(
                pair[1].red > pair[0].red,
                "a lightness sweep must keep getting lighter, got {pair:?}"
            );
        }
    }

    /// `from` above `to` is a legitimate thing to ask for and needs no special
    /// case. Compared as 8-bit colours because that is what is displayed: the
    /// two directions interpolate in the opposite order, so the f32s differ in
    /// the last bit or two.
    #[test]
    fn a_backwards_sweep_is_the_forwards_one_reversed() {
        let forwards = lightness_ramp(6).colors();
        let mut backwards = OklabRamp {
            from: 1.0,
            to: 0.0,
            ..lightness_ramp(6)
        }
        .colors();
        backwards.reverse();

        let hex = |colors: &[Srgba]| colors.iter().copied().map(color_to_hex).collect::<Vec<_>>();
        assert_eq!(hex(&forwards), hex(&backwards));
    }

    #[test]
    fn a_step_count_outside_the_bounds_is_clamped_rather_than_obeyed() {
        assert_eq!(lightness_ramp(0).colors().len(), OKLAB_MIN_STEPS);
        assert_eq!(lightness_ramp(1).colors().len(), OKLAB_MIN_STEPS);
        assert_eq!(lightness_ramp(9999).colors().len(), OKLAB_MAX_STEPS);
    }

    /// The whole point of reducing chroma rather than clipping channels: a
    /// chroma sweep that runs off the end of sRGB must still give distinct
    /// swatches, not the same corner colour repeated.
    #[test]
    fn an_out_of_gamut_sweep_keeps_its_steps_distinct() {
        let ramp = OklabRamp {
            axis: OklabAxis::A,
            lightness: 0.65,
            a: 0.0,
            b: 0.0,
            from: -OKLAB_AB_LIMIT,
            to: OKLAB_AB_LIMIT,
            steps: 8,
        };
        let colors = ramp.colors();
        for pair in colors.windows(2) {
            assert_ne!(pair[0], pair[1], "a clipped ramp would repeat a colour");
        }
    }

    #[test]
    fn every_generated_colour_is_displayable() {
        for ramp in DEFAULT_OKLAB_RAMPS {
            for color in ramp.colors() {
                for channel in [color.red, color.green, color.blue, color.alpha] {
                    assert!(
                        (0.0..=1.0).contains(&channel),
                        "{ramp:?} produced {color:?}, which is not an sRGB colour"
                    );
                }
            }
        }
    }

    #[test]
    fn generated_colours_are_opaque() {
        for color in OklabRamps::default().colors() {
            assert_eq!(color.alpha, 1.0, "a transparent ramp would paint nothing");
        }
    }

    /// A colour that already fits must come back untouched — the search is only
    /// for the ones that do not.
    #[test]
    fn an_in_gamut_colour_survives_the_gamut_search() {
        let inside = Oklaba::lab(0.6, 0.02, 0.03);
        assert_eq!(to_srgba_in_gamut(inside), Srgba::from(inside));
    }

    #[test]
    fn the_ramps_flatten_into_one_palette() {
        let ramps = OklabRamps::default();
        let expected: usize = ramps.step_counts().iter().sum();
        assert_eq!(ramps.colors().len(), expected);
    }

    #[test]
    fn a_flattened_index_maps_back_to_the_ramp_that_made_it() {
        let ramps = OklabRamps {
            ramps: vec![lightness_ramp(3), lightness_ramp(4), lightness_ramp(2)],
        };
        assert_eq!(ramps.ramp_of(0), 0);
        assert_eq!(ramps.ramp_of(2), 0);
        assert_eq!(ramps.ramp_of(3), 1);
        assert_eq!(ramps.ramp_of(6), 1);
        assert_eq!(ramps.ramp_of(7), 2);
        assert_eq!(
            ramps.ramp_of(999),
            2,
            "an index past the end can only be a selection not yet clamped"
        );
    }

    #[test]
    fn a_ramps_offset_is_where_its_first_colour_landed() {
        let ramps = OklabRamps {
            ramps: vec![lightness_ramp(3), lightness_ramp(4), lightness_ramp(2)],
        };
        assert_eq!(ramps.offset_of(0), 0);
        assert_eq!(ramps.offset_of(1), 3);
        assert_eq!(ramps.offset_of(2), 7);

        // The pair has to be consistent, or clicking a swatch would edit the
        // wrong ramp.
        for ramp in 0..ramps.len() {
            assert_eq!(ramps.ramp_of(ramps.offset_of(ramp)), ramp);
        }
    }

    #[test]
    fn switching_the_axis_resets_the_sweep_into_the_new_units() {
        let ramp = lightness_ramp(5).with_axis(OklabAxis::A);
        assert_eq!(ramp.axis, OklabAxis::A);
        assert_eq!((ramp.from, ramp.to), OKLAB_CHROMA_SWEEP);

        // The fixed values are kept: switching back must not have lost them.
        assert_eq!(ramp.lightness, 0.5);
    }

    #[test]
    fn switching_to_the_axis_already_swept_changes_nothing() {
        let ramp = lightness_ramp(5);
        assert_eq!(ramp.with_axis(OklabAxis::Lightness), ramp);
    }

    #[test]
    fn a_fixed_value_is_clamped_to_its_axis_limits() {
        let ramp = lightness_ramp(5).with_value(OklabAxis::A, 99.0);
        assert_eq!(ramp.a, OKLAB_AB_LIMIT);
        assert_eq!(lightness_ramp(5).with_value(OklabAxis::Lightness, -1.0).lightness, 0.0);
    }

    #[test]
    fn a_ramp_fixes_the_two_axes_it_does_not_sweep() {
        assert_eq!(
            lightness_ramp(5).fixed_axes(),
            vec![OklabAxis::A, OklabAxis::B]
        );
        assert_eq!(
            lightness_ramp(5).with_axis(OklabAxis::B).fixed_axes(),
            vec![OklabAxis::Lightness, OklabAxis::A]
        );
    }

    #[test]
    fn an_axis_round_trips_through_its_label() {
        for axis in OklabAxis::ALL {
            assert_eq!(OklabAxis::parse(axis.label()), Some(axis));
        }
        assert_eq!(OklabAxis::parse("z"), None);
    }

    #[test]
    fn a_ramp_describes_its_own_sweep() {
        assert_eq!(lightness_ramp(6).describe(), "L 0.00..1.00 x6");
    }
}
