//! Tray controller (port of C# `Ui/TrayApplicationContext.cs` minus WinForms). OWNED BY TRACK C.
//!
//! All OS access goes through the port traits below; fence-win implements them for real (wired in
//! apps/mousefence), the tests here implement them with fakes. That is how the B-rows of docs/rust2/parity.tsv
//! (tray, hotkeys, balloons, startup, pause, display change, hook health, Intel warning, settings round trip)
//! are proven without a real hook, a real tray icon, a real hotkey or a real cursor move on the owner's PC.
//!
//! - [`Controller`] (controller.rs): startup, toggles, pause, the 3 s probe and its outage latch, the 750 ms
//!   display debounce, Configure balloons and the isolation warning, the Intel warning, the settings process
//!   round trip, exit.
//! - [`Guard`] (guard.rs): C# `MouseGuard` minus the hook callback (intent vs handle, the liveness probe,
//!   reinstall) over a [`HookHost`]; implements [`GuardPort`].
//! - view.rs: what the tray shows ([`TrayView`]) and the ids that tie native events back to commands.
#![forbid(unsafe_code)]

use fence_config::{Loaded, Settings};
use fence_types::{EngineConfig, HookHealthVerdict, Hotkey, MonitorInfo, PlanReport, Policy, RuntimeState};

mod controller;
mod guard;
mod view;

pub use controller::{Controller, Event, Flow, HOOK_PROBE_INTERVAL_MS, Ports, RECONFIGURE_DEBOUNCE_MS};
pub use guard::{Guard, HookHost};
pub use view::{MenuCommand, MenuEntry, TimerId};

/// Tray icon state (C# `_iconClosed`, `_iconOpen`, `_iconPaused`; paused also shows when the hook is down).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayIcon {
    Closed,
    Open,
    Paused,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuItem {
    pub text: String,
    pub shortcut: String,
    pub checked: bool,
    pub enabled: bool,
}

/// Everything the tray shows (C# `UpdateUi` + `RebuildMenuText`). Menu order is fixed: gate, side, game,
/// pause, separator, settings, exit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayView {
    pub icon: TrayIcon,
    pub tooltip: String,
    pub gate: MenuItem,
    pub side: MenuItem,
    pub game: MenuItem,
    pub pause: MenuItem,
    pub settings: MenuItem,
    pub exit: MenuItem,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BalloonKind {
    Info,
    Warning,
}

/// One balloon (C# `ShowBalloonTip(ms, "MouseFence", text, icon)`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Balloon {
    pub millis: u32,
    pub kind: BalloonKind,
    pub text: String,
}

/// The four global hotkey slots (C# `_hotkey`, `_confineHotkey`, `_pauseHotkey`, `_sideHotkey`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HotkeySlot {
    Gate,
    Confine,
    Pause,
    Side,
    MarkAccident,
}

pub trait TrayPort {
    fn probe_notifications(&mut self) {}
    /// Put the icon in the notification area (C# `new NotifyIcon { Visible = true, Icon = closed,
    /// Text = "MouseFence" }`, before any hotkey or Configure balloon).
    fn add_icon(&mut self, icon: TrayIcon, tooltip: &str);
    fn show(&mut self, view: &TrayView);
    fn balloon(&mut self, balloon: &Balloon);
    /// C# `_tray.Visible = false` at exit.
    fn remove_icon(&mut self);
}

pub trait HotkeyPort {
    /// C# `HotKeyWindow.Register`: unregister the slot, then register with MOD_NOREPEAT; false when the
    /// chord has no key or no modifier, or Windows refused it.
    fn register(&mut self, slot: HotkeySlot, hotkey: Hotkey) -> bool;
    fn unregister(&mut self, slot: HotkeySlot);
}

/// The hook owner (C# `MouseGuard`): runs on its own thread in fence-win.
pub trait GuardPort {
    fn metrics(&self) -> Option<fence_types::HookMetrics> {
        None
    }
    fn pause_trace(&mut self) {}
    fn configure_trace(&mut self, _settings: &fence_config::Settings, _monitors: &[MonitorInfo], _state: RuntimeState) {
    }
    fn drain_trace(&mut self, _state: RuntimeState) -> usize {
        0
    }
    fn mark_accident(&mut self, _state: RuntimeState) -> usize {
        0
    }
    fn configure(&mut self, config: EngineConfig);
    fn start(&mut self);
    fn stop(&mut self);
    fn enabled(&self) -> bool;
    fn hook_installed(&self) -> bool;
    fn set_gate_open(&mut self, on: bool);
    fn set_side_contain(&mut self, on: bool);
    fn set_confine(&mut self, on: bool);
    /// C# `MouseGuard.CheckHealth`: a Dead verdict has already attempted a reinstall when it returns.
    fn check_health(&mut self) -> HookHealthVerdict;
    /// 2.0 hook fault counters since the hook thread started: (contained panics J08, failed cursor placements
    /// that were let through J07). The probe logs them when they change (JUDGE-C S1).
    fn fault_counts(&self) -> (u64, u64) {
        (0, 0)
    }
}

pub trait SystemPort {
    fn monitors(&self) -> Vec<MonitorInfo>;
    fn has_intel_gpu(&self) -> bool;
    fn os_ui_is_turkish(&self) -> bool;
    /// One diagnostic line (production: OutputDebugString; 1.x had no log).
    fn log(&self, line: &str);
}

pub trait AutoStartPort {
    /// C# `AutoStart.Apply`: set or delete the per-user Run value. Tests use a fake; the real Run key is
    /// never written by any automated test (brief: no Run entry).
    fn apply(&mut self, enable: bool);
}

/// One-shot or periodic UI-thread timers (C# `System.Windows.Forms.Timer`; Win32 SetTimer semantics:
/// `start` restarts a running timer, ticks repeat until `stop`).
pub trait TimerPort {
    fn start(&mut self, timer: TimerId, millis: u32);
    fn stop(&mut self, timer: TimerId);
}

/// Settings and the live-state file (PLAN T15-T16).
pub trait StorePort {
    /// C# `Settings.Load()` (fence-config `load`: v2 file, else the 1.x import, else fresh defaults).
    fn load(&mut self) -> Loaded;
    /// Re-read the v2 file after the settings process exited 0. `Err` unless it loaded cleanly from the v2
    /// file (a failed reload never replaces the live settings).
    fn reload_saved(&mut self) -> Result<Settings, String>;
    /// Rewrite `state.txt` (fence-types `launch::state_to_text`) for the open settings window.
    fn write_state(&mut self, state: &RuntimeState);
}

/// The settings window is a separate process (PLAN T16): the same exe with `--settings --state ...`. Its exit
/// code comes back later as [`Event::SettingsExited`].
pub trait SettingsProcessPort {
    /// Start it; false when it could not start.
    fn launch(&mut self, state: &RuntimeState) -> bool;
}

/// The pure engine and layout rules the tray calls (fence-engine). A port so tests can script a plan report
/// (demotions, pruned exclusions, isolated screens) and the Intel rotation rule.
pub trait CorePort {
    /// The engine input and the report for one Configure (C# Configure lines 136-237).
    fn plan(&self, policy: &Policy, monitors: &[MonitorInfo]) -> (EngineConfig, PlanReport);
    /// C# `GuardCore.IsArrowRotationHotkey`.
    fn is_arrow_rotation_hotkey(&self, hotkey: &Hotkey) -> bool;
}

/// The production [`CorePort`]: fence-engine's plan builder and layout rule.
#[derive(Clone, Copy, Debug, Default)]
pub struct EngineCore;

impl CorePort for EngineCore {
    fn plan(&self, policy: &Policy, monitors: &[MonitorInfo]) -> (EngineConfig, PlanReport) {
        fence_engine::plan::build(policy, monitors)
    }

    fn is_arrow_rotation_hotkey(&self, h: &Hotkey) -> bool {
        fence_engine::layout::is_arrow_rotation_hotkey(h.vk, h.ctrl, h.alt, h.shift, h.win)
    }
}
