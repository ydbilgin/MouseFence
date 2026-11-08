//! The settings process (`mousefence.exe --settings --state "gate=..,side=..,confine=..,paused=..,hook=.."`, PLAN
//! T16): it loads the v2 settings, shows the window, saves through fence-config on Save and exits 0 (saved), 1
//! (cancelled or closed) or 2 (the window could not run). While open it re-reads the live switches from the state
//! file the tray rewrites (every 400 ms, C# `_runtimeTimer`) and follows display changes (C#
//! `DisplaySettingsChanged` + the 700 ms `_displayRefreshTimer`).
//!
//! A save that fence-config refuses or cannot write keeps the window open with the draft and an inline message
//! (sealed screen, repairable values, or "couldn't write that settings file"), so no edit is lost.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use eframe::egui;
use fence_config::{Paths, SaveOutcome, Settings};
use fence_types::launch::{EXIT_CANCELLED, EXIT_FAILED, EXIT_SAVED, STATE_FILE_NAME, state_from_text};
use fence_types::{MonitorInfo, RuntimeState};

use crate::backend::{Backend, RealBackend};
use crate::editor::{Dialogs, Editor, Outcome};
use crate::theme::Theme;
use crate::ui::{Services, SettingsUi, SystemTheme};

/// Where a saved result goes (fence-config in the product, a fake in tests).
pub trait Store {
    fn save(&mut self, settings: &mut Settings) -> SaveOutcome;
}

/// fence-config's atomic v2 save, with the engine's seal check injected.
pub struct ConfigStore {
    pub paths: Paths,
    pub backend: Rc<dyn Backend>,
    pub monitors: Vec<MonitorInfo>,
}

impl Store for ConfigStore {
    fn save(&mut self, settings: &mut Settings) -> SaveOutcome {
        let backend = self.backend.clone();
        let monitors = self.monitors.clone();
        fence_config::save(settings, &self.paths, &move |s: &Settings| {
            let mut copy = s.clone();
            backend.validate_no_seal(&mut copy, &monitors)
        })
    }
}

/// After a frame: `Some(exit code)` when the window should close. A Save result goes through the store first;
/// a refused or failed save reopens the draft with an inline message.
pub fn settle_outcome(editor: &mut Editor, store: &mut dyn Store) -> Option<i32> {
    match editor.outcome.clone() {
        Outcome::Open => None,
        Outcome::Cancelled => Some(EXIT_CANCELLED),
        Outcome::Saved(result) => {
            let mut s = *result;
            match store.save(&mut s) {
                SaveOutcome::Saved => Some(EXIT_SAVED),
                SaveOutcome::RefusedRepairable => {
                    editor.save_failed(&editor.tr.t("ImportInvalid"));
                    None
                }
                SaveOutcome::RefusedSealed(issues) => {
                    match issues.into_iter().next() {
                        Some(issue) => editor.show_seal_issue(issue),
                        None => editor.save_failed(&editor.tr.t("ImportInvalid")),
                    }
                    None
                }
                SaveOutcome::Failed(_) => {
                    editor.save_failed(&editor.tr.t("ExportFailed"));
                    None
                }
            }
        }
    }
}

/// The live switches: the state file when it parses, else the value the tray passed on the command line.
pub fn read_state_file(path: Option<&Path>, fallback: RuntimeState) -> RuntimeState {
    path.and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| state_from_text(&t))
        .unwrap_or(fallback)
}

/// `%LOCALAPPDATA%\MouseFence\v2\state.txt`.
pub fn state_file_path() -> Option<PathBuf> {
    fence_win::local_appdata_dir().map(|d| d.join("MouseFence").join("v2").join(STATE_FILE_NAME))
}

/// The exit code of a finished run: eframe's error is a failure; a window closed without a decision is a cancel.
pub fn exit_code(run: Result<(), String>, decided: Option<i32>) -> i32 {
    match run {
        Err(_) => EXIT_FAILED,
        Ok(()) => decided.unwrap_or(EXIT_CANCELLED),
    }
}

struct AppServices {
    dialogs: crate::win::Win32Dialogs,
    system: SystemTheme,
    backend: Rc<dyn Backend>,
}

impl Services for AppServices {
    fn windows_key_down(&self) -> bool {
        fence_win::hotkeys::windows_key_down()
    }
    fn dialogs(&mut self) -> &mut dyn Dialogs {
        &mut self.dialogs
    }
    fn identify(&mut self, monitors: &[MonitorInfo], names: &[(String, String)], theme: &Theme) {
        let backend = self.backend.clone();
        let normalize = move |s: &str| backend.normalize_monitor_name(s);
        crate::identify::flash_in_background(crate::identify::overlay_specs(
            monitors,
            names,
            theme.accent,
            &normalize,
        ));
    }
    fn system_theme(&self) -> SystemTheme {
        self.system
    }
}

struct SettingsApp {
    ui: SettingsUi,
    services: AppServices,
    store: ConfigStore,
    decided: Rc<std::cell::Cell<Option<i32>>>,
    state_file: Option<PathBuf>,
    initial: RuntimeState,
    next_state_poll: f64,
    next_monitor_poll: f64,
    pending_monitors: Option<f64>,
}

impl eframe::App for SettingsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        if now >= self.next_state_poll {
            self.next_state_poll = now + 0.4;
            let state = read_state_file(self.state_file.as_deref(), self.initial);
            if self.ui.editor.live != Some(state) {
                self.ui.editor.set_live(Some(state));
            }
        }
        if now >= self.next_monitor_poll {
            self.next_monitor_poll = now + 1.0;
            let current = fence_win::enumerate_monitors();
            if !current.is_empty() && current != self.ui.editor.monitors && self.pending_monitors.is_none() {
                self.pending_monitors = Some(now + 0.7);
            }
        }
        if let Some(at) = self.pending_monitors
            && now >= at
        {
            self.pending_monitors = None;
            let current = fence_win::enumerate_monitors();
            if !current.is_empty() && current != self.ui.editor.monitors {
                self.store.monitors = current.clone();
                self.ui.editor.apply_monitors(current);
            }
        }
        // The window's close box is Cancel (C# DialogResult.Cancel).
        if ctx.input(|i| i.viewport().close_requested()) && self.decided.get().is_none() {
            self.ui.editor.cancel();
        }
        self.ui.show(ui, &mut self.services);
        if self.decided.get().is_none()
            && let Some(code) = settle_outcome(&mut self.ui.editor, &mut self.store)
        {
            self.decided.set(Some(code));
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(400));
    }
}

/// `--settings`: show the window until it closes; returns a `fence_types::launch::EXIT_*` code.
pub fn run_settings(state: RuntimeState) -> i32 {
    let Some(appdata) = fence_win::appdata_dir() else {
        return EXIT_FAILED;
    };
    let paths = Paths::under_appdata(&appdata);
    let monitors = fence_win::enumerate_monitors();
    let loaded = fence_config::load_production(&paths, &monitors, fence_win::has_intel_gpu());
    let backend: Rc<dyn Backend> = Rc::new(RealBackend);
    let tr = crate::text::resolve(&loaded.settings.language, fence_win::os_ui_is_turkish());
    let state_file = state_file_path();
    let live = read_state_file(state_file.as_deref(), state);
    let editor = Editor::new(&loaded.settings, monitors.clone(), backend.clone(), tr, Some(live));
    let system = crate::win::system_theme();
    let ui = SettingsUi::new(editor, system);
    let decided = Rc::new(std::cell::Cell::new(None));
    let app = SettingsApp {
        ui,
        services: AppServices {
            dialogs: crate::win::Win32Dialogs,
            system,
            backend: backend.clone(),
        },
        store: ConfigStore {
            paths,
            backend,
            monitors,
        },
        decided: decided.clone(),
        state_file,
        initial: state,
        next_state_poll: 0.0,
        next_monitor_poll: 1.0,
        pending_monitors: None,
    };
    let title = tr.t("SettingsTitle");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_icon(egui::IconData {
                rgba: include_bytes!("../../../assets/app-48.rgba").to_vec(),
                width: 48,
                height: 48,
            })
            .with_inner_size([1064.0, 712.0])
            .with_min_inner_size([720.0, 560.0])
            .with_taskbar(false),
        renderer: eframe::Renderer::Glow,
        run_and_return: true,
        centered: true,
        ..Default::default()
    };
    let run = eframe::run_native(
        "MouseFence settings",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_fonts(crate::ui::font_definitions(true));
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| e.to_string());
    if let Err(e) = &run {
        eprintln!("settings window failed: {e}");
    }
    exit_code(run, decided.get())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{TempFile, fake, mon};
    use crate::text::Tr;
    use fence_i18n::Lang;
    use fence_types::SealIssue;

    struct FakeStore {
        answer: SaveOutcome,
        saved: Vec<Settings>,
    }

    impl Store for FakeStore {
        fn save(&mut self, settings: &mut Settings) -> SaveOutcome {
            self.saved.push(settings.clone());
            self.answer.clone()
        }
    }

    fn editor() -> Editor {
        let ms = vec![
            mon("left", 1, true, 0, 0, 100, 100),
            mon("right", 2, false, 100, 0, 200, 100),
        ];
        Editor::new(&Settings::default(), ms, fake(), Tr(Lang::En), None)
    }

    // Parity row B037 (PLAN T16, new in 2.0).
    #[test]
    fn b037_settings_process_reads_live_state_exits_0_after_save_1_on_cancel_2_on_failure() {
        // Live state: the state file wins when it parses, else the --state value.
        let fallback = RuntimeState {
            gate_open: false,
            side_contain: true,
            confine: false,
            paused: false,
            hook_installed: true,
        };
        let file = TempFile::new("state");
        assert_eq!(read_state_file(Some(&file.0), fallback), fallback, "no file yet");
        std::fs::write(&file.0, "gate=1,side=0,confine=1,paused=0,hook=1").expect("write temp");
        let read = read_state_file(Some(&file.0), fallback);
        assert!(read.gate_open && !read.side_contain && read.confine);
        std::fs::write(&file.0, "garbage").expect("write temp");
        assert_eq!(
            read_state_file(Some(&file.0), fallback),
            fallback,
            "an unreadable file keeps the last known value"
        );
        let args: Vec<String> = fence_types::launch::settings_args(&fallback);
        assert_eq!(fence_types::launch::parse_settings_args(&args), Some(fallback));

        // Open: nothing to do.
        let mut e = editor();
        let mut store = FakeStore {
            answer: SaveOutcome::Saved,
            saved: vec![],
        };
        assert_eq!(settle_outcome(&mut e, &mut store), None);
        // Save -> through the store -> exit 0 with the saved draft.
        e.draft.up_cross_min = 20;
        e.save();
        assert_eq!(settle_outcome(&mut e, &mut store), Some(EXIT_SAVED));
        assert_eq!(store.saved.len(), 1);
        assert_eq!(store.saved[0].up_cross_min, 20);
        // Cancel -> exit 1, nothing saved.
        let mut e = editor();
        e.cancel();
        assert_eq!(settle_outcome(&mut e, &mut store), Some(EXIT_CANCELLED));
        assert_eq!(store.saved.len(), 1);
        // A failed write keeps the window open with the draft and says so.
        let mut e = editor();
        let mut failing = FakeStore {
            answer: SaveOutcome::Failed("disk full".into()),
            saved: vec![],
        };
        e.draft.up_cross_min = 33;
        e.save();
        assert_eq!(settle_outcome(&mut e, &mut failing), None);
        assert_eq!(e.outcome, Outcome::Open);
        assert_eq!(e.draft.up_cross_min, 33);
        assert_eq!(e.inline.text, Tr(Lang::En).t("ExportFailed"));
        assert!(e.inline.error);
        // A sealed refusal names the screen and offers the boundary.
        let mut sealed = FakeStore {
            answer: SaveOutcome::RefusedSealed(vec![SealIssue {
                stable_id: "right".into(),
                focus_to_stable_id: "left".into(),
                focus_direction: fence_types::Direction::Left,
                focus_corridor_width: 100,
            }]),
            saved: vec![],
        };
        e.save();
        assert_eq!(settle_outcome(&mut e, &mut sealed), None);
        assert_eq!(e.inline.text, Tr(Lang::En).f("InlineSealError", &["Screen 2"]));
        assert!(e.inline.action);
        let mut repairable = FakeStore {
            answer: SaveOutcome::RefusedRepairable,
            saved: vec![],
        };
        e.save();
        assert_eq!(settle_outcome(&mut e, &mut repairable), None);
        assert_eq!(e.inline.text, Tr(Lang::En).t("ImportInvalid"));
        // Exit codes of the whole run.
        assert_eq!(exit_code(Err("no GL".into()), None), EXIT_FAILED);
        assert_eq!(exit_code(Ok(()), None), EXIT_CANCELLED, "closed without a decision");
        assert_eq!(exit_code(Ok(()), Some(EXIT_SAVED)), EXIT_SAVED);
    }
}
