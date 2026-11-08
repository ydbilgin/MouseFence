//! The tray controller: C# `TrayApplicationContext` statement for statement, minus WinForms.
//!
//! The host (apps/mousefence) creates the ports, calls [`Controller::start`] once, then feeds every native
//! event through [`Controller::handle`] from its message loop, outside the window procedure. Differences
//! from 1.1.1 are the PLAN's: the settings window is a separate process (T16, B029), and the J rows of
//! docs/rust2/parity.tsv, each in its own commit.

use fence_config::{LoadSource, Settings};
use fence_i18n::Lang;
use fence_types::{HookHealthVerdict, Hotkey, MonitorInfo, RuntimeState, launch};

use crate::{
    AutoStartPort, Balloon, BalloonKind, CorePort, GuardPort, HotkeyPort, HotkeySlot, MenuCommand, SettingsProcessPort,
    StorePort, SystemPort, TimerId, TimerPort, TrayIcon, TrayPort, TrayView,
};

/// C# `_reconfigTimer.Interval`.
pub const RECONFIGURE_DEBOUNCE_MS: u32 = 750;
/// C# `HookProbeIntervalMs`.
pub const HOOK_PROBE_INTERVAL_MS: u32 = 3000;

/// Every port the controller talks through.
pub struct Ports {
    pub tray: Box<dyn TrayPort>,
    pub hotkeys: Box<dyn HotkeyPort>,
    pub guard: Box<dyn GuardPort>,
    pub system: Box<dyn SystemPort>,
    pub autostart: Box<dyn AutoStartPort>,
    pub timers: Box<dyn TimerPort>,
    pub store: Box<dyn StorePort>,
    pub settings_process: Box<dyn SettingsProcessPort>,
    pub core: Box<dyn CorePort>,
}

/// One native event, already on the UI thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// C# `_tray.DoubleClick`.
    TrayDoubleClick,
    Menu(MenuCommand),
    Hotkey(HotkeySlot),
    /// C# `SystemEvents.DisplaySettingsChanged` (WM_DISPLAYCHANGE on the tray window).
    DisplayChanged,
    Timer(TimerId),
    /// The settings process ended with this exit code.
    SettingsExited(i32),
}

/// Whether the host keeps its loop running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Exit,
}

pub struct Controller {
    ports: Ports,
    settings: Settings,
    lang: Lang,
    gate_open: bool,
    side_contain: bool,
    confine: bool,
    hook_outage_reported: bool,
    /// Hook fault counters already written to the log (contained panics, failed placements).
    hook_faults_logged: (u64, u64),
    /// Stable-keyed latch: warn once per bad layout, re-arm when it heals.
    isolated_devices: Vec<String>,
    /// Did the pause hotkey actually register? (for the escape-key hint)
    pause_registered: bool,
    view: TrayView,
    display_subscribed: bool,
    settings_process_open: bool,
    exited: bool,
    trace_marks: usize,
    trace_timer_active: bool,
}

impl Controller {
    pub fn new(ports: Ports) -> Self {
        Self {
            ports,
            settings: Settings::default(),
            lang: Lang::En,
            gate_open: false,
            side_contain: false,
            confine: false,
            hook_outage_reported: false,
            hook_faults_logged: (0, 0),
            isolated_devices: Vec::new(),
            pause_registered: false,
            view: TrayView::initial(),
            display_subscribed: false,
            settings_process_open: false,
            exited: false,
            trace_marks: 0,
            trace_timer_active: false,
        }
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn lang(&self) -> Lang {
        self.lang
    }

    pub fn view(&self) -> &TrayView {
        &self.view
    }

    pub fn settings_process_open(&self) -> bool {
        self.settings_process_open
    }

    /// The live switches (the `--state` argument and state.txt for the settings process).
    pub fn runtime_state(&self) -> RuntimeState {
        RuntimeState {
            gate_open: self.gate_open,
            side_contain: self.side_contain,
            confine: self.confine,
            paused: !self.ports.guard.enabled(),
            hook_installed: self.ports.guard.hook_installed(),
        }
    }

    // ---- text helpers ----

    fn t(&self, key: &str) -> String {
        fence_i18n::get(self.lang, key).to_string()
    }

    fn tf(&self, key: &str, args: &[&str]) -> String {
        fence_i18n::format(self.lang, key, args)
    }

    fn balloon(&mut self, millis: u32, kind: BalloonKind, text: String) {
        self.ports.tray.balloon(&Balloon { millis, kind, text });
    }

    fn log(&self, line: &str) {
        self.ports.system.log(line);
    }

    fn apply_language(&mut self) {
        self.lang = fence_i18n::resolve(&self.settings.language, self.ports.system.os_ui_is_turkish());
    }

    // ---- startup (C# constructor, Ui/TrayApplicationContext.cs:47-98) ----

    /// Load the settings and bring the tray up, in the 1.x order.
    pub fn start(&mut self) {
        let loaded = self.ports.store.load();
        match &loaded.source {
            LoadSource::V2 | LoadSource::ImportedV1 | LoadSource::Fresh => {}
            LoadSource::LastGood { problem } => self.log(&format!("settings: using last-good copy ({problem})")),
            LoadSource::DefaultsNotSaved { problem } => self.log(&format!(
                "settings: unreadable, running on unsaved defaults ({problem})"
            )),
        }
        self.settings = loaded.settings;
        self.apply_language();

        self.ports.tray.add_icon(TrayIcon::Closed, "MouseFence");
        if matches!(loaded.source, LoadSource::DefaultsNotSaved { .. }) {
            // 2.0 (J04, JUDGE-C ruling 2): say it in words once the icon exists; 1.1.1 silently used defaults.
            let text = self.t("TipSettingsUnreadable");
            self.balloon(8000, BalloonKind::Warning, text);
        }

        self.register_hotkeys(); // before configure() so pause_registered is known for the first isolation warning
        self.configure();
        self.apply_startup_choices();

        self.display_subscribed = true;
        self.ports.timers.start(TimerId::HookProbe, HOOK_PROBE_INTERVAL_MS);

        self.rebuild_menu_text();
        self.update_ui();
        self.warn_intel_rotation();
    }

    /// J02 (C# `GuardStartup.Apply`, commit 16c1753): gate, side and confine from the saved choices, then the
    /// hook starts unless StartPaused is on. 1.1.1 set only gate and side and always started, so
    /// StartConfineOn and StartPaused were saved and shown in Settings but never applied.
    fn apply_startup_choices(&mut self) {
        self.set_gate(!self.settings.start_gate_closed);
        self.set_side(self.settings.start_side_contain_on);
        self.set_confine(self.settings.start_confine_on);
        if !self.settings.start_paused {
            self.ports.guard.start();
        }
    }

    fn set_gate(&mut self, on: bool) {
        self.gate_open = on;
        self.ports.guard.set_gate_open(on);
    }

    fn set_side(&mut self, on: bool) {
        self.side_contain = on;
        self.ports.guard.set_side_contain(on);
    }

    fn set_confine(&mut self, on: bool) {
        self.confine = on;
        self.ports.guard.set_confine(on);
    }

    // ---- events ----

    pub fn handle(&mut self, event: Event) -> Flow {
        if self.exited {
            return Flow::Exit;
        }
        match event {
            Event::TrayDoubleClick => self.toggle_gate(),
            Event::Menu(cmd) => match cmd {
                MenuCommand::Gate => self.toggle_gate(),
                MenuCommand::Side => self.toggle_side(),
                MenuCommand::Game => self.toggle_game(),
                MenuCommand::Pause => self.toggle_pause(),
                MenuCommand::Settings => self.open_settings(),
                MenuCommand::Exit => {
                    self.exit();
                    return Flow::Exit;
                }
            },
            Event::Hotkey(slot) => match slot {
                HotkeySlot::Gate => self.toggle_gate(),
                HotkeySlot::Confine => self.toggle_game(),
                HotkeySlot::Pause => self.toggle_pause(),
                HotkeySlot::Side => self.toggle_side(),
                HotkeySlot::MarkAccident => {
                    if self.settings.edge_trace && fence_engine::fixes::Fixes::ALL.j24_edge_trace {
                        let state = self.runtime_state();
                        self.trace_marks = self.ports.guard.mark_accident(state);
                        self.update_ui();
                    }
                }
            },
            Event::DisplayChanged => self.on_display_settings_changed(),
            Event::Timer(TimerId::Reconfigure) => {
                self.ports.timers.stop(TimerId::Reconfigure);
                self.configure();
                self.update_ui();
            }
            Event::Timer(TimerId::HookProbe) => self.on_hook_probe(),
            Event::Timer(TimerId::TraceDrain) => {
                let state = self.runtime_state();
                self.trace_marks = self.ports.guard.drain_trace(state);
            }
            Event::SettingsExited(code) => self.on_settings_exited(code),
        }
        Flow::Continue
    }

    /// One change often raises several events: restart the 750 ms one-shot timer (debounce).
    fn on_display_settings_changed(&mut self) {
        if !self.display_subscribed {
            return;
        }
        self.ports.timers.stop(TimerId::Reconfigure);
        self.ports.timers.start(TimerId::Reconfigure, RECONFIGURE_DEBOUNCE_MS);
    }

    /// 2.0 (JUDGE-C S1): the J07/J08 counters reach the log on the 3 s probe when they changed, so a fail-open
    /// or a contained panic during his trial leaves a trace. No balloon: the move already passed safely.
    fn log_hook_faults(&mut self) {
        let now = self.ports.guard.fault_counts();
        if now == self.hook_faults_logged {
            return;
        }
        self.hook_faults_logged = now;
        self.log(&format!(
            "hook: {} contained panic(s), {} failed cursor placement(s) let through, since the hook started",
            now.0, now.1
        ));
    }

    /// Liveness probe: a Dead verdict has already retried the install; announce one outage once.
    fn on_hook_probe(&mut self) {
        self.ports.tray.probe_notifications();
        if let Some(m) = self.ports.guard.metrics() {
            self.log(&format!(
            "hook metrics since start: callbacks={} sampled={} max_us={} p99_upper_us={} reinstall_attempts={} degraded={} panics={} trace_dropped={}",
            m.callbacks,m.sampled_steps,m.max_step_us,m.p99_upper_us,m.reinstall_attempts,m.degraded,m.panics,m.trace_dropped));
        }
        self.log_hook_faults();
        if !self.ports.guard.enabled() {
            return; // deliberately paused: nothing to keep alive
        }
        let verdict = self.ports.guard.check_health();
        if verdict == HookHealthVerdict::Inconclusive {
            return;
        }
        if verdict == HookHealthVerdict::Alive {
            self.hook_outage_reported = false; // re-arm, so a later outage is announced again
            return;
        }
        self.update_ui();
        if self.hook_outage_reported {
            return; // same outage (or a failed retry): don't nag every tick
        }
        self.hook_outage_reported = true;
        if self.ports.guard.hook_installed() {
            let text = self.t("TipGuardRestored");
            self.balloon(4000, BalloonKind::Info, text);
        } else {
            let text = self.t("TipGuardLost");
            self.balloon(8000, BalloonKind::Warning, text);
        }
    }

    // ---- Configure (Ui/TrayApplicationContext.cs:134-247) ----

    fn configure(&mut self) {
        fence_config::normalize(&mut self.settings);
        let monitors = self.ports.system.monitors();
        let policy = fence_config::to_policy(&self.settings);
        let (config, report) = self.ports.core.plan(&policy, &monitors);
        self.ports.guard.configure(config);
        let state = self.runtime_state();
        self.ports.guard.configure_trace(&self.settings, &monitors, state);
        let tracing = self.settings.edge_trace && fence_engine::fixes::Fixes::ALL.j24_edge_trace;
        if tracing && !self.trace_timer_active {
            self.ports.timers.start(TimerId::TraceDrain, 2000);
            self.trace_timer_active = true;
        } else if !tracing && self.trace_timer_active {
            self.ports.timers.stop(TimerId::TraceDrain);
            self.trace_timer_active = false;
        }

        if report.has_safety_demotions {
            let text = self.t("GeometrySealDemoted");
            self.balloon(5000, BalloonKind::Info, text);
        }
        if report.pruned {
            let text = self.t("ExcludePruned");
            self.balloon(5000, BalloonKind::Warning, text);
        }
        let labels: Vec<String> = report
            .isolated
            .iter()
            .enumerate()
            .map(|(i, id)| self.monitor_reference(id, report.isolated_indices.get(i).copied(), &monitors))
            .collect();
        self.warn_isolation_if_changed(report.isolated.clone(), labels);
    }

    /// Warn (once per distinct bad layout) when a screen is isolated; latch on the stable ids.
    fn warn_isolation_if_changed(&mut self, devices: Vec<String>, labels: Vec<String>) {
        let mut now = devices.clone();
        now.sort();
        let mut before = self.isolated_devices.clone();
        before.sort();
        let changed = now != before;
        self.isolated_devices = devices;
        if !self.isolated_devices.is_empty() && changed {
            let pause = self.pause_key_label();
            let text = self.tf("TipIsolated", &[&labels.join(", "), &pause]);
            self.balloon(5000, BalloonKind::Warning, text);
        }
    }

    /// C# `MonitorReference`: "index" or "index — name".
    fn monitor_reference(&self, stable_id: &str, index: Option<u32>, monitors: &[MonitorInfo]) -> String {
        let index = index
            .or_else(|| monitors.iter().find(|m| m.stable_id == stable_id).map(|m| m.index))
            .unwrap_or(0);
        let name = self.settings.monitor_name(stable_id);
        if name.is_empty() {
            index.to_string()
        } else {
            format!("{index} — {name}")
        }
    }

    fn pause_key_label(&self) -> String {
        let pause = self.settings.pause_hotkey();
        if !pause.is_set() {
            return hotkey_text(&pause); // "(none)": intentionally cleared
        }
        if self.pause_registered {
            hotkey_text(&pause)
        } else {
            self.t("NoneRegistered")
        }
    }

    // ---- hotkeys (Ui/TrayApplicationContext.cs:273-286) ----

    fn hotkey_fail(&mut self, hotkey: &Hotkey) {
        let text = self.tf("TipHotkeyFail", &[&hotkey_text(hotkey)]);
        self.balloon(3000, BalloonKind::Warning, text);
    }

    fn register_hotkeys(&mut self) {
        let gate = self.settings.hotkey();
        if !self.ports.hotkeys.register(HotkeySlot::Gate, gate) {
            self.hotkey_fail(&gate);
        }
        let confine = self.settings.confine_hotkey();
        if (confine.is_set() || !fence_engine::fixes::Fixes::ALL.j16_shortcut_validation)
            && !self.ports.hotkeys.register(HotkeySlot::Confine, confine)
        {
            self.hotkey_fail(&confine);
        }
        let side = self.settings.side_hotkey();
        if side.is_set() && !self.ports.hotkeys.register(HotkeySlot::Side, side) {
            self.hotkey_fail(&side);
        }
        let pause = self.settings.pause_hotkey();
        self.pause_registered = self.ports.hotkeys.register(HotkeySlot::Pause, pause);
        if pause.is_set() && !self.pause_registered {
            // only nag if a key was set but couldn't register (not when cleared)
            self.hotkey_fail(&pause);
        }
        if self.settings.edge_trace && fence_engine::fixes::Fixes::ALL.j24_edge_trace {
            let mark = self.settings.mark_accident_hotkey();
            if mark.is_set() && !self.ports.hotkeys.register(HotkeySlot::MarkAccident, mark) {
                self.hotkey_fail(&mark);
            }
        }
    }

    fn rebuild_menu_text(&mut self) {
        self.view.settings.text = self.t("MenuSettings");
        self.view.exit.text = self.t("MenuExit");
        self.view.game.text = self.t("MenuGame");
    }

    // ---- toggles (Ui/TrayApplicationContext.cs:295-333) ----

    fn toggle_game(&mut self) {
        self.set_confine(!self.confine);
        self.update_ui();
        if self.ports.guard.enabled() {
            let text = self.t(if self.confine { "TipGameOn" } else { "TipGameOff" });
            self.balloon(1200, BalloonKind::Info, text);
        }
    }

    fn toggle_gate(&mut self) {
        self.set_gate(!self.gate_open);
        self.update_ui();
        if self.ports.guard.enabled() {
            let text = self.t(if self.gate_open { "TipOpen" } else { "TipClosed" });
            self.balloon(1000, BalloonKind::Info, text);
        }
    }

    fn toggle_side(&mut self) {
        self.set_side(!self.side_contain);
        self.update_ui();
        if self.ports.guard.enabled() {
            let text = self.t(if self.side_contain { "TipSideOn" } else { "TipSideOff" });
            self.balloon(1000, BalloonKind::Info, text);
        }
    }

    fn toggle_pause(&mut self) {
        self.ports.guard.pause_trace();
        if self.ports.guard.enabled() {
            self.ports.guard.stop();
        } else {
            self.ports.guard.start();
        }
        self.hook_outage_reported = false; // a manual pause/resume cycle re-arms the outage warning
        self.update_ui();
        // A keyboard user who pauses to escape a trap can't see the grey icon: confirm with a balloon. If a
        // resume couldn't install the hook, say THAT instead of claiming the barrier is back on.
        if !self.ports.guard.enabled() {
            let text = self.t("TipPaused");
            self.balloon(1200, BalloonKind::Info, text);
        } else if self.ports.guard.hook_installed() {
            let text = self.t("TipResumed");
            self.balloon(1200, BalloonKind::Info, text);
        } else {
            let text = self.t("TipGuardLost");
            self.balloon(8000, BalloonKind::Warning, text);
        }
    }

    // ---- UpdateUi (Ui/TrayApplicationContext.cs:335-378) ----

    fn update_ui(&mut self) {
        let hk = hotkey_text(&self.settings.hotkey());
        if !self.ports.guard.enabled() {
            self.view.icon = TrayIcon::Paused;
            self.view.tooltip = self.tf("TrayPaused", &[&hk]);
            self.view.gate.text = self.t("GatePausedMenu");
            self.view.gate.enabled = false;
            self.view.side.text = self.t("SideGatePausedMenu");
            self.view.side.enabled = false;
            self.view.game.enabled = false;
            self.view.pause.text = self.t("MenuResume");
            self.view.pause.checked = true;
        } else {
            self.view.gate.enabled = true;
            self.view.side.enabled = true;
            self.view.pause.text = self.t("MenuPause");
            self.view.pause.checked = false;

            let open = self.gate_open;
            self.view.icon = if open { TrayIcon::Open } else { TrayIcon::Closed };
            self.view.tooltip = self.tf(if open { "TrayOpen" } else { "TrayClosed" }, &[&hk]);
            self.view.gate.text = self.t(if open { "GateOpenMenu" } else { "GateClosedMenu" });
            self.view.gate.checked = open;

            let side_on = self.side_contain;
            self.view.side.text = self.t(if side_on {
                "SideContainOnMenu"
            } else {
                "SideContainOffMenu"
            });
            self.view.side.checked = side_on;

            self.view.game.checked = self.confine;
            self.view.game.enabled = true;

            // Honesty gate: running with no hook installed means the fence is down, whatever the toggles say.
            if !self.ports.guard.hook_installed() {
                self.view.icon = TrayIcon::Paused;
                self.view.tooltip = self.tf("TrayGuardLost", &[&hk]);
            }
        }
        if fence_engine::fixes::Fixes::ALL.j21_ui_keepers {
            self.view.gate.text = self.t(if self.ports.guard.enabled() {
                "HotkeyHead"
            } else {
                "GatePausedMenu"
            });
            self.view.side.text = self.t(if self.ports.guard.enabled() {
                "SideHotkeyHead"
            } else {
                "SideGatePausedMenu"
            });
            self.view.game.text = self.t("MenuGame");
            self.view.pause.text = self.t("MenuPause");
            for (item, hotkey) in [
                (&mut self.view.gate, self.settings.hotkey()),
                (&mut self.view.side, self.settings.side_hotkey()),
                (&mut self.view.game, self.settings.confine_hotkey()),
                (&mut self.view.pause, self.settings.pause_hotkey()),
            ] {
                item.shortcut = if hotkey.vk == 0 {
                    String::new()
                } else {
                    hotkey_text(&hotkey)
                };
            }
        }
        if self.settings.edge_trace && fence_engine::fixes::Fixes::ALL.j24_edge_trace {
            self.view.tooltip = format!(
                "{} • {}",
                self.view.tooltip,
                self.tf("TraceMarks", &[&self.trace_marks.to_string()])
            );
        }
        self.ports.tray.show(&self.view);
        if self.settings_process_open {
            let state = self.runtime_state();
            self.ports.store.write_state(&state);
        }
    }

    // ---- settings (C# OpenSettings, Ui/TrayApplicationContext.cs:380-399; PLAN T16) ----

    fn open_settings(&mut self) {
        if self.settings_process_open {
            self.log("settings: the window is already open");
            return;
        }
        let state = self.runtime_state();
        self.ports.store.write_state(&state);
        if self.ports.settings_process.launch(&state) {
            self.settings_process_open = true;
        } else {
            self.log("settings: the settings process could not start");
        }
    }

    fn on_settings_exited(&mut self, code: i32) {
        self.settings_process_open = false;
        match code {
            launch::EXIT_SAVED => match self.ports.store.reload_saved() {
                Ok(saved) => self.apply_saved_settings(saved),
                Err(problem) => self.log(&format!("settings: saved file could not be reloaded ({problem})")),
            },
            launch::EXIT_CANCELLED => {}
            other => self.log(&format!("settings: the settings process failed (exit code {other})")),
        }
    }

    /// C# after `ShowDialog() == OK`. The settings process already saved the file (fence-config `save`,
    /// atomic, validated), so the tray adopts the reloaded copy instead of writing it again.
    fn apply_saved_settings(&mut self, saved: Settings) {
        if self.settings.edge_trace && fence_engine::fixes::Fixes::ALL.j24_edge_trace {
            self.ports.hotkeys.unregister(HotkeySlot::MarkAccident);
        }
        self.ports.hotkeys.unregister(HotkeySlot::Gate);
        self.ports.hotkeys.unregister(HotkeySlot::Confine);
        self.ports.hotkeys.unregister(HotkeySlot::Pause);
        self.ports.hotkeys.unregister(HotkeySlot::Side);
        self.settings = saved;
        self.apply_language();
        self.ports.autostart.apply(self.settings.auto_start);
        self.register_hotkeys(); // before configure() so pause_registered is fresh for any isolation warning
        self.configure();
        self.rebuild_menu_text();
        self.update_ui();
        self.warn_intel_rotation();
    }

    // ---- Intel warning (Ui/TrayApplicationContext.cs:405-432) ----

    fn warn_intel_rotation(&mut self) {
        if !self.ports.system.has_intel_gpu() {
            return;
        }
        if self.settings.intel_defaults_applied {
            let text = self.tf(
                "TipIntelDefaults",
                &[
                    &hotkey_text(&self.settings.hotkey()),
                    &hotkey_text(&self.settings.side_hotkey()),
                ],
            );
            self.balloon(8000, BalloonKind::Info, text);
            return;
        }
        let mut conflicts = Vec::new();
        for hk in [
            self.settings.hotkey(),
            self.settings.side_hotkey(),
            self.settings.confine_hotkey(),
            self.settings.pause_hotkey(),
        ] {
            if self.ports.core.is_arrow_rotation_hotkey(&hk) {
                conflicts.push(hotkey_text(&hk));
            }
        }
        if !conflicts.is_empty() {
            let text = self.tf("TipIntelRotation", &[&conflicts.join(", ")]);
            self.balloon(8000, BalloonKind::Warning, text);
        }
    }

    // ---- exit (Ui/TrayApplicationContext.cs:434-452) ----

    fn exit(&mut self) {
        if self.trace_timer_active {
            self.ports.timers.stop(TimerId::TraceDrain);
        }
        if self.settings.edge_trace && fence_engine::fixes::Fixes::ALL.j24_edge_trace {
            let state = self.runtime_state();
            self.ports.guard.drain_trace(state);
            self.ports.hotkeys.unregister(HotkeySlot::MarkAccident);
        }
        self.display_subscribed = false;
        self.ports.timers.stop(TimerId::Reconfigure);
        self.ports.timers.stop(TimerId::HookProbe);
        self.ports.tray.remove_icon();
        self.ports.guard.stop();
        self.ports.hotkeys.unregister(HotkeySlot::Gate);
        self.ports.hotkeys.unregister(HotkeySlot::Confine);
        self.ports.hotkeys.unregister(HotkeySlot::Pause);
        self.ports.hotkeys.unregister(HotkeySlot::Side);
        self.exited = true;
    }
}

/// C# `HotKeyText()` and siblings (fence-config).
fn hotkey_text(hotkey: &Hotkey) -> String {
    fence_config::hotkey_text(hotkey)
}
