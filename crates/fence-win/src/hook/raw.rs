//! Message-only background mouse receiver; never creates a visible/focusable window.
use windows_sys::Win32::{
    Foundation::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Input::*, WindowsAndMessaging::*},
};

#[derive(Debug)]
pub(super) struct Listener {
    hwnd: usize,
}

impl Listener {
    pub fn new() -> Option<Self> {
        let name: Vec<u16> = "MouseFence.RawMouse.2\0".encode_utf16().collect();
        // SAFETY: class/window pointers live through these synchronous calls; window is message-only.
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class = WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance,
                lpszClassName: name.as_ptr(),
                ..std::mem::zeroed()
            };
            RegisterClassW(&class);
            let hwnd = CreateWindowExW(
                0,
                name.as_ptr(),
                name.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                return None;
            }
            let device = RAWINPUTDEVICE {
                usUsagePage: 1,
                usUsage: 2,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            };
            if RegisterRawInputDevices(&device, 1, std::mem::size_of::<RAWINPUTDEVICE>() as u32) == 0 {
                DestroyWindow(hwnd);
                return None;
            }
            Some(Self { hwnd: hwnd as usize })
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        // SAFETY: only this listener owns the registration and its message-only window.
        unsafe {
            let device = RAWINPUTDEVICE {
                usUsagePage: 1,
                usUsage: 2,
                dwFlags: RIDEV_REMOVE,
                hwndTarget: std::ptr::null_mut(),
            };
            RegisterRawInputDevices(&device, 1, std::mem::size_of::<RAWINPUTDEVICE>() as u32);
            DestroyWindow(self.hwnd as HWND);
        }
    }
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // No Rust unwind may escape Win32, including an engine fault on raw input.
    let _ = std::panic::catch_unwind(|| {
        if msg == WM_INPUT {
            // SAFETY: fixed, aligned RAWINPUT storage. Mouse reports fit; oversized/HID reports are ignored.
            unsafe {
                let mut raw: RAWINPUT = std::mem::zeroed();
                let mut size = std::mem::size_of::<RAWINPUT>() as u32;
                let read = GetRawInputData(
                    lp as HRAWINPUT,
                    RID_INPUT,
                    (&mut raw as *mut RAWINPUT).cast(),
                    &mut size,
                    std::mem::size_of::<RAWINPUTHEADER>() as u32,
                );
                if read != u32::MAX
                    && read >= std::mem::size_of::<RAWINPUTHEADER>() as u32
                    && raw.header.dwType == RIM_TYPEMOUSE
                {
                    let mouse = raw.data.mouse;
                    if mouse.usFlags & MOUSE_MOVE_ABSOLUTE == 0 {
                        super::thread::raw_mouse(
                            mouse.lLastX,
                            mouse.lLastY,
                            mouse.Anonymous.Anonymous.usButtonFlags,
                            crate::qpc::now_us(),
                        );
                    }
                }
            }
        }
    });
    // SAFETY: required cleanup for WM_INPUT, plus default handling of other messages.
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}
