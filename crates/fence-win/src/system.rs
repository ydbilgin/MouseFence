//! Read-only system queries (Intel GPU, UI language, theme, high contrast), the process DPI mode and the
//! debug log. Each query keeps its decision in a pure helper so the rule is tested without the machine.

use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;
use windows_sys::Win32::Graphics::Gdi::{DISPLAY_DEVICEW, EnumDisplayDevicesW};
use windows_sys::Win32::System::Diagnostics::Debug::OutputDebugStringW;
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows_sys::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows_sys::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETHIGHCONTRAST, SystemParametersInfoW};

use crate::wide::{from_wide, to_wide};

/// The replay CLI may reuse its parent's console; redirected stdout is kept intact. Never creates a console.
pub fn attach_parent_console_if_needed() {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_OUTPUT_HANDLE};
    // SAFETY: only read our stdout handle; attach solely to an existing parent console.
    unsafe {
        let stdout = GetStdHandle(STD_OUTPUT_HANDLE);
        if stdout.is_null() || stdout == INVALID_HANDLE_VALUE {
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

/// `LANG_TURKISH` (primary language id).
const LANG_TURKISH: u16 = 0x1F;
/// The registry value C# `Theming.IsSystemDark` reads.
pub const PERSONALIZE_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
pub const APPS_USE_LIGHT_THEME: &str = "AppsUseLightTheme";

/// C# `Native.HasIntelGpu` rule: an adapter description containing "Intel" (ordinal, ignore case).
pub fn is_intel_adapter(description: &str) -> bool {
    description.to_ascii_lowercase().contains("intel")
}

/// C# `Native.HasIntelGpu`: read-only adapter enumeration, hard cap of 64 adapters, failures mean "no".
pub fn has_intel_gpu() -> bool {
    for i in 0..64u32 {
        // SAFETY: DISPLAY_DEVICEW is plain data; zero is valid.
        let mut dd: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
        dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
        // SAFETY: a null device name enumerates adapters; dd is a valid out-pointer with cb set.
        if unsafe { EnumDisplayDevicesW(std::ptr::null(), i, &mut dd, 0) } == 0 {
            break;
        }
        if is_intel_adapter(&from_wide(&dd.DeviceString)) {
            return true;
        }
    }
    false
}

/// The primary language of a LANGID is Turkish.
pub fn langid_is_turkish(langid: u16) -> bool {
    langid & 0x3FF == LANG_TURKISH
}

/// The user's UI language is Turkish (C# `CurrentUICulture.TwoLetterISOLanguageName == "tr"`).
pub fn os_ui_is_turkish() -> bool {
    // SAFETY: no arguments.
    langid_is_turkish(unsafe { GetUserDefaultUILanguage() })
}

/// C# `k?.GetValue("AppsUseLightTheme") is int v && v == 0`.
pub fn dark_from_apps_use_light_theme(value: Option<u32>) -> bool {
    value == Some(0)
}

/// Read a REG_DWORD under HKCU; `None` when absent or of another type.
pub fn read_hkcu_dword(subkey: &str, value: &str) -> Option<u32> {
    let k = to_wide(subkey);
    let v = to_wide(value);
    let mut data: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: valid NUL-terminated names and a 4-byte out buffer whose size is passed.
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            k.as_ptr(),
            v.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            &mut data as *mut u32 as *mut core::ffi::c_void,
            &mut size,
        )
    };
    (rc == 0).then_some(data)
}

/// C# `Theming.IsSystemDark`.
pub fn is_system_dark() -> bool {
    dark_from_apps_use_light_theme(read_hkcu_dword(PERSONALIZE_KEY, APPS_USE_LIGHT_THEME))
}

/// C# `SystemInformation.HighContrast`.
pub fn high_contrast() -> bool {
    // SAFETY: HIGHCONTRASTW is plain data; zero is valid.
    let mut hc: HIGHCONTRASTW = unsafe { std::mem::zeroed() };
    hc.cbSize = std::mem::size_of::<HIGHCONTRASTW>() as u32;
    // SAFETY: hc is a valid out-pointer and its size is passed.
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            hc.cbSize,
            &mut hc as *mut HIGHCONTRASTW as *mut core::ffi::c_void,
            0,
        )
    };
    ok != 0 && hc.dwFlags & HCF_HIGHCONTRASTON != 0
}

/// Per-monitor DPI awareness v2 for the tray process (1.x: ApplicationConfiguration PerMonitorV2), so
/// monitor rectangles, hook points and cursor placement are all physical pixels. False when Windows refused
/// (already set, for example by a manifest).
pub fn enable_per_monitor_dpi_awareness() -> bool {
    // SAFETY: plain Win32 call with a predefined context value.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) != 0 }
}

/// The text written to the debugger output for one log line.
pub fn debug_line(line: &str) -> String {
    format!("MouseFence: {line}\n")
}

/// OutputDebugString; invisible unless a debugger or DebugView listens.
pub fn debug_log(line: &str) {
    let w = to_wide(&debug_line(line));
    // SAFETY: NUL-terminated string.
    unsafe { OutputDebugStringW(w.as_ptr()) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s_win_intel_rule_matches_csharp() {
        assert!(is_intel_adapter("Intel(R) UHD Graphics 770"));
        assert!(is_intel_adapter("INTEL Iris Xe"));
        assert!(!is_intel_adapter("Synthetic Display Adapter"));
        assert!(!is_intel_adapter(""));
    }

    #[test]
    fn s_win_turkish_langid_rule() {
        assert!(langid_is_turkish(0x041F), "tr-TR");
        assert!(!langid_is_turkish(0x0409), "en-US");
        assert!(!langid_is_turkish(0x0407), "de-DE");
    }

    #[test]
    fn s_win_dark_theme_rule_matches_csharp() {
        assert!(dark_from_apps_use_light_theme(Some(0)));
        assert!(!dark_from_apps_use_light_theme(Some(1)));
        assert!(!dark_from_apps_use_light_theme(None), "absent value: light");
    }

    #[test]
    fn s_win_system_queries_run_read_only() {
        // These read the live machine and change nothing; the values are his and are not asserted.
        let _ = has_intel_gpu();
        let _ = os_ui_is_turkish();
        let _ = is_system_dark();
        let _ = high_contrast();
        assert_eq!(debug_line("x"), "MouseFence: x\n");
    }
}
