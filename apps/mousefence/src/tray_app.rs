//! The resident tray process: single instance, the hidden tray window, the hook thread, the controller and
//! the message loop. Never run by any automated test (it installs the real hook): the owner's hand checks
//! H01-H09 are its first runs.

use std::path::PathBuf;
use std::rc::Rc;

use fence_app::{Controller, Flow};
use fence_engine::decide::ClassicEngine;
use fence_types::EngineConfig;
use fence_win::cursor::SystemCursor;
use fence_win::hook::{HookThread, LowLevelMouseHook};
use fence_win::single_instance::{self, InstanceCheck};
use fence_win::tray::{TrayWindow, WindowKind};

use crate::adapters;
use crate::store::FileStore;

pub const EXIT_INSTANCE_CHECK_FAILED: i32 = 4;
pub const EXIT_NO_WINDOW: i32 = 5;
pub const EXIT_NO_HOOK_THREAD: i32 = 6;

/// What the single-instance result means for the process: `Some(code)` = exit now.
pub fn instance_exit_code(check: &InstanceCheck) -> Option<i32> {
    match check {
        InstanceCheck::Acquired(_) => None,
        InstanceCheck::AlreadyRunning => Some(0), // C#: `if (!createdNew) return;`, silently
        InstanceCheck::Failed(_) => Some(EXIT_INSTANCE_CHECK_FAILED),
    }
}

pub fn run() -> i32 {
    if let Some(root) = fence_win::appdata_dir() {
        fence_win::error_log::install_panic_hook(root.join("MouseFence/v2/error.log"));
    }
    fence_win::system::enable_per_monitor_dpi_awareness();

    let instance = single_instance::acquire(fence_win::SINGLE_INSTANCE_MUTEX);
    if let Some(code) = instance_exit_code(&instance) {
        if let InstanceCheck::Failed(err) = instance {
            fence_win::debug_log(&format!("single-instance check failed (Win32 error {err})"));
        }
        return code;
    }

    let window = match TrayWindow::create(WindowKind::HiddenTopLevel) {
        Ok(w) => Rc::new(w),
        Err(e) => {
            fence_win::debug_log(&format!("tray window could not be created: {e}"));
            return EXIT_NO_WINDOW;
        }
    };

    let hook = match HookThread::spawn(
        Box::new(ClassicEngine::new(EngineConfig::default())),
        Box::new(LowLevelMouseHook::default()),
        Box::new(SystemCursor),
    ) {
        Ok(t) => t,
        Err(e) => {
            fence_win::debug_log(&format!("hook thread could not start: {e}"));
            return EXIT_NO_HOOK_THREAD;
        }
    };
    // Startup diagnostics are outside callbacks and are never executed by automated tests.
    if let Some(root) = fence_win::appdata_dir() {
        let context = hook.shared().context.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let _ = fence_win::error_log::append(&root.join("MouseFence/v2/error.log"), "hook startup", &context);
    }

    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("mousefence.exe"));
    let exe_dir = exe.parent().map(PathBuf::from).unwrap_or_default();
    let store = FileStore::new(
        &fence_win::appdata_dir().unwrap_or_else(|| exe_dir.clone()),
        &fence_win::local_appdata_dir().unwrap_or(exe_dir),
    );

    let mut controller = Controller::new(adapters::ports(window.clone(), hook, store, exe));
    controller.start();
    window.run(|ev| match adapters::to_event(ev) {
        Some(event) => controller.handle(event) == Flow::Continue,
        None => true,
    });
    // Dropping the controller drops the guard and with it the hook thread (unhook, quit, join), before the
    // window and the single-instance mutex go.
    drop(controller);
    drop(window);
    drop(instance);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s_app_a_second_instance_exits_silently_with_zero() {
        assert_eq!(instance_exit_code(&InstanceCheck::AlreadyRunning), Some(0));
        assert_eq!(
            instance_exit_code(&InstanceCheck::Failed(5)),
            Some(EXIT_INSTANCE_CHECK_FAILED)
        );
    }
}
