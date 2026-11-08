//! One tray per user session (C# Program.cs:71-72): a named mutex, taken with initial ownership; when it
//! already exists another instance runs and this one exits silently with code 0. 1.x and 2.0 share
//! [`crate::SINGLE_INSTANCE_MUTEX`], so they can never enforce at the same time. Tests use their own names.

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows_sys::Win32::System::Threading::{CreateMutexW, ReleaseMutex};

use crate::wide::to_wide;

/// The held mutex; released and closed on drop (process exit in production).
#[derive(Debug)]
pub struct SingleInstance(usize);

impl Drop for SingleInstance {
    fn drop(&mut self) {
        // SAFETY: the handle came from CreateMutexW and is closed once.
        unsafe {
            ReleaseMutex(self.0 as HANDLE);
            CloseHandle(self.0 as HANDLE);
        }
    }
}

#[derive(Debug)]
pub enum InstanceCheck {
    Acquired(SingleInstance),
    /// Another instance holds the name (C# `createdNew == false`). An access-denied open (an instance of
    /// another integrity level) counts as running too.
    AlreadyRunning,
    /// CreateMutex failed for another reason (Win32 error code).
    Failed(u32),
}

/// C# `new Mutex(initiallyOwned: true, name, out createdNew)`.
pub fn acquire(name: &str) -> InstanceCheck {
    let w = to_wide(name);
    // SAFETY: NUL-terminated name, default security.
    let (h, err) = unsafe {
        let h = CreateMutexW(std::ptr::null(), 1, w.as_ptr());
        (h, GetLastError())
    };
    if h.is_null() {
        return if err == ERROR_ACCESS_DENIED {
            InstanceCheck::AlreadyRunning
        } else {
            InstanceCheck::Failed(err)
        };
    }
    if err == ERROR_ALREADY_EXISTS {
        // SAFETY: the handle was just opened.
        unsafe { CloseHandle(h) };
        return InstanceCheck::AlreadyRunning;
    }
    InstanceCheck::Acquired(SingleInstance(h as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_name(tag: &str) -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("MouseFenceTest_SingleInstance_{}_{tag}_{nanos}", std::process::id())
    }

    #[test]
    fn b017_second_instance_sees_the_first_and_the_name_frees_on_exit() {
        let name = test_name("b017");
        assert_ne!(name, crate::SINGLE_INSTANCE_MUTEX, "never the real name");
        let first = match acquire(&name) {
            InstanceCheck::Acquired(g) => g,
            other => panic!("first instance must acquire: {other:?}"),
        };
        assert!(
            matches!(acquire(&name), InstanceCheck::AlreadyRunning),
            "a second instance must exit"
        );
        drop(first);
        assert!(
            matches!(acquire(&name), InstanceCheck::Acquired(_)),
            "the name frees when the first exits"
        );
    }

    #[test]
    fn b017_production_name_is_the_1x_name() {
        assert_eq!(crate::SINGLE_INSTANCE_MUTEX, "MouseFence_SingleInstance_2C7A");
    }
}
