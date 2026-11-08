//! The hook thread (PLAN T9, B027): the low-level mouse hook gets its own thread and message loop at NORMAL
//! priority, so no tray work (menus, balloons, settings reloads) can delay a mouse move. 1.x ran the hook on
//! the UI thread.
//!
//! The tray thread drives it with commands over a channel, woken by a posted thread message; the replies it
//! waits for (install, uninstall) come from the hook thread, never the other way round: the hook callback
//! never waits on the tray. Toggles travel through the atomics in [`HookShared`].
//!
//! The installer and the cursor are injected: production passes [`LowLevelMouseHook`] and
//! `cursor::SystemCursor`; tests pass fakes, so no test installs a real hook or moves the cursor.

use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

use fence_types::Decider;
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{
    GetCurrentThread, GetCurrentThreadId, GetThreadPriority, SetThreadPriority, THREAD_PRIORITY_NORMAL,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, MSG, MSLLHOOKSTRUCT, PM_NOREMOVE, PeekMessageW, PostThreadMessageW,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_MOUSE_LL, WM_APP,
};

use super::core::{CursorPort, HookCore, HookEvent, HookShared, StepOutcome};

/// The thread message that tells the hook thread to drain its command channel.
const WM_APP_HOOK_COMMAND: u32 = WM_APP + 0x31;
/// How long the tray thread waits for an install/uninstall reply before it assumes failure.
const REPLY_TIMEOUT: Duration = Duration::from_secs(5);

/// Installs and removes the real hook. Called on the hook thread only.
pub trait HookInstaller: Send {
    fn diagnostic(&self) -> String {
        "fake installer".into()
    }
    /// Runs before this thread creates any window or hook. Fakes never change desktops.
    fn prepare_thread(&mut self) -> bool {
        true
    }
    fn finish_thread(&mut self) {}
    /// `SetWindowsHookEx(WH_MOUSE_LL, ...)`; true when a handle is held afterwards.
    fn install(&mut self) -> bool;
    /// `UnhookWindowsHookEx`; safe when nothing is installed.
    fn uninstall(&mut self);
}

/// The production installer: a real `WH_MOUSE_LL` hook whose callback runs [`HookCore::step`].
/// Never used by tests (PLAN T19); the owner's hand check H01 is the only run of a real hook.
#[derive(Debug, Default)]
pub struct LowLevelMouseHook {
    handle: usize,
    raw: Option<super::raw::Listener>,
    desktop: Option<super::desktop::InputDesktop>,
    diagnostic: String,
}

impl HookInstaller for LowLevelMouseHook {
    fn diagnostic(&self) -> String {
        self.diagnostic.clone()
    }
    fn prepare_thread(&mut self) -> bool {
        let inherited = super::desktop::context();
        match super::desktop::InputDesktop::attach() {
            Ok(desktop) => {
                self.desktop = Some(desktop);
                self.diagnostic = format!("inherited=[{inherited}] bound=[{}]", super::desktop::context());
                true
            }
            Err(error) => {
                crate::debug_log(&format!("hook desktop unavailable: {error}"));
                self.diagnostic = error;
                false
            }
        }
    }
    fn finish_thread(&mut self) {
        self.desktop = None;
    }
    fn install(&mut self) -> bool {
        if self.handle != 0 {
            return true;
        }
        // SAFETY: plain Win32 calls; the callback is a static `extern "system"` fn that never unwinds.
        let h = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(ll_mouse_proc), GetModuleHandleW(std::ptr::null()), 0) };
        self.handle = h as usize;
        if self.handle == 0 {
            // SAFETY: read immediately after SetWindowsHookExW failed.
            self.diagnostic = format!("{} SetWindowsHookExW error={}", super::desktop::context(), unsafe {
                windows_sys::Win32::Foundation::GetLastError()
            });
        }
        if self.handle != 0 {
            self.raw = super::raw::Listener::new();
            if self.raw.is_none() {
                crate::debug_log("raw mouse unavailable; Intent crossings remain held");
            }
        }
        self.handle != 0
    }

    fn uninstall(&mut self) {
        if self.handle != 0 {
            // SAFETY: the handle came from SetWindowsHookExW on this thread.
            unsafe { UnhookWindowsHookEx(self.handle as _) };
            self.handle = 0;
            self.raw = None;
        }
    }
}

/// What the hook thread reports about itself (tests prove T9 with it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HookThreadInfo {
    pub thread_id: u32,
    /// `GetThreadPriority` on the hook thread (0 = THREAD_PRIORITY_NORMAL).
    pub priority: i32,
}

enum Command {
    Install(SyncSender<bool>),
    Uninstall(SyncSender<()>),
    ResetEngine,
    SetEngine(Box<dyn Decider>),
    Info(SyncSender<HookThreadInfo>),
    Simulate(HookEvent, SyncSender<StepOutcome>),
    CursorPos(SyncSender<Option<(i32, i32)>>),
    Retire(SyncSender<ThreadResources>),
    Quit,
}

struct ThreadResources {
    ctx: HookCtx,
    installer: Box<dyn HookInstaller>,
}

enum ThreadControl {
    Continue,
    Quit,
    Retire(SyncSender<ThreadResources>),
}

struct HookCtx {
    core: HookCore,
    cursor: Box<dyn CursorPort + Send>,
}

thread_local! {
    /// The step and its cursor, owned by the hook thread. The callback and the command handler both run on
    /// this thread (Windows calls a low-level hook on the thread that installed it, inside GetMessage), so a
    /// RefCell is enough; a re-entrant call finds it borrowed and passes the event.
    static CTX: RefCell<Option<HookCtx>> = const { RefCell::new(None) };
    /// The counters, reachable even when CTX is borrowed (re-entrant callback).
    static SHARED: OnceLock<Arc<HookShared>> = const { OnceLock::new() };
}

/// Read the callback arguments. `l_param` is read only for `n_code >= 0` (HC_ACTION).
///
/// # Safety
/// For `n_code >= 0`, `l_param` must point to a valid `MSLLHOOKSTRUCT` (Windows guarantees this).
unsafe fn event_from_raw(n_code: i32, w_param: WPARAM, l_param: LPARAM, qpc_us: u64) -> HookEvent {
    let mut ev = HookEvent {
        n_code,
        message: w_param as u32,
        x: 0,
        y: 0,
        flags: 0,
        time_ms: 0,
        qpc_us,
    };
    if n_code >= 0 && l_param != 0 {
        // SAFETY: guaranteed by the caller contract above.
        let data = unsafe { std::ptr::read(l_param as *const MSLLHOOKSTRUCT) };
        ev.x = data.pt.x;
        ev.y = data.pt.y;
        ev.flags = data.flags;
        ev.time_ms = data.time;
    }
    ev
}

/// The real `WH_MOUSE_LL` callback. Every path is contained: a panic can never cross this boundary.
unsafe extern "system" fn ll_mouse_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    let entry_us = crate::qpc::now_us();
    let outcome = catch_unwind(|| {
        // SAFETY: Windows passes a valid MSLLHOOKSTRUCT for n_code >= 0.
        let ev = unsafe { event_from_raw(n_code, w_param, l_param, entry_us) };
        CTX.try_with(|cell| match cell.try_borrow_mut() {
            Ok(mut slot) => match slot.as_mut() {
                Some(ctx) => ctx.core.step(&ev, ctx.cursor.as_mut()),
                None => StepOutcome::CallNext,
            },
            Err(_) => {
                // Re-entered (e.g. while placing the cursor): still count it, then pass it on.
                let _ = SHARED.try_with(|s| {
                    if let Some(s) = s.get() {
                        s.callbacks.fetch_add(1, Ordering::Relaxed);
                    }
                });
                StepOutcome::CallNext
            }
        })
        .unwrap_or(StepOutcome::CallNext)
    })
    .unwrap_or(StepOutcome::CallNext);
    match outcome {
        StepOutcome::Swallow => 1,
        // SAFETY: the documented pass-through; hhk is ignored since Windows 7.
        StepOutcome::CallNext => unsafe { CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param) },
    }
}

/// The calling thread's Win32 id (to compare with [`HookThreadInfo::thread_id`]).
pub fn current_thread_id() -> u32 {
    // SAFETY: no arguments.
    unsafe { GetCurrentThreadId() }
}

/// Handle to the hook thread, held by the tray thread.
pub struct HookThread {
    tx: Sender<Command>,
    thread_id: u32,
    shared: Arc<HookShared>,
    join: Option<JoinHandle<()>>,
    done: Receiver<()>,
}

impl HookThread {
    /// Start the thread with its engine, installer and cursor. Nothing is installed until [`Self::install`].
    pub fn spawn(
        engine: Box<dyn Decider>,
        installer: Box<dyn HookInstaller>,
        cursor: Box<dyn CursorPort + Send>,
    ) -> std::io::Result<Self> {
        Self::spawn_shared(engine, installer, cursor, Arc::new(HookShared::default()))
    }

    fn spawn_shared(
        engine: Box<dyn Decider>,
        installer: Box<dyn HookInstaller>,
        cursor: Box<dyn CursorPort + Send>,
        shared: Arc<HookShared>,
    ) -> std::io::Result<Self> {
        crate::qpc::initialize();
        let (tx, rx) = mpsc::channel::<Command>();
        let (ready_tx, ready_rx) = mpsc::sync_channel::<u32>(1);
        let thread_shared = shared.clone();
        let (done_tx, done) = mpsc::sync_channel(1);
        let join = std::thread::Builder::new()
            .name("mousefence-hook".to_string())
            .spawn(move || {
                hook_thread_main(engine, installer, cursor, thread_shared, rx, ready_tx);
                let _ = done_tx.send(());
            })?;
        let thread_id = ready_rx
            .recv_timeout(REPLY_TIMEOUT)
            .map_err(|e| std::io::Error::other(format!("hook thread did not start: {e}")))?;
        Ok(Self {
            tx,
            thread_id,
            shared,
            join: Some(join),
            done,
        })
    }

    pub fn shared(&self) -> &Arc<HookShared> {
        &self.shared
    }

    pub fn thread_id(&self) -> u32 {
        self.thread_id
    }

    /// Retire the old loop before starting its replacement. A non-answering loop is never joined or
    /// duplicated: the caller gets failure after a bounded wait and the guard limits further attempts.
    pub fn restart(&mut self) -> bool {
        let (tx, rx) = mpsc::sync_channel(1);
        if !self.send(Command::Retire(tx)) {
            self.record_failure("retire command could not wake hook thread");
            return false;
        }
        let Ok(resources) = rx.recv_timeout(REPLY_TIMEOUT) else {
            self.record_failure("retire acknowledgement timed out; no replacement started");
            return false;
        };
        if let Some(join) = self.join.take() {
            // Resources are published only after unhooking, destroying the raw window and releasing
            // the desktop. Do not block the tray on a JoinHandle, even at this final boundary.
            drop(join);
        }
        let ThreadResources { ctx, installer } = resources;
        let next = match Self::spawn_shared(ctx.core.into_engine(), installer, ctx.cursor, self.shared.clone()) {
            Ok(next) => next,
            Err(error) => {
                self.record_failure(&format!("replacement thread failed: {error}"));
                return false;
            }
        };
        *self = next;
        self.reset_engine();
        self.install()
    }

    fn record_failure(&self, detail: &str) {
        let mut context = self.shared.context.lock().unwrap_or_else(|e| e.into_inner());
        *context = format!("{detail}; {}", context.as_str());
    }

    /// Probe on the hook desktop; the tray may have inherited a background launcher desktop.
    pub fn cursor_pos(&self) -> Option<(i32, i32)> {
        let (tx, rx) = mpsc::sync_channel(1);
        if !self.send(Command::CursorPos(tx)) {
            return None;
        }
        rx.recv_timeout(REPLY_TIMEOUT).ok().flatten()
    }

    fn send(&self, cmd: Command) -> bool {
        if self.tx.send(cmd).is_err() {
            return false;
        }
        // SAFETY: posting a message with no pointers to a thread we started.
        unsafe { PostThreadMessageW(self.thread_id, WM_APP_HOOK_COMMAND, 0, 0) != 0 }
    }

    /// Install the hook on the hook thread; true when a handle is held. Waits for the reply.
    pub fn install(&self) -> bool {
        let (tx, rx) = mpsc::sync_channel(1);
        self.send(Command::Install(tx));
        rx.recv_timeout(REPLY_TIMEOUT).unwrap_or(false)
    }

    /// Remove the hook. Waits for the reply.
    pub fn uninstall(&self) {
        let (tx, rx) = mpsc::sync_channel(1);
        self.send(Command::Uninstall(tx));
        let _ = rx.recv_timeout(REPLY_TIMEOUT);
    }

    /// C# `_core.Reset()`, run on the hook thread in order with the moves.
    pub fn reset_engine(&self) {
        self.send(Command::ResetEngine);
    }

    /// Replace the engine (built on the caller's thread) and reset it (C# `MouseGuard.Configure`).
    pub fn set_engine(&self, engine: Box<dyn Decider>) {
        self.send(Command::SetEngine(engine));
    }

    /// Thread id and priority, asked on the hook thread itself.
    pub fn info(&self) -> Option<HookThreadInfo> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.send(Command::Info(tx));
        rx.recv_timeout(REPLY_TIMEOUT).ok()
    }

    /// Run one synthetic event through the same step the callback runs, on the hook thread (tests and
    /// diagnostics; with a fake cursor it never moves the real one).
    pub fn simulate(&self, ev: HookEvent) -> Option<StepOutcome> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.send(Command::Simulate(ev, tx));
        rx.recv_timeout(REPLY_TIMEOUT).ok()
    }
}

impl Drop for HookThread {
    fn drop(&mut self) {
        let sent = self.send(Command::Quit);
        if let Some(join) = self.join.take() {
            // Never freeze the tray forever when a loop stopped pumping.
            if sent && self.done.recv_timeout(REPLY_TIMEOUT).is_ok() {
                let _ = join.join();
            }
        }
    }
}

fn hook_thread_main(
    engine: Box<dyn Decider>,
    mut installer: Box<dyn HookInstaller>,
    cursor: Box<dyn CursorPort + Send>,
    shared: Arc<HookShared>,
    rx: Receiver<Command>,
    ready: SyncSender<u32>,
) {
    let prepared = installer.prepare_thread();
    *shared.context.lock().unwrap_or_else(|e| e.into_inner()) = installer.diagnostic();
    // SAFETY: plain Win32 calls on the current thread; MSG is plain data, zero is a valid value.
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_NORMAL);
        // Create this thread's message queue before anyone posts to it.
        PeekMessageW(&mut msg, std::ptr::null_mut(), WM_APP, WM_APP, PM_NOREMOVE);
    }
    SHARED.with(|s| {
        let _ = s.set(shared.clone());
    });
    CTX.with(|c| {
        *c.borrow_mut() = Some(HookCtx {
            core: HookCore::new(engine, shared.clone()),
            cursor,
        })
    });
    // SAFETY: no arguments.
    if ready.send(unsafe { GetCurrentThreadId() }).is_err() {
        installer.finish_thread();
        CTX.with(|c| c.borrow_mut().take());
        return;
    }

    let retire = loop {
        // SAFETY: msg is a valid out-pointer; a low-level hook callback runs inside this call.
        let r = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
        if r == 0 || r == -1 {
            break None;
        }
        if msg.hwnd.is_null() && msg.message == WM_APP_HOOK_COMMAND {
            match drain_commands(&rx, installer.as_mut(), &shared, prepared) {
                ThreadControl::Continue => {}
                ThreadControl::Quit => break None,
                ThreadControl::Retire(reply) => break Some(reply),
            }
            continue;
        }
        // SAFETY: msg came from GetMessageW.
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    };
    installer.uninstall();
    shared.installed.store(false, Ordering::SeqCst);
    installer.finish_thread();
    let ctx = CTX.with(|c| c.borrow_mut().take());
    if let (Some(reply), Some(ctx)) = (retire, ctx) {
        let _ = reply.send(ThreadResources { ctx, installer });
    }
}

fn with_ctx<R>(f: impl FnOnce(&mut HookCtx) -> R) -> Option<R> {
    CTX.with(|c| c.try_borrow_mut().ok().and_then(|mut slot| slot.as_mut().map(f)))
}

pub(super) fn raw_mouse(dx: i32, dy: i32, flags: u16, now: u64) {
    with_ctx(|ctx| ctx.core.raw_mouse(dx, dy, flags, now));
}

/// Run every queued command. False when the thread should end.
fn drain_commands(
    rx: &Receiver<Command>,
    installer: &mut dyn HookInstaller,
    shared: &HookShared,
    prepared: bool,
) -> ThreadControl {
    loop {
        let cmd = match rx.try_recv() {
            Ok(c) => c,
            Err(mpsc::TryRecvError::Empty) => return ThreadControl::Continue,
            Err(mpsc::TryRecvError::Disconnected) => return ThreadControl::Quit,
        };
        match cmd {
            Command::Install(reply) => {
                shared.install_attempts.fetch_add(1, Ordering::Relaxed);
                let ok = prepared && catch_unwind(AssertUnwindSafe(|| installer.install())).unwrap_or(false);
                *shared.context.lock().unwrap_or_else(|e| e.into_inner()) = installer.diagnostic();
                shared.installed.store(ok, Ordering::SeqCst);
                let _ = reply.send(ok);
            }
            Command::Uninstall(reply) => {
                let _ = catch_unwind(AssertUnwindSafe(|| installer.uninstall()));
                shared.installed.store(false, Ordering::SeqCst);
                let _ = reply.send(());
            }
            Command::ResetEngine => {
                with_ctx(|ctx| ctx.core.reset(ctx.cursor.as_ref()));
            }
            Command::SetEngine(engine) => {
                with_ctx(|ctx| ctx.core.set_engine(engine, ctx.cursor.as_ref()));
            }
            Command::Info(reply) => {
                // SAFETY: plain queries about the current thread.
                let info = unsafe {
                    HookThreadInfo {
                        thread_id: GetCurrentThreadId(),
                        priority: GetThreadPriority(GetCurrentThread()),
                    }
                };
                let _ = reply.send(info);
            }
            Command::Simulate(ev, reply) => {
                let out = with_ctx(|ctx| ctx.core.step(&ev, ctx.cursor.as_mut())).unwrap_or(StepOutcome::CallNext);
                let _ = reply.send(out);
            }
            Command::CursorPos(reply) => {
                let _ = reply.send(with_ctx(|ctx| ctx.cursor.cursor_pos()).flatten());
            }
            Command::Retire(reply) => return ThreadControl::Retire(reply),
            Command::Quit => return ThreadControl::Quit,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn f10_native_callback_time_and_bunched_qpc_are_preserved() {
        use windows_sys::Win32::UI::WindowsAndMessaging::MSLLHOOKSTRUCT;
        // SAFETY: this Win32 structure is plain data; no hook is installed.
        let mut raw: MSLLHOOKSTRUCT = unsafe { std::mem::zeroed() };
        raw.pt.x = -37;
        raw.pt.y = 19;
        raw.time = u32::MAX - 1;
        raw.flags = 4;
        // SAFETY: raw stays alive during both reads.
        let first = unsafe { super::event_from_raw(0, 0x200, &raw as *const _ as isize, 1_234_567) };
        let second = unsafe { super::event_from_raw(0, 0x200, &raw as *const _ as isize, 1_234_567) };
        assert_eq!(
            (first.x, first.y, first.time_ms, first.qpc_us),
            (-37, 19, u32::MAX - 1, 1_234_567)
        );
        assert_eq!(first, second, "bunched reports must retain their real shared timestamp");
        // SAFETY: negative nCode must never dereference this intentionally invalid pointer.
        let negative = unsafe { super::event_from_raw(-1, 0x200, 1, 42) };
        assert_eq!((negative.time_ms, negative.qpc_us), (0, 42));
    }
    use super::super::core::testkit::*;
    use super::*;
    use std::sync::Mutex;

    /// Records install/uninstall; never calls SetWindowsHookEx.
    struct FakeInstaller {
        ok: bool,
        log: Arc<Mutex<Vec<&'static str>>>,
    }

    impl HookInstaller for FakeInstaller {
        fn install(&mut self) -> bool {
            self.log.lock().unwrap().push("install");
            self.ok
        }
        fn uninstall(&mut self) {
            self.log.lock().unwrap().push("uninstall");
        }
    }

    /// A cursor that can cross threads and never moves the real one.
    struct SendCursor(Arc<Mutex<FakeCursor>>);
    impl CursorPort for SendCursor {
        fn set_cursor_pos(&mut self, x: i32, y: i32) -> bool {
            self.0.lock().unwrap().set_cursor_pos(x, y)
        }
        fn cursor_pos(&self) -> Option<(i32, i32)> {
            self.0.lock().unwrap().cursor_pos()
        }
    }

    type Spawned = (
        HookThread,
        Arc<Mutex<Vec<&'static str>>>,
        Arc<Mutex<Vec<Seen>>>,
        Arc<Mutex<FakeCursor>>,
    );

    fn spawn(ok: bool) -> Spawned {
        let (mut d, seen) = ScriptDecider::new(true);
        d.block_above = Some((0, 0));
        let log = Arc::new(Mutex::new(Vec::new()));
        let cursor = Arc::new(Mutex::new(FakeCursor::accepting()));
        let t = HookThread::spawn(
            Box::new(d),
            Box::new(FakeInstaller { ok, log: log.clone() }),
            Box::new(SendCursor(cursor.clone())),
        )
        .expect("hook thread starts");
        (t, log, seen, cursor)
    }

    #[test]
    fn recovery_fresh_thread_preserves_counters_toggles_and_reseeds_fake_cursor() {
        let (mut t, log, seen, cursor) = spawn(true);
        t.shared().set_toggles(true, true, true);
        assert!(t.install());
        assert_eq!(t.simulate(HookEvent::mouse_move(1, 1)), Some(StepOutcome::CallNext));
        cursor.lock().unwrap().pos = Some((42, 43));
        let old_id = t.join.as_ref().unwrap().thread().id();
        assert!(t.restart());
        assert_ne!(t.join.as_ref().unwrap().thread().id(), old_id);
        assert_eq!(t.cursor_pos(), Some((42, 43)), "probe runs on the fake hook desktop");
        assert_eq!(t.shared().counters().callbacks, 1);
        assert_eq!(t.shared().metrics().reinstall_attempts, 1);
        assert!(t.shared().confine.load(Ordering::Relaxed));
        assert_eq!(
            t.shared().toggles(),
            fence_types::Toggles {
                gate_open: true,
                side_contain: true
            }
        );
        assert_eq!(*log.lock().unwrap(), vec!["install", "uninstall", "install"]);
        assert!(
            seen.lock()
                .unwrap()
                .iter()
                .any(|s| matches!(s, Seen::Decide(42, 43, _)))
        );
    }

    #[test]
    fn recovery_prepares_each_new_thread_before_install_and_finishes_after_unhook() {
        struct DesktopInstaller(Arc<Mutex<Vec<&'static str>>>);
        impl HookInstaller for DesktopInstaller {
            fn prepare_thread(&mut self) -> bool {
                let mut log = self.0.lock().unwrap();
                let ready = !log.is_empty();
                log.push("prepare");
                ready // emulate an unavailable desktop on first startup, available on retry
            }
            fn install(&mut self) -> bool {
                self.0.lock().unwrap().push("install");
                true
            }
            fn uninstall(&mut self) {
                self.0.lock().unwrap().push("uninstall");
            }
            fn finish_thread(&mut self) {
                self.0.lock().unwrap().push("finish");
            }
        }
        let log = Arc::new(Mutex::new(Vec::new()));
        let (d, _) = ScriptDecider::new(true);
        let mut t = HookThread::spawn(
            Box::new(d),
            Box::new(DesktopInstaller(log.clone())),
            Box::new(FakeCursor::accepting()),
        )
        .unwrap();
        assert!(!t.install(), "never install on an unprepared desktop");
        assert!(t.restart());
        drop(t);
        assert_eq!(
            *log.lock().unwrap(),
            vec![
                "prepare",
                "uninstall",
                "finish",
                "prepare",
                "install",
                "uninstall",
                "finish"
            ]
        );
    }

    #[test]
    fn j08_the_hook_thread_survives_a_panicking_engine() {
        let (mut d, _seen) = ScriptDecider::new(true);
        d.panic_on_decide = true;
        d.panic_on_reset = true;
        let log = Arc::new(Mutex::new(Vec::new()));
        let cursor = Arc::new(Mutex::new(FakeCursor::accepting()));
        let t = HookThread::spawn(
            Box::new(d),
            Box::new(FakeInstaller { ok: true, log }),
            Box::new(SendCursor(cursor.clone())),
        )
        .expect("hook thread starts");
        assert!(t.install());
        assert_eq!(t.simulate(HookEvent::mouse_move(1, -1)), Some(StepOutcome::CallNext));
        t.reset_engine();
        assert_eq!(t.simulate(HookEvent::mouse_move(2, -2)), Some(StepOutcome::CallNext));
        assert!(t.info().is_some(), "the loop still answers after the panics");
        assert!(t.shared().installed(), "the hook stays in");
        let c = t.shared().counters();
        assert_eq!(c.callbacks, 2);
        assert_eq!(c.panics, 5, "two steps, two failed recovery seeds, and one reset");
        assert!(cursor.lock().unwrap().placed.is_empty());
    }

    #[test]
    fn s_win_hook_thread_runs_its_own_loop_at_normal_priority() {
        let (t, _log, _seen, _cur) = spawn(true);
        let info = t.info().expect("the hook thread answers");
        // SAFETY: plain query.
        let me = unsafe { GetCurrentThreadId() };
        assert_ne!(info.thread_id, me, "the hook has its own thread");
        assert_eq!(info.thread_id, t.thread_id());
        assert_eq!(
            info.priority, THREAD_PRIORITY_NORMAL,
            "normal priority, never time-critical (T9)"
        );
    }

    #[test]
    fn s_win_hook_thread_install_uninstall_track_the_handle() {
        let (t, log, _seen, _cur) = spawn(true);
        assert!(!t.shared().installed());
        assert!(t.install());
        assert!(t.shared().installed());
        t.uninstall();
        assert!(!t.shared().installed());
        drop(t);
        assert_eq!(
            *log.lock().unwrap(),
            vec!["install", "uninstall", "uninstall"],
            "quit unhooks too"
        );
    }

    #[test]
    fn s_win_hook_thread_failed_install_leaves_no_handle() {
        let (t, _log, _seen, _cur) = spawn(false);
        assert!(!t.install());
        assert!(!t.shared().installed());
    }

    #[test]
    fn j26_reinstall_attempts_include_failures_but_exclude_initial_install() {
        let (t, _, _, _) = spawn(false);
        assert_eq!(t.shared().metrics().reinstall_attempts, 0);
        assert!(!t.install());
        assert_eq!(t.shared().metrics().reinstall_attempts, 0);
        assert!(!t.install());
        assert!(!t.install());
        assert_eq!(t.shared().metrics().reinstall_attempts, 2);
    }

    #[test]
    fn s_win_hook_thread_commands_run_in_order_with_moves() {
        let (t, _log, seen, cur) = spawn(true);
        t.shared().set_toggles(true, true, false);
        assert_eq!(t.simulate(HookEvent::mouse_move(50, -10)), Some(StepOutcome::Swallow));
        assert_eq!(cur.lock().unwrap().placed, vec![(50, 0)], "fake cursor only");
        t.reset_engine();
        assert_eq!(t.simulate(HookEvent::mouse_move(50, 10)), Some(StepOutcome::CallNext));
        let (d2, seen2) = ScriptDecider::new(true);
        t.set_engine(Box::new(d2));
        assert_eq!(t.simulate(HookEvent::mouse_move(60, 10)), Some(StepOutcome::CallNext));
        let s = seen.lock().unwrap().clone();
        let reset_at = s
            .iter()
            .position(|x| *x == Seen::Reset)
            .expect("reset reached the engine");
        assert!(matches!(s[reset_at - 1], Seen::Decide(50, -10, _)));
        assert!(
            matches!(s[reset_at + 1], Seen::Decide(50, 0, _)),
            "seeded from the (fake) cursor the block placed (J03)"
        );
        assert!(matches!(s[reset_at + 2], Seen::Decide(50, 10, _)));
        let s2 = seen2.lock().unwrap().clone();
        assert_eq!(s2.first(), Some(&Seen::Reset), "a new engine starts reset");
        assert!(matches!(s2[1], Seen::Decide(50, 0, _)), "and seeded");
        assert_eq!(t.shared().counters().callbacks, 3);
    }
}
