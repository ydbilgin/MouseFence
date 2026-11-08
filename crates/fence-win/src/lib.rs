//! MouseFence Windows layer. OWNED BY TRACK C. The only crate (with the settings UI's identify overlay)
//! allowed to use `unsafe`, and only for Win32 calls through windows-sys.
//!
//! Rules for every test in this crate (PLAN T19; the owner games fullscreen on this PC):
//! - never install a real WH_MOUSE_LL hook, never call SetCursorPos or ClipCursor, never send input;
//! - never register a real MouseFence chord (tests use F24-based chords) and never take the real
//!   single-instance mutex name (tests use a per-process test name);
//! - never write `HKCU\...\Run` (autostart takes the key path as a parameter; tests use a throwaway key);
//! - never read or write the owner's settings files (tests use temp folders);
//! - read-only display enumeration may run, but tests print no coordinates (his desk layout is private);
//! - never call Shell_NotifyIcon or TrackPopupMenu (tests build the data and the menu, and stop there);
//! - windows made by tests are message-only (HWND_MESSAGE), never shown, and are destroyed at the end.
//!
//! Modules: `hook` (the hook step as a pure function plus the hook thread, PLAN T9-T12), `monitors`,
//! `hotkeys`, `tray` (native notification icon, menu, balloons, the hidden window and its loop), `icons`
//! (the three embedded multi-resolution tray icons), `autostart`, `single_instance`, `system` (Intel GPU,
//! UI language, theme, high contrast, DPI awareness, debug log), `cursor` and `wide` (UTF-16 helpers).

use std::path::PathBuf;

use fence_types::MonitorInfo;

pub mod autostart;
pub mod cursor;
pub mod error_log;
pub mod hook;
pub mod hotkeys;
pub mod icons;
pub mod monitors;
pub mod notifications;
pub mod qpc;
pub mod single_instance;
pub mod system;
pub mod trace_ring;
pub mod tray;
pub mod wide;

/// The 1.x single-instance mutex. 1.x and 2.0 share it so they can never run (and enforce) at once.
pub const SINGLE_INSTANCE_MUTEX: &str = "MouseFence_SingleInstance_2C7A";

/// Live monitors, sorted and numbered like C# `MonitorInfo.All()` (EnumDisplayMonitors + GetMonitorInfo +
/// EnumDisplayDevices interface path, then `layout::sort_and_number` and key disambiguation). Read-only.
pub fn enumerate_monitors() -> Vec<MonitorInfo> {
    monitors::enumerate()
}

/// `%APPDATA%` (roaming), where the settings live.
pub fn appdata_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

/// `%LOCALAPPDATA%`, where the state file and future traces live.
pub fn local_appdata_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

/// C# `Native.HasIntelGpu`.
pub fn has_intel_gpu() -> bool {
    system::has_intel_gpu()
}

/// OS UI language is Turkish (C# `CultureInfo.CurrentUICulture.TwoLetterISOLanguageName == "tr"`).
pub fn os_ui_is_turkish() -> bool {
    system::os_ui_is_turkish()
}

/// Apps use the dark theme (C# `Theming.IsSystemDark`, registry AppsUseLightTheme == 0).
pub fn is_system_dark() -> bool {
    system::is_system_dark()
}

/// High contrast is on (C# `SystemInformation.HighContrast`).
pub fn high_contrast() -> bool {
    system::high_contrast()
}

/// One line to the debugger output (OutputDebugString). The tray has no console and 1.x kept no log file;
/// this is where "logged" goes (settings process failures, load problems), visible in DebugView only.
pub fn debug_log(line: &str) {
    system::debug_log(line)
}
