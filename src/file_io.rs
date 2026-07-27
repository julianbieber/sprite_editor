//! Loading and saving a document as a PNG plus a JSON sidecar.
//!
//! The PNG *is* the atlas: same dimensions, same pixel order. The sidecar
//! carries what the image cannot — tile size, tile counts, and the palette.
//!
//! "The palette" is both of the ways of specifying one. `palette` is whichever
//! was live, so `selected_index` always indexes something that exists; the
//! `generated` block carries the Oklab ramps and, when they were in charge, the
//! hand-edited palette they displaced. A document therefore reopens exactly as
//! it was left, in the mode it was left in, with the other specification still
//! one button away.

use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::atlas::AtlasDocument;
use crate::config::{FILE_FORMAT_VERSION, METADATA_SUFFIX, UNTITLED_BASE};
use crate::document::DocumentPath;
use crate::oklab::{OklabRamp, OklabRamps};
use crate::palette::{Palette, PaletteData, PaletteMode};

// ============================================================================
// REQUESTS
// ============================================================================

/// Write the document back to its path.
#[derive(Message, Debug, Clone, Copy)]
pub struct SaveRequested;

// ============================================================================
// ERRORS
// ============================================================================

#[derive(Debug)]
pub enum FileError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Image(String),
    NotFound(PathBuf),
    InvalidFormat(String),
}

impl std::fmt::Display for FileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileError::Io(e) => write!(f, "IO error: {e}"),
            FileError::Json(e) => write!(f, "JSON error: {e}"),
            FileError::Image(e) => write!(f, "image error: {e}"),
            FileError::NotFound(p) => write!(f, "file not found: {}", p.display()),
            FileError::InvalidFormat(s) => write!(f, "invalid format: {s}"),
        }
    }
}

impl std::error::Error for FileError {}

impl From<std::io::Error> for FileError {
    fn from(e: std::io::Error) -> Self {
        FileError::Io(e)
    }
}

impl From<serde_json::Error> for FileError {
    fn from(e: serde_json::Error) -> Self {
        FileError::Json(e)
    }
}

impl From<image::ImageError> for FileError {
    fn from(e: image::ImageError) -> Self {
        FileError::Image(e.to_string())
    }
}

// ============================================================================
// METADATA
// ============================================================================

/// The generated palette, as stored in the sidecar.
///
/// The presence of `stashed_palette` is what says the ramps were in charge —
/// the same invariant [`PaletteMode`] holds in memory, rather than a second
/// boolean that could disagree with it. A file in `list` mode still carries its
/// ramps, so switching back after a reopen finds them where they were left.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedPaletteData {
    pub ramps: Vec<OklabRamp>,
    /// The hand-edited palette parked while the ramps generate `palette`.
    /// Present exactly when they do; its absence means `palette` *is* the
    /// hand-edited one.
    #[serde(default)]
    pub stashed_palette: Option<PaletteData>,
}

/// The JSON sidecar.
///
/// `generated` is optional and the version stays at `1.0`: the field is purely
/// additive, so a file written before ramps existed still loads (it simply has
/// no block), and a build that predates them still loads a file that has one —
/// serde drops the key it does not know. Bumping the version would have broken
/// both directions to describe a change that breaks neither.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtlasMetadata {
    #[serde(default = "default_version")]
    pub version: String,
    pub tile_size: u32,
    pub width_in_tiles: u32,
    pub height_in_tiles: u32,
    /// The palette that was being drawn with, whichever specification produced
    /// it — so `selected_index` indexes something that exists either way.
    pub palette: PaletteData,
    #[serde(default)]
    pub generated: Option<GeneratedPaletteData>,
}

fn default_version() -> String {
    FILE_FORMAT_VERSION.to_string()
}

impl AtlasMetadata {
    pub fn describe(
        document: &AtlasDocument,
        palette: &Palette,
        mode: &PaletteMode,
        ramps: &OklabRamps,
    ) -> Self {
        Self {
            version: FILE_FORMAT_VERSION.to_string(),
            tile_size: document.tile_size,
            width_in_tiles: document.width_in_tiles,
            height_in_tiles: document.height_in_tiles,
            palette: palette.to_data(),
            generated: Some(GeneratedPaletteData {
                ramps: ramps.ramps.clone(),
                stashed_palette: mode.stashed().map(Palette::to_data),
            }),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != FILE_FORMAT_VERSION {
            return Err(format!(
                "unsupported file version {} (expected {})",
                self.version, FILE_FORMAT_VERSION
            ));
        }
        if self.tile_size == 0 {
            return Err("tile size must be greater than 0".to_string());
        }
        if self.width_in_tiles == 0 || self.height_in_tiles == 0 {
            return Err("atlas must be at least one tile in each direction".to_string());
        }
        Ok(())
    }
}

// ============================================================================
// PATHS
// ============================================================================

pub fn png_path(base: &Path) -> PathBuf {
    let mut path = base.to_path_buf();
    path.set_extension("png");
    path
}

pub fn metadata_path(base: &Path) -> PathBuf {
    let mut path = base.to_path_buf();
    let name = base
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| UNTITLED_BASE.to_string());
    path.set_file_name(format!("{name}.{METADATA_SUFFIX}"));
    path
}

// ============================================================================
// CODEC
// ============================================================================

/// Everything a sidecar restores.
///
/// A struct rather than a tuple because two of the fields are palettes — the one
/// being drawn with and the one parked behind it — and swapping them at a call
/// site would compile.
pub struct LoadedProject {
    pub document: AtlasDocument,
    pub palette: Palette,
    pub mode: PaletteMode,
    pub ramps: OklabRamps,
}

/// Writes the PNG and the sidecar. The PNG goes first, so a failure there
/// leaves no sidecar describing an image that was never written.
pub fn save_project(
    document: &AtlasDocument,
    palette: &Palette,
    mode: &PaletteMode,
    ramps: &OklabRamps,
    base: &Path,
) -> Result<(), FileError> {
    let width = document.width_in_pixels();
    let height = document.height_in_pixels();

    let buffer = image::RgbaImage::from_raw(width, height, document.to_rgba8())
        .ok_or_else(|| FileError::Image("pixel buffer does not match dimensions".to_string()))?;
    buffer.save(png_path(base))?;

    let metadata = AtlasMetadata::describe(document, palette, mode, ramps);
    fs::write(metadata_path(base), serde_json::to_string_pretty(&metadata)?)?;

    Ok(())
}

/// Rebuilds both palette specifications and the mode from a sidecar.
///
/// The generated palette is **regenerated from the ramps** rather than read back
/// out of `palette.colors`, even though a save writes the two agreeing. The
/// ramps are the specification: a sidecar edited by hand must show what its
/// ramps now say, and `document.rs` relies on the palette matching the ramps
/// that produced it from the first frame rather than from the first ramp edit.
/// Only the selection is carried across, clamped like any other.
fn restore_palettes(metadata: Option<&AtlasMetadata>) -> (Palette, PaletteMode, OklabRamps) {
    let Some(metadata) = metadata else {
        return (Palette::new(), PaletteMode::default(), OklabRamps::default());
    };

    let live = metadata.palette.to_palette();
    let Some(generated) = metadata.generated.as_ref() else {
        // Written before ramps existed: the palette is the hand-edited one and
        // the ramps start from the built-in set.
        return (live, PaletteMode::default(), OklabRamps::default());
    };

    let ramps = OklabRamps {
        ramps: generated.ramps.clone(),
    }
    .sanitised();

    let Some(stashed) = generated.stashed_palette.as_ref() else {
        return (live, PaletteMode::default(), ramps);
    };

    let mut mode = PaletteMode::default();
    mode.stash(stashed.to_palette());

    let colors = ramps.colors();
    let palette = Palette {
        selected: live.selected.min(colors.len().saturating_sub(1)),
        colors,
    };
    (palette, mode, ramps)
}

/// Reads a document. The sidecar is optional — a bare PNG loads with the
/// fallback tile size and the built-in palette.
pub fn load_project(
    base: &Path,
    fallback_tile_size: u32,
) -> Result<LoadedProject, FileError> {
    let png = png_path(base);
    if !png.exists() {
        return Err(FileError::NotFound(png));
    }

    let metadata = match fs::read_to_string(metadata_path(base)) {
        Ok(json) => {
            let metadata: AtlasMetadata = serde_json::from_str(&json)?;
            metadata.validate().map_err(FileError::InvalidFormat)?;
            Some(metadata)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(FileError::Io(e)),
    };

    let tile_size = metadata
        .as_ref()
        .map(|m| m.tile_size)
        .unwrap_or(fallback_tile_size)
        .max(1);

    let decoded = image::ImageReader::open(&png)?.decode()?.to_rgba8();
    let (width, height) = (decoded.width(), decoded.height());

    // Refuse a ragged image rather than silently padding it — a partial tile
    // would make every tile boundary after it a lie.
    if width == 0 || height == 0 {
        return Err(FileError::InvalidFormat("image is empty".to_string()));
    }
    if width % tile_size != 0 || height % tile_size != 0 {
        return Err(FileError::InvalidFormat(format!(
            "image is {width}x{height}, not a whole multiple of the {tile_size}px tile size"
        )));
    }

    let pixels = AtlasDocument::pixels_from_rgba8(decoded.as_raw());
    let document = AtlasDocument::from_pixels(
        tile_size,
        width / tile_size,
        height / tile_size,
        pixels,
    )
    .map_err(FileError::InvalidFormat)?;

    let (palette, mode, ramps) = restore_palettes(metadata.as_ref());

    Ok(LoadedProject {
        document,
        palette,
        mode,
        ramps,
    })
}

// ============================================================================
// PLUGIN
// ============================================================================

pub struct AtlasIoPlugin;

impl Plugin for AtlasIoPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SaveRequested>()
            .add_systems(Startup, load_document_at_startup.in_set(DocumentLoad))
            .add_systems(Update, save_document_on_request);
    }
}

/// Startup loading, so the chrome can be built after it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocumentLoad;

// ============================================================================
// SYSTEMS
// ============================================================================

fn load_document_at_startup(
    mut document: ResMut<AtlasDocument>,
    mut palette: ResMut<Palette>,
    mut mode: ResMut<PaletteMode>,
    mut ramps: ResMut<OklabRamps>,
    mut path: ResMut<DocumentPath>,
    options: Res<crate::cli::LaunchOptions>,
) {
    let Some(base) = path.base.clone() else {
        info!("no path given — starting a new one-tile atlas");
        return;
    };

    match load_project(&base, options.tile_size) {
        Ok(loaded) => {
            info!(
                "loaded {} ({}x{} tiles of {}px, {} palette)",
                png_path(&base).display(),
                loaded.document.width_in_tiles,
                loaded.document.height_in_tiles,
                loaded.document.tile_size,
                loaded.mode.label(),
            );
            // All of it or none of it: the mode says which of the two palettes
            // is live, so installing one without the others would misread it.
            *document = loaded.document;
            *palette = loaded.palette;
            *mode = loaded.mode;
            *ramps = loaded.ramps;
            path.existed_on_disk = true;
        }
        Err(FileError::NotFound(missing)) => {
            info!(
                "{} does not exist yet — starting a new atlas to save there",
                missing.display()
            );
        }
        Err(e) => {
            error!("could not load {}: {e}", base.display());
        }
    }
}

fn save_document_on_request(
    mut requests: MessageReader<SaveRequested>,
    mut document: ResMut<AtlasDocument>,
    palette: Res<Palette>,
    mode: Res<PaletteMode>,
    ramps: Res<OklabRamps>,
    mut path: ResMut<DocumentPath>,
) {
    if requests.read().count() == 0 {
        return;
    }

    let base = path
        .base
        .clone()
        .unwrap_or_else(|| PathBuf::from(UNTITLED_BASE));

    match save_project(&document, &palette, &mode, &ramps, &base) {
        Ok(()) => {
            info!("saved {}", png_path(&base).display());
            document.unsaved = false;
            path.base = Some(base);
            path.existed_on_disk = true;
        }
        Err(e) => {
            // Leave `unsaved` set so the status strip keeps warning.
            error!("could not save {}: {e}", base.display());
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{OKLAB_AB_LIMIT, OKLAB_MAX_STEPS};
    use crate::oklab::OklabAxis;
    use tempfile::tempdir;

    const RED: Srgba = Srgba::new(1.0, 0.0, 0.0, 1.0);
    const BLUE: Srgba = Srgba::new(0.0, 0.0, 1.0, 1.0);

    #[test]
    fn paths_are_derived_from_the_base() {
        let base = Path::new("/tmp/project/demo");
        assert_eq!(png_path(base), Path::new("/tmp/project/demo.png"));
        assert_eq!(
            metadata_path(base),
            Path::new("/tmp/project/demo.atlas.json")
        );
    }

    /// A save in the hand-edited mode, which is what most of these tests are
    /// about — the generated half has its own section below.
    fn save_plain(document: &AtlasDocument, palette: &Palette, base: &Path) {
        save_project(
            document,
            palette,
            &PaletteMode::default(),
            &OklabRamps::default(),
            base,
        )
        .unwrap();
    }

    #[test]
    fn save_then_load_round_trips_pixels_and_palette() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("demo");

        let mut document = AtlasDocument::with_tile_size(4);
        document.add_tile_to_right();
        document.set_pixel_at(0, 0, RED);
        document.set_pixel_at(7, 3, BLUE);
        let mut palette = Palette::new();
        palette.select(4);

        save_plain(&document, &palette, &base);
        let loaded = load_project(&base, 8).unwrap();

        assert_eq!(loaded.document.tile_size, 4);
        assert_eq!(loaded.document.width_in_tiles, 2);
        assert_eq!(loaded.document.pixels, document.pixels);
        assert_eq!(loaded.palette.colors, palette.colors);
        assert_eq!(loaded.palette.selected, 4);
        assert!(!loaded.document.unsaved);
        assert!(!loaded.mode.is_generated());
    }

    /// The bug the row-major switch fixes: a multi-tile atlas used to come back
    /// scrambled.
    #[test]
    fn a_wide_atlas_round_trips_pixel_for_pixel() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("wide");

        let mut document = AtlasDocument::with_tile_size(2);
        for _ in 0..4 {
            document.add_tile_to_right();
        }
        // A distinct colour per pixel, so any reordering shows up.
        let width = document.width_in_pixels();
        for y in 0..document.height_in_pixels() {
            for x in 0..width {
                let v = (y * width + x) as f32 / (width * document.height_in_pixels()) as f32;
                document.set_pixel_at(x, y, Srgba::new(v, 1.0 - v, 0.5, 1.0));
            }
        }

        save_plain(&document, &Palette::new(), &base);
        let loaded = load_project(&base, 2).unwrap().document;

        assert_eq!(loaded.width_in_tiles, 5);
        for y in 0..document.height_in_pixels() {
            for x in 0..width {
                let (want, got) = (document.get_pixel_at(x, y), loaded.get_pixel_at(x, y));
                // 8-bit round trip, so compare at that resolution.
                assert!(
                    (want.red - got.red).abs() < 0.01 && (want.green - got.green).abs() < 0.01,
                    "pixel ({x},{y}) changed: {want:?} -> {got:?}"
                );
            }
        }
    }

    #[test]
    fn transparency_survives_the_round_trip() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("alpha");

        let mut document = AtlasDocument::with_tile_size(2);
        document.set_pixel_at(0, 0, RED);
        save_plain(&document, &Palette::new(), &base);

        let loaded = load_project(&base, 2).unwrap().document;
        assert_eq!(loaded.get_pixel_at(1, 1).alpha, 0.0, "alpha was flattened");
        assert_eq!(loaded.get_pixel_at(0, 0).alpha, 1.0);
    }

    #[test]
    fn a_missing_png_is_reported_as_not_found() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("absent");
        assert!(matches!(
            load_project(&base, 8),
            Err(FileError::NotFound(_))
        ));
    }

    #[test]
    fn a_bare_png_loads_with_the_fallback_tile_size() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("bare");

        let document = AtlasDocument::with_dimensions(4, 2, 1);
        save_plain(&document, &Palette::new(), &base);
        fs::remove_file(metadata_path(&base)).unwrap();

        let loaded = load_project(&base, 4).unwrap();
        assert_eq!(loaded.document.tile_size, 4);
        assert_eq!(loaded.document.width_in_tiles, 2);
        assert_eq!(loaded.palette.len(), Palette::new().len());
        assert_eq!(
            loaded.ramps.ramps,
            OklabRamps::default().ramps,
            "no sidecar means the built-in ramps, not none at all"
        );
    }

    #[test]
    fn an_image_that_is_not_a_whole_number_of_tiles_is_rejected() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("ragged");

        // 6x6 pixels cannot be tiled by 4.
        let document = AtlasDocument::with_dimensions(6, 1, 1);
        save_plain(&document, &Palette::new(), &base);

        let mut metadata: AtlasMetadata =
            serde_json::from_str(&fs::read_to_string(metadata_path(&base)).unwrap()).unwrap();
        metadata.tile_size = 4;
        fs::write(
            metadata_path(&base),
            serde_json::to_string_pretty(&metadata).unwrap(),
        )
        .unwrap();

        assert!(matches!(
            load_project(&base, 4),
            Err(FileError::InvalidFormat(_))
        ));
    }

    #[test]
    fn a_future_format_version_is_rejected() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("future");

        let document = AtlasDocument::with_tile_size(4);
        save_plain(&document, &Palette::new(), &base);

        let mut metadata: AtlasMetadata =
            serde_json::from_str(&fs::read_to_string(metadata_path(&base)).unwrap()).unwrap();
        metadata.version = "9.9".to_string();
        fs::write(
            metadata_path(&base),
            serde_json::to_string_pretty(&metadata).unwrap(),
        )
        .unwrap();

        assert!(matches!(
            load_project(&base, 4),
            Err(FileError::InvalidFormat(_))
        ));
    }

    // ------------------------------------------------------------------------
    // The generated palette in the sidecar
    // ------------------------------------------------------------------------

    /// The editor's state after a switch to the generated mode, which is what
    /// `document.rs` builds and what a save has to be able to reproduce.
    fn generated_state(ramps: &OklabRamps, hand_edited: Palette) -> (Palette, PaletteMode) {
        let mut mode = PaletteMode::default();
        mode.stash(hand_edited);
        (Palette::with_colors(ramps.colors()), mode)
    }

    /// The whole point of storing them: a document saved with ramps in charge
    /// comes back with those ramps, still in charge, still editable.
    #[test]
    fn a_generated_palette_round_trips_as_ramps() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("generated");

        let mut ramps = OklabRamps::default();
        ramps.ramps[1] = ramps.ramps[1].with_axis(OklabAxis::B);
        ramps.ramps[2].steps = 4;
        let (mut palette, mode) = generated_state(&ramps, Palette::new());
        palette.select(3);

        save_project(&AtlasDocument::new(), &palette, &mode, &ramps, &base).unwrap();
        let loaded = load_project(&base, 8).unwrap();

        assert_eq!(loaded.ramps.ramps, ramps.ramps);
        assert!(loaded.mode.is_generated());
        assert_eq!(loaded.palette.colors, ramps.colors());
        assert_eq!(loaded.palette.selected, 3);
    }

    /// The hand-edited palette is parked, not discarded, so it has to survive
    /// the file too — otherwise a save in the generated mode would quietly cost
    /// you the list you curated.
    #[test]
    fn the_parked_hand_edited_palette_survives_a_generated_save() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("parked");

        let mut hand_edited = Palette::with_colors(vec![RED, BLUE, RED]);
        hand_edited.select(2);
        let ramps = OklabRamps::default();
        let (palette, mode) = generated_state(&ramps, hand_edited.clone());

        save_project(&AtlasDocument::new(), &palette, &mode, &ramps, &base).unwrap();
        let mut loaded = load_project(&base, 8).unwrap();

        let restored = loaded
            .mode
            .restore()
            .expect("the parked palette must come back with the file");
        assert_eq!(restored.colors, hand_edited.colors);
        assert_eq!(restored.selected, 2);
    }

    /// The ramps are stored whichever mode is live, so switching to `oklab`
    /// after a reopen finds them where they were left rather than reset.
    #[test]
    fn the_ramps_are_stored_even_when_the_list_is_in_charge() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("list-mode");

        let mut ramps = OklabRamps::default();
        ramps.ramps.truncate(1);
        ramps.ramps[0].steps = 5;

        save_project(
            &AtlasDocument::new(),
            &Palette::new(),
            &PaletteMode::default(),
            &ramps,
            &base,
        )
        .unwrap();
        let loaded = load_project(&base, 8).unwrap();

        assert!(!loaded.mode.is_generated(), "the list was in charge");
        assert_eq!(loaded.palette.colors, Palette::new().colors);
        assert_eq!(loaded.ramps.ramps, ramps.ramps);
    }

    /// The block is additive, so a sidecar written before ramps existed still
    /// loads — as a hand-edited palette with the built-in ramps behind it.
    #[test]
    fn a_sidecar_without_the_generated_block_still_loads() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("legacy");

        let mut palette = Palette::new();
        palette.select(6);
        save_plain(&AtlasDocument::new(), &palette, &base);

        // Strip the block back out, leaving a 1.0 file as it used to be written.
        let mut metadata: AtlasMetadata =
            serde_json::from_str(&fs::read_to_string(metadata_path(&base)).unwrap()).unwrap();
        metadata.generated = None;
        fs::write(
            metadata_path(&base),
            serde_json::to_string_pretty(&metadata).unwrap(),
        )
        .unwrap();
        assert!(!fs::read_to_string(metadata_path(&base))
            .unwrap()
            .contains("ramps"));

        let loaded = load_project(&base, 8).unwrap();
        assert!(!loaded.mode.is_generated());
        assert_eq!(loaded.palette.selected, 6);
        assert_eq!(loaded.ramps.ramps, OklabRamps::default().ramps);
    }

    /// The ramps are the specification, so the colours follow them rather than
    /// the other way round — which is what makes hand-editing the JSON work.
    #[test]
    fn a_hand_edited_ramp_wins_over_the_stored_colours() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("hand-edited");

        let ramps = OklabRamps::default();
        let (palette, mode) = generated_state(&ramps, Palette::new());
        save_project(&AtlasDocument::new(), &palette, &mode, &ramps, &base).unwrap();

        let mut metadata: AtlasMetadata =
            serde_json::from_str(&fs::read_to_string(metadata_path(&base)).unwrap()).unwrap();
        let generated = metadata.generated.as_mut().unwrap();
        generated.ramps[0].steps = 2;
        // Deliberately left disagreeing with the ramps.
        let stale = metadata.palette.colors.len();
        fs::write(
            metadata_path(&base),
            serde_json::to_string_pretty(&metadata).unwrap(),
        )
        .unwrap();

        let loaded = load_project(&base, 8).unwrap();
        assert_eq!(loaded.ramps.step_counts()[0], 2);
        assert_eq!(loaded.palette.len(), loaded.ramps.colors().len());
        assert_ne!(loaded.palette.len(), stale, "the stale colours must not win");
    }

    /// A sidecar is plain JSON and can be edited into nonsense. It has to come
    /// back inside the editor's own rules rather than into a state no widget
    /// could have produced.
    #[test]
    fn a_nonsense_ramp_is_clamped_on_the_way_in() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("nonsense");

        let ramps = OklabRamps::default();
        let (palette, mode) = generated_state(&ramps, Palette::new());
        save_project(&AtlasDocument::new(), &palette, &mode, &ramps, &base).unwrap();

        let mut metadata: AtlasMetadata =
            serde_json::from_str(&fs::read_to_string(metadata_path(&base)).unwrap()).unwrap();
        let generated = metadata.generated.as_mut().unwrap();
        generated.ramps[0].steps = 9999;
        generated.ramps[0].a = 42.0;
        generated.ramps[1].from = -17.0;
        generated.ramps.truncate(2);
        fs::write(
            metadata_path(&base),
            serde_json::to_string_pretty(&metadata).unwrap(),
        )
        .unwrap();

        let loaded = load_project(&base, 8).unwrap();
        let first = loaded.ramps.get(0).unwrap();
        assert_eq!(first.steps, OKLAB_MAX_STEPS);
        assert_eq!(first.a, OKLAB_AB_LIMIT);
        assert_eq!(loaded.ramps.get(1).unwrap().from, 0.0, "clamped to L's range");
        assert_eq!(loaded.palette.colors, loaded.ramps.colors());
    }

    /// The editor refuses to remove the last ramp; a file with none has to be
    /// treated the same way rather than leaving nothing to draw with.
    #[test]
    fn a_sidecar_with_no_ramps_falls_back_to_the_built_in_ones() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("empty-ramps");

        save_plain(&AtlasDocument::new(), &Palette::new(), &base);
        let mut metadata: AtlasMetadata =
            serde_json::from_str(&fs::read_to_string(metadata_path(&base)).unwrap()).unwrap();
        metadata.generated.as_mut().unwrap().ramps.clear();
        fs::write(
            metadata_path(&base),
            serde_json::to_string_pretty(&metadata).unwrap(),
        )
        .unwrap();

        let loaded = load_project(&base, 8).unwrap();
        assert_eq!(loaded.ramps.ramps, OklabRamps::default().ramps);
    }

    /// An axis is a named value in the file, not an integer, so that a
    /// hand-edited sidecar reads as what it is.
    #[test]
    fn the_swept_axis_is_stored_by_name() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("axis-name");

        let mut ramps = OklabRamps::default();
        ramps.ramps[0] = ramps.ramps[0].with_axis(OklabAxis::B);
        save_project(
            &AtlasDocument::new(),
            &Palette::new(),
            &PaletteMode::default(),
            &ramps,
            &base,
        )
        .unwrap();

        let json = fs::read_to_string(metadata_path(&base)).unwrap();
        assert!(json.contains(r#""axis": "b""#), "got: {json}");
        assert!(json.contains(r#""axis": "lightness""#));
    }

    #[test]
    fn saving_writes_both_files() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("both");
        save_plain(&AtlasDocument::new(), &Palette::new(), &base);

        assert!(png_path(&base).exists());
        assert!(metadata_path(&base).exists());
    }
}
