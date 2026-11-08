//! fence-win behind fence-app's ports. Thin: every rule lives in fence-app (tested with fakes) or in fence-win
//! (tested on message-only windows and pure functions); this file only forwards.

use std::path::PathBuf;
use std::rc::Rc;

use fence_app::{
    AutoStartPort, Balloon, BalloonKind, EngineCore, Event, Guard, HookHost, HotkeyPort, HotkeySlot, MenuCommand,
    MenuEntry, Ports, SettingsProcessPort, SystemPort, TimerId, TimerPort, TrayIcon, TrayPort, TrayView,
};
use fence_engine::decide::ClassicEngine;
use fence_types::{EngineConfig, Hotkey, MonitorInfo, RuntimeState, launch};
use fence_win::autostart::{self, RUN_KEY, RUN_VALUE};
use fence_win::hook::HookThread;
use fence_win::tray::{NativeMenuEntry, TrayIconKind, TrayWindow, WindowEvent};

use crate::store::FileStore;

pub fn icon_kind(icon: TrayIcon) -> TrayIconKind {
    match icon {
        TrayIcon::Closed => TrayIconKind::Closed,
        TrayIcon::Open => TrayIconKind::Open,
        TrayIcon::Paused => TrayIconKind::Paused,
    }
}

/// The native menu for a view, in the view's fixed order.
pub fn native_menu(view: &TrayView) -> Vec<NativeMenuEntry> {
    view.menu()
        .into_iter()
        .map(|e| match e {
            MenuEntry::Item(cmd, item) => NativeMenuEntry::Item {
                id: cmd.id(),
                text: if item.shortcut.is_empty() {
                    item.text.clone()
                } else {
                    format!("{}\t{}", item.text, item.shortcut)
                },
                checked: item.checked,
                enabled: item.enabled,
            },
            MenuEntry::Separator => NativeMenuEntry::Separator,
        })
        .collect()
}

/// A window event as a controller event (`None`: not ours, ignore).
pub fn to_event(ev: WindowEvent) -> Option<Event> {
    match ev {
        WindowEvent::TrayDoubleClick => Some(Event::TrayDoubleClick),
        WindowEvent::MenuCommand(id) => MenuCommand::from_id(id).map(Event::Menu),
        WindowEvent::Hotkey(id) => HotkeySlot::from_id(id).map(Event::Hotkey),
        WindowEvent::Timer(id) => TimerId::from_id(id).map(Event::Timer),
        WindowEvent::DisplayChanged => Some(Event::DisplayChanged),
        WindowEvent::SettingsExited(code) => Some(Event::SettingsExited(code)),
    }
}

pub struct WinTray(pub Rc<TrayWindow>);

impl TrayPort for WinTray {
    fn probe_notifications(&mut self) {
        self.0.probe_notifications();
    }
    fn add_icon(&mut self, icon: TrayIcon, tooltip: &str) {
        self.0.add_icon(icon_kind(icon), tooltip);
    }
    fn show(&mut self, view: &TrayView) {
        self.0.update(icon_kind(view.icon), &view.tooltip);
        self.0.set_menu(native_menu(view));
    }
    fn balloon(&mut self, b: &Balloon) {
        self.0.balloon(&b.text, b.kind == BalloonKind::Warning, b.millis);
    }
    fn remove_icon(&mut self) {
        self.0.remove_icon();
    }
}

pub struct WinHotkeys(pub Rc<TrayWindow>);

impl HotkeyPort for WinHotkeys {
    fn register(&mut self, slot: HotkeySlot, hotkey: Hotkey) -> bool {
        self.0.register_hotkey(slot.id(), &hotkey)
    }
    fn unregister(&mut self, slot: HotkeySlot) {
        self.0.unregister_hotkey(slot.id());
    }
}

pub struct WinTimers(pub Rc<TrayWindow>);

impl TimerPort for WinTimers {
    fn start(&mut self, timer: TimerId, millis: u32) {
        self.0.set_timer(timer.id(), millis);
    }
    fn stop(&mut self, timer: TimerId) {
        self.0.kill_timer(timer.id());
    }
}

pub struct WinSystem {
    pub error_log: Option<PathBuf>,
}

impl SystemPort for WinSystem {
    fn monitors(&self) -> Vec<MonitorInfo> {
        fence_win::enumerate_monitors()
    }
    fn has_intel_gpu(&self) -> bool {
        fence_win::has_intel_gpu()
    }
    fn os_ui_is_turkish(&self) -> bool {
        fence_win::os_ui_is_turkish()
    }
    fn log(&self, line: &str) {
        fence_win::debug_log(line);
        if let Some(path) = &self.error_log {
            let _ = fence_win::error_log::append(path, "tray", line);
        }
    }
}

/// Writes the real Run key; reached only from the tray's settings-saved path (B013), never from a test.
pub struct WinAutoStart {
    pub exe: PathBuf,
}

impl AutoStartPort for WinAutoStart {
    fn apply(&mut self, enable: bool) {
        let r = autostart::apply(RUN_KEY, RUN_VALUE, enable, &self.exe);
        if let autostart::AutoStartResult::Failed(code) = r {
            fence_win::debug_log(&format!("autostart: Win32 error {code} (non-fatal)"));
        }
    }
}

/// The hook thread as the guard's host.
pub struct WinHookHost(pub HookThread, TraceBridge);

#[derive(Default)]
struct TraceBridge {
    epoch_ready: bool,
    folder: Option<PathBuf>,
    store: Option<fence_trace::edge::TraceStore>,
    header: Option<fence_trace::edge::TraceHeader>,
    buffer: Vec<fence_types::EdgeRecord>,
    marks: usize,
    runtime: Option<RuntimeState>,
}

impl WinHookHost {
    fn publish_trace_runtime(&mut self, state: RuntimeState) {
        self.1.runtime = Some(state);
        if let Some(header) = &mut self.1.header {
            header.set_runtime(state);
            if let Some(store) = &mut self.1.store {
                store.update_header(header.clone());
            }
        }
    }
    fn resume_trace(&self) {
        let enabled = self.1.epoch_ready && self.1.store.is_some() && self.1.runtime.is_some_and(|s| !s.paused);
        self.0
            .shared()
            .trace_enabled
            .store(enabled, std::sync::atomic::Ordering::Release);
    }
}

fn trace_error(error: std::io::Error) {
    let line = format!("edge trace: {error}");
    fence_win::debug_log(&line);
    if let Some(root) = fence_win::appdata_dir() {
        let _ = fence_win::error_log::append(&root.join("MouseFence/v2/error.log"), "trace", &line);
    }
}

impl HookHost for WinHookHost {
    fn recovery_log(&mut self, message: &str) {
        let context = self
            .0
            .shared()
            .context
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let line = format!("{message} {context}");
        fence_win::debug_log(&line);
        if let Some(root) = fence_win::appdata_dir() {
            let _ = fence_win::error_log::append(&root.join("MouseFence/v2/error.log"), "hook", &line);
        }
    }
    fn recover(&mut self) -> bool {
        self.pause_trace();
        let installed = self.0.restart();
        // Restart retains shared counters, trace ring, and toggles; only the producer thread changes.
        self.1.epoch_ready = self.0.info().is_some();
        if let Some(old) = self.1.runtime {
            self.publish_trace_runtime(RuntimeState {
                hook_installed: installed,
                ..old
            });
        }
        self.resume_trace();
        installed
    }
    fn metrics(&self) -> Option<fence_types::HookMetrics> {
        fence_engine::fixes::Fixes::ALL
            .j26_hook_metrics
            .then(|| self.0.shared().metrics())
    }
    fn pause_trace(&mut self) {
        self.0
            .shared()
            .trace_enabled
            .store(false, std::sync::atomic::Ordering::Release);
        // Commands run on the producer thread. This acknowledgement follows any in-flight callback,
        // so all old-epoch records are published before the old header is drained.
        self.1.epoch_ready = self.0.info().is_some();
        if let Some(old) = self.1.runtime {
            self.drain_trace(old);
        }
    }
    fn configure_trace(&mut self, settings: &fence_config::Settings, monitors: &[MonitorInfo], state: RuntimeState) {
        let shared = self.0.shared();
        shared.trace_enabled.store(false, std::sync::atomic::Ordering::Relaxed);
        self.1.runtime = Some(state);
        if !settings.edge_trace || !fence_engine::fixes::Fixes::ALL.j24_edge_trace {
            self.1.buffer.clear();
            shared.trace.drain(&mut self.1.buffer);
            self.1.buffer.clear();
            self.1.store = None;
            self.1.header = None;
            self.1.marks = 0;
            return;
        }
        let rate = shared.estimated_rate_milli.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1000.0;
        let mut header =
            fence_trace::edge::TraceHeader::from_settings(settings, monitors, if rate > 0.0 { rate } else { 1.0 });
        header.set_runtime(state);
        if let Some(store) = &mut self.1.store {
            store.update_header(header.clone());
        } else if let Some(folder) = &self.1.folder {
            self.1.store = Some(fence_trace::edge::TraceStore::new(folder.clone(), header.clone()));
        }
        self.1.header = Some(header);
        self.resume_trace();
    }
    fn drain_trace(&mut self, state: RuntimeState) -> usize {
        self.1.runtime = Some(state);
        if let Some(store) = &mut self.1.store {
            self.1.buffer.clear();
            self.0.shared().trace.drain(&mut self.1.buffer);
            if let Some(header) = &mut self.1.header {
                let rate = self
                    .0
                    .shared()
                    .estimated_rate_milli
                    .load(std::sync::atomic::Ordering::Relaxed) as f64
                    / 1000.0;
                if rate > 0.0 {
                    header.estimated_rate = rate;
                }
                header.set_runtime(state);
                store.update_header(header.clone());
            }
            if let Err(error) = store.ingest(&self.1.buffer, fence_win::qpc::now_us()) {
                trace_error(error);
            }
            self.1.buffer.clear();
        }
        self.1.marks
    }
    fn mark_accident(&mut self, state: RuntimeState) -> usize {
        self.drain_trace(state);
        if let Some(store) = &mut self.1.store {
            match store.mark_accident(fence_win::qpc::now_us()) {
                Ok(count) => self.1.marks = count,
                Err(error) => trace_error(error),
            }
        }
        self.1.marks
    }
    fn install(&mut self) -> bool {
        self.pause_trace();
        let installed = self.0.install();
        if let Some(old) = self.1.runtime {
            self.publish_trace_runtime(RuntimeState {
                paused: false,
                hook_installed: installed,
                ..old
            });
        }
        self.resume_trace();
        installed
    }
    fn uninstall(&mut self) {
        self.pause_trace();
        self.0.uninstall();
        if let Some(old) = self.1.runtime {
            self.publish_trace_runtime(RuntimeState {
                paused: true,
                hook_installed: false,
                ..old
            });
        }
    }
    fn installed(&self) -> bool {
        self.0.shared().installed()
    }
    fn reset_engine(&mut self) {
        self.0.reset_engine()
    }
    fn set_engine_config(&mut self, config: EngineConfig) {
        self.pause_trace();
        // Built here, on the tray thread; the hook thread only swaps it in.
        self.0.set_engine(Box::new(ClassicEngine::new(config)));
        self.1.epoch_ready &= self.0.info().is_some();
    }
    fn callbacks(&self) -> u64 {
        self.0.shared().counters().callbacks
    }
    fn fault_counts(&self) -> (u64, u64) {
        let c = self.0.shared().counters();
        (c.panics, c.degraded)
    }
    fn cursor_pos(&self) -> Option<(i32, i32)> {
        self.0.cursor_pos()
    }
    fn set_toggles(&mut self, gate_open: bool, side_contain: bool, confine: bool) {
        self.pause_trace();
        self.0.shared().set_toggles(gate_open, side_contain, confine);
        if let Some(old) = self.1.runtime {
            self.publish_trace_runtime(RuntimeState {
                gate_open,
                side_contain,
                confine,
                ..old
            });
        }
        self.resume_trace();
    }
}

/// Starts `mousefence.exe --settings --state ...` and posts its exit code back to the tray window.
pub struct SettingsLauncher {
    pub exe: PathBuf,
    pub notify_hwnd: usize,
}

impl SettingsProcessPort for SettingsLauncher {
    fn launch(&mut self, state: &RuntimeState) -> bool {
        let child = std::process::Command::new(&self.exe)
            .args(launch::settings_args(state))
            .spawn();
        let Ok(mut child) = child else {
            return false;
        };
        let hwnd = self.notify_hwnd;
        let waiter = std::thread::Builder::new()
            .name("mousefence-settings-wait".to_string())
            .spawn(move || {
                let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(launch::EXIT_FAILED);
                fence_win::tray::post_settings_exit(hwnd, code);
            });
        waiter.is_ok()
    }
}

pub fn ports(window: Rc<TrayWindow>, hook: HookThread, store: FileStore, exe: PathBuf) -> Ports {
    Ports {
        tray: Box::new(WinTray(window.clone())),
        hotkeys: Box::new(WinHotkeys(window.clone())),
        guard: Box::new(Guard::new(WinHookHost(
            hook,
            TraceBridge {
                folder: fence_win::local_appdata_dir().map(|p| p.join("MouseFence/v2/traces")),
                ..TraceBridge::default()
            },
        ))),
        system: Box::new(WinSystem {
            error_log: fence_win::appdata_dir().map(|p| p.join("MouseFence/v2/error.log")),
        }),
        autostart: Box::new(WinAutoStart { exe: exe.clone() }),
        timers: Box::new(WinTimers(window.clone())),
        store: Box::new(store),
        settings_process: Box::new(SettingsLauncher {
            exe,
            notify_hwnd: window.hwnd_value(),
        }),
        core: Box::new(EngineCore),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_production_host_restarts_fake_thread_and_preserves_trace_runtime() {
        struct Installer;
        impl fence_win::hook::HookInstaller for Installer {
            fn install(&mut self) -> bool {
                true
            }
            fn uninstall(&mut self) {}
        }
        struct Cursor;
        impl fence_win::hook::CursorPort for Cursor {
            fn cursor_pos(&self) -> Option<(i32, i32)> {
                Some((7, 9))
            }
            fn set_cursor_pos(&mut self, _: i32, _: i32) -> bool {
                panic!("test must never place a cursor")
            }
        }
        let hook = HookThread::spawn(
            Box::new(ClassicEngine::new(EngineConfig::default())),
            Box::new(Installer),
            Box::new(Cursor),
        )
        .unwrap();
        let shared = hook.shared().clone();
        let mut host = WinHookHost(
            hook,
            TraceBridge {
                runtime: Some(RuntimeState {
                    gate_open: true,
                    side_contain: true,
                    paused: false,
                    confine: false,
                    hook_installed: false,
                }),
                ..TraceBridge::default()
            },
        );
        host.set_toggles(true, true, false);
        assert!(host.install());
        assert!(host.recover());
        assert!(std::sync::Arc::ptr_eq(&shared, host.0.shared()));
        assert_eq!(host.cursor_pos(), Some((7, 9)));
        assert!(host.1.epoch_ready);
        let runtime = host.1.runtime.unwrap();
        assert!(runtime.hook_installed && runtime.gate_open && runtime.side_contain && !runtime.paused);
        assert_eq!(shared.metrics().reinstall_attempts, 1);
    }

    #[test]
    fn j24_trace_epoch_flush_preserves_old_policy_layout_and_unpaused_crossing() {
        use fence_types::{EdgeRecord, Rect};
        struct Temp(PathBuf);
        impl Drop for Temp {
            fn drop(&mut self) {
                if !self.0.is_absolute()
                    || !self.0.starts_with(std::env::temp_dir())
                    || !self
                        .0
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with("mf-trace-epoch-"))
                {
                    return;
                }
                if let Ok(entries) = std::fs::read_dir(&self.0) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.parent() == Some(self.0.as_path())
                            && path
                                .file_name()
                                .is_some_and(|n| n.to_string_lossy().starts_with("cross-"))
                        {
                            let _ = std::fs::remove_file(path);
                        }
                    }
                }
                let _ = std::fs::remove_dir(&self.0);
            }
        }
        struct Installer(bool);
        impl fence_win::hook::HookInstaller for Installer {
            fn install(&mut self) -> bool {
                self.0 = true;
                true
            }
            fn uninstall(&mut self) {
                self.0 = false;
            }
        }
        struct Cursor;
        impl fence_win::hook::CursorPort for Cursor {
            fn set_cursor_pos(&mut self, _: i32, _: i32) -> bool {
                panic!("no cursor placement in epoch test")
            }
            fn cursor_pos(&self) -> Option<(i32, i32)> {
                None
            }
        }
        let dir = std::env::temp_dir().join(format!(
            "mf-trace-epoch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _temp = Temp(dir.clone());
        let hook = HookThread::spawn(
            Box::new(ClassicEngine::new(EngineConfig::default())),
            Box::new(Installer(false)),
            Box::new(Cursor),
        )
        .unwrap();
        let mut host = WinHookHost(
            hook,
            TraceBridge {
                folder: Some(dir.clone()),
                ..TraceBridge::default()
            },
        );
        let settings = fence_config::Settings {
            edge_trace: true,
            up_cross_min: 3,
            ..fence_config::Settings::default()
        };
        let monitors = vec![
            MonitorInfo {
                stable_id: "SYN-MAIN".into(),
                device: "DISPLAY1".into(),
                bounds: Rect::new(0, 0, 1000, 1000),
                primary: true,
                index: 1,
            },
            MonitorInfo {
                stable_id: "SYN-TOP".into(),
                device: "DISPLAY2".into(),
                bounds: Rect::new(0, -1000, 1000, 0),
                primary: false,
                index: 2,
            },
        ];
        let state = RuntimeState {
            gate_open: true,
            side_contain: true,
            confine: false,
            paused: false,
            hook_installed: true,
        };
        host.set_engine_config(EngineConfig::default());
        host.configure_trace(&settings, &monitors, state);
        assert!(host.install());
        let now = fence_win::qpc::now_us();
        for (age, y, action) in [(2_000_000, 0, 0), (1_000_000, -10, 2), (400_000, -9, 0)] {
            assert!(host.0.shared().trace.push(EdgeRecord {
                features: [0; 12],
                qpc_us: now - age,
                time_ms: (now / 1000 - age / 1000) as u32,
                x: 500,
                y,
                action,
                model_state: 0
            }));
        }
        host.uninstall();
        let changed = fence_config::Settings {
            up_cross_min: 12,
            ..settings
        };
        let mut changed_monitors = monitors.clone();
        changed_monitors[0].bounds.r = 900;
        host.set_engine_config(EngineConfig::default());
        host.configure_trace(
            &changed,
            &changed_monitors,
            RuntimeState {
                paused: true,
                hook_installed: false,
                ..state
            },
        );
        host.drain_trace(RuntimeState {
            paused: true,
            hook_installed: false,
            ..state
        });
        let paths: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|p| p.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
            .collect();
        assert_eq!(paths.len(), 1);
        let text = std::fs::read_to_string(&paths[0]).unwrap();
        let header: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!(header["settings"]["UpCrossMin"], 3);
        assert_eq!(header["layout"][0]["rect"][2], 1000);
        assert_eq!(header["paused"], false, "the queued crossing happened before pause");
        assert!(host.1.epoch_ready);
        assert!(!host.0.shared().trace_enabled.load(std::sync::atomic::Ordering::Acquire));
        drop(host);
        assert!(
            dir.is_absolute()
                && dir.starts_with(std::env::temp_dir())
                && dir
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("mf-trace-epoch-")
        );
        for path in paths {
            assert_eq!(path.parent(), Some(dir.as_path()));
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn f08_tray_log_port_persists_probe_faults_in_a_temporary_log() {
        let dir = std::env::temp_dir().join(format!(
            "mf-tray-log-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let path = dir.join("error.log");
        let system = WinSystem {
            error_log: Some(path.clone()),
        };
        system.log("hook: 1 contained panic, 1 degraded");
        assert!(std::fs::read_to_string(&path).unwrap().contains("1 contained panic"));
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }

    #[test]
    fn j21_native_menu_displays_shortcuts_in_the_right_column() {
        let mut v = TrayView::initial();
        v.gate.text = "Upward passage".into();
        v.gate.shortcut = "Ctrl+Alt+F24".into();
        assert!(matches!(&native_menu(&v)[0],NativeMenuEntry::Item{text,..} if text=="Upward passage\tCtrl+Alt+F24"));
    }

    #[test]
    fn s_app_window_events_map_to_controller_events() {
        assert_eq!(to_event(WindowEvent::TrayDoubleClick), Some(Event::TrayDoubleClick));
        for c in MenuCommand::ALL {
            assert_eq!(to_event(WindowEvent::MenuCommand(c.id())), Some(Event::Menu(c)));
        }
        assert_eq!(to_event(WindowEvent::MenuCommand(0)), None);
        for s in HotkeySlot::ALL {
            assert_eq!(to_event(WindowEvent::Hotkey(s.id())), Some(Event::Hotkey(s)));
        }
        assert_eq!(HotkeySlot::Gate.id(), fence_win::hotkeys::HOTKEY_ID_BASE);
        assert_eq!(to_event(WindowEvent::Hotkey(1)), None);
        for t in [TimerId::Reconfigure, TimerId::HookProbe] {
            assert_eq!(to_event(WindowEvent::Timer(t.id())), Some(Event::Timer(t)));
        }
        assert_eq!(to_event(WindowEvent::Timer(99)), None);
        assert_eq!(to_event(WindowEvent::DisplayChanged), Some(Event::DisplayChanged));
        assert_eq!(to_event(WindowEvent::SettingsExited(2)), Some(Event::SettingsExited(2)));
    }

    #[test]
    fn s_app_native_menu_follows_the_view() {
        let mut v = TrayView::initial();
        v.gate.text = "g".into();
        v.gate.checked = true;
        v.side.enabled = false;
        let m = native_menu(&v);
        assert_eq!(m.len(), 7);
        assert_eq!(
            m[0],
            NativeMenuEntry::Item {
                id: MenuCommand::Gate.id(),
                text: "g".into(),
                checked: true,
                enabled: true
            }
        );
        assert!(matches!(m[1], NativeMenuEntry::Item { enabled: false, .. }));
        assert_eq!(m[4], NativeMenuEntry::Separator);
        assert!(matches!(m[6], NativeMenuEntry::Item { id, .. } if id == MenuCommand::Exit.id()));
        assert_eq!(icon_kind(TrayIcon::Paused), TrayIconKind::Paused);
    }

    #[test]
    fn s_app_a_missing_settings_exe_reports_a_failed_launch() {
        // Nothing starts: the path does not exist. No window, no process.
        let mut l = SettingsLauncher {
            exe: std::env::temp_dir()
                .join("MouseFenceTest-missing")
                .join("no-such-mousefence.exe"),
            notify_hwnd: 0,
        };
        let state = RuntimeState {
            gate_open: false,
            side_contain: true,
            confine: false,
            paused: false,
            hook_installed: true,
        };
        assert!(!l.launch(&state));
    }
}
