//! The few Win32 calls the settings window makes itself (windows-sys): the identify overlay windows, the file
//! dialogs and the restore confirmation (C# `SaveFileDialog`, `OpenFileDialog`, `MessageBox`), the high-contrast
//! system colours (C# `SystemColors`), the virtual-screen rectangle and process DPI awareness. Everything else
//! that touches Windows lives in fence-win (Track C). This is the only module of the crate with `unsafe`.

use std::path::PathBuf;

use eframe::egui::Color32;
use fence_types::MonitorInfo;

use crate::editor::Dialogs;
use crate::identify::{OverlayHost, OverlaySpec};
use crate::theme::SystemColors;
use crate::ui::SystemTheme;

/// The OS facts for the theme: high contrast and dark mode from fence-win, the six system colours from
/// `GetSysColor`.
pub fn system_theme() -> SystemTheme {
    SystemTheme {
        high_contrast: fence_win::high_contrast(),
        system_dark: fence_win::is_system_dark(),
        colors: system_colors(),
    }
}

/// WinForms `Filter` text ("Name|*.json|All|*.*") to the Win32 double-NUL list.
pub fn filter_to_win32(filter: &str) -> Vec<u16> {
    let mut out: Vec<u16> = Vec::new();
    for part in filter.split('|') {
        out.extend(part.encode_utf16());
        out.push(0);
    }
    out.push(0);
    out
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The real dialogs, owned by the settings window.
pub struct Win32Dialogs;

#[cfg(windows)]
mod imp {
    use super::*;
    use std::sync::Once;

    use windows_sys::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        BeginPaint, CreateFontW, CreateSolidBrush, DT_CENTER, DT_END_ELLIPSIS, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER,
        DeleteObject, DrawTextW, EndPaint, EnumDisplayMonitors, FillRect, GetMonitorInfoW, GetSysColor, HDC, HMONITOR,
        MONITORINFO, MONITORINFOEXW, PAINTSTRUCT, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, GetSaveFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT,
        OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    use windows_sys::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetProcessDpiAwarenessContext,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetClientRect,
        GetSystemMetrics, GetWindowLongPtrW, IDC_ARROW, IDYES, IsWindow, LWA_ALPHA, LoadCursorW, MA_NOACTIVATE,
        MB_ICONQUESTION, MB_YESNO, MSG, MessageBoxW, PM_REMOVE, PeekMessageW, RegisterClassExW, SM_CXVIRTUALSCREEN,
        SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_SHOWNOACTIVATE, SetLayeredWindowAttributes,
        SetTimer, SetWindowLongPtrW, ShowWindow, TranslateMessage, WM_ERASEBKGND, WM_MOUSEACTIVATE, WM_NCDESTROY,
        WM_PAINT, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    };

    const CLASS: &str = "MouseFenceIdentifyOverlay2";

    fn colorref(c: Color32) -> COLORREF {
        c.r() as u32 | (c.g() as u32) << 8 | (c.b() as u32) << 16
    }

    fn color32(c: u32) -> Color32 {
        Color32::from_rgb((c & 0xFF) as u8, (c >> 8 & 0xFF) as u8, (c >> 16 & 0xFF) as u8)
    }

    pub fn system_colors() -> SystemColors {
        // COLOR_WINDOW 5, COLOR_WINDOWTEXT 8, COLOR_GRAYTEXT 17, COLOR_HIGHLIGHT 13, COLOR_BTNFACE 15, COLOR_BTNTEXT 18.
        // SAFETY: GetSysColor reads a system value and has no preconditions.
        let get = |i: i32| color32(unsafe { GetSysColor(i) });
        SystemColors {
            window: get(5),
            window_text: get(8),
            gray_text: get(17),
            highlight: get(13),
            control: get(15),
            control_text: get(18),
        }
    }

    /// The virtual desktop as (left, top, right, bottom) in physical pixels.
    pub fn virtual_screen() -> (i32, i32, i32, i32) {
        // SAFETY: GetSystemMetrics has no preconditions.
        unsafe {
            let (x, y) = (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN));
            (
                x,
                y,
                x + GetSystemMetrics(SM_CXVIRTUALSCREEN),
                y + GetSystemMetrics(SM_CYVIRTUALSCREEN),
            )
        }
    }

    pub fn set_dpi_awareness() {
        // SAFETY: setting the process DPI awareness once before any window exists; failure (already set) is fine.
        unsafe {
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
    }

    unsafe extern "system" fn enum_proc(monitor: HMONITOR, _dc: HDC, _rect: *mut RECT, data: LPARAM) -> i32 {
        // SAFETY: `data` is the `&mut Vec<MonitorInfo>` passed to EnumDisplayMonitors below, alive for the call.
        let out = unsafe { &mut *(data as *mut Vec<MonitorInfo>) };
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        // SAFETY: `info` is a properly sized MONITORINFOEXW.
        if unsafe { GetMonitorInfoW(monitor, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO) } != 0 {
            let r = info.monitorInfo.rcMonitor;
            let len = info
                .szDevice
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(info.szDevice.len());
            let device = String::from_utf16_lossy(&info.szDevice[..len]);
            out.push(MonitorInfo {
                device: device.clone(),
                stable_id: device,
                bounds: fence_types::Rect::new(r.left, r.top, r.right, r.bottom),
                primary: info.monitorInfo.dwFlags & 1 != 0,
                index: 0,
            });
        }
        1
    }

    /// Read-only monitor list for the `--identify` harness while fence-win is a pre-join stub: GDI device names as
    /// ids, the primary first and the rest left to right, numbered from 1.
    pub fn enumerate_monitors_fallback() -> Vec<MonitorInfo> {
        let mut out: Vec<MonitorInfo> = Vec::new();
        // SAFETY: the callback only writes into `out`, which outlives the call.
        unsafe {
            EnumDisplayMonitors(
                std::ptr::null_mut(),
                std::ptr::null(),
                Some(enum_proc),
                &mut out as *mut _ as LPARAM,
            );
        }
        out.sort_by_key(|m| (!m.primary, m.bounds.l, m.bounds.t));
        for (i, m) in out.iter_mut().enumerate() {
            m.index = i as u32 + 1;
        }
        out
    }

    struct OverlayData {
        text: Vec<u16>,
        font_pt: f32,
        back: Color32,
    }

    unsafe extern "system" fn overlay_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match msg {
            WM_TIMER => {
                // SAFETY: our own window, destroyed on our own thread.
                unsafe { DestroyWindow(hwnd) };
                0
            }
            WM_ERASEBKGND => 1,
            WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
            WM_PAINT => {
                // SAFETY: standard paint sequence on our own window; the user data pointer was set by `open` and is
                // freed only in WM_NCDESTROY, so it is valid here; every GDI object made here is deleted here.
                unsafe {
                    let mut ps = PAINTSTRUCT::default();
                    let dc = BeginPaint(hwnd, &mut ps);
                    let data = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const OverlayData;
                    if !data.is_null() {
                        let data = &*data;
                        let mut rc = RECT::default();
                        GetClientRect(hwnd, &mut rc);
                        let brush = CreateSolidBrush(colorref(data.back));
                        FillRect(dc, &rc, brush);
                        DeleteObject(brush);
                        let dpi = GetDpiForWindow(hwnd).max(96) as f32;
                        let height = -((data.font_pt * dpi / 72.0).round() as i32);
                        let face = wide("Segoe UI");
                        let font = CreateFontW(height, 0, 0, 0, 700, 0, 0, 0, 1, 0, 0, 5, 0, face.as_ptr());
                        let old = SelectObject(dc, font);
                        SetTextColor(dc, 0x00FF_FFFF);
                        SetBkMode(dc, TRANSPARENT as i32);
                        let mut text = data.text.clone();
                        let len = text.len() as i32;
                        DrawTextW(
                            dc,
                            text.as_mut_ptr(),
                            len,
                            &mut rc,
                            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
                        );
                        SelectObject(dc, old);
                        DeleteObject(font);
                    }
                    EndPaint(hwnd, &ps);
                }
                0
            }
            WM_NCDESTROY => {
                // SAFETY: the pointer came from Box::into_raw in `open`; it is cleared before being freed once.
                unsafe {
                    let data = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut OverlayData;
                    if !data.is_null() {
                        drop(Box::from_raw(data));
                    }
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
            // SAFETY: default handling for every other message.
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    fn register_class() {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            let name = wide(CLASS);
            // SAFETY: a class with a static name buffer leaked for the process lifetime and our window procedure.
            unsafe {
                let class = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    lpfnWndProc: Some(overlay_proc),
                    hInstance: GetModuleHandleW(std::ptr::null()),
                    hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
                    lpszClassName: name.leak().as_ptr(),
                    ..Default::default()
                };
                RegisterClassExW(&class);
            }
        });
    }

    /// Overlay windows on the calling thread (the thread must pump them: `run_harness` does).
    pub struct Win32Overlays {
        windows: Vec<HWND>,
    }

    impl Win32Overlays {
        pub fn new() -> Self {
            register_class();
            Self { windows: Vec::new() }
        }
    }

    impl Default for Win32Overlays {
        fn default() -> Self {
            Self::new()
        }
    }

    impl OverlayHost for Win32Overlays {
        fn open(&mut self, spec: &OverlaySpec) -> bool {
            let class = wide(CLASS);
            let title = wide("MouseFence");
            let b = spec.bounds;
            // SAFETY: creating a top-level popup of our registered class on this thread; the user data box is owned
            // by the window from here on and freed in WM_NCDESTROY.
            unsafe {
                let hwnd = CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                    class.as_ptr(),
                    title.as_ptr(),
                    WS_POPUP,
                    b.l,
                    b.t,
                    b.width() as i32,
                    b.height() as i32,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    GetModuleHandleW(std::ptr::null()),
                    std::ptr::null(),
                );
                if hwnd.is_null() {
                    return false;
                }
                let data = Box::new(OverlayData {
                    text: spec.text.encode_utf16().collect(),
                    font_pt: spec.font_pt,
                    back: spec.back,
                });
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(data) as isize);
                SetLayeredWindowAttributes(hwnd, 0, (spec.opacity * 255.0).round() as u8, LWA_ALPHA);
                SetTimer(hwnd, 1, spec.duration_ms, None);
                ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                self.windows.push(hwnd);
            }
            true
        }

        fn pump(&mut self) {
            // SAFETY: a standard message pump for this thread's windows.
            unsafe {
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }

        fn open_count(&self) -> usize {
            // SAFETY: IsWindow accepts any handle value.
            self.windows.iter().filter(|h| unsafe { IsWindow(**h) } != 0).count()
        }

        fn close_all(&mut self) {
            for h in self.windows.drain(..) {
                // SAFETY: only windows this thread created; a destroyed handle is skipped by IsWindow.
                unsafe {
                    if IsWindow(h) != 0 {
                        DestroyWindow(h);
                    }
                }
            }
            self.pump();
        }
    }

    fn file_dialog(save: bool, title: &str, filter: &str) -> Option<PathBuf> {
        let filter = filter_to_win32(filter);
        let title = wide(title);
        let ext = wide("json");
        let mut buf = vec![0u16; 1024];
        if save {
            for (i, c) in "MouseFence-settings.json".encode_utf16().enumerate() {
                buf[i] = c;
            }
        }
        // SAFETY: every pointer refers to a live, NUL-terminated local buffer for the duration of the call.
        unsafe {
            let mut ofn = OPENFILENAMEW {
                lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
                hwndOwner: GetActiveWindow(),
                lpstrFilter: filter.as_ptr(),
                nFilterIndex: 1,
                lpstrFile: buf.as_mut_ptr(),
                nMaxFile: buf.len() as u32,
                lpstrTitle: title.as_ptr(),
                lpstrDefExt: ext.as_ptr(),
                Flags: OFN_EXPLORER
                    | OFN_NOCHANGEDIR
                    | OFN_PATHMUSTEXIST
                    | if save { OFN_OVERWRITEPROMPT } else { OFN_FILEMUSTEXIST },
                ..Default::default()
            };
            let ok = if save {
                GetSaveFileNameW(&mut ofn)
            } else {
                GetOpenFileNameW(&mut ofn)
            };
            if ok == 0 {
                return None;
            }
        }
        let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
        Some(PathBuf::from(String::from_utf16_lossy(&buf[..len])))
    }

    impl Dialogs for Win32Dialogs {
        fn pick_backup_path(&mut self, title: &str, filter: &str) -> Option<PathBuf> {
            file_dialog(true, title, filter)
        }
        fn pick_restore_path(&mut self, title: &str, filter: &str) -> Option<PathBuf> {
            file_dialog(false, title, filter)
        }
        fn confirm_restore(&mut self, text: &str, caption: &str) -> bool {
            let (t, c) = (wide(text), wide(caption));
            // SAFETY: NUL-terminated local strings; the owner is this thread's active window (the settings window).
            unsafe { MessageBoxW(GetActiveWindow(), t.as_ptr(), c.as_ptr(), MB_YESNO | MB_ICONQUESTION) == IDYES }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn system_colors() -> SystemColors {
        SystemColors::default()
    }
    pub fn virtual_screen() -> (i32, i32, i32, i32) {
        (0, 0, 1920, 1080)
    }
    pub fn set_dpi_awareness() {}
    pub fn enumerate_monitors_fallback() -> Vec<MonitorInfo> {
        Vec::new()
    }
    pub struct Win32Overlays;
    impl Win32Overlays {
        pub fn new() -> Self {
            Self
        }
    }
    impl OverlayHost for Win32Overlays {
        fn open(&mut self, _spec: &OverlaySpec) -> bool {
            false
        }
        fn pump(&mut self) {}
        fn open_count(&self) -> usize {
            0
        }
        fn close_all(&mut self) {}
    }
    impl Dialogs for Win32Dialogs {
        fn pick_backup_path(&mut self, _title: &str, _filter: &str) -> Option<PathBuf> {
            None
        }
        fn pick_restore_path(&mut self, _title: &str, _filter: &str) -> Option<PathBuf> {
            None
        }
        fn confirm_restore(&mut self, _text: &str, _caption: &str) -> bool {
            false
        }
    }
    fn _unused(_: Color32) {}
}

pub use imp::{Win32Overlays, enumerate_monitors_fallback, set_dpi_awareness, system_colors, virtual_screen};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s_ui_file_filter_becomes_the_win32_double_nul_list() {
        let f = filter_to_win32("A (*.json)|*.json|All|*.*");
        let s = String::from_utf16_lossy(&f);
        assert_eq!(s, "A (*.json)\0*.json\0All\0*.*\0\0");
    }
}
