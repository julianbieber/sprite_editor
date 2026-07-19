//! File I/O operations for the sprite atlas editor
//!
//! Handles saving and loading atlas projects as PNG + JSON files.

use std::fs;
use std::path::{Path, PathBuf};
use std::io::Error as IoError;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::atlas::Atlas;
use crate::config::{FILE_FORMAT_VERSION, IMAGE_EXTENSION, METADATA_EXTENSION};
use crate::editor::Editor;
use crate::palette::{ColorData, Palette, PaletteData};

// ============================================================================
// ERROR TYPES
// ============================================================================

/// Custom error type for file operations
#[derive(Debug)]
pub enum FileError {
    /// Error from std::io
    IoError(IoError),
    /// Error from JSON serialization/deserialization
    JsonError(serde_json::Error),
    /// File not found
    FileNotFound(PathBuf),
    /// Invalid file format or version
    InvalidFormat(String),
    /// Missing required file
    MissingFile(PathBuf),
}

impl From<IoError> for FileError {
    fn from(error: IoError) -> Self {
        FileError::IoError(error)
    }
}

impl From<serde_json::Error> for FileError {
    fn from(error: serde_json::Error) -> Self {
        FileError::JsonError(error)
    }
}

impl From<image::ImageError> for FileError {
    fn from(error: image::ImageError) -> Self {
        FileError::IoError(IoError::new(std::io::ErrorKind::Other, error.to_string()))
    }
}

impl std::fmt::Display for FileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileError::IoError(e) => write!(f, "IO error: {}", e),
            FileError::JsonError(e) => write!(f, "JSON error: {}", e),
            FileError::FileNotFound(p) => write!(f, "File not found: {}", p.display()),
            FileError::InvalidFormat(s) => write!(f, "Invalid format: {}", s),
            FileError::MissingFile(p) => write!(f, "Missing file: {}", p.display()),
        }
    }
}

impl std::error::Error for FileError {}

impl From<String> for FileError {
    fn from(error: String) -> Self {
        FileError::InvalidFormat(error)
    }
}

// ============================================================================
// METADATA STRUCTURES
// ============================================================================

/// Metadata stored in the JSON file alongside the PNG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtlasMetadata {
    /// File format version
    #[serde(default = "default_version")]
    pub version: String,
    /// Size of each tile in pixels
    pub tile_size: u32,
    /// Number of tiles wide
    pub width_in_tiles: u32,
    /// Number of tiles tall
    pub height_in_tiles: u32,
    /// The color palette
    pub palette: PaletteData,
    /// Timestamp of creation (ISO 8601)
    #[serde(default)]
    pub created_at: Option<String>,
    /// Timestamp of last modification (ISO 8601)
    #[serde(default)]
    pub modified_at: Option<String>,
}

fn default_version() -> String {
    FILE_FORMAT_VERSION.to_string()
}

impl AtlasMetadata {
    /// Create metadata from an editor state
    pub fn from_editor(editor: &Editor) -> Self {
        Self {
            version: FILE_FORMAT_VERSION.to_string(),
            tile_size: editor.tile_size,
            width_in_tiles: editor.atlas.width_in_tiles,
            height_in_tiles: editor.atlas.height_in_tiles,
            palette: PaletteData::from_palette(&editor.palette),
            created_at: None,
            modified_at: None,
        }
    }

    /// Validate the metadata
    pub fn validate(&self) -> Result<(), String> {
        if self.version != FILE_FORMAT_VERSION {
            return Err(format!(
                "Unsupported file version: {} (expected {})",
                self.version, FILE_FORMAT_VERSION
            ));
        }

        if self.tile_size == 0 {
            return Err("Tile size must be greater than 0".to_string());
        }

        if self.width_in_tiles == 0 {
            return Err("Atlas width must be greater than 0".to_string());
        }

        if self.height_in_tiles == 0 {
            return Err("Atlas height must be greater than 0".to_string());
        }

        Ok(())
    }
}

// ============================================================================
// FILE OPERATIONS
// ============================================================================

/// Get the PNG path for a given base path
pub fn get_png_path(base_path: &Path) -> PathBuf {
    base_path.with_extension("png")
}

/// Get the metadata path for a given base path
pub fn get_metadata_path(base_path: &Path) -> PathBuf {
    let mut path = base_path.to_path_buf();
    let file_name = path.file_name().unwrap_or_default();
    path.set_file_name(format!(
        "{}.atlas.json",
        file_name.to_string_lossy()
    ));
    path
}

/// Save the editor state to files
pub fn save_project(editor: &Editor, base_path: &Path) -> Result<(), FileError> {
    // Save the PNG image
    let png_path = get_png_path(base_path);
    editor.atlas.save_to_png(&png_path)?;

    // Create and save the metadata
    let metadata = AtlasMetadata::from_editor(editor);
    let metadata_path = get_metadata_path(base_path);
    let json = serde_json::to_string_pretty(&metadata)?;
    fs::write(&metadata_path, json)?;

    Ok(())
}

/// Load a project from files
pub fn load_project(base_path: &Path) -> Result<Editor, FileError> {
    let png_path = get_png_path(base_path);
    let metadata_path = get_metadata_path(base_path);

    // Check files exist
    if !png_path.exists() {
        return Err(FileError::FileNotFound(png_path));
    }
    if !metadata_path.exists() {
        return Err(FileError::FileNotFound(metadata_path));
    }

    // Load the PNG
    let tile_size = load_tile_size_from_metadata(&metadata_path)?;
    let mut editor = Editor::with_tile_size(tile_size);
    editor.atlas = Atlas::load_from_png(&png_path, tile_size)?;

    // Load the metadata
    let metadata = load_metadata(&metadata_path)?;
    metadata.validate().map_err(FileError::InvalidFormat)?;

    // Apply metadata to editor
    editor.atlas.width_in_tiles = metadata.width_in_tiles;
    editor.atlas.height_in_tiles = metadata.height_in_tiles;
    editor.palette = metadata.palette.to_palette();

    Ok(editor)
}

/// Load just the tile size from metadata (needed to load PNG correctly)
fn load_tile_size_from_metadata(metadata_path: &Path) -> Result<u32, FileError> {
    let metadata_json = fs::read_to_string(metadata_path)?;
    let metadata: AtlasMetadata = serde_json::from_str(&metadata_json)?;
    Ok(metadata.tile_size)
}

/// Load metadata from JSON file
fn load_metadata(metadata_path: &Path) -> Result<AtlasMetadata, FileError> {
    let json = fs::read_to_string(metadata_path)?;
    let metadata = serde_json::from_str(&json)?;
    Ok(metadata)
}

/// Export just the atlas image (without metadata)
pub fn export_atlas_image(editor: &Editor, path: &Path) -> Result<(), FileError> {
    editor.atlas.save_to_png(path)?;
    Ok(())
}

/// Import an existing PNG as a new atlas
pub fn import_png(path: &Path, tile_size: u32) -> Result<Editor, FileError> {
    let atlas = Atlas::load_from_png(path, tile_size)?;
    let editor = Editor {
        atlas,
        palette: Palette::new(),
        viewport: crate::editor::Viewport::new(),
        history: crate::history::History::new(),
        tile_size,
    };
    Ok(editor)
}

// ============================================================================
// DEFAULT PALETTE
// ============================================================================

/// Create the default palette file
pub fn create_default_palette_file(path: &Path) -> Result<(), FileError> {
    let palette = Palette::new();
    let data = palette.to_data();
    let json = serde_json::to_string_pretty(&data)?;
    fs::write(path, json)?;
    Ok(())
}

// ============================================================================
// PATH UTILITIES
// ============================================================================

/// Get the user's documents directory (platform-specific)
pub fn get_documents_dir() -> PathBuf {
    if cfg!(target_os = "windows") {
        if let Some(documents) = std::env::var_os("USERPROFILE") {
            return PathBuf::from(documents).join("Documents");
        }
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join("Documents");
        }
    } else {
        // Linux and others
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    }

    PathBuf::from(".")
}

/// Get the default directory for saving projects
pub fn get_default_save_dir() -> PathBuf {
    get_documents_dir().join("bevy_sprite_editor")
}

/// Ensure a directory exists
pub fn ensure_dir_exists(dir: &Path) -> Result<(), FileError> {
    if !dir.exists() {
        fs::create_dir_all(dir)?;
    }
    Ok(())
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_metadata_serialization() {
        let editor = Editor::new();
        let metadata = AtlasMetadata::from_editor(&editor);

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: AtlasMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.tile_size, metadata.tile_size);
        assert_eq!(deserialized.width_in_tiles, metadata.width_in_tiles);
    }

    #[test]
    fn test_metadata_validation() {
        let metadata = AtlasMetadata {
            version: FILE_FORMAT_VERSION.to_string(),
            tile_size: 8,
            width_in_tiles: 1,
            height_in_tiles: 1,
            palette: PaletteData {
                colors: vec![],
                selected_index: None,
            },
            created_at: None,
            modified_at: None,
        };

        assert!(metadata.validate().is_ok());
    }

    #[test]
    fn test_metadata_validation_fails() {
        let metadata = AtlasMetadata {
            version: "0.1".to_string(), // Wrong version
            tile_size: 8,
            width_in_tiles: 1,
            height_in_tiles: 1,
            palette: PaletteData {
                colors: vec![],
                selected_index: None,
            },
            created_at: None,
            modified_at: None,
        };

        assert!(metadata.validate().is_err());
    }

    #[test]
    fn test_path_utilities() {
        let base_path = Path::new("/path/to/my_project");

        let png_path = get_png_path(base_path);
        assert_eq!(png_path, Path::new("/path/to/my_project.png"));

        let metadata_path = get_metadata_path(base_path);
        assert_eq!(
            metadata_path,
            Path::new("/path/to/my_project.atlas.json")
        );
    }
}
