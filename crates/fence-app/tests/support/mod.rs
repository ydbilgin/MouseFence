//! Fake ports for the controller tests. Every fake writes into one shared [`World`], so a test can assert
//! the order of calls across ports (startup order, the settings round trip, exit) and drive a fake clock.
//! Nothing here touches Windows: no hook, no tray icon, no hotkey, no registry, no file.
#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use fence_app::{
    AutoStartPort, Balloon, Controller, CorePort, Event, Flow, GuardPort, HotkeyPort, HotkeySlot, Ports,
    SettingsProcessPort, StorePort, SystemPort, TimerId, TimerPort, TrayIcon, TrayPort, TrayView,
};
use fence_config::{LoadSource, Loaded, Settings};
use fence_i18n::Lang;
use fence_types::{EngineConfig, HookHealthVerdict, Hotkey, MonitorInfo, PlanReport, Policy, Rect, RuntimeState};

#[derive(Debug)]
pub struct World {
    pub log: Vec<String>,
    pub views: Vec<TrayView>,
    pub balloons: Vec<Balloon>,
    pub icon_added: Option<(TrayIcon, String)>,
    // hotkeys
    pub registered: HashMap<HotkeySlot, Hotkey>,
    pub refuse: HashSet<HotkeySlot>,
    // guard (C# MouseGuard over a fake hook)
    pub enabled: bool,
    pub installed: bool,
    pub install_ok: bool,
    pub reinstall_ok: bool,
    pub verdicts: VecDeque<HookHealthVerdict>,
    /// (contained panics, failed placements) the fake hook reports (J08, J07).
    pub faults: (u64, u64),
    pub tracing: bool,
    pub trace_marks: usize,
    pub notification_probes: usize,
    pub metrics: Option<fence_types::HookMetrics>,
    pub gate: bool,
    pub side: bool,
    pub confine: bool,
    pub configs: usize,
    // system
    pub monitors: Vec<MonitorInfo>,
    pub intel: bool,
    pub turkish: bool,
    pub logs: Vec<String>,
    // autostart
    pub autostart: Vec<bool>,
    // fake clock and timers: id -> (deadline, interval)
    pub now: u64,
    pub timers: HashMap<TimerId, (u64, u32)>,
    // store
    pub load: Option<Loaded>,
    pub reload: VecDeque<Result<Settings, String>>,
    pub states: Vec<RuntimeState>,
    // settings process
    pub launch_ok: bool,
    pub launches: Vec<RuntimeState>,
    // core
    pub reports: VecDeque<PlanReport>,
    pub rotation: Vec<Hotkey>,
}

impl Default for World {
    fn default() -> Self {
        Self {
            log: Vec::new(),
            views: Vec::new(),
            balloons: Vec::new(),
            icon_added: None,
            registered: HashMap::new(),
            refuse: HashSet::new(),
            enabled: false,
            installed: false,
            install_ok: true,
            reinstall_ok: true,
            verdicts: VecDeque::new(),
            faults: (0, 0),
            tracing: false,
            trace_marks: 0,
            notification_probes: 0,
            metrics: None,
            gate: false,
            side: false,
            confine: false,
            configs: 0,
            monitors: vec![synthetic_monitor(1, "SYN-MAIN", Rect::new(0, 0, 1920, 1080), true)],
            intel: false,
            turkish: false,
            logs: Vec::new(),
            autostart: Vec::new(),
            now: 0,
            timers: HashMap::new(),
            load: None,
            reload: VecDeque::new(),
            states: Vec::new(),
            launch_ok: true,
            launches: Vec::new(),
            reports: VecDeque::new(),
            rotation: Vec::new(),
        }
    }
}

pub type W = Rc<RefCell<World>>;

pub fn synthetic_monitor(index: u32, id: &str, bounds: Rect, primary: bool) -> MonitorInfo {
    MonitorInfo {
        device: format!(r"\\.\DISPLAY{index}"),
        stable_id: id.to_string(),
        bounds,
        primary,
        index,
    }
}

fn push(w: &W, line: String) {
    w.borrow_mut().log.push(line);
}

pub struct FakeTray(pub W);
impl TrayPort for FakeTray {
    fn probe_notifications(&mut self) {
        self.0.borrow_mut().notification_probes += 1;
    }
    fn add_icon(&mut self, icon: TrayIcon, tooltip: &str) {
        push(&self.0, format!("tray.add_icon({icon:?},{tooltip})"));
        self.0.borrow_mut().icon_added = Some((icon, tooltip.to_string()));
    }
    fn show(&mut self, view: &TrayView) {
        push(&self.0, "tray.show".into());
        self.0.borrow_mut().views.push(view.clone());
    }
    fn balloon(&mut self, balloon: &Balloon) {
        push(&self.0, format!("tray.balloon({},{:?})", balloon.millis, balloon.kind));
        self.0.borrow_mut().balloons.push(balloon.clone());
    }
    fn remove_icon(&mut self) {
        push(&self.0, "tray.remove_icon".into());
        self.0.borrow_mut().icon_added = None;
    }
}

pub struct FakeHotkeys(pub W);
impl HotkeyPort for FakeHotkeys {
    fn register(&mut self, slot: HotkeySlot, hotkey: Hotkey) -> bool {
        push(&self.0, format!("hotkeys.register({slot:?})"));
        let mut w = self.0.borrow_mut();
        w.registered.remove(&slot);
        if !hotkey.is_set() || w.refuse.contains(&slot) {
            return false;
        }
        w.registered.insert(slot, hotkey);
        true
    }
    fn unregister(&mut self, slot: HotkeySlot) {
        push(&self.0, format!("hotkeys.unregister({slot:?})"));
        self.0.borrow_mut().registered.remove(&slot);
    }
}

pub struct FakeGuard(pub W);
impl GuardPort for FakeGuard {
    fn metrics(&self) -> Option<fence_types::HookMetrics> {
        self.0.borrow().metrics
    }
    fn configure_trace(&mut self, s: &Settings, _: &[MonitorInfo], _: RuntimeState) {
        self.0.borrow_mut().tracing = s.edge_trace;
    }
    fn drain_trace(&mut self, _: RuntimeState) -> usize {
        let w = self.0.borrow();
        w.trace_marks
    }
    fn mark_accident(&mut self, _: RuntimeState) -> usize {
        let mut w = self.0.borrow_mut();
        if w.tracing {
            w.trace_marks += 1;
        }
        w.trace_marks
    }
    fn configure(&mut self, _config: EngineConfig) {
        push(&self.0, "guard.configure".into());
        self.0.borrow_mut().configs += 1;
    }
    fn start(&mut self) {
        push(&self.0, "guard.start".into());
        let mut w = self.0.borrow_mut();
        w.enabled = true;
        if !w.installed {
            w.installed = w.install_ok;
        }
    }
    fn stop(&mut self) {
        push(&self.0, "guard.stop".into());
        let mut w = self.0.borrow_mut();
        w.enabled = false;
        w.installed = false;
    }
    fn enabled(&self) -> bool {
        self.0.borrow().enabled
    }
    fn hook_installed(&self) -> bool {
        self.0.borrow().installed
    }
    fn set_gate_open(&mut self, on: bool) {
        push(&self.0, format!("guard.gate({on})"));
        self.0.borrow_mut().gate = on;
    }
    fn set_side_contain(&mut self, on: bool) {
        push(&self.0, format!("guard.side({on})"));
        self.0.borrow_mut().side = on;
    }
    fn set_confine(&mut self, on: bool) {
        push(&self.0, format!("guard.confine({on})"));
        self.0.borrow_mut().confine = on;
    }
    fn check_health(&mut self) -> HookHealthVerdict {
        let mut w = self.0.borrow_mut();
        let v = w.verdicts.pop_front().unwrap_or(HookHealthVerdict::Inconclusive);
        if v == HookHealthVerdict::Dead {
            w.installed = w.reinstall_ok;
        }
        w.log.push(format!("guard.check_health({v:?})"));
        v
    }
    fn fault_counts(&self) -> (u64, u64) {
        self.0.borrow().faults
    }
}

pub struct FakeSystem(pub W);
impl SystemPort for FakeSystem {
    fn monitors(&self) -> Vec<MonitorInfo> {
        push(&self.0, "system.monitors".into());
        self.0.borrow().monitors.clone()
    }
    fn has_intel_gpu(&self) -> bool {
        push(&self.0, "system.intel".into());
        self.0.borrow().intel
    }
    fn os_ui_is_turkish(&self) -> bool {
        self.0.borrow().turkish
    }
    fn log(&self, line: &str) {
        self.0.borrow_mut().logs.push(line.to_string());
    }
}

pub struct FakeAutoStart(pub W);
impl AutoStartPort for FakeAutoStart {
    fn apply(&mut self, enable: bool) {
        push(&self.0, format!("autostart.apply({enable})"));
        self.0.borrow_mut().autostart.push(enable);
    }
}

pub struct FakeTimers(pub W);
impl TimerPort for FakeTimers {
    fn start(&mut self, timer: TimerId, millis: u32) {
        push(&self.0, format!("timers.start({timer:?},{millis})"));
        let mut w = self.0.borrow_mut();
        let due = w.now + millis as u64;
        w.timers.insert(timer, (due, millis));
    }
    fn stop(&mut self, timer: TimerId) {
        push(&self.0, format!("timers.stop({timer:?})"));
        self.0.borrow_mut().timers.remove(&timer);
    }
}

pub struct FakeStore(pub W);
impl StorePort for FakeStore {
    fn load(&mut self) -> Loaded {
        push(&self.0, "store.load".into());
        self.0.borrow_mut().load.take().unwrap_or(Loaded {
            settings: Settings::default(),
            source: LoadSource::Fresh,
        })
    }
    fn reload_saved(&mut self) -> Result<Settings, String> {
        push(&self.0, "store.reload".into());
        self.0
            .borrow_mut()
            .reload
            .pop_front()
            .unwrap_or(Err("no file".to_string()))
    }
    fn write_state(&mut self, state: &RuntimeState) {
        push(&self.0, "store.write_state".into());
        self.0.borrow_mut().states.push(*state);
    }
}

pub struct FakeLauncher(pub W);
impl SettingsProcessPort for FakeLauncher {
    fn launch(&mut self, state: &RuntimeState) -> bool {
        push(&self.0, "settings.launch".into());
        let mut w = self.0.borrow_mut();
        w.launches.push(*state);
        w.launch_ok
    }
}

pub struct FakeCore(pub W);
impl CorePort for FakeCore {
    fn plan(&self, _policy: &Policy, _monitors: &[MonitorInfo]) -> (EngineConfig, PlanReport) {
        push(&self.0, "core.plan".into());
        let report = self.0.borrow_mut().reports.pop_front().unwrap_or_default();
        (EngineConfig::default(), report)
    }
    fn is_arrow_rotation_hotkey(&self, hotkey: &Hotkey) -> bool {
        self.0.borrow().rotation.contains(hotkey)
    }
}

pub fn ports(w: &W) -> Ports {
    Ports {
        tray: Box::new(FakeTray(w.clone())),
        hotkeys: Box::new(FakeHotkeys(w.clone())),
        guard: Box::new(FakeGuard(w.clone())),
        system: Box::new(FakeSystem(w.clone())),
        autostart: Box::new(FakeAutoStart(w.clone())),
        timers: Box::new(FakeTimers(w.clone())),
        store: Box::new(FakeStore(w.clone())),
        settings_process: Box::new(FakeLauncher(w.clone())),
        core: Box::new(FakeCore(w.clone())),
    }
}

/// A controller over the fakes, with a fake clock.
pub struct Harness {
    pub ctl: Controller,
    pub w: W,
}

impl Harness {
    pub fn new(world: World) -> Self {
        let w = Rc::new(RefCell::new(world));
        Self {
            ctl: Controller::new(ports(&w)),
            w,
        }
    }

    /// A started controller on `settings` loaded from the v2 file.
    pub fn started(settings: Settings) -> Self {
        let mut h = Self::new(World {
            load: Some(Loaded {
                settings,
                source: LoadSource::V2,
            }),
            ..World::default()
        });
        h.ctl.start();
        h
    }

    pub fn ev(&mut self, e: Event) -> Flow {
        self.ctl.handle(e)
    }

    /// Advance the fake clock, firing due timers in deadline order (periodic, as SetTimer).
    pub fn advance(&mut self, ms: u64) {
        let end = self.w.borrow().now + ms;
        loop {
            let next = {
                let w = self.w.borrow();
                w.timers
                    .iter()
                    .filter(|(_, (due, _))| *due <= end)
                    .map(|(id, (due, _))| (*due, id.id()))
                    .min()
            };
            let Some((due, raw)) = next else { break };
            let id = TimerId::from_id(raw).expect("known timer");
            {
                let mut w = self.w.borrow_mut();
                w.now = due;
                if let Some(t) = w.timers.get_mut(&id) {
                    t.0 = due + t.1 as u64;
                }
            }
            self.ctl.handle(Event::Timer(id));
        }
        self.w.borrow_mut().now = end;
    }

    pub fn clear_log(&self) {
        let mut w = self.w.borrow_mut();
        w.log.clear();
        w.balloons.clear();
        w.views.clear();
    }

    pub fn log(&self) -> Vec<String> {
        self.w.borrow().log.clone()
    }

    pub fn balloons(&self) -> Vec<Balloon> {
        self.w.borrow().balloons.clone()
    }

    pub fn last_view(&self) -> TrayView {
        self.w.borrow().views.last().cloned().expect("a view was shown")
    }

    pub fn count(&self, line: &str) -> usize {
        self.w.borrow().log.iter().filter(|l| *l == line).count()
    }
}

pub fn en(key: &str) -> String {
    fence_i18n::get(Lang::En, key).to_string()
}

pub fn enf(key: &str, args: &[&str]) -> String {
    fence_i18n::format(Lang::En, key, args)
}

pub fn hk(h: &Hotkey) -> String {
    fence_config::hotkey_text(h)
}

/// Index of the first log line equal to `line` (panics with the log when absent).
pub fn pos(log: &[String], line: &str) -> usize {
    log.iter()
        .position(|l| l == line)
        .unwrap_or_else(|| panic!("`{line}` not in log: {log:#?}"))
}
