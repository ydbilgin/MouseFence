//! Bind only the dedicated hook thread, before it owns windows/hooks. Never switch the visible desktop,
//! change window stations, elevate, or move another process between sessions.
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, HANDLE},
    Security::{
        GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
        TokenIntegrityLevel,
    },
    System::{
        RemoteDesktop::ProcessIdToSessionId,
        StationsAndDesktops::*,
        Threading::{GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId, OpenProcessToken},
    },
};

#[derive(Debug)]
pub(super) struct InputDesktop {
    previous: usize,
    input: usize,
}

fn name(handle: HANDLE) -> String {
    // source: diagnostic-only default buffer size; overlong object names are reported as unknown.
    let mut buffer = [0u16; 256];
    let mut needed = 0;
    // SAFETY: buffer is writable and its size is passed in bytes. No user content is read.
    if unsafe { GetUserObjectInformationW(handle, UOI_NAME, buffer.as_mut_ptr().cast(), 512, &mut needed) } == 0 {
        return "unknown".into();
    }
    String::from_utf16_lossy(&buffer[..buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len())])
}

pub(super) fn context() -> String {
    let mut session = u32::MAX;
    // SAFETY: read-only process/thread and desktop object queries; no handles are owned here.
    unsafe {
        ProcessIdToSessionId(GetCurrentProcessId(), &mut session);
        format!(
            "pid={} tid={} session={} station={} desktop={} integrity_rid={:?}",
            GetCurrentProcessId(),
            GetCurrentThreadId(),
            session,
            name(GetProcessWindowStation()),
            name(GetThreadDesktop(GetCurrentThreadId())),
            integrity_rid()
        )
    }
}

/// Log only the mandatory integrity RID, never a token value, user SID, account name, or credential.
fn integrity_rid() -> Option<u32> {
    let mut token = std::ptr::null_mut();
    // SAFETY: query-only access to our own token. Aligned storage holds the OS-returned label and SID.
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return None;
        }
        let result = (|| {
            let mut needed = 0;
            GetTokenInformation(token, TokenIntegrityLevel, std::ptr::null_mut(), 0, &mut needed);
            if needed == 0 {
                return None;
            }
            let mut buffer = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
            if GetTokenInformation(
                token,
                TokenIntegrityLevel,
                buffer.as_mut_ptr().cast(),
                needed,
                &mut needed,
            ) == 0
            {
                return None;
            }
            let label = &*buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>();
            let count = *GetSidSubAuthorityCount(label.Label.Sid);
            (count > 0).then(|| *GetSidSubAuthority(label.Label.Sid, u32::from(count - 1)))
        })();
        CloseHandle(token);
        result
    }
}

impl InputDesktop {
    pub(super) fn attach() -> Result<Self, String> {
        let before = context();
        let mut session = u32::MAX;
        // source: Windows session-0 isolation and interactive window-station model. Do not attempt to
        // bridge sessions or grant desktop rights; a service must be launched in the user's session.
        // https://learn.microsoft.com/en-us/windows/win32/winstation/window-stations
        unsafe {
            ProcessIdToSessionId(GetCurrentProcessId(), &mut session);
        }
        if session == 0 || !name(unsafe { GetProcessWindowStation() }).eq_ignore_ascii_case("WinSta0") {
            return Err(format!("{before} interactive session/window station required"));
        }
        // SAFETY: opens the input desktop in this process's window station, without activating it.
        let input = unsafe {
            OpenInputDesktop(
                0,
                0,
                DESKTOP_READOBJECTS | DESKTOP_WRITEOBJECTS | DESKTOP_HOOKCONTROL | DESKTOP_CREATEWINDOW,
            )
        };
        if input.is_null() {
            // SAFETY: read immediately after the failing Win32 call.
            return Err(format!("{before} OpenInputDesktop error={}", unsafe { GetLastError() }));
        }
        // SAFETY: current thread has not created windows/hooks. This changes only its association.
        let previous = unsafe { GetThreadDesktop(GetCurrentThreadId()) };
        if unsafe { SetThreadDesktop(input) } == 0 {
            // SAFETY: preserve error before closing the newly opened, unbound handle.
            let error = unsafe { GetLastError() };
            let input_name = name(input);
            unsafe { CloseDesktop(input) };
            return Err(format!("{before} input={input_name} SetThreadDesktop error={error}"));
        }
        Ok(Self {
            previous: previous as usize,
            input: input as usize,
        })
    }
}

impl Drop for InputDesktop {
    fn drop(&mut self) {
        // SAFETY: finish_thread runs after unhooking and destroying its raw-input window, on the same
        // thread that attached. The original desktop handle is borrowed; only input is ours to close.
        unsafe {
            SetThreadDesktop(self.previous as HDESK);
            CloseDesktop(self.input as HDESK);
        }
    }
}
