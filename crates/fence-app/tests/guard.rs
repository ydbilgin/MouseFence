//! The guard (C# MouseGuard minus the callback) over a fake hook host, and over the real fence-win hook
//! thread with a fake installer and a fake cursor (rows B024, B027 and J03; Track C). No test installs a real hook,
//! moves the cursor or sends input.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use fence_app::{Guard, GuardPort, HookHost};
use fence_types::{Action, Decider, Decision, EngineConfig, HookHealthVerdict, Toggles};
use fence_win::hook::{CursorPort, HookEvent, HookInstaller, HookThread, StepOutcome, current_thread_id};

// ---------------------------------------------------------------- a scripted host

#[derive(Debug, Default)]
struct HostState {
    log: Vec<String>,
    installed: bool,
    refuse_install: bool,
    callbacks: u64,
    callbacks_on_install: u64,
    cursor: Option<(i32, i32)>,
    toggles: (bool, bool, bool),
    now_ms: u64,
    recovery_log: Vec<String>,
}

#[derive(Clone, Default)]
struct FakeHost(Rc<RefCell<HostState>>);

impl HookHost for FakeHost {
    fn monotonic_ms(&self) -> u64 {
        self.0.borrow().now_ms
    }
    fn recovery_log(&mut self, message: &str) {
        self.0.borrow_mut().recovery_log.push(message.into());
    }
    fn install(&mut self) -> bool {
        let mut s = self.0.borrow_mut();
        s.log.push("install".into());
        s.installed = !s.refuse_install;
        if s.installed {
            s.callbacks += s.callbacks_on_install;
        }
        s.installed
    }
    fn uninstall(&mut self) {
        let mut s = self.0.borrow_mut();
        s.log.push("uninstall".into());
        s.installed = false;
    }
    fn installed(&self) -> bool {
        self.0.borrow().installed
    }
    fn reset_engine(&mut self) {
        self.0.borrow_mut().log.push("reset".into());
    }
    fn set_engine_config(&mut self, _config: EngineConfig) {
        self.0.borrow_mut().log.push("configure".into());
    }
    fn callbacks(&self) -> u64 {
        self.0.borrow().callbacks
    }
    fn cursor_pos(&self) -> Option<(i32, i32)> {
        self.0.borrow().cursor
    }
    fn set_toggles(&mut self, gate_open: bool, side_contain: bool, confine: bool) {
        self.0.borrow_mut().toggles = (gate_open, side_contain, confine);
    }
}

#[test]
fn recovery_handle_without_callbacks_is_bounded_and_only_traffic_rearms() {
    let (mut g, host) = guard();
    host.0.borrow_mut().cursor = Some((0, 0));
    g.start();
    g.check_health();
    take_log(&host);
    for (time, expected) in [(0, 1), (1, 1), (2999, 1), (3000, 2), (8999, 2), (9000, 3), (999_999, 3)] {
        let mut s = host.0.borrow_mut();
        s.now_ms = time;
        s.cursor.as_mut().unwrap().0 += 1;
        drop(s);
        assert_eq!(g.check_health(), HookHealthVerdict::Dead);
        assert_eq!(
            host.0.borrow().log.iter().filter(|s| s.as_str() == "install").count(),
            expected
        );
        assert_eq!(
            host.0
                .borrow()
                .recovery_log
                .iter()
                .filter(|s| s.contains("attempt="))
                .count(),
            expected
        );
    }
    assert!(g.hook_installed(), "a successful install is not callback evidence");
    host.0.borrow_mut().callbacks += 1;
    assert_eq!(g.check_health(), HookHealthVerdict::Alive);
    host.0.borrow_mut().cursor = Some((100, 0));
    assert_eq!(g.check_health(), HookHealthVerdict::Dead);
    assert!(host.0.borrow().recovery_log.last().unwrap().contains("attempt=1"));
}

#[test]
fn recovery_failed_install_retries_without_further_cursor_motion_and_stop_cancels() {
    let (mut g, host) = guard();
    host.0.borrow_mut().refuse_install = true;
    g.start();
    take_log(&host);
    g.check_health();
    assert_eq!(take_log(&host), vec!["reset", "install"]);
    host.0.borrow_mut().now_ms = 3000;
    g.check_health();
    assert_eq!(take_log(&host), vec!["reset", "install"]);
    g.stop();
    host.0.borrow_mut().now_ms = 9000;
    g.check_health();
    assert!(take_log(&host).is_empty());
    assert!(host.0.borrow().recovery_log[0].contains("reason=missing_handle"));
}

#[test]
fn recovery_callbacks_delivered_during_install_are_liveness_evidence() {
    let (mut g, host) = guard();
    host.0.borrow_mut().cursor = Some((0, 0));
    g.start();
    g.check_health();
    host.0.borrow_mut().callbacks_on_install = 1;
    host.0.borrow_mut().cursor = Some((1, 0));
    assert_eq!(g.check_health(), HookHealthVerdict::Dead);
    // No later input: the callback arriving during reinstall still proves the replacement works.
    assert_eq!(g.check_health(), HookHealthVerdict::Alive);
}

fn guard() -> (Guard<FakeHost>, FakeHost) {
    let host = FakeHost::default();
    (Guard::new(host.clone()), host)
}

fn take_log(h: &FakeHost) -> Vec<String> {
    std::mem::take(&mut h.0.borrow_mut().log)
}

#[test]
fn b024_enabled_is_the_intent_and_hook_installed_the_handle() {
    let (mut g, host) = guard();
    assert!(!g.enabled() && !g.hook_installed());
    g.start();
    assert!(g.enabled() && g.hook_installed());
    assert_eq!(
        take_log(&host),
        vec!["reset", "install"],
        "Start resets the engine, then hooks"
    );

    // Windows refused the hook: still enabled (the intent), no handle (the fence is down).
    let (mut g, host) = guard();
    host.0.borrow_mut().refuse_install = true;
    g.start();
    assert!(g.enabled() && !g.hook_installed());
}

#[test]
fn b024_start_with_a_handle_held_only_rearms_and_stop_unhooks_once() {
    let (mut g, host) = guard();
    g.start();
    take_log(&host);
    g.start();
    assert!(
        take_log(&host).is_empty(),
        "a held handle is kept; no second reset or hook"
    );
    g.stop();
    assert!(!g.enabled() && !g.hook_installed());
    assert_eq!(take_log(&host), vec!["uninstall"]);
    g.stop();
    assert!(take_log(&host).is_empty(), "nothing to unhook");
}

#[test]
fn b024_start_and_stop_reset_the_probe() {
    let (mut g, host) = guard();
    host.0.borrow_mut().callbacks = 100;
    host.0.borrow_mut().cursor = Some((0, 0));
    g.start();
    // First probe after Start: no baseline, so a moved cursor is not evidence.
    host.0.borrow_mut().cursor = Some((50, 50));
    assert_eq!(g.check_health(), HookHealthVerdict::Inconclusive);
    // Callbacks seen before Start never count as traffic since the last probe.
    host.0.borrow_mut().cursor = Some((60, 60));
    assert_eq!(g.check_health(), HookHealthVerdict::Dead);
}

#[test]
fn b024_dead_reinstalls_resetting_the_engine() {
    let (mut g, host) = guard();
    host.0.borrow_mut().cursor = Some((0, 0));
    g.start();
    assert_eq!(g.check_health(), HookHealthVerdict::Inconclusive, "baseline");
    take_log(&host);
    host.0.borrow_mut().cursor = Some((5, 0));
    assert_eq!(g.check_health(), HookHealthVerdict::Dead, "moved, not one callback");
    assert_eq!(
        take_log(&host),
        vec!["uninstall", "reset", "install"],
        "the stale handle goes, the engine re-baselines, a fresh hook goes in"
    );
    assert!(g.hook_installed());

    // Traffic since the last probe: Alive, nothing touched.
    host.0.borrow_mut().callbacks += 3;
    host.0.borrow_mut().cursor = Some((9, 0));
    assert_eq!(g.check_health(), HookHealthVerdict::Alive);
    assert!(take_log(&host).is_empty());

    // A failed reinstall leaves the guard enabled with no handle.
    host.0.borrow_mut().refuse_install = true;
    host.0.borrow_mut().cursor = Some((20, 0));
    assert_eq!(g.check_health(), HookHealthVerdict::Dead);
    assert!(g.enabled() && !g.hook_installed());
}

#[test]
fn b024_a_paused_guard_probes_inconclusive_and_touches_nothing() {
    let (mut g, host) = guard();
    host.0.borrow_mut().cursor = Some((0, 0));
    assert_eq!(g.check_health(), HookHealthVerdict::Inconclusive);
    host.0.borrow_mut().cursor = Some((99, 99));
    assert_eq!(g.check_health(), HookHealthVerdict::Inconclusive);
    assert!(take_log(&host).is_empty());
}

#[test]
fn b024_configure_hands_a_new_engine_and_toggles_reach_the_host() {
    let (mut g, host) = guard();
    g.configure(EngineConfig::default());
    assert_eq!(take_log(&host), vec!["configure"]);
    g.set_gate_open(true);
    assert_eq!(host.0.borrow().toggles, (true, false, false));
    g.set_side_contain(true);
    g.set_confine(true);
    assert_eq!(host.0.borrow().toggles, (true, true, true));
    assert!(g.gate_open() && g.side_contain() && g.confine());
}

// ---------------------------------------------------------------- over the real hook thread

/// One decision the engine saw: x, y, the toggles, and its confine flag at that moment.
type SeenMove = (i32, i32, Toggles, bool);

/// Records what the hook thread's engine saw; never blocks.
struct Recorder {
    seen: Arc<Mutex<Vec<SeenMove>>>,
    confine: bool,
}

impl Decider for Recorder {
    fn needs_processing(&self) -> bool {
        true
    }
    fn decide(&mut self, event: fence_types::MoveEvent, toggles: Toggles) -> Decision {
        let (x, y) = (event.x, event.y);
        self.seen.lock().unwrap().push((x, y, toggles, self.confine));
        Decision {
            action: Action::Pass,
            x,
            y,
        }
    }
    fn reset(&mut self) {}
    fn set_confine(&mut self, on: bool) {
        self.confine = on;
    }
    fn confine(&self) -> bool {
        self.confine
    }
}

struct NoHook;
impl HookInstaller for NoHook {
    fn install(&mut self) -> bool {
        true
    }
    fn uninstall(&mut self) {}
}

struct NoCursor;
impl CursorPort for NoCursor {
    fn set_cursor_pos(&mut self, _x: i32, _y: i32) -> bool {
        true
    }
    fn cursor_pos(&self) -> Option<(i32, i32)> {
        None
    }
}

/// The same adapter shape apps/mousefence uses, over the real thread.
struct ThreadHost(HookThread);

impl HookHost for ThreadHost {
    fn install(&mut self) -> bool {
        self.0.install()
    }
    fn uninstall(&mut self) {
        self.0.uninstall()
    }
    fn installed(&self) -> bool {
        self.0.shared().installed()
    }
    fn reset_engine(&mut self) {
        self.0.reset_engine()
    }
    fn set_engine_config(&mut self, config: EngineConfig) {
        self.0
            .set_engine(Box::new(fence_engine::decide::ClassicEngine::new(config)))
    }
    fn callbacks(&self) -> u64 {
        self.0.shared().counters().callbacks
    }
    fn cursor_pos(&self) -> Option<(i32, i32)> {
        None
    }
    fn set_toggles(&mut self, gate_open: bool, side_contain: bool, confine: bool) {
        self.0.shared().set_toggles(gate_open, side_contain, confine)
    }
}

#[test]
fn b027_hook_has_its_own_normal_priority_loop_and_toggles_arrive_by_atomics() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let thread = HookThread::spawn(
        Box::new(Recorder {
            seen: seen.clone(),
            confine: false,
        }),
        Box::new(NoHook),
        Box::new(NoCursor),
    )
    .expect("hook thread");
    let info = thread.info().expect("the hook thread answers on its own loop");
    assert_ne!(info.thread_id, current_thread_id(), "not the tray thread");
    assert_eq!(info.priority, 0, "THREAD_PRIORITY_NORMAL (T9), never time-critical");

    let mut g = Guard::new(ThreadHost(thread));
    g.start();
    assert!(g.hook_installed());

    // The tray thread flips the toggles; the hook step reads them without any message to the tray thread
    // (this test thread runs no message loop at all while the hook thread works).
    g.set_gate_open(true);
    g.set_side_contain(false);
    g.set_confine(true);
    let out = g.host().0.simulate(HookEvent::mouse_move(10, 20));
    assert_eq!(out, Some(StepOutcome::CallNext));
    let first = seen.lock().unwrap()[0];
    assert_eq!(
        first,
        (
            10,
            20,
            Toggles {
                gate_open: true,
                side_contain: false
            },
            true
        )
    );
    g.set_gate_open(false);
    g.host().0.simulate(HookEvent::mouse_move(11, 20));
    assert!(!seen.lock().unwrap()[1].2.gate_open, "the next move sees the new value");
    assert_eq!(g.host().0.shared().counters().callbacks, 2);
    g.stop();
    assert!(!g.hook_installed());
}

// ---------------------------------------------------------------- J03 seeded reset (lesson L9)

/// A one-seam model with C# GuardCore's memory rule: main is y >= 0, the top screen is y < 0, and a closed
/// gate blocks a move from main to the top back onto the seam. As `GuardCore.Decide`, a move with no last
/// position passes unchecked (`if (!HaveLast) return Pass`) and becomes the last position.
#[derive(Default)]
struct SeamModel {
    last: Option<(i32, i32)>,
    confine: bool,
}

impl Decider for SeamModel {
    fn needs_processing(&self) -> bool {
        true
    }
    fn decide(&mut self, event: fence_types::MoveEvent, toggles: Toggles) -> Decision {
        let (x, y) = (event.x, event.y);
        let crosses_up = self.last.is_some_and(|(_, ly)| ly >= 0 && y < 0);
        let d = if crosses_up && !toggles.gate_open {
            Decision {
                action: Action::Block,
                x,
                y: 0,
            }
        } else {
            Decision {
                action: Action::Pass,
                x,
                y,
            }
        };
        self.last = Some((d.x, d.y));
        d
    }
    fn reset(&mut self) {
        self.last = None;
    }
    fn set_confine(&mut self, on: bool) {
        self.confine = on;
    }
    fn confine(&self) -> bool {
        self.confine
    }
}

/// The owner's desk in miniature: where the cursor is, and where the hook placed it. Never the real cursor.
#[derive(Debug, Default)]
struct Desk {
    pos: Option<(i32, i32)>,
    placed: Vec<(i32, i32)>,
}

#[derive(Clone, Default)]
struct DeskCursor(Arc<Mutex<Desk>>);

impl DeskCursor {
    fn at(pos: Option<(i32, i32)>) -> Self {
        Self(Arc::new(Mutex::new(Desk {
            pos,
            placed: Vec::new(),
        })))
    }
    fn set(&self, pos: (i32, i32)) {
        self.0.lock().unwrap().pos = Some(pos);
    }
    fn placed(&self) -> Vec<(i32, i32)> {
        self.0.lock().unwrap().placed.clone()
    }
}

impl CursorPort for DeskCursor {
    fn set_cursor_pos(&mut self, x: i32, y: i32) -> bool {
        let mut d = self.0.lock().unwrap();
        d.placed.push((x, y));
        d.pos = Some((x, y));
        true
    }
    fn cursor_pos(&self) -> Option<(i32, i32)> {
        self.0.lock().unwrap().pos
    }
}

/// The apps/mousefence adapter shape over the real hook thread; Configure hands it a fresh seam model.
struct SeamHost {
    thread: HookThread,
    desk: DeskCursor,
}

impl HookHost for SeamHost {
    fn install(&mut self) -> bool {
        self.thread.install()
    }
    fn uninstall(&mut self) {
        self.thread.uninstall()
    }
    fn installed(&self) -> bool {
        self.thread.shared().installed()
    }
    fn reset_engine(&mut self) {
        self.thread.reset_engine()
    }
    fn set_engine_config(&mut self, _config: EngineConfig) {
        self.thread.set_engine(Box::new(SeamModel::default()))
    }
    fn callbacks(&self) -> u64 {
        self.thread.shared().counters().callbacks
    }
    fn cursor_pos(&self) -> Option<(i32, i32)> {
        self.desk.cursor_pos()
    }
    fn set_toggles(&mut self, gate_open: bool, side_contain: bool, confine: bool) {
        self.thread.shared().set_toggles(gate_open, side_contain, confine)
    }
}

/// A guard over the real hook thread (fake installer, desk cursor), gate closed, not started.
fn seam_guard(desk: &DeskCursor) -> Guard<SeamHost> {
    let thread = HookThread::spawn(Box::new(SeamModel::default()), Box::new(NoHook), Box::new(desk.clone()))
        .expect("hook thread");
    let mut g = Guard::new(SeamHost {
        thread,
        desk: desk.clone(),
    });
    g.set_gate_open(false);
    g
}

fn move_to(g: &Guard<SeamHost>, x: i32, y: i32) -> Option<StepOutcome> {
    g.host().thread.simulate(HookEvent::mouse_move(x, y))
}

#[test]
fn j03_after_start_the_first_move_across_a_closed_seam_is_blocked() {
    let desk = DeskCursor::at(Some((500, 300)));
    let mut g = seam_guard(&desk);
    g.start();
    assert_eq!(
        move_to(&g, 500, -5),
        Some(StepOutcome::Swallow),
        "seeded at (500, 300): the very first move is judged as a crossing"
    );
    assert_eq!(
        desk.placed(),
        vec![(500, 0)],
        "held on the seam (desk cursor, not the real one)"
    );
}

#[test]
fn j03_without_a_cursor_position_the_first_move_passes_as_in_1x() {
    // GetCursorPos failed: no seed, so 1.1.1's rule applies and the first move is not judged. This is the
    // L9 hole the seed closes whenever the position is known.
    let desk = DeskCursor::at(None);
    let mut g = seam_guard(&desk);
    g.start();
    assert_eq!(move_to(&g, 500, -5), Some(StepOutcome::CallNext));
    assert!(desk.placed().is_empty());
    assert_eq!(move_to(&g, 500, 10), Some(StepOutcome::CallNext));
    assert_eq!(
        move_to(&g, 500, -5),
        Some(StepOutcome::Swallow),
        "from the second move on the engine judges as before"
    );
}

#[test]
fn j03_a_reinstall_seeds_from_where_the_cursor_is_now() {
    let desk = DeskCursor::at(Some((500, 300)));
    let mut g = seam_guard(&desk);
    g.start();
    assert_eq!(g.check_health(), HookHealthVerdict::Inconclusive, "baseline");
    // The hook went deaf: the cursor moved to (800, 200) and no callback arrived.
    desk.set((800, 200));
    assert_eq!(g.check_health(), HookHealthVerdict::Dead);
    assert!(g.hook_installed(), "reinstalled");
    assert_eq!(move_to(&g, 800, -3), Some(StepOutcome::Swallow));
    assert_eq!(
        desk.placed(),
        vec![(800, 0)],
        "judged from (800, 200), the position at the reinstall"
    );
}

#[test]
fn j03_a_new_engine_from_configure_is_seeded_too() {
    let desk = DeskCursor::at(Some((500, 300)));
    let mut g = seam_guard(&desk);
    g.start();
    desk.set((300, 100));
    g.configure(EngineConfig::default()); // the display-change path: a fresh engine, reset
    assert_eq!(move_to(&g, 300, -1), Some(StepOutcome::Swallow));
    assert_eq!(desk.placed(), vec![(300, 0)]);
    // An open gate still lets the same first move through: the seed only gives the engine a baseline.
    g.set_gate_open(true);
    desk.set((300, 100));
    g.configure(EngineConfig::default());
    assert_eq!(move_to(&g, 300, -1), Some(StepOutcome::CallNext));
}

// ---------------------------------------------------------------- J03 with the real engine (join, JUDGE-C N1/N2)

/// The adapter shape of apps/mousefence (WinHookHost) over the real hook thread, but with the real 2.0 engine
/// (fence-engine ClassicEngine) and a desk cursor. Synthetic layout only.
struct EngineHost {
    thread: HookThread,
    desk: DeskCursor,
}

impl HookHost for EngineHost {
    fn install(&mut self) -> bool {
        self.thread.install()
    }
    fn uninstall(&mut self) {
        self.thread.uninstall()
    }
    fn installed(&self) -> bool {
        self.thread.shared().installed()
    }
    fn reset_engine(&mut self) {
        self.thread.reset_engine()
    }
    fn set_engine_config(&mut self, config: EngineConfig) {
        self.thread
            .set_engine(Box::new(fence_engine::decide::ClassicEngine::new(config)))
    }
    fn callbacks(&self) -> u64 {
        self.thread.shared().counters().callbacks
    }
    fn cursor_pos(&self) -> Option<(i32, i32)> {
        self.desk.cursor_pos()
    }
    fn set_toggles(&mut self, gate_open: bool, side_contain: bool, confine: bool) {
        self.thread.shared().set_toggles(gate_open, side_contain, confine)
    }
}

fn synthetic(id: &str, index: u32, primary: bool, l: i32, t: i32, r: i32, b: i32) -> fence_types::MonitorInfo {
    fence_types::MonitorInfo {
        device: format!(r"\\.\DISPLAY{index}"),
        stable_id: id.into(),
        bounds: fence_types::Rect::new(l, t, r, b),
        primary,
        index,
    }
}

/// Main (primary) with a top screen above it and a kept-off screen to its right, default settings, gate closed;
/// the engine config comes from fence-config and fence-engine's plan, as the tray builds it.
fn engine_guard(desk: &DeskCursor) -> Guard<EngineHost> {
    let monitors = vec![
        synthetic("SYN-MAIN", 1, true, 0, 0, 1920, 1080),
        synthetic("SYN-TOP", 2, false, 0, -1080, 1920, 0),
        synthetic("SYN-KEPT-OFF", 3, false, 1920, 0, 3840, 1080),
    ];
    let settings = fence_config::Settings {
        excluded_devices: vec!["SYN-KEPT-OFF".into()],
        ..fence_config::Settings::default()
    };
    let (config, _) = fence_engine::plan::build(&fence_config::to_policy(&settings), &monitors);
    assert!(
        config.has_top && !config.forbidden.is_empty(),
        "a top screen and a walled screen"
    );
    let engine = fence_engine::decide::ClassicEngine::new(config.clone());
    let thread = HookThread::spawn(Box::new(engine), Box::new(NoHook), Box::new(desk.clone())).expect("hook thread");
    let mut g = Guard::new(EngineHost {
        thread,
        desk: desk.clone(),
    });
    g.set_gate_open(false);
    g.configure(config);
    g
}

fn move_engine(g: &Guard<EngineHost>, x: i32, y: i32) -> Option<StepOutcome> {
    g.host().thread.simulate(HookEvent::mouse_move(x, y))
}

#[test]
fn j03_real_engine_the_first_move_after_start_a_shallow_slide_over_the_closed_gate_is_blocked() {
    // The only way up is main -> top, so 1.1.1's sole-up safety escape lets a deliberate push through even with
    // the gate closed (P068); a shallow fast slide (dx 60, dy 5) is not a push and must be held. Without the
    // seed the engine has no last point and 1.1.1 passes the first move unjudged (shown by the second guard).
    let desk = DeskCursor::at(Some((500, 2)));
    let mut g = engine_guard(&desk);
    g.start();
    assert_eq!(
        move_engine(&g, 560, -3),
        Some(StepOutcome::Swallow),
        "judged from the seed"
    );
    let placed = desk.placed();
    assert_eq!(placed.len(), 1);
    assert!(placed[0].1 >= 0 && placed[0].0 < 1920, "held on main: {placed:?}");

    let unknown = DeskCursor::at(None);
    let mut unseeded = engine_guard(&unknown);
    unseeded.start();
    assert_eq!(
        move_engine(&unseeded, 560, -3),
        Some(StepOutcome::CallNext),
        "no cursor position: the 1.1.1 first-move hole"
    );
}

#[test]
fn j03_real_engine_a_seed_inside_a_walled_screen_still_walls_the_next_move() {
    // N1: the cursor sits inside the kept-off screen when the engine is reset. The seed's decision (a corrected
    // point outside the wall) is recorded but nothing is placed; the next real move inside the wall is still
    // walled back onto a usable screen, as 1.1.1 walls a first move with no last point.
    let desk = DeskCursor::at(Some((2500, 500)));
    let mut g = engine_guard(&desk);
    g.start();
    assert!(desk.placed().is_empty(), "the seed places nothing");
    assert_eq!(move_engine(&g, 2510, 505), Some(StepOutcome::Swallow));
    let placed = desk.placed();
    assert_eq!(placed.len(), 1);
    assert!(
        placed[0].0 < 1920 && placed[0].1 >= 0,
        "back on main, never inside the wall: {placed:?}"
    );
}
