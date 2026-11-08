//! MouseFence settings window (port of C# `Ui/` behaviour, not of its WinForms drawing code). OWNED BY TRACK D.
//!
//! Decisions (PLAN T16-T17): egui/eframe in a separate process that exists only while the owner has the
//! window open; functional parity with the 1.x window (four pages, map with clickable gates, boundary
//! drawer, screen names and keep-pointer-off, Advanced numbers, Shortcuts capture and validation, General
//! with backup/restore, EN/TR, System/Light/Dark/high contrast), proven by headless tests that drive
//! `egui::Context` with raw input (no window, no GPU). Screenshots and the identify overlay run only
//! through low_run on the invisible desktop.
//!
//! Layout: `editor` holds the behaviour (one method per C# `SettingsForm` handler), `boundary` the map model,
//! `ui` draws both with egui, `backend` is the port to Tracks A and B, `process` is the settings process
//! (`--settings`), `screenshot` and `identify` are the dev modes, `headless` and `raster` run and paint frames
//! without a window, `win` holds the module's few Win32 calls.

pub mod backend;
pub mod boundary;
pub mod editor;
pub mod headless;
pub mod identify;
pub mod png;
pub mod process;
pub mod raster;
pub mod screenshot;
pub mod text;
pub mod theme;
pub mod ui;
pub mod win;

#[cfg(test)]
mod testkit;
#[cfg(test)]
mod tests;

use fence_types::RuntimeState;

/// Open the settings window and block until it closes. Returns a `fence_types::launch::EXIT_*` code.
pub fn run_settings(state: RuntimeState) -> i32 {
    process::run_settings(state)
}

/// Dev/QA modes that run before the single-instance check (C# Program.cs:16-72): `--screenshot`,
/// `--screenshot-demo`, `--identify`. Returns `Some(exit code)` when `args` (without the exe path) name one.
pub fn run_cli(args: &[String]) -> Option<i32> {
    if let Some(parsed) = screenshot::parse_args(args) {
        return Some(match parsed {
            Ok(a) => screenshot::run(&a),
            Err(e) => {
                eprintln!("{e}");
                2
            }
        });
    }
    if args.first().map(String::as_str) == Some("--identify") {
        return Some(identify::run_identify_cli(args));
    }
    None
}
