//! Loading and saving a document as a PNG plus a JSON sidecar.
//!
//! The PNG *is* the atlas: same dimensions, same pixel order. The sidecar
//! carries what the image cannot — tile size, tile counts, and the palette.

use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::atlas::AtlasDocument;
use crate::config::{FILE_FORMAT_VERSION, METADATA_SUFFIX, UNTITLED_BASE};
use crate::document::DocumentPath;
use crate::palette::{Palette, PaletteData};

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

/// The JSON sidecar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtlasMetadata {
    #[serde(default = "default_version")]
    pub version: String,
    pub tile_size: u32,
    pub width_in_tiles: u32,
    pub height_in_tiles: u32,
    pub palette: PaletteData,
}

fn default_version() -> String {
    FILE_FORMAT_VERSION.to_string()
}

impl AtlasMetadata {
    pub fn describe(document: &AtlasDocument, palette: &Palette) -> Self {
        Self {
            version: FILE_FORMAT_VERSION.to_string(),
            tile_size: document.tile_size,
            width_in_tiles: document.width_in_tiles,
            height_in_tiles: document.height_in_tiles,
            palette: palette.to_data(),
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

/// Writes the PNG and the sidecar. The PNG goes first, so a failure there
/// leaves no sidecar describing an image that was never written.
pub fn save_project(
    document: &AtlasDocument,
    palette: &Palette,
    base: &Path,
) -> Result<(), FileError> {
    let width = document.width_in_pixels();
    let height = document.height_in_pixels();

    let buffer = image::RgbaImage::from_raw(width, height, document.to_rgba8())
        .ok_or_else(|| FileError::Image("pixel buffer does not match dimensions".to_string()))?;
    buffer.save(png_path(base))?;

    let metadata = AtlasMetadata::describe(document, palette);
    fs::write(metadata_path(base), serde_json::to_string_pretty(&metadata)?)?;

    Ok(())
}

/// Reads a document. The sidecar is optional — a bare PNG loads with the
/// fallback tile size and the built-in palette.
pub fn load_project(
    base: &Path,
    fallback_tile_size: u32,
) -> Result<(AtlasDocument, Palette), FileError> {
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

    let palette = metadata
        .map(|m| m.palette.to_palette())
        .unwrap_or_else(Palette::new);

    Ok((document, palette))
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
    mut path: ResMut<DocumentPath>,
    options: Res<crate::cli::LaunchOptions>,
) {
    let Some(base) = path.base.clone() else {
        info!("no path given — starting a new one-tile atlas");
        return;
    };

    match load_project(&base, options.tile_size) {
        Ok((loaded_document, loaded_palette)) => {
            info!(
                "loaded {} ({}x{} tiles of {}px)",
                png_path(&base).display(),
                loaded_document.width_in_tiles,
                loaded_document.height_in_tiles,
                loaded_document.tile_size
            );
            // Replace both or neither.
            *document = loaded_document;
            *palette = loaded_palette;
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
    mut path: ResMut<DocumentPath>,
) {
    if requests.read().count() == 0 {
        return;
    }

    let base = path
        .base
        .clone()
        .unwrap_or_else(|| PathBuf::from(UNTITLED_BASE));

    match save_project(&document, &palette, &base) {
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

        save_project(&document, &palette, &base).unwrap();
        let (loaded, loaded_palette) = load_project(&base, 8).unwrap();

        assert_eq!(loaded.tile_size, 4);
        assert_eq!(loaded.width_in_tiles, 2);
        assert_eq!(loaded.pixels, document.pixels);
        assert_eq!(loaded_palette.colors, palette.colors);
        assert_eq!(loaded_palette.selected, 4);
        assert!(!loaded.unsaved);
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

        save_project(&document, &Palette::new(), &base).unwrap();
        let (loaded, _) = load_project(&base, 2).unwrap();

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
        save_project(&document, &Palette::new(), &base).unwrap();

        let (loaded, _) = load_project(&base, 2).unwrap();
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
        save_project(&document, &Palette::new(), &base).unwrap();
        fs::remove_file(metadata_path(&base)).unwrap();

        let (loaded, palette) = load_project(&base, 4).unwrap();
        assert_eq!(loaded.tile_size, 4);
        assert_eq!(loaded.width_in_tiles, 2);
        assert_eq!(palette.len(), Palette::new().len());
    }

    #[test]
    fn an_image_that_is_not_a_whole_number_of_tiles_is_rejected() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("ragged");

        // 6x6 pixels cannot be tiled by 4.
        let document = AtlasDocument::with_dimensions(6, 1, 1);
        save_project(&document, &Palette::new(), &base).unwrap();

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
        save_project(&document, &Palette::new(), &base).unwrap();

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

    #[test]
    fn saving_writes_both_files() {
        let dir = tempdir().unwrap();
        let base = dir.path().join("both");
        save_project(&AtlasDocument::new(), &Palette::new(), &base).unwrap();

        assert!(png_path(&base).exists());
        assert!(metadata_path(&base).exists());
    }
}
