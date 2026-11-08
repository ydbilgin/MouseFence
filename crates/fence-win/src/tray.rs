//! The native tray (PLAN T16: no GUI toolkit in the resident process): a hidden window that owns the
//! notification icon, its menu, balloons, hotkeys and timers, and the message loop that hands each event to
//! the controller (fence-app) outside the window procedure, so the controller is never re-entered.
//!
//! 1.x parity (Ui/TrayApplicationContext.cs): the icon starts with the closed padlock and the tip
//! "MouseFence"; a double click with any button toggles the gate (WinForms NotifyIcon raises DoubleClick for
//! left, middle and right); the context menu opens on right-button-up; balloons carry the title "MouseFence"
//! with an info or warning icon. The icon is re-added when Explorer restarts (TaskbarCreated).
//!
//! Tests create only message-only windows (never shown, no broadcasts), post messages to them, and never call
//! Shell_NotifyIcon or TrackPopupMenu.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;

use fence_types::Hotkey;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIIF_WARNING, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW,
    GWLP_USERDATA, GetCursorPos, GetMessageW, GetSystemMetrics, GetWindowLongPtrW, HICON, HMENU, HWND_MESSAGE,
    KillTimer, MF_CHECKED, MF_GRAYED, MF_SEPARATOR, MF_STRING, MSG, PM_REMOVE, PeekMessageW, PostMessageW,
    RegisterClassExW, RegisterWindowMessageW, SM_CXSMICON, SetForegroundWindow, SetTimer, SetWindowLongPtrW,
    TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenuEx, TranslateMessage, WM_APP, WM_DISPLAYCHANGE,
    WM_HOTKEY, WM_LBUTTONDBLCLK, WM_MBUTTONDBLCLK, WM_NULL, WM_RBUTTONDBLCLK, WM_RBUTTONUP, WM_SETTINGCHANGE, WM_TIMER,
    WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_OVERLAPPED,
};

use crate::hotkeys;
use crate::icons::{self, Icon};
use crate::notifications::{Notice, QuietBalloons, SystemNotificationState};
use crate::wide::{fill_wide, to_wide};

/// The notification icon's callback message.
pub const WM_APP_TRAY: u32 = WM_APP + 0x10;
/// Posted by the settings-process waiter thread; `wParam` = the exit code (as u32 bits).
pub const WM_APP_SETTINGS_EXIT: u32 = WM_APP + 0x11;
/// The balloon title (C# `ShowBalloonTip(ms, "MouseFence", ...)`).
pub const BALLOON_TITLE: &str = "MouseFence";
const TRAY_UID: u32 = 1;
const CLASS_NAME: &str = "MouseFence2.TrayWindow";

/// Where the window lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowKind {
    /// A hidden top-level window (never shown): receives WM_DISPLAYCHANGE and TaskbarCreated broadcasts,
    /// like the hidden window behind C# `SystemEvents`. Production.
    HiddenTopLevel,
    /// A message-only window: no broadcasts, never visible. Tests.
    MessageOnly,
}

/// What the window saw, handed to the controller by [`TrayWindow::run`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowEvent {
    TrayDoubleClick,
    MenuCommand(u32),
    Hotkey(i32),
    Timer(usize),
    DisplayChanged,
    SettingsExited(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayIconKind {
    Closed,
    Open,
    Paused,
}

/// One entry of the native context menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeMenuEntry {
    Item {
        id: u32,
        text: String,
        checked: bool,
        enabled: bool,
    },
    Separator,
}

/// What a notification-icon callback means (pure, `lParam` low word with the default icon version).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayCallback {
    DoubleClick,
    ContextMenu,
}

pub fn tray_callback_action(mouse_message: u32) -> Option<TrayCallback> {
    match mouse_message {
        WM_LBUTTONDBLCLK | WM_MBUTTONDBLCLK | WM_RBUTTONDBLCLK => Some(TrayCallback::DoubleClick),
        WM_RBUTTONUP => Some(TrayCallback::ContextMenu),
        _ => None,
    }
}

/// AppendMenu flags for one item (ToolStripMenuItem Checked / Enabled).
pub fn menu_item_flags(checked: bool, enabled: bool) -> u32 {
    MF_STRING | if checked { MF_CHECKED } else { 0 } | if enabled { 0 } else { MF_GRAYED }
}

/// A NOTIFYICONDATAW for our icon with `flags`; the caller fills what the flags name.
pub fn notify_data(hwnd: HWND, flags: u32) -> NOTIFYICONDATAW {
    // SAFETY: NOTIFYICONDATAW is plain data; zero is valid.
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_UID;
    nid.uFlags = flags;
    nid.uCallbackMessage = WM_APP_TRAY;
    nid
}

/// The icon-and-tip update (NIM_ADD / NIM_MODIFY). The tip is cut to 127 UTF-16 units.
pub fn icon_data(hwnd: HWND, icon: HICON, tip: &str) -> NOTIFYICONDATAW {
    let mut nid = notify_data(hwnd, NIF_MESSAGE | NIF_ICON | NIF_TIP);
    nid.hIcon = icon;
    fill_wide(&mut nid.szTip, tip);
    nid
}

/// A balloon (NIM_MODIFY + NIF_INFO). Text is cut to 255 units; the timeout is passed as 1.x passed it
/// (Windows Vista and later use the accessibility setting instead).
pub fn balloon_data(hwnd: HWND, text: &str, warning: bool, millis: u32) -> NOTIFYICONDATAW {
    let mut nid = notify_data(hwnd, NIF_INFO);
    fill_wide(&mut nid.szInfo, text);
    fill_wide(&mut nid.szInfoTitle, BALLOON_TITLE);
    nid.dwInfoFlags = if warning { NIIF_WARNING } else { NIIF_INFO };
    nid.Anonymous.uTimeout = millis;
    nid
}

struct WindowState {
    events: RefCell<VecDeque<WindowEvent>>,
    menu: RefCell<Vec<NativeMenuEntry>>,
    taskbar_created: u32,
    taskbar_restarted: Cell<bool>,
    theme_changed: Cell<bool>,
}

impl WindowState {
    fn push(&self, ev: WindowEvent) {
        if let Ok(mut q) = self.events.try_borrow_mut() {
            q.push_back(ev);
        }
    }
}

/// An owned popup menu, destroyed on drop.
pub struct NativeMenu(usize);

impl NativeMenu {
    pub fn handle(&self) -> HMENU {
        self.0 as HMENU
    }
}

impl Drop for NativeMenu {
    fn drop(&mut self) {
        // SAFETY: created by CreatePopupMenu, destroyed once.
        unsafe { DestroyMenu(self.0 as HMENU) };
    }
}

/// Build the popup menu (not shown). Order, checks and grey state follow `entries`.
pub fn build_menu(entries: &[NativeMenuEntry]) -> Option<NativeMenu> {
    // SAFETY: plain Win32 call.
    let h = unsafe { CreatePopupMenu() };
    if h.is_null() {
        return None;
    }
    let menu = NativeMenu(h as usize);
    for e in entries {
        match e {
            NativeMenuEntry::Item {
                id,
                text,
                checked,
                enabled,
            } => {
                let w = to_wide(text);
                // SAFETY: valid menu and NUL-terminated text.
                unsafe { AppendMenuW(h, menu_item_flags(*checked, *enabled), *id as usize, w.as_ptr()) };
            }
            NativeMenuEntry::Separator => {
                // SAFETY: valid menu.
                unsafe { AppendMenuW(h, MF_SEPARATOR, 0, std::ptr::null()) };
            }
        }
    }
    Some(menu)
}

/// Show the context menu at the cursor and return the chosen command id (0 = none). Production only.
///
/// # Safety
/// `hwnd` must be our live tray window on this thread.
unsafe fn show_menu(hwnd: HWND, state: &WindowState) -> u32 {
    let entries = state.menu.borrow().clone();
    let Some(menu) = build_menu(&entries) else {
        return 0;
    };
    let mut pt = POINT { x: 0, y: 0 };
    // SAFETY: documented pattern for notification-icon menus (foreground first, WM_NULL after).
    unsafe {
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenuEx(
            menu.handle(),
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY,
            pt.x,
            pt.y,
            hwnd,
            std::ptr::null(),
        );
        PostMessageW(hwnd, WM_NULL, 0, 0);
        cmd as u32
    }
}

/// # Safety
/// Called by Windows for our window class only.
unsafe fn handle_message(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    // SAFETY: GWLP_USERDATA holds the WindowState pointer the TrayWindow keeps alive, or 0.
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const WindowState;
    if ptr.is_null() {
        return None;
    }
    // SAFETY: see above; cleared before the state is dropped.
    let state = unsafe { &*ptr };
    match msg {
        WM_APP_TRAY => {
            match tray_callback_action(lparam as u32 & 0xFFFF) {
                Some(TrayCallback::DoubleClick) => state.push(WindowEvent::TrayDoubleClick),
                Some(TrayCallback::ContextMenu) => {
                    // SAFETY: our live window.
                    let cmd = unsafe { show_menu(hwnd, state) };
                    if cmd != 0 {
                        state.push(WindowEvent::MenuCommand(cmd));
                    }
                }
                None => {}
            }
            Some(0)
        }
        WM_HOTKEY => {
            state.push(WindowEvent::Hotkey(wparam as i32));
            Some(0)
        }
        WM_TIMER => {
            state.push(WindowEvent::Timer(wparam));
            Some(0)
        }
        WM_DISPLAYCHANGE => {
            state.push(WindowEvent::DisplayChanged);
            None
        }
        WM_SETTINGCHANGE => {
            state.theme_changed.set(true);
            None
        }
        WM_APP_SETTINGS_EXIT => {
            state.push(WindowEvent::SettingsExited(wparam as u32 as i32));
            Some(0)
        }
        m if m != 0 && m == state.taskbar_created => {
            state.taskbar_restarted.set(true);
            Some(0)
        }
        _ => None,
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: our class's window procedure; a panic never crosses this boundary.
    let handled = catch_unwind(AssertUnwindSafe(|| unsafe {
        handle_message(hwnd, msg, wparam, lparam)
    }));
    match handled {
        Ok(Some(r)) => r,
        // SAFETY: default handling for everything we do not take.
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn register_class() -> bool {
    static ATOM: OnceLock<u16> = OnceLock::new();
    *ATOM.get_or_init(|| {
        let name = to_wide(CLASS_NAME);
        // SAFETY: WNDCLASSEXW is plain data; the class name string is leaked once for the process lifetime.
        unsafe {
            let mut wc: WNDCLASSEXW = std::mem::zeroed();
            wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
            wc.lpfnWndProc = Some(wndproc);
            wc.hInstance = GetModuleHandleW(std::ptr::null());
            wc.lpszClassName = Box::leak(name.into_boxed_slice()).as_ptr();
            RegisterClassExW(&wc)
        }
    }) != 0
}

/// The tray's hidden window. Lives on the thread that created it.
pub struct TrayWindow {
    hwnd: HWND,
    state: Box<WindowState>,
    icons: RefCell<Option<[Icon; 3]>>,
    current: RefCell<(TrayIconKind, String)>,
    /// The icon should be in the notification area (between add_icon and remove_icon).
    wanted: Cell<bool>,
    /// Windows accepted NIM_ADD.
    added: Cell<bool>,
    quiet: RefCell<QuietBalloons>,
}

impl TrayWindow {
    pub fn create(kind: WindowKind) -> std::io::Result<Self> {
        if !register_class() {
            return Err(std::io::Error::last_os_error());
        }
        let class = to_wide(CLASS_NAME);
        let title = to_wide("MouseFence");
        let (ex, parent) = match kind {
            WindowKind::HiddenTopLevel => (WS_EX_TOOLWINDOW, std::ptr::null_mut()),
            WindowKind::MessageOnly => (0, HWND_MESSAGE),
        };
        // SAFETY: registered class, valid strings; the window is never shown (no WS_VISIBLE, no ShowWindow).
        let hwnd = unsafe {
            CreateWindowExW(
                ex,
                class.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                parent,
                std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            )
        };
        if hwnd.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let taskbar = to_wide("TaskbarCreated");
        let state = Box::new(WindowState {
            events: RefCell::new(VecDeque::new()),
            menu: RefCell::new(Vec::new()),
            // SAFETY: NUL-terminated name.
            taskbar_created: unsafe { RegisterWindowMessageW(taskbar.as_ptr()) },
            taskbar_restarted: Cell::new(false),
            theme_changed: Cell::new(false),
        });
        // SAFETY: the pointer stays valid until Drop clears it before the Box is freed.
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, &*state as *const WindowState as isize) };
        Ok(Self {
            hwnd,
            state,
            icons: RefCell::new(None),
            current: RefCell::new((TrayIconKind::Closed, "MouseFence".to_string())),
            wanted: Cell::new(false),
            added: Cell::new(false),
            quiet: RefCell::new(QuietBalloons::default()),
        })
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// The window handle as a number, for threads that post to it.
    pub fn hwnd_value(&self) -> usize {
        self.hwnd as usize
    }

    fn icon_handle(&self, kind: TrayIconKind) -> HICON {
        let mut icons = self.icons.borrow_mut();
        if icons.is_none() {
            // SAFETY: plain query; the process is DPI aware, so this is the small-icon size at system DPI.
            let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.clamp(16, 64) as u32;
            let make = |b: &[u8]| icons::create_icon(b, size);
            let [closed, open, paused] = icons::taskbar_icons();
            if let (Some(c), Some(o), Some(p)) = (make(closed), make(open), make(paused)) {
                *icons = Some([c, o, p]);
            }
        }
        match icons.as_ref() {
            Some(set) => set[kind as usize].handle(),
            None => std::ptr::null_mut(),
        }
    }

    fn notify(&self, message: u32, nid: &NOTIFYICONDATAW) -> bool {
        // SAFETY: nid is fully initialized with our window.
        unsafe { Shell_NotifyIconW(message, nid) != 0 }
    }

    fn try_add(&self) {
        let (kind, tip) = self.current.borrow().clone();
        let nid = icon_data(self.hwnd, self.icon_handle(kind), &tip);
        self.added.set(self.notify(NIM_ADD, &nid));
    }

    /// Put the icon in the notification area (C# `new NotifyIcon { Visible = true, ... }`).
    pub fn add_icon(&self, kind: TrayIconKind, tip: &str) {
        *self.current.borrow_mut() = (kind, tip.to_string());
        self.wanted.set(true);
        self.try_add();
    }

    /// Change icon and tip (C# `_tray.Icon = ...; _tray.Text = ...`).
    pub fn update(&self, kind: TrayIconKind, tip: &str) {
        let changed = *self.current.borrow() != (kind, tip.to_string());
        *self.current.borrow_mut() = (kind, tip.to_string());
        if !self.wanted.get() {
            return;
        }
        if !self.added.get() {
            self.try_add();
        } else if changed {
            let nid = icon_data(self.hwnd, self.icon_handle(kind), tip);
            self.notify(NIM_MODIFY, &nid);
        }
    }

    /// The menu shown on the next right click.
    pub fn set_menu(&self, entries: Vec<NativeMenuEntry>) {
        *self.state.menu.borrow_mut() = entries;
    }

    /// C# `ShowBalloonTip(millis, "MouseFence", text, Info|Warning)`.
    pub fn balloon(&self, text: &str, warning: bool, millis: u32) {
        let ready = self.quiet.borrow_mut().submit(
            &SystemNotificationState,
            Notice {
                text: text.into(),
                warning,
                millis,
            },
        );
        if self.added.get()
            && let Some(notice) = ready
        {
            self.notify(
                NIM_MODIFY,
                &balloon_data(self.hwnd, &notice.text, notice.warning, notice.millis),
            );
        }
    }

    pub fn probe_notifications(&self) {
        if !self.added.get() {
            return;
        }
        loop {
            let next = self.quiet.borrow_mut().pop_ready(&SystemNotificationState);
            let Some(notice) = next else {
                break;
            };
            self.notify(
                NIM_MODIFY,
                &balloon_data(self.hwnd, &notice.text, notice.warning, notice.millis),
            );
        }
    }

    /// C# `_tray.Visible = false`.
    pub fn remove_icon(&self) {
        self.wanted.set(false);
        if self.added.replace(false) {
            self.notify(NIM_DELETE, &notify_data(self.hwnd, 0));
        }
    }

    /// SetTimer: (re)start a periodic timer; WM_TIMER arrives as [`WindowEvent::Timer`].
    pub fn set_timer(&self, id: usize, millis: u32) -> bool {
        // SAFETY: our window; no TIMERPROC, so WM_TIMER reaches the window procedure.
        unsafe { SetTimer(self.hwnd, id, millis, None) != 0 }
    }

    pub fn kill_timer(&self, id: usize) {
        // SAFETY: our window.
        unsafe { KillTimer(self.hwnd, id) };
    }

    pub fn register_hotkey(&self, id: i32, hotkey: &Hotkey) -> bool {
        hotkeys::register(self.hwnd_value(), id, hotkey)
    }

    pub fn unregister_hotkey(&self, id: i32) {
        hotkeys::unregister(self.hwnd_value(), id)
    }

    fn after_dispatch(&self) {
        if self.state.theme_changed.replace(false) {
            *self.icons.borrow_mut() = None;
            if self.wanted.get() && self.added.get() {
                let (kind, tip) = self.current.borrow().clone();
                let nid = icon_data(self.hwnd, self.icon_handle(kind), &tip);
                self.notify(NIM_MODIFY, &nid);
            }
        }
        if self.state.taskbar_restarted.replace(false) && self.wanted.get() {
            self.try_add();
        }
    }

    fn pop(&self) -> Option<WindowEvent> {
        self.state.events.borrow_mut().pop_front()
    }

    /// The message loop. Each event goes to `on_event` after its message was dispatched, so the controller
    /// never runs inside the window procedure. Returns when `on_event` returns false or WM_QUIT arrives.
    pub fn run(&self, mut on_event: impl FnMut(WindowEvent) -> bool) {
        // SAFETY: MSG is plain data.
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        loop {
            // SAFETY: valid out-pointer.
            let r = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
            if r == 0 || r == -1 {
                return;
            }
            // SAFETY: msg came from GetMessageW.
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            self.after_dispatch();
            while let Some(ev) = self.pop() {
                if !on_event(ev) {
                    return;
                }
            }
        }
    }

    /// Dispatch whatever is queued for this thread now and return the events (tests; never blocks).
    pub fn pump_pending(&self) -> Vec<WindowEvent> {
        // SAFETY: MSG is plain data.
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        // SAFETY: valid out-pointer; messages of this thread only.
        while unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
            // SAFETY: msg came from PeekMessageW.
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        self.after_dispatch();
        std::iter::from_fn(|| self.pop()).collect()
    }
}

impl Drop for TrayWindow {
    fn drop(&mut self) {
        self.remove_icon();
        // SAFETY: clear the state pointer before the Box is freed, then destroy our window.
        unsafe {
            SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            DestroyWindow(self.hwnd);
        }
    }
}

/// Post the settings process's exit code to the tray window (from the waiter thread).
pub fn post_settings_exit(hwnd_value: usize, code: i32) -> bool {
    // SAFETY: posting plain integers; a stale handle makes the call fail harmlessly.
    unsafe { PostMessageW(hwnd_value as HWND, WM_APP_SETTINGS_EXIT, code as u32 as usize, 0) != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_change_invalidates_cached_taskbar_icons() {
        let window = TrayWindow::create(WindowKind::MessageOnly).unwrap();
        assert!(!window.icon_handle(TrayIconKind::Closed).is_null());
        assert!(window.icons.borrow().is_some());
        // WM_SETTINGCHANGE is a synchronous system message; Windows rejects PostMessageW.
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW(window.hwnd(), WM_SETTINGCHANGE, 0, 0) };
        window.pump_pending();
        assert!(window.icons.borrow().is_none());
        assert!(!window.icon_handle(TrayIconKind::Closed).is_null());
    }
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetMenuItemCount, GetMenuItemInfoW, MENUITEMINFOW, MFS_CHECKED, MFS_DISABLED, MFT_SEPARATOR, MIIM_FTYPE,
        MIIM_ID, MIIM_STATE, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    };

    #[test]
    fn s_win_tray_callback_matches_winforms_notifyicon() {
        for dbl in [WM_LBUTTONDBLCLK, WM_MBUTTONDBLCLK, WM_RBUTTONDBLCLK] {
            assert_eq!(tray_callback_action(dbl), Some(TrayCallback::DoubleClick));
        }
        assert_eq!(tray_callback_action(WM_RBUTTONUP), Some(TrayCallback::ContextMenu));
        for other in [WM_MOUSEMOVE, WM_LBUTTONDOWN, WM_LBUTTONUP] {
            assert_eq!(tray_callback_action(other), None);
        }
    }

    #[test]
    fn s_win_native_menu_keeps_order_checks_and_grey_state() {
        let entries = vec![
            NativeMenuEntry::Item {
                id: 1,
                text: "gate".into(),
                checked: true,
                enabled: true,
            },
            NativeMenuEntry::Item {
                id: 2,
                text: "side".into(),
                checked: false,
                enabled: false,
            },
            NativeMenuEntry::Separator,
            NativeMenuEntry::Item {
                id: 6,
                text: "exit".into(),
                checked: false,
                enabled: true,
            },
        ];
        let menu = build_menu(&entries).expect("menu");
        // SAFETY: valid menu handle.
        assert_eq!(unsafe { GetMenuItemCount(menu.handle()) }, 4);
        let info = |pos: u32| {
            // SAFETY: MENUITEMINFOW is plain data; we ask for id, state and type by position.
            unsafe {
                let mut mi: MENUITEMINFOW = std::mem::zeroed();
                mi.cbSize = std::mem::size_of::<MENUITEMINFOW>() as u32;
                mi.fMask = MIIM_ID | MIIM_STATE | MIIM_FTYPE;
                assert_ne!(GetMenuItemInfoW(menu.handle(), pos, 1, &mut mi), 0);
                mi
            }
        };
        let gate = info(0);
        assert_eq!(gate.wID, 1);
        assert_ne!(gate.fState & MFS_CHECKED, 0);
        assert_eq!(gate.fState & MFS_DISABLED, 0);
        let side = info(1);
        assert_eq!(side.wID, 2);
        assert_ne!(side.fState & MFS_DISABLED, 0, "greyed when disabled");
        assert_ne!(info(2).fType & MFT_SEPARATOR, 0);
        assert_eq!(info(3).wID, 6);
        assert_eq!(menu_item_flags(true, false), MF_STRING | MF_CHECKED | MF_GRAYED);
    }

    #[test]
    fn s_win_notify_data_cuts_tip_and_balloon_and_sets_title_and_icon_kind() {
        let long: String = "x".repeat(300);
        let nid = icon_data(std::ptr::null_mut(), std::ptr::null_mut(), &long);
        assert_eq!(nid.uFlags, NIF_MESSAGE | NIF_ICON | NIF_TIP);
        assert_eq!(nid.uCallbackMessage, WM_APP_TRAY);
        assert_eq!(nid.szTip.iter().position(|&c| c == 0), Some(127), "tip <= 127 chars");
        let b = balloon_data(std::ptr::null_mut(), &long, true, 8000);
        assert_eq!(b.uFlags, NIF_INFO);
        assert_eq!(b.dwInfoFlags, NIIF_WARNING);
        assert_eq!(b.szInfo.iter().position(|&c| c == 0), Some(255));
        assert_eq!(crate::wide::from_wide(&b.szInfoTitle), "MouseFence");
        // SAFETY: reading the union field we wrote.
        assert_eq!(unsafe { b.Anonymous.uTimeout }, 8000);
        assert_eq!(
            balloon_data(std::ptr::null_mut(), "i", false, 1000).dwInfoFlags,
            NIIF_INFO
        );
    }

    #[test]
    fn s_win_window_turns_messages_into_events_outside_the_window_procedure() {
        let w = TrayWindow::create(WindowKind::MessageOnly).expect("message-only window");
        // SAFETY: posting to our own window.
        unsafe {
            PostMessageW(w.hwnd(), WM_APP_TRAY, 1, WM_LBUTTONDBLCLK as isize);
            PostMessageW(w.hwnd(), WM_APP_TRAY, 1, WM_MOUSEMOVE as isize);
            PostMessageW(w.hwnd(), WM_DISPLAYCHANGE, 32, 0);
        }
        let hwnd_value = w.hwnd_value();
        std::thread::spawn(move || {
            assert!(post_settings_exit(hwnd_value, 0));
            assert!(post_settings_exit(hwnd_value, -1073741819)); // a crash code survives the round trip
        })
        .join()
        .unwrap();
        assert_eq!(
            w.pump_pending(),
            vec![
                WindowEvent::TrayDoubleClick,
                WindowEvent::DisplayChanged,
                WindowEvent::SettingsExited(0),
                WindowEvent::SettingsExited(-1073741819),
            ]
        );
    }

    #[test]
    fn s_win_window_timer_fires_and_stops() {
        let w = TrayWindow::create(WindowKind::MessageOnly).expect("message-only window");
        assert!(w.set_timer(7, 10));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut got = Vec::new();
        while got.is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(15));
            got = w.pump_pending();
        }
        assert!(
            got.contains(&WindowEvent::Timer(7)),
            "WM_TIMER arrives as Timer(id): {got:?}"
        );
        w.kill_timer(7);
        let _ = w.pump_pending();
        std::thread::sleep(std::time::Duration::from_millis(60));
        assert!(
            !w.pump_pending().contains(&WindowEvent::Timer(7)),
            "no tick after KillTimer"
        );
    }
}
