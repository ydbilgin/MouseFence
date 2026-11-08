//! The real cursor. `SystemCursor::set_cursor_pos` is the only SetCursorPos call in MouseFence and runs only
//! inside the real hook step; no test constructs a `SystemCursor` for placing (PLAN T19).

use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetCursorPos, SetCursorPos};

use crate::hook::CursorPort;

/// GetCursorPos (read-only). `None` when Windows refuses (for example on a desktop that is not the input
/// desktop).
pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut p = POINT { x: 0, y: 0 };
    // SAFETY: p is a valid out-pointer.
    if unsafe { GetCursorPos(&mut p) } != 0 {
        Some((p.x, p.y))
    } else {
        None
    }
}

/// The production cursor for the hook thread.
#[derive(Debug, Default)]
pub struct SystemCursor;

impl CursorPort for SystemCursor {
    fn set_cursor_pos(&mut self, x: i32, y: i32) -> bool {
        // SAFETY: plain Win32 call. Moves the real cursor: production hook step only.
        unsafe { SetCursorPos(x, y) != 0 }
    }

    fn cursor_pos(&self) -> Option<(i32, i32)> {
        cursor_pos()
    }
}
