//! A Bevy editor for sprite atlases.
//!
//! An atlas is one image split into square tiles, edited pixel by pixel. This
//! version supports the main workflow: open a file, paint pixels with a palette
//! colour, add tiles, scroll a wide atlas, save.
//!
//! ## Assembly
//!
//! Seven plugins, each specced in
//! `~/fun_repos/hobby-mimisbrunnr/notes/bevy_sprite_editor/initial_editor/fixup/`:
//!
//! - [`document`] — `AtlasDocument` + `Palette`, the only mutable state
//! - [`file_io`] — PNG + JSON load and save
//! - [`canvas_view`] — camera viewport, scroll, cursor → pixel
//! - [`canvas_render`] — the atlas texture and the tile grid
//! - [`ui_chrome`] — Feathers palette sidebar and status strip
//! - [`input`] — keys and mouse to edit requests
//! - [`dev_harness`] — scripted actions and screenshots
//!
//! Input never mutates state: it emits requests that [`document`] applies. That
//! keeps the dev harness honest, since it emits exactly the same ones.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

mod atlas;
mod canvas_render;
mod canvas_view;
mod cli;
mod config;
mod dev_harness;
mod document;
mod file_io;
// Kept compiling and tested, but deliberately not wired into the editor:
// undo/redo is out of scope for this version.
#[allow(dead_code)]
mod history;
mod input;
mod oklab;
mod palette;
mod ui_chrome;

use crate::canvas_render::CanvasRenderPlugin;
use crate::canvas_view::{CanvasView, CanvasViewPlugin};
use crate::config::{
    CANVAS_BACKGROUND, CANVAS_LAYER, DEFAULT_WINDOW_HEIGHT, DEFAULT_WINDOW_WIDTH, WINDOW_TITLE,
};
use crate::dev_harness::DevHarnessPlugin;
use crate::document::{seed_document, DocumentStatePlugin};
use crate::file_io::{AtlasIoPlugin, DocumentLoad};
use crate::input::EditorInputPlugin;
use crate::ui_chrome::{ChromeBuild, UiChromePlugin};

// ============================================================================
// CAMERAS
// ============================================================================

/// Draws the atlas. Its `viewport` is fitted to the region the chrome leaves
/// free, which is what decouples canvas world space from the sidebar width.
#[derive(Component)]
pub struct CanvasCamera;

/// Draws the bevy_ui tree, and clears the window.
#[derive(Component)]
pub struct UiCamera;

fn main() {
    let options = match cli::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        // `--help`, or a parse we refuse to guess at.
        Err(message) => {
            println!("{message}");
            return;
        }
    };

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: WINDOW_TITLE.to_string(),
                        resolution: (DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT).into(),
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                })
                // Scaled-up atlas pixels must stay hard-edged.
                .set(ImagePlugin::default_nearest()),
        )
        .insert_resource(CanvasView {
            pixel_scale: options.pixel_scale,
            ..default()
        })
        .insert_resource(options)
        .add_plugins((
            DocumentStatePlugin,
            AtlasIoPlugin,
            CanvasViewPlugin,
            CanvasRenderPlugin,
            UiChromePlugin,
            EditorInputPlugin,
            DevHarnessPlugin,
        ))
        // Cross-plugin startup order, declared here because no single feature
        // plugin owns it: seed a blank document, replace it with what is on
        // disk, then derive the palette theme and sidebar from the result.
        .configure_sets(Startup, ChromeBuild.after(DocumentLoad))
        .add_systems(PreStartup, seed_document)
        .add_systems(Startup, spawn_cameras)
        .run();
}

/// Two cameras: the canvas camera draws the atlas inside its viewport, then the
/// UI camera draws the chrome over the whole window without clearing.
///
/// The canvas is on [`CANVAS_LAYER`] so only the canvas camera sees it — that is
/// what stops the full-window UI camera from redrawing the atlas across the
/// sidebar. The sidebar and status strip are opaque and cover exactly the region
/// the canvas viewport does not, so the whole window is always painted.
fn spawn_cameras(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: 0,
            clear_color: ClearColorConfig::Custom(CANVAS_BACKGROUND),
            ..default()
        },
        RenderLayers::layer(CANVAS_LAYER),
        CanvasCamera,
    ));

    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        IsDefaultUiCamera,
        UiCamera,
    ));
}
