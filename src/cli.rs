//! Command-line parsing, done by hand to keep the dependency list short.
//!
//! ```text
//! bevy_sprite_editor [PATH] [options]
//!
//!   PATH                  document base path; .png / .atlas.json suffixes are stripped
//!   --tile-size N         tile size for a new document (default 8)
//!   --scale N             screen pixels per atlas pixel (default 32)
//!   --script "..."        dev harness action script
//!   --screenshot FILE     capture to FILE once the script finishes, then exit
//!   --warmup-frames N     frames to wait before the script starts (default 30)
//!   --settle-frames N     frames to wait before capturing (default 30)
//!   --help                print this and exit
//! ```

use std::path::PathBuf;

use bevy::prelude::*;

use crate::config::{DEFAULT_PIXEL_SCALE, DEFAULT_TILE_SIZE};

/// Everything argv configures. Read at startup only, so a run is reproducible
/// from its command line.
#[derive(Resource, Debug, Clone)]
pub struct LaunchOptions {
    pub path: Option<PathBuf>,
    pub tile_size: u32,
    pub pixel_scale: f32,
    pub script: Option<String>,
    pub screenshot_path: Option<PathBuf>,
    /// Frames to let the window settle before the first scripted step. The
    /// window manager often resizes the window a frame or two in, which changes
    /// the canvas rect — acting before that has landed would be racy.
    pub warmup_frames: u32,
    pub settle_frames: u32,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            path: None,
            tile_size: DEFAULT_TILE_SIZE,
            pixel_scale: DEFAULT_PIXEL_SCALE,
            script: None,
            screenshot_path: None,
            warmup_frames: 30,
            settle_frames: 30,
        }
    }
}

pub const USAGE: &str = "\
bevy_sprite_editor [PATH] [options]

  PATH                  document base path; .png / .atlas.json suffixes are stripped
  --tile-size N         tile size for a new document (default 8)
  --scale N             screen pixels per atlas pixel (default 32)
  --script \"...\"        dev harness action script, e.g. \"select 3; paint 2,3; tile+\"
  --screenshot FILE     capture to FILE once the script finishes, then exit
  --warmup-frames N     frames to wait before the script starts (default 30)
  --settle-frames N     frames to wait before capturing (default 30)
  --help                print this and exit

Keys: left click paints, Space adds a tile, Left/Right or A/D scroll, Ctrl+S saves.
Script verbs: select N | paint X,Y | tile+[N] | scroll+[N] | scroll-[N] | save | wait [N]";

/// Parses argv. Unknown flags and unparsable numbers warn and are ignored rather
/// than aborting — the editor should still open.
pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<LaunchOptions, String> {
    let mut options = LaunchOptions::default();
    let mut args = args.into_iter().peekable();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => return Err(USAGE.to_string()),
            "--tile-size" => match take_number::<u32, _>(&mut args, "--tile-size") {
                Some(n) if n > 0 => options.tile_size = n,
                Some(_) => warn!("--tile-size must be greater than 0; keeping default"),
                None => {}
            },
            "--scale" => match take_number::<f32, _>(&mut args, "--scale") {
                Some(n) if n > 0.0 => options.pixel_scale = n,
                Some(_) => warn!("--scale must be greater than 0; keeping default"),
                None => {}
            },
            "--settle-frames" => {
                if let Some(n) = take_number::<u32, _>(&mut args, "--settle-frames") {
                    options.settle_frames = n;
                }
            }
            "--warmup-frames" => {
                if let Some(n) = take_number::<u32, _>(&mut args, "--warmup-frames") {
                    options.warmup_frames = n;
                }
            }
            "--script" => options.script = take_value(&mut args, "--script"),
            "--screenshot" => {
                options.screenshot_path = take_value(&mut args, "--screenshot").map(PathBuf::from);
            }
            other if other.starts_with('-') => {
                warn!("ignoring unknown option {other}");
            }
            positional => {
                if options.path.is_none() {
                    options.path = Some(strip_known_suffixes(positional));
                } else {
                    warn!("ignoring extra positional argument {positional}");
                }
            }
        }
    }

    Ok(options)
}

fn take_value<I: Iterator<Item = String>>(args: &mut I, flag: &str) -> Option<String> {
    match args.next() {
        Some(value) => Some(value),
        None => {
            warn!("{flag} needs a value; ignoring it");
            None
        }
    }
}

fn take_number<T: std::str::FromStr, I: Iterator<Item = String>>(
    args: &mut I,
    flag: &str,
) -> Option<T> {
    let raw = take_value(args, flag)?;
    match raw.parse::<T>() {
        Ok(value) => Some(value),
        Err(_) => {
            warn!("{flag} value {raw:?} is not a number; ignoring it");
            None
        }
    }
}

/// `demo`, `demo.png` and `demo.atlas.json` all mean the same document.
fn strip_known_suffixes(raw: &str) -> PathBuf {
    if let Some(base) = raw.strip_suffix(".atlas.json") {
        return PathBuf::from(base);
    }
    let path = PathBuf::from(raw);
    match path.extension().and_then(|e| e.to_str()) {
        Some("png") | Some("json") => path.with_extension(""),
        _ => path,
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> LaunchOptions {
        parse(args.iter().map(|s| s.to_string())).expect("should parse")
    }

    #[test]
    fn no_arguments_gives_the_defaults() {
        let options = parse_args(&[]);
        assert_eq!(options.path, None);
        assert_eq!(options.tile_size, DEFAULT_TILE_SIZE);
        assert_eq!(options.pixel_scale, DEFAULT_PIXEL_SCALE);
        assert!(options.script.is_none());
    }

    #[test]
    fn a_positional_argument_is_the_document_path() {
        assert_eq!(parse_args(&["demo"]).path, Some(PathBuf::from("demo")));
    }

    #[test]
    fn known_suffixes_are_stripped_so_all_three_spellings_agree() {
        let bare = parse_args(&["art/demo"]).path;
        assert_eq!(parse_args(&["art/demo.png"]).path, bare);
        assert_eq!(parse_args(&["art/demo.atlas.json"]).path, bare);
    }

    #[test]
    fn an_unrelated_extension_is_left_alone() {
        assert_eq!(
            parse_args(&["sprites.v2"]).path,
            Some(PathBuf::from("sprites.v2"))
        );
    }

    #[test]
    fn flags_are_parsed() {
        let options = parse_args(&[
            "demo",
            "--tile-size",
            "16",
            "--scale",
            "8",
            "--script",
            "paint 0,0",
            "--screenshot",
            "/tmp/shot.png",
            "--settle-frames",
            "9",
            "--warmup-frames",
            "11",
        ]);
        assert_eq!(options.tile_size, 16);
        assert_eq!(options.pixel_scale, 8.0);
        assert_eq!(options.script.as_deref(), Some("paint 0,0"));
        assert_eq!(options.screenshot_path, Some(PathBuf::from("/tmp/shot.png")));
        assert_eq!(options.settle_frames, 9);
        assert_eq!(options.warmup_frames, 11);
    }

    #[test]
    fn help_is_reported_as_an_early_exit() {
        assert!(parse(["--help".to_string()]).is_err());
    }

    #[test]
    fn a_bad_number_falls_back_to_the_default() {
        let options = parse_args(&["--tile-size", "banana"]);
        assert_eq!(options.tile_size, DEFAULT_TILE_SIZE);
    }

    #[test]
    fn a_zero_tile_size_is_refused() {
        assert_eq!(parse_args(&["--tile-size", "0"]).tile_size, DEFAULT_TILE_SIZE);
    }

    #[test]
    fn unknown_flags_do_not_stop_the_editor_opening() {
        let options = parse_args(&["--wat", "demo"]);
        assert_eq!(options.path, Some(PathBuf::from("demo")));
    }

    #[test]
    fn a_dangling_flag_value_is_ignored() {
        let options = parse_args(&["demo", "--scale"]);
        assert_eq!(options.pixel_scale, DEFAULT_PIXEL_SCALE);
    }
}
