//! Start with Windows (port of C# `Platform/AutoStart.cs`): the per-user Run value "MouseFence" holds the
//! quoted exe path when enabled and is deleted when disabled; every error is non-fatal.
//!
//! The key path is a parameter so tests write a throwaway `HKCU\Software\MouseFenceTest-<pid>` key and never
//! the real Run key (brief: no Run entry). Only the tray's settings-save path passes [`RUN_KEY`].

use std::path::Path;

use windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW,
};

use crate::wide::to_wide;

/// `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (C# `RunKey`).
pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// The value name (C# `ValueName`), shared with 1.x on purpose: one autostart entry for the product.
pub const RUN_VALUE: &str = "MouseFence";

/// C# `$"\"{Application.ExecutablePath}\""`.
pub fn quoted_exe(exe: &Path) -> String {
    format!("\"{}\"", exe.display())
}

/// What happened (the caller ignores errors, as 1.x did; tests read it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoStartResult {
    Written,
    Deleted,
    /// The key does not exist (C# `OpenSubKey` returned null and Apply returned).
    NoKey,
    /// A Win32 error code (C# swallowed the exception).
    Failed(u32),
}

/// C# `AutoStart.Apply(enable)` against `HKCU\<key_path>`.
pub fn apply(key_path: &str, value_name: &str, enable: bool, exe: &Path) -> AutoStartResult {
    let path = to_wide(key_path);
    let name = to_wide(value_name);
    let mut key: HKEY = std::ptr::null_mut();
    // SAFETY: valid NUL-terminated path and out-pointer.
    let rc = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, KEY_SET_VALUE, &mut key) };
    if rc != 0 {
        return if rc == ERROR_FILE_NOT_FOUND {
            AutoStartResult::NoKey
        } else {
            AutoStartResult::Failed(rc)
        };
    }
    let result = if enable {
        let data = to_wide(&quoted_exe(exe));
        // SAFETY: REG_SZ data is the NUL-terminated UTF-16 string, its byte length passed.
        let rc = unsafe {
            RegSetValueExW(
                key,
                name.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr() as *const u8,
                (data.len() * 2) as u32,
            )
        };
        if rc == 0 {
            AutoStartResult::Written
        } else {
            AutoStartResult::Failed(rc)
        }
    } else {
        // SAFETY: valid key and name.
        let rc = unsafe { RegDeleteValueW(key, name.as_ptr()) };
        if rc == 0 || rc == ERROR_FILE_NOT_FOUND {
            AutoStartResult::Deleted
        } else {
            AutoStartResult::Failed(rc)
        }
    };
    // SAFETY: the key was opened above.
    unsafe { RegCloseKey(key) };
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::System::Registry::{
        KEY_WRITE, REG_OPTION_NON_VOLATILE, RRF_RT_REG_SZ, RegCreateKeyExW, RegDeleteKeyW, RegDeleteTreeW, RegGetValueW,
    };

    /// The throwaway key every autostart test uses; asserted never to be the Run key.
    fn test_key() -> String {
        format!(r"Software\MouseFenceTest-{}", std::process::id())
    }

    fn create_key(path: &str) {
        let p = to_wide(path);
        let mut key: HKEY = std::ptr::null_mut();
        // SAFETY: valid path and out-pointers.
        let rc = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                p.as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        assert_eq!(rc, 0, "create the throwaway test key");
        // SAFETY: opened above.
        unsafe { RegCloseKey(key) };
    }

    fn read_value(path: &str, name: &str) -> Option<String> {
        let p = to_wide(path);
        let n = to_wide(name);
        let mut buf = [0u16; 1024];
        let mut size = (buf.len() * 2) as u32;
        // SAFETY: valid names; buf and its byte size.
        let rc = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                p.as_ptr(),
                n.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buf.as_mut_ptr() as *mut core::ffi::c_void,
                &mut size,
            )
        };
        (rc == 0).then(|| crate::wide::from_wide(&buf))
    }

    fn delete_key(path: &str) {
        assert!(
            path.starts_with(r"Software\MouseFenceTest-"),
            "tests only delete their own key"
        );
        let p = to_wide(path);
        // SAFETY: valid path under HKCU.
        unsafe {
            RegDeleteTreeW(HKEY_CURRENT_USER, p.as_ptr());
            RegDeleteKeyW(HKEY_CURRENT_USER, p.as_ptr());
        }
    }

    #[test]
    fn b019_autostart_writes_the_quoted_path_deletes_it_and_never_fails_loudly() {
        let key = test_key();
        assert_ne!(key, RUN_KEY, "never the real Run key");
        assert!(!key.contains(r"CurrentVersion\Run"));
        let temp_exe = std::env::temp_dir().join("Synthetic Folder").join("mousefence.exe");
        let exe = temp_exe.as_path();

        // Missing key: nothing happens, no panic (C# OpenSubKey null -> return).
        assert_eq!(apply(&key, RUN_VALUE, true, exe), AutoStartResult::NoKey);

        create_key(&key);
        assert_eq!(apply(&key, RUN_VALUE, true, exe), AutoStartResult::Written);
        assert_eq!(
            read_value(&key, RUN_VALUE).as_deref(),
            Some(format!("\"{}\"", exe.display()).as_str())
        );
        assert_eq!(apply(&key, RUN_VALUE, false, exe), AutoStartResult::Deleted);
        assert_eq!(read_value(&key, RUN_VALUE), None);
        // Deleting a missing value is fine (throwOnMissingValue: false).
        assert_eq!(apply(&key, RUN_VALUE, false, exe), AutoStartResult::Deleted);
        delete_key(&key);
        assert_eq!(
            apply(&key, RUN_VALUE, false, exe),
            AutoStartResult::NoKey,
            "the test key is gone"
        );
    }

    #[test]
    fn b019_value_name_and_key_match_1x() {
        assert_eq!(RUN_KEY, r"Software\Microsoft\Windows\CurrentVersion\Run");
        assert_eq!(RUN_VALUE, "MouseFence");
        let exe = std::env::temp_dir().join("a b").join("c.exe");
        assert_eq!(quoted_exe(&exe), format!("\"{}\"", exe.display()));
    }
}
