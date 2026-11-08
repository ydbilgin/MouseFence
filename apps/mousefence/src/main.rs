//! MouseFence 2.0 entry point. OWNED BY TRACK C (Track D owns what `run_cli` and `run_settings` do).
//!
//! Order (C# Program.cs): dev CLI modes first (they run beside a live instance), then the settings
//! process, then the single-instance check and the tray app (B018).
//!
//! Exit codes of the tray path: 0 after Exit or when another instance (1.x or 2.0) already runs (B017);
//! 4 the single-instance check failed; 5 the tray window could not be made; 6 the hook thread could not start.
#![cfg_attr(not(test), windows_subsystem = "windows")]

use std::process::ExitCode;

use fence_types::RuntimeState;

mod adapters;
mod store;
mod tray_app;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--replay") {
        fence_win::system::attach_parent_console_if_needed();
    }
    let code = dispatch(
        &args,
        fence_settings_ui::run_cli,
        fence_settings_ui::run_settings,
        tray_app::run,
    );
    ExitCode::from(code.clamp(0, 255) as u8)
}

/// Replay is headless and runs before all application paths. C# `Program.Main` routing follows:
/// `--screenshot`, `--screenshot-demo`, `--identify` (Track D's `run_cli`), then
/// `--settings --state ...`, and only then the tray (which takes the single-instance mutex itself).
fn dispatch(
    args: &[String],
    cli: impl FnOnce(&[String]) -> Option<i32>,
    settings: impl FnOnce(RuntimeState) -> i32,
    tray: impl FnOnce() -> i32,
) -> i32 {
    if let Some(code) = fence_trace::edge::run_replay_cli(args) {
        return code;
    }
    if let Some(code) = cli(args) {
        return code;
    }
    if let Some(state) = fence_types::launch::parse_settings_args(args) {
        return settings(state);
    }
    tray()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn route(args: &[&str]) -> (i32, Vec<&'static str>) {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let calls = RefCell::new(Vec::new());
        let code = dispatch(
            &args,
            |a| {
                calls.borrow_mut().push("cli");
                // Track D's matcher: these three names are the dev CLI modes.
                matches!(
                    a.first().map(String::as_str),
                    Some("--screenshot") | Some("--screenshot-demo") | Some("--identify")
                )
                .then_some(10)
            },
            |_| {
                calls.borrow_mut().push("settings");
                20
            },
            || {
                calls.borrow_mut().push("tray+single-instance");
                30
            },
        );
        (code, calls.into_inner())
    }

    #[test]
    fn b018_dev_cli_modes_run_before_the_single_instance_check() {
        for mode in ["--screenshot", "--screenshot-demo", "--identify"] {
            assert_eq!(route(&[mode, "x.png"]), (10, vec!["cli"]), "{mode}");
        }
    }

    #[test]
    fn b018_the_settings_process_runs_before_the_single_instance_check() {
        let state = "gate=1,side=1,confine=0,paused=0,hook=1";
        assert_eq!(route(&["--settings", "--state", state]), (20, vec!["cli", "settings"]));
    }

    #[test]
    fn b018_everything_else_is_the_tray() {
        assert_eq!(route(&[]), (30, vec!["cli", "tray+single-instance"]));
        assert_eq!(
            route(&["--settings", "--state", "garbage"]),
            (30, vec!["cli", "tray+single-instance"]),
            "an unreadable state is not a settings launch"
        );
    }

    #[test]
    fn j24_replay_dispatch_never_enters_cli_settings_or_tray() {
        let args = vec!["--replay".to_string()];
        assert_eq!(
            dispatch(
                &args,
                |_| panic!("other CLI"),
                |_| panic!("settings"),
                || panic!("tray/mutex")
            ),
            2
        );
    }
}
