//! Global hotkeys (port of C# `Platform/HotKeyWindow.cs`). 1.x used one hidden window per hotkey with the
//! id 0xB10C; 2.0 registers the four on the tray window with ids 0xB10C.. so WM_HOTKEY names the slot.
//!
//! `register` unregisters the id first, refuses a chord without a key or without a modifier (and then
//! registers nothing), and adds MOD_NOREPEAT. Tests register only F24 chords and fire them by posting
//! WM_HOTKEY to their own window; no key is ever sent.

use fence_types::Hotkey;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey};

/// The 1.x hotkey id; slot `n` uses `HOTKEY_ID_BASE + n`.
pub const HOTKEY_ID_BASE: i32 = 0xB10C;

/// Read-only state of either Windows key. egui-winit deliberately omits Super on Windows.
pub fn windows_key_down() -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LWIN, VK_RWIN};
    // SAFETY: read-only keyboard state, no input is sent.
    unsafe { GetAsyncKeyState(VK_LWIN as i32) < 0 || GetAsyncKeyState(VK_RWIN as i32) < 0 }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistrationError {
    Incomplete,
    Windows(u32),
}

/// The `fsModifiers` for a chord, or `None` when it has no key or no modifier (C# `modifiers == 0 || vk == 0`).
pub fn register_modifiers(hotkey: &Hotkey) -> Option<u32> {
    let m = hotkey.modifiers();
    if m == 0 || hotkey.vk == 0 {
        None
    } else {
        Some(m | MOD_NOREPEAT)
    }
}

/// C# `HotKeyWindow.Register`: unregister the id, then register with MOD_NOREPEAT. False when the chord is
/// incomplete or Windows refused it (another app holds it). `hwnd` is the window handle as a number
/// (`TrayWindow::hwnd_value`); Windows validates it, Rust never dereferences it.
pub fn register(hwnd: usize, id: i32, hotkey: &Hotkey) -> bool {
    register_detailed(hwnd, id, hotkey).is_ok()
}

/// The registration result includes Windows' conflict error while always releasing the old chord first.
pub fn register_detailed(hwnd: usize, id: i32, hotkey: &Hotkey) -> Result<(), RegistrationError> {
    let hwnd = hwnd as HWND;
    // SAFETY: plain Win32 calls; an invalid handle makes them fail, nothing is dereferenced here.
    unsafe { UnregisterHotKey(hwnd, id) };
    let Some(mods) = register_modifiers(hotkey) else {
        return Err(RegistrationError::Incomplete);
    };
    // SAFETY: as above.
    if unsafe { RegisterHotKey(hwnd, id, mods, hotkey.vk) } != 0 {
        Ok(())
    } else {
        // SAFETY: immediately reads this thread's last Win32 error.
        Err(RegistrationError::Windows(unsafe { GetLastError() }))
    }
}

/// C# `HotKeyWindow.Unregister`.
pub fn unregister(hwnd: usize, id: i32) {
    // SAFETY: plain Win32 call; unregistering an unknown id is harmless.
    unsafe { UnregisterHotKey(hwnd as HWND, id) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tray::{TrayWindow, WindowEvent, WindowKind};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, VK_F24};
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_HOTKEY};

    const F24: u32 = VK_F24 as u32;

    fn chord(ctrl: bool, alt: bool, shift: bool, win: bool) -> Hotkey {
        Hotkey {
            vk: F24,
            ctrl,
            alt,
            shift,
            win,
        }
    }

    #[test]
    fn b020_modifiers_add_norepeat_and_refuse_incomplete_chords() {
        assert_eq!(
            register_modifiers(&chord(true, true, false, false)),
            Some(MOD_CONTROL | MOD_ALT | MOD_NOREPEAT)
        );
        assert_eq!(
            register_modifiers(&chord(false, false, true, true)),
            Some(MOD_SHIFT | MOD_WIN | MOD_NOREPEAT)
        );
        assert_eq!(
            register_modifiers(&chord(false, false, false, false)),
            None,
            "no modifier"
        );
        assert_eq!(
            register_modifiers(&Hotkey {
                vk: 0,
                ctrl: true,
                ..Hotkey::default()
            }),
            None,
            "no key"
        );
    }

    #[test]
    fn b020_register_reregister_and_posted_wm_hotkey_on_f24_chords() {
        // Two message-only windows (never shown). F24 chords only; WM_HOTKEY is posted, no key is sent.
        let a = TrayWindow::create(WindowKind::MessageOnly).expect("window a");
        let b = TrayWindow::create(WindowKind::MessageOnly).expect("window b");
        let id = HOTKEY_ID_BASE;
        let c1 = chord(true, true, true, false); // Ctrl+Alt+Shift+F24
        let c2 = chord(true, false, true, false); // Ctrl+Shift+F24

        assert!(register(a.hwnd_value(), id, &c1), "first registration");
        assert_eq!(
            register_detailed(b.hwnd_value(), id, &c1),
            Err(RegistrationError::Windows(1409)),
            "the chord is held by window a"
        );
        assert!(
            register(a.hwnd_value(), id, &c2),
            "re-register the same id with another chord"
        );
        assert!(
            register(b.hwnd_value(), id + 1, &c1),
            "re-register unregistered the old chord first, so c1 is free again"
        );
        unregister(b.hwnd_value(), id + 1);

        // Incomplete chords register nothing, and still free what the id held.
        assert!(!register(a.hwnd_value(), id, &chord(false, false, false, false)));
        assert!(
            register(b.hwnd_value(), id + 2, &c2),
            "c2 was freed by the refused registration"
        );
        unregister(b.hwnd_value(), id + 2);

        // WM_HOTKEY with our id reaches the window as a Hotkey event.
        assert!(register(a.hwnd_value(), id + 3, &c1));
        // SAFETY: posting to our own window.
        unsafe { PostMessageW(a.hwnd(), WM_HOTKEY, (id + 3) as usize, 0) };
        let events = a.pump_pending();
        assert_eq!(events, vec![WindowEvent::Hotkey(id + 3)]);
        unregister(a.hwnd_value(), id + 3);
    }
}
