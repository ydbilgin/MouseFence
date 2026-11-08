//! The hook step as a pure function (port of C# `MouseGuard.HookCallback`, Platform/MouseGuard.cs:182-204).
//!
//! No Windows call happens here: the caller turns the raw callback arguments into a [`HookEvent`] and acts
//! on the returned [`StepOutcome`] (`CallNext` = CallNextHookEx, `Swallow` = return 1). Cursor placement
//! goes through [`CursorPort`], so tests use a fake and never move the owner's cursor.
//!
//! Rules (in this order, as 1.1.1):
//! 1. every delivered event is counted first; the counter is the liveness evidence (B021, HookHealth);
//! 2. `nCode < 0`, anything but WM_MOUSEMOVE, and a decider that needs no processing pass (B021);
//! 3. injected input (LLMHF_INJECTED or LLMHF_LOWER_IL_INJECTED, our own SetCursorPos included) passes (B022);
//! 4. Block places the cursor at the decided point and swallows the move; Pass calls the next hook (B023);
//! 5. a panic anywhere in the step is caught here: the event passes, the panic is counted and the engine is
//!    reset (B028/J08). Nothing unwinds into the `extern "system"` callback (PLAN T12).
//!
//! 2.0 also seeds the engine from GetCursorPos after every reset (J03, see [`HookCore::reset`]), and a Block
//! whose SetCursorPos fails lets the move pass and counts it as degraded (J07, PLAN T10).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use fence_types::{Action, Decider, EdgeRecord, EdgeState, MoveEvent, Toggles};
pub const TIMING_BUCKET_US: [u64; 10] = [1, 2, 5, 10, 20, 50, 100, 200, 500, 1000];

/// `WM_MOUSEMOVE` (winuser.h). Kept here so the pure step needs no bindings; a test pins it to windows-sys.
pub const WM_MOUSEMOVE: u32 = 0x0200;
/// `LLMHF_INJECTED`.
pub const LLMHF_INJECTED: u32 = 0x0000_0001;
/// `LLMHF_LOWER_IL_INJECTED`.
pub const LLMHF_LOWER_IL_INJECTED: u32 = 0x0000_0002;

/// State shared between the tray thread and the hook thread. Plain atomics: the hook never takes a lock and
/// never waits for the tray (B027). C# kept `GateOpen`/`SideContain` on MouseGuard and `Confine` on the core.
#[derive(Debug, Default)]
pub struct HookShared {
    /// Written outside callbacks, read only by tray diagnostics.
    pub context: std::sync::Mutex<String>,
    /// Upward passage open (C# `MouseGuard.GateOpen`).
    pub gate_open: AtomicBool,
    /// Side protection on (C# `MouseGuard.SideContain`).
    pub side_contain: AtomicBool,
    /// Game mode (C# `GuardCore.Confine`); copied into the engine at the start of each step.
    pub confine: AtomicBool,
    /// A hook handle is held (C# `HookInstalled`). Written only by the hook thread.
    pub installed: AtomicBool,
    /// Every delivered callback (C# `_callbacks`).
    pub callbacks: AtomicU64,
    /// Panics caught inside the step (2.0, PLAN T12).
    pub panics: AtomicU64,
    /// Blocks whose cursor placement failed and were let through (2.0, PLAN T10).
    pub degraded: AtomicU64,
    pub trace_enabled: AtomicBool,
    pub trace: crate::trace_ring::TraceRing,
    pub estimated_rate_milli: AtomicU32,
    pub timing_buckets: [AtomicU64; 10],
    pub max_step_us: AtomicU64,
    pub install_attempts: AtomicU64,
}

/// A snapshot of the counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HookCounters {
    pub callbacks: u64,
    pub panics: u64,
    pub degraded: u64,
}

impl HookShared {
    pub fn record_timing(&self, micros: u64) {
        self.record_timing_published(micros, || {});
    }

    // The observer lets the regression inspect exactly the publication boundary without scheduling races.
    fn record_timing_published(&self, micros: u64, after_publish: impl FnOnce()) {
        let bucket = TIMING_BUCKET_US.iter().position(|edge| micros <= *edge).unwrap_or(9);
        self.max_step_us.fetch_max(micros, Ordering::Relaxed);
        // An acquired bucket must also expose the maximum for every sample it includes.
        self.timing_buckets[bucket].fetch_add(1, Ordering::Release);
        after_publish();
    }
    pub fn metrics(&self) -> fence_types::HookMetrics {
        let buckets = self.timing_buckets.each_ref().map(|n| n.load(Ordering::Acquire));
        let total = buckets.iter().fold(0u64, |sum, n| sum.saturating_add(*n));
        let max = self.max_step_us.load(Ordering::Relaxed);
        let rank = total - total / 100;
        let mut seen = 0u64;
        let mut p99 = 0;
        if total > 0 {
            for (i, count) in buckets.iter().enumerate() {
                seen = seen.saturating_add(*count);
                if seen >= rank {
                    p99 = if i == 9 { max.max(1000) } else { TIMING_BUCKET_US[i] };
                    break;
                }
            }
        }
        fence_types::HookMetrics {
            callbacks: self.callbacks.load(Ordering::Relaxed),
            sampled_steps: total,
            max_step_us: max,
            p99_upper_us: p99,
            reinstall_attempts: self.install_attempts.load(Ordering::Relaxed).saturating_sub(1),
            panics: self.panics.load(Ordering::Relaxed),
            degraded: self.degraded.load(Ordering::Relaxed),
            trace_dropped: self.trace.dropped.load(Ordering::Relaxed),
        }
    }
    pub fn toggles(&self) -> Toggles {
        Toggles {
            gate_open: self.gate_open.load(Ordering::Relaxed),
            side_contain: self.side_contain.load(Ordering::Relaxed),
        }
    }

    pub fn set_toggles(&self, gate_open: bool, side_contain: bool, confine: bool) {
        self.gate_open.store(gate_open, Ordering::Relaxed);
        self.side_contain.store(side_contain, Ordering::Relaxed);
        self.confine.store(confine, Ordering::Relaxed);
    }

    pub fn counters(&self) -> HookCounters {
        HookCounters {
            callbacks: self.callbacks.load(Ordering::Relaxed),
            panics: self.panics.load(Ordering::Relaxed),
            degraded: self.degraded.load(Ordering::Relaxed),
        }
    }

    pub fn installed(&self) -> bool {
        self.installed.load(Ordering::SeqCst)
    }
}

/// One low-level mouse callback, already read from `MSLLHOOKSTRUCT` (only when `n_code >= 0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HookEvent {
    pub n_code: i32,
    /// The `wParam` message (WM_MOUSEMOVE, WM_LBUTTONDOWN, ...).
    pub message: u32,
    pub x: i32,
    pub y: i32,
    /// `MSLLHOOKSTRUCT.flags`.
    pub flags: u32,
    pub time_ms: u32,
    pub qpc_us: u64,
}

impl HookEvent {
    /// A physical (not injected) move to `(x, y)`, `nCode == HC_ACTION`.
    pub const fn mouse_move(x: i32, y: i32) -> Self {
        Self {
            n_code: 0,
            message: WM_MOUSEMOVE,
            x,
            y,
            flags: 0,
            time_ms: 0,
            qpc_us: 0,
        }
    }
}

/// What the callback does with the event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepOutcome {
    /// `CallNextHookEx`: the event goes on unchanged.
    CallNext,
    /// Return 1: the move is swallowed (the cursor was placed by the step).
    Swallow,
}

/// Cursor access for the step. The real one (fence-win `cursor::SystemCursor`) calls SetCursorPos and
/// GetCursorPos; tests use fakes, so no test ever moves the owner's cursor.
pub trait CursorPort {
    /// SetCursorPos; true when Windows accepted it.
    fn set_cursor_pos(&mut self, x: i32, y: i32) -> bool;
    /// GetCursorPos; `None` when Windows refused it.
    fn cursor_pos(&self) -> Option<(i32, i32)>;
}

/// The hook step and the engine it drives. Lives on the hook thread only.
pub struct HookCore {
    raw_dx: i32,
    raw_dy: i32,
    raw_buttons: u16,
    raw_since: Option<u64>,
    engine: Box<dyn Decider>,
    shared: Arc<HookShared>,
    fixes: fence_engine::fixes::Fixes,
}

impl HookCore {
    pub fn new(engine: Box<dyn Decider>, shared: Arc<HookShared>) -> Self {
        Self {
            raw_dx: 0,
            raw_dy: 0,
            raw_buttons: 0,
            raw_since: None,
            engine,
            shared,
            fixes: fence_engine::fixes::Fixes::ALL,
        }
    }

    pub fn shared(&self) -> &Arc<HookShared> {
        &self.shared
    }

    /// Batch only device counts into <=200Hz updates, independent of display scaling.
    pub fn raw_mouse(&mut self, dx: i32, dy: i32, flags: u16, now: u64) {
        for i in 0..5 {
            if flags & (1 << (2 * i)) != 0 {
                self.raw_buttons |= 1 << i;
            }
            if flags & (2 << (2 * i)) != 0 {
                self.raw_buttons &= !(1 << i);
            }
        }
        let since = self.raw_since.get_or_insert(now);
        if now.saturating_sub(*since) > 32_000 {
            self.raw_dx = 0;
            self.raw_dy = 0;
            *since = now;
        }
        self.raw_dx = self.raw_dx.saturating_add(dx);
        self.raw_dy = self.raw_dy.saturating_add(dy);
        if now.saturating_sub(*since) < 5000 {
            return;
        }
        let (x, y) = (self.raw_dx, self.raw_dy);
        self.raw_dx = 0;
        self.raw_dy = 0;
        self.raw_since = Some(now);
        self.engine.raw_motion(x, y, self.raw_buttons, now);
        if self.shared.trace_enabled.load(Ordering::Acquire) && self.fixes.j24_edge_trace {
            let mut features = [0; 12];
            features[6] = i32::from(self.raw_buttons);
            features[10] = x;
            features[11] = y;
            self.shared.trace.push(EdgeRecord {
                qpc_us: now,
                time_ms: 0,
                x,
                y,
                action: 3,
                model_state: 0,
                features,
            });
        }
    }

    pub fn engine(&self) -> &dyn Decider {
        self.engine.as_ref()
    }

    pub(super) fn into_engine(self) -> Box<dyn Decider> {
        self.engine
    }

    /// One callback (C# `HookCallback`). Never panics and never unwinds.
    pub fn step(&mut self, ev: &HookEvent, cursor: &mut dyn CursorPort) -> StepOutcome {
        let started = if self.fixes.j26_hook_metrics {
            crate::qpc::now_us()
        } else {
            0
        };
        // Stamp EVERY delivered event before any early-out: the counter is the liveness evidence (B021).
        self.shared.callbacks.fetch_add(1, Ordering::Relaxed);
        let outcome = match catch_unwind(AssertUnwindSafe(|| self.step_inner(ev, cursor))) {
            Ok(outcome) => outcome,
            Err(_) => {
                // B028/J08: the move passes, the panic is counted, and the engine state (possibly half
                // updated) is dropped so the next move starts from a clean baseline.
                self.shared.panics.fetch_add(1, Ordering::Relaxed);
                if self.fixes.j22_seed_after_panic {
                    self.reset_and_seed(|| Some((ev.x, ev.y)));
                } else {
                    let _ = catch_unwind(AssertUnwindSafe(|| self.engine.reset()));
                }
                StepOutcome::CallNext
            }
        };
        if self.fixes.j26_hook_metrics {
            self.shared.record_timing(crate::qpc::now_us().saturating_sub(started));
        }
        outcome
    }

    fn step_inner(&mut self, ev: &HookEvent, cursor: &mut dyn CursorPort) -> StepOutcome {
        self.sync_confine();
        if ev.n_code < 0 || ev.message != WM_MOUSEMOVE || !self.engine.needs_processing() {
            return StepOutcome::CallNext;
        }
        // Ignore injected events (includes our own SetCursorPos clamps): avoids recursion (B022).
        if ev.flags & (LLMHF_INJECTED | LLMHF_LOWER_IL_INJECTED) != 0 {
            return StepOutcome::CallNext;
        }
        if let Some((x, y)) = cursor.cursor_pos() {
            self.engine.observe(x, y);
        }
        let toggles = self.shared.toggles();
        let d = self.engine.decide(
            MoveEvent {
                x: ev.x,
                y: ev.y,
                time_ms: ev.time_ms,
                qpc_us: ev.qpc_us,
            },
            toggles,
        );
        let edge = if self.shared.trace_enabled.load(Ordering::Acquire) && self.fixes.j24_edge_trace {
            Some(self.engine.edge_state())
        } else {
            None
        };
        if d.action == Action::Block {
            // SetCursorPos(bx, by); the placement re-enters as injected (B023).
            if cursor.set_cursor_pos(d.x, d.y) {
                self.record_edge(ev, edge, 1, false, (d.x, d.y));
                return StepOutcome::Swallow;
            }
            // J07 (PLAN T10): 1.1.1 ignored the result and swallowed the move anyway, so a refused placement
            // left the cursor frozen where it was. 2.0 fails open: the move passes, it is counted as degraded,
            // and the engine (which recorded the target it could not place) restarts from the point the
            // cursor really goes to.
            self.shared.degraded.fetch_add(1, Ordering::Relaxed);
            self.record_edge(ev, edge, 0, true, (ev.x, ev.y));
            self.reset_and_seed(|| Some((ev.x, ev.y)));
            return StepOutcome::CallNext;
        }
        self.record_edge(ev, edge, 0, false, (ev.x, ev.y));
        StepOutcome::CallNext
    }

    fn record_edge(&self, ev: &HookEvent, edge: Option<EdgeState>, action: u8, degraded: bool, accepted: (i32, i32)) {
        let Some(edge) = edge else {
            return;
        };
        self.shared
            .estimated_rate_milli
            .store((edge.estimated_rate * 1000.0) as u32, Ordering::Relaxed);
        let crossed = if degraded { edge.attempt_crossed } else { edge.crossed };
        if edge.near || crossed {
            let mut features = edge.features;
            features[10] = accepted.0;
            features[11] = accepted.1;
            if action == 1 && features[7] == 0 {
                features[7] = 8;
            }
            self.shared.trace.push(EdgeRecord {
                features,
                qpc_us: ev.qpc_us,
                time_ms: ev.time_ms,
                x: ev.x,
                y: ev.y,
                action: if crossed { 2 } else { action },
                model_state: edge.model_state | 128 | if degraded { 64 } else { 0 },
            });
        }
    }

    /// Game mode lives in the engine (C# `GuardCore.Confine`); the tray writes the atomic, the step copies it
    /// in before `needs_processing`, which reads it.
    fn sync_confine(&mut self) {
        let want = self.shared.confine.load(Ordering::Relaxed);
        if self.engine.confine() != want {
            self.engine.set_confine(want);
        }
    }

    /// C# `_core.Reset()` (Start, reinstall, Configure), then J03: the engine is seeded from GetCursorPos.
    ///
    /// The reset forgets the last position (C# `HaveLast = false`), and 1.1.1 then let the first move pass
    /// unchecked (GuardCore.Decide: `if (!HaveLast) return Pass`), so a move across a closed seam right after
    /// a Start, a reinstall or a display change went through (lesson L9). 2.0 hands the cursor's current
    /// position to the engine at once, on the hook thread, so the first real move is judged from where the
    /// cursor is. The seed is not a move: its decision is dropped and nothing is placed or swallowed. When
    /// GetCursorPos fails, or the engine needs no processing (it would not see the next moves, so a seed
    /// could only go stale), the engine stays unseeded as in 1.1.1. Contained like the step.
    pub fn reset(&mut self, cursor: &dyn CursorPort) {
        self.reset_and_seed(|| cursor.cursor_pos());
    }

    fn reset_and_seed(&mut self, seed: impl FnOnce() -> Option<(i32, i32)>) {
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            self.engine.reset();
            self.sync_confine();
            if !self.engine.needs_processing() {
                return;
            }
            if let Some((x, y)) = seed() {
                let toggles = self.shared.toggles();
                let _ = self.engine.decide(MoveEvent::untimed(x, y), toggles);
            }
        }));
        if outcome.is_err() {
            self.shared.panics.fetch_add(1, Ordering::Relaxed);
            let _ = catch_unwind(AssertUnwindSafe(|| self.engine.reset()));
        }
    }

    /// C# `MouseGuard.Configure`: a freshly compiled engine replaces the old one (built off the hook thread),
    /// then the state is reset and seeded (re-baseline, J03).
    pub fn set_engine(&mut self, engine: Box<dyn Decider>, cursor: &dyn CursorPort) {
        self.engine = engine;
        self.reset(cursor);
    }
}

#[cfg(test)]
pub(crate) mod testkit {
    //! Fakes shared by the hook tests: a scripted decider and a cursor that records instead of moving.
    use super::*;
    use fence_types::Decision;
    use std::sync::Mutex;

    /// What the scripted decider saw.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum Seen {
        Decide(i32, i32, Toggles),
        Reset,
        SetConfine(bool),
    }

    pub struct ScriptDecider {
        pub needs: bool,
        /// Moves at or above this Y are blocked to `(x, block_to_y)`.
        pub block_above: Option<(i32, i32)>,
        pub panic_on_decide: bool,
        pub panic_once: bool,
        pub panic_on_reset: bool,
        pub panic_on_needs: bool,
        pub panic_on_set_confine: bool,
        pub confine: bool,
        pub log: Arc<Mutex<Vec<Seen>>>,
    }

    impl ScriptDecider {
        pub fn new(needs: bool) -> (Self, Arc<Mutex<Vec<Seen>>>) {
            let log = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    needs,
                    block_above: None,
                    panic_on_decide: false,
                    panic_once: false,
                    panic_on_reset: false,
                    panic_on_needs: false,
                    panic_on_set_confine: false,
                    confine: false,
                    log: log.clone(),
                },
                log,
            )
        }
    }

    impl Decider for ScriptDecider {
        fn needs_processing(&self) -> bool {
            if self.panic_on_needs {
                panic!("scripted needs_processing panic (test)");
            }
            self.needs || self.confine
        }
        fn decide(&mut self, event: MoveEvent, toggles: Toggles) -> Decision {
            let (x, y) = (event.x, event.y);
            self.log.lock().unwrap().push(Seen::Decide(x, y, toggles));
            if self.panic_once {
                self.panic_once = false;
                panic!("one contained test panic");
            }
            if self.panic_on_decide {
                panic!("scripted decider panic (test)");
            }
            match self.block_above {
                Some((limit, to_y)) if y < limit => Decision {
                    action: Action::Block,
                    x,
                    y: to_y,
                },
                _ => Decision {
                    action: Action::Pass,
                    x,
                    y,
                },
            }
        }
        fn reset(&mut self) {
            self.log.lock().unwrap().push(Seen::Reset);
            if self.panic_on_reset {
                panic!("scripted reset panic (test)");
            }
        }
        fn set_confine(&mut self, on: bool) {
            self.confine = on;
            self.log.lock().unwrap().push(Seen::SetConfine(on));
            if self.panic_on_set_confine {
                panic!("scripted set_confine panic (test)");
            }
        }
        fn confine(&self) -> bool {
            self.confine
        }
    }

    /// Records placements; never touches the real cursor.
    #[derive(Debug, Default)]
    pub struct FakeCursor {
        pub accept: bool,
        pub pos: Option<(i32, i32)>,
        pub placed: Vec<(i32, i32)>,
    }

    impl FakeCursor {
        pub fn accepting() -> Self {
            Self {
                accept: true,
                pos: None,
                placed: Vec::new(),
            }
        }
    }

    impl CursorPort for FakeCursor {
        fn set_cursor_pos(&mut self, x: i32, y: i32) -> bool {
            self.placed.push((x, y));
            if self.accept {
                self.pos = Some((x, y));
            }
            self.accept
        }
        fn cursor_pos(&self) -> Option<(i32, i32)> {
            self.pos
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn j26_published_overflow_already_exposes_its_maximum_to_the_tray() {
        let s = super::HookShared::default();
        s.record_timing_published(1501, || {
            let m = s.metrics();
            assert_eq!(m.sampled_steps, 1);
            assert!(m.max_step_us >= 1501);
            assert!(m.p99_upper_us >= 1501);
        });
    }
    #[test]
    fn j26_histogram_uses_nearest_rank_upper_bounds_and_counts_overflow() {
        let s = super::HookShared::default();
        assert_eq!(s.metrics(), fence_types::HookMetrics::default());
        for _ in 0..99 {
            s.record_timing(2);
        }
        s.record_timing(1501);
        assert_eq!(
            (
                s.metrics().sampled_steps,
                s.metrics().max_step_us,
                s.metrics().p99_upper_us
            ),
            (100, 1501, 2)
        );
        s.record_timing(1700);
        assert_eq!(s.metrics().p99_upper_us, 1700);
        for (micros, upper) in [
            (0, 1),
            (1, 1),
            (3, 5),
            (11, 20),
            (51, 100),
            (201, 500),
            (501, 1000),
            (1001, 1001),
        ] {
            let one = super::HookShared::default();
            one.record_timing(micros);
            assert_eq!(one.metrics().p99_upper_us, upper);
        }
    }

    #[test]
    fn j26_step_records_early_returns_and_contained_panics() {
        let (mut d, _) = super::testkit::ScriptDecider::new(true);
        d.panic_once = true;
        let (mut core, shared) = core_with(d);
        let mut cursor = super::testkit::FakeCursor::accepting();
        core.step(&super::HookEvent::mouse_move(1, 1), &mut cursor);
        let mut ev = super::HookEvent::mouse_move(2, 2);
        ev.n_code = -1;
        core.step(&ev, &mut cursor);
        let m = shared.metrics();
        assert_eq!((m.callbacks, m.sampled_steps, m.panics), (2, 2, 1));
        assert!(m.p99_upper_us >= 1);
    }
    use super::testkit::*;
    use super::*;

    #[test]
    fn j24_failed_confine_correction_does_not_label_an_unrelated_free_seam_guarded() {
        use fence_engine::{compile::compile, decide::ClassicEngine};
        use fence_types::{BoundaryDefault, EngineConfig, Policy, Rect, Screen};
        let screens = [
            Screen {
                stable_id: "SYN-MAIN".into(),
                bounds: Rect::new(0, 0, 1000, 1000),
                primary: true,
                excluded: false,
            },
            Screen {
                stable_id: "SYN-TOP".into(),
                bounds: Rect::new(0, -1000, 1500, 0),
                primary: false,
                excluded: false,
            },
            Screen {
                stable_id: "SYN-RIGHT".into(),
                bounds: Rect::new(1000, 0, 2000, 1000),
                primary: false,
                excluded: false,
            },
        ];
        let policy = Policy {
            side_default: BoundaryDefault::Free,
            ..Policy::default()
        };
        let e = ClassicEngine::new(EngineConfig {
            monitors: screens.iter().map(|s| s.bounds).collect(),
            transitions: compile(&policy, &screens, false).transitions,
            ..EngineConfig::default()
        });
        let shared = Arc::new(HookShared::default());
        shared.set_toggles(true, true, true);
        shared.trace_enabled.store(true, Ordering::Release);
        let mut core = HookCore::new(Box::new(e), shared.clone());
        let mut cursor = FakeCursor {
            accept: false,
            pos: Some((999, 500)),
            placed: Vec::new(),
        };
        core.step(
            &HookEvent {
                time_ms: 1,
                qpc_us: 1000,
                ..HookEvent::mouse_move(1010, 500)
            },
            &mut cursor,
        );
        let mut out = Vec::new();
        shared.trace.drain(&mut out);
        assert!(
            out.is_empty(),
            "the Free right exit is far from the guarded Up corridor"
        );
        cursor.pos = Some((950, 100));
        core.step(
            &HookEvent {
                time_ms: 2,
                qpc_us: 2000,
                ..HookEvent::mouse_move(1200, -10)
            },
            &mut cursor,
        );
        shared.trace.drain(&mut out);
        assert_eq!(
            out.len(),
            1,
            "an actual guarded second seam after a Free first seam is recorded"
        );
        assert_eq!((out[0].action, out[0].model_state & 64), (2, 64));
    }

    #[test]
    fn j24_hook_trace_is_opt_in_sparse_and_keeps_native_times_crossings_and_holds() {
        use fence_engine::{compile::compile, decide::ClassicEngine};
        use fence_types::{EngineConfig, Policy, Rect, Screen};
        let screens = [
            Screen {
                stable_id: "SYN-MAIN".into(),
                bounds: Rect::new(0, 0, 1000, 1000),
                primary: true,
                excluded: false,
            },
            Screen {
                stable_id: "SYN-TOP".into(),
                bounds: Rect::new(0, -1000, 1000, 0),
                primary: false,
                excluded: false,
            },
        ];
        let e = ClassicEngine::new(EngineConfig {
            monitors: screens.iter().map(|s| s.bounds).collect(),
            transitions: compile(&Policy::default(), &screens, false).transitions,
            ..EngineConfig::default()
        });
        let shared = Arc::new(HookShared::default());
        shared.set_toggles(true, true, false);
        let mut core = HookCore::new(Box::new(e), shared.clone());
        let mut cur = FakeCursor::accepting();
        let ev = |ms, x, y| HookEvent {
            time_ms: ms,
            qpc_us: ms as u64 * 1000,
            ..HookEvent::mouse_move(x, y)
        };
        cur.pos = Some((500, 0));
        core.step(&ev(1, 500, -10), &mut cur);
        let mut out = Vec::new();
        shared.trace.drain(&mut out);
        assert!(out.is_empty(), "off by default");
        shared.trace_enabled.store(true, Ordering::Relaxed);
        cur.pos = Some((500, 500));
        core.step(&ev(2, 501, 500), &mut cur);
        shared.trace.drain(&mut out);
        assert!(out.is_empty(), "interior moves are not recorded");
        cur.pos = Some((500, 65));
        core.step(&ev(3, 500, 64), &mut cur);
        cur.pos = Some((500, 0));
        assert_eq!(core.step(&ev(4, 500, -1), &mut cur), StepOutcome::Swallow);
        assert_eq!(core.step(&ev(5, 500, -10), &mut cur), StepOutcome::CallNext);
        let injected = HookEvent {
            flags: LLMHF_INJECTED,
            ..ev(6, 500, -20)
        };
        core.step(&injected, &mut cur);
        shared.trace.drain(&mut out);
        assert_eq!(
            out.iter().map(|r| (r.time_ms, r.qpc_us, r.action)).collect::<Vec<_>>(),
            vec![(3, 3000, 0), (4, 4000, 1), (5, 5000, 2)]
        );
        assert_eq!(shared.estimated_rate_milli.load(Ordering::Relaxed), 1000);
    }

    #[test]
    fn f10_physical_report_times_reach_the_decider_without_rewriting() {
        struct Timed {
            want: MoveEvent,
            seen: Arc<AtomicBool>,
        }
        impl Decider for Timed {
            fn needs_processing(&self) -> bool {
                true
            }
            fn decide(&mut self, event: MoveEvent, _: Toggles) -> fence_types::Decision {
                assert_eq!(event, self.want);
                self.seen.store(true, Ordering::Relaxed);
                fence_types::Decision {
                    action: Action::Pass,
                    x: event.x,
                    y: event.y,
                }
            }
            fn reset(&mut self) {}
            fn set_confine(&mut self, _: bool) {}
            fn confine(&self) -> bool {
                false
            }
        }
        let seen = Arc::new(AtomicBool::new(false));
        let want = MoveEvent {
            x: 12,
            y: -34,
            time_ms: 567,
            qpc_us: 890_123,
        };
        let mut core = HookCore::new(
            Box::new(Timed {
                want,
                seen: seen.clone(),
            }),
            Arc::new(HookShared::default()),
        );
        let ev = HookEvent {
            time_ms: want.time_ms,
            qpc_us: want.qpc_us,
            ..HookEvent::mouse_move(want.x, want.y)
        };
        assert_eq!(core.step(&ev, &mut FakeCursor::accepting()), StepOutcome::CallNext);
        assert!(seen.load(Ordering::Relaxed));
    }

    #[test]
    fn j10_observed_origin_ignores_stale_source_clamps_warps_and_top_permission() {
        use fence_engine::decide::ClassicEngine;
        use fence_types::{EngineConfig, Rect};
        let config = EngineConfig {
            has_top: true,
            barrier_y: 0,
            gates: vec![(24, 76)],
            has_sides: true,
            main: Rect::new(0, 0, 100, 100),
            monitors: vec![
                Rect::new(0, 0, 100, 100),
                Rect::new(-100, 0, 0, 100),
                Rect::new(0, -100, 100, 0),
            ],
            ..EngineConfig::default()
        };
        let cases = [
            ((-1, 50), (0, 50), (1, 51), false, StepOutcome::CallNext, None),
            ((-1, 5), (50, 90), (51, 91), false, StepOutcome::CallNext, None),
            ((-1, 50), (50, 50), (51, 51), true, StepOutcome::CallNext, None),
            ((-1, 50), (0, 50), (-1, 51), true, StepOutcome::Swallow, Some((0, 51))),
            ((50, 50), (50, -50), (51, -49), false, StepOutcome::CallNext, None),
            ((50, -50), (50, 0), (50, -1), false, StepOutcome::Swallow, Some((50, 0))),
            ((25, 0), (23, 0), (25, -10), false, StepOutcome::Swallow, Some((25, 0))),
            ((23, 0), (25, 0), (25, -10), false, StepOutcome::CallNext, None),
            ((50, 90), (50, 0), (50, -1), false, StepOutcome::Swallow, Some((50, 0))),
        ];
        for (history, origin, target, confine, outcome, placed) in cases {
            let mut engine = ClassicEngine::new(config.clone());
            engine.decide_xy(history.0, history.1, true, true);
            let shared = Arc::new(HookShared::default());
            shared.set_toggles(true, true, confine);
            let mut core = HookCore::new(Box::new(engine), shared);
            let mut cur = FakeCursor {
                accept: true,
                pos: Some(origin),
                placed: Vec::new(),
            };
            assert_eq!(
                core.step(&HookEvent::mouse_move(target.0, target.1), &mut cur),
                outcome,
                "history={history:?}, observed={origin:?}, target={target:?}"
            );
            assert_eq!(cur.placed.last().copied(), placed);
        }
    }

    #[test]
    fn j10_observed_origin_compiled_directions_and_forbidden_entry() {
        use fence_engine::{compile::compile, decide::ClassicEngine};
        use fence_types::{BoundaryDefault, EngineConfig, Policy, Rect, Screen};
        let source = Rect::new(0, 0, 100, 100);
        let cases = [
            (
                Rect::new(0, -100, 100, 0),
                (50, 1),
                (50, -10),
                (51, -11),
                (51, -1),
                (51, 2),
            ),
            (
                Rect::new(0, 100, 100, 200),
                (50, 98),
                (50, 110),
                (51, 111),
                (51, 100),
                (51, 97),
            ),
            (
                Rect::new(-100, 0, 0, 100),
                (1, 50),
                (-10, 50),
                (-11, 51),
                (-1, 51),
                (2, 51),
            ),
            (
                Rect::new(100, 0, 200, 100),
                (98, 50),
                (110, 50),
                (111, 51),
                (100, 51),
                (97, 51),
            ),
        ];
        for (dest, from, to, within, target, hold) in cases {
            let screens = [
                Screen {
                    stable_id: "SYN-A".into(),
                    bounds: source,
                    primary: true,
                    excluded: false,
                },
                Screen {
                    stable_id: "SYN-B".into(),
                    bounds: dest,
                    primary: false,
                    excluded: false,
                },
            ];
            let policy = Policy {
                upward_default: BoundaryDefault::Blocked,
                downward_default: BoundaryDefault::Blocked,
                side_default: BoundaryDefault::Blocked,
                safety_escapes: false,
                passage_inset_px: 0,
                edge_holdback_px: 2,
                ..Policy::default()
            };
            let config = EngineConfig {
                monitors: vec![source, dest],
                transitions: compile(&policy, &screens, false).transitions,
                ..EngineConfig::default()
            };
            for (history, origin, attempt, want) in [(from, to, within, None), (to, from, target, Some(hold))] {
                let mut e = ClassicEngine::new(config.clone());
                e.set_last(history.0, history.1);
                let shared = Arc::new(HookShared::default());
                shared.set_toggles(true, true, false);
                let mut core = HookCore::new(Box::new(e), shared);
                let mut cur = FakeCursor {
                    accept: true,
                    pos: Some(origin),
                    placed: Vec::new(),
                };
                core.step(&HookEvent::mouse_move(attempt.0, attempt.1), &mut cur);
                assert_eq!(cur.placed.last().copied(), want);
            }
        }
        let mut e = ClassicEngine::new(EngineConfig {
            monitors: vec![Rect::new(-100, 0, 0, 100), Rect::new(100, 0, 200, 100)],
            forbidden: vec![Rect::new(0, 0, 100, 100)],
            ..EngineConfig::default()
        });
        e.set_last(-1, 10);
        let mut core = HookCore::new(Box::new(e), Arc::new(HookShared::default()));
        let mut cur = FakeCursor {
            accept: true,
            pos: Some((100, 80)),
            placed: Vec::new(),
        };
        core.step(&HookEvent::mouse_move(50, 20), &mut cur);
        assert_eq!(cur.placed, vec![(100, 80)]);
    }

    fn core_with(decider: ScriptDecider) -> (HookCore, Arc<HookShared>) {
        let shared = Arc::new(HookShared::default());
        (HookCore::new(Box::new(decider), shared.clone()), shared)
    }

    #[test]
    fn j22_contained_panic_resets_and_seeds_from_the_passed_event_without_placement() {
        let (mut d, log) = ScriptDecider::new(true);
        d.panic_once = true;
        let (mut core, shared) = core_with(d);
        let mut cursor = FakeCursor::accepting();
        assert_eq!(
            core.step(&HookEvent::mouse_move(42, 24), &mut cursor),
            StepOutcome::CallNext
        );
        assert_eq!(shared.counters().panics, 1);
        assert!(cursor.placed.is_empty());
        assert_eq!(
            *log.lock().unwrap(),
            vec![
                Seen::Decide(42, 24, shared.toggles()),
                Seen::Reset,
                Seen::Decide(42, 24, shared.toggles())
            ]
        );
        core.step(&HookEvent::mouse_move(43, 25), &mut cursor);
        assert_eq!(shared.counters().panics, 1, "recovery permits the next move");
    }

    fn decides(log: &Arc<std::sync::Mutex<Vec<Seen>>>) -> usize {
        log.lock()
            .unwrap()
            .iter()
            .filter(|s| matches!(s, Seen::Decide(..)))
            .count()
    }

    #[test]
    fn s_win_hook_constants_match_windows_sys() {
        use windows_sys::Win32::UI::WindowsAndMessaging as wm;
        assert_eq!(WM_MOUSEMOVE, wm::WM_MOUSEMOVE);
        assert_eq!(LLMHF_INJECTED, wm::LLMHF_INJECTED);
        assert_eq!(LLMHF_LOWER_IL_INJECTED, wm::LLMHF_LOWER_IL_INJECTED);
    }

    #[test]
    fn b021_every_event_is_counted_before_any_early_out() {
        let (d, log) = ScriptDecider::new(true);
        let (mut core, shared) = core_with(d);
        let mut cur = FakeCursor::accepting();
        let negative = HookEvent {
            n_code: -1,
            ..HookEvent::mouse_move(1, 1)
        };
        let click = HookEvent {
            message: 0x0201, // WM_LBUTTONDOWN
            ..HookEvent::mouse_move(1, 1)
        };
        assert_eq!(core.step(&negative, &mut cur), StepOutcome::CallNext);
        assert_eq!(core.step(&click, &mut cur), StepOutcome::CallNext);
        assert_eq!(core.step(&HookEvent::mouse_move(5, 5), &mut cur), StepOutcome::CallNext);
        assert_eq!(shared.counters().callbacks, 3, "every delivered event is counted");
        assert_eq!(
            decides(&log),
            1,
            "only the WM_MOUSEMOVE with nCode >= 0 reaches the engine"
        );
    }

    #[test]
    fn b021_not_needs_processing_passes_without_deciding() {
        let (d, log) = ScriptDecider::new(false);
        let (mut core, shared) = core_with(d);
        let mut cur = FakeCursor::accepting();
        assert_eq!(
            core.step(&HookEvent::mouse_move(10, -50), &mut cur),
            StepOutcome::CallNext
        );
        assert_eq!(decides(&log), 0);
        assert_eq!(shared.counters().callbacks, 1);
        assert!(cur.placed.is_empty());
    }

    #[test]
    fn b021_confine_toggle_makes_the_engine_need_processing() {
        // C# NeedsProcessing reads Confine; the atomic reaches the engine before that check.
        let (d, log) = ScriptDecider::new(false);
        let (mut core, shared) = core_with(d);
        let mut cur = FakeCursor::accepting();
        shared.confine.store(true, Ordering::Relaxed);
        core.step(&HookEvent::mouse_move(3, 4), &mut cur);
        let seen = log.lock().unwrap().clone();
        assert_eq!(seen[0], Seen::SetConfine(true));
        assert!(matches!(seen[1], Seen::Decide(3, 4, _)));
    }

    #[test]
    fn b022_injected_moves_pass_untouched() {
        for flags in [
            LLMHF_INJECTED,
            LLMHF_LOWER_IL_INJECTED,
            LLMHF_INJECTED | LLMHF_LOWER_IL_INJECTED,
        ] {
            let (mut d, log) = ScriptDecider::new(true);
            d.block_above = Some((0, 0));
            let (mut core, shared) = core_with(d);
            let mut cur = FakeCursor::accepting();
            let ev = HookEvent {
                flags,
                ..HookEvent::mouse_move(100, -40)
            };
            assert_eq!(core.step(&ev, &mut cur), StepOutcome::CallNext, "flags {flags:#x}");
            assert_eq!(decides(&log), 0, "an injected move never reaches the engine");
            assert!(cur.placed.is_empty(), "an injected move never moves the cursor");
            assert_eq!(shared.counters().callbacks, 1);
        }
    }

    #[test]
    fn b023_block_places_the_cursor_and_swallows_pass_calls_next() {
        let (mut d, log) = ScriptDecider::new(true);
        d.block_above = Some((0, 0));
        let (mut core, shared) = core_with(d);
        shared.set_toggles(true, false, false);
        let mut cur = FakeCursor::accepting();
        assert_eq!(
            core.step(&HookEvent::mouse_move(200, -30), &mut cur),
            StepOutcome::Swallow
        );
        assert_eq!(cur.placed, vec![(200, 0)], "SetCursorPos(target)");
        assert_eq!(
            core.step(&HookEvent::mouse_move(210, 40), &mut cur),
            StepOutcome::CallNext
        );
        assert_eq!(cur.placed.len(), 1, "Pass places nothing");
        let toggles = log
            .lock()
            .unwrap()
            .iter()
            .find_map(|s| match s {
                Seen::Decide(_, _, t) => Some(*t),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            toggles,
            Toggles {
                gate_open: true,
                side_contain: false
            },
            "the live toggles reach the decision"
        );
    }

    #[test]
    fn b028_a_panic_in_the_step_is_contained_and_counted() {
        let (mut d, log) = ScriptDecider::new(true);
        d.panic_on_decide = true;
        let (mut core, shared) = core_with(d);
        let mut cur = FakeCursor::accepting();
        let outcome = core.step(&HookEvent::mouse_move(1, 2), &mut cur);
        assert_eq!(outcome, StepOutcome::CallNext, "the event passes through");
        assert_eq!(
            shared.counters().panics,
            2,
            "the step and failed recovery seed are counted"
        );
        assert_eq!(shared.counters().callbacks, 1);
        assert!(cur.placed.is_empty());
        assert_eq!(
            log.lock().unwrap().last(),
            Some(&Seen::Reset),
            "the engine is re-baselined"
        );
        // The core still works afterwards.
        assert_eq!(core.step(&HookEvent::mouse_move(1, 3), &mut cur), StepOutcome::CallNext);
        assert_eq!(shared.counters().panics, 4);
    }

    #[test]
    fn j03_reset_seeds_the_engine_from_the_cursor_without_placing_it() {
        let (d, log) = ScriptDecider::new(true);
        let (mut core, shared) = core_with(d);
        shared.set_toggles(true, false, false);
        let cur = FakeCursor {
            accept: true,
            pos: Some((40, 300)),
            placed: Vec::new(),
        };
        core.reset(&cur);
        assert_eq!(
            *log.lock().unwrap(),
            vec![
                Seen::Reset,
                Seen::Decide(
                    40,
                    300,
                    Toggles {
                        gate_open: true,
                        side_contain: false
                    }
                )
            ],
            "reset, then one decision at the cursor with the live toggles"
        );
        assert!(cur.placed.is_empty(), "the seed is not a move: nothing is placed");
        assert_eq!(shared.counters().callbacks, 0, "the seed is not a callback");
    }

    #[test]
    fn j03_no_seed_without_a_cursor_position_or_when_the_engine_needs_nothing() {
        // GetCursorPos failed: the engine stays unseeded, as 1.1.1.
        let (d, log) = ScriptDecider::new(true);
        let (mut core, _shared) = core_with(d);
        core.reset(&FakeCursor::accepting());
        assert_eq!(*log.lock().unwrap(), vec![Seen::Reset]);

        // An engine that would not see the next moves is not seeded (a seed could only go stale).
        let (d, log) = ScriptDecider::new(false);
        let (mut core, _shared) = core_with(d);
        let cur = FakeCursor {
            accept: true,
            pos: Some((1, 1)),
            placed: Vec::new(),
        };
        core.reset(&cur);
        assert_eq!(*log.lock().unwrap(), vec![Seen::Reset]);
    }

    #[test]
    fn j03_the_seed_sees_the_live_confine() {
        // Game mode alone makes the engine need processing; the atomic reaches it before the seed.
        let (d, log) = ScriptDecider::new(false);
        let (mut core, shared) = core_with(d);
        shared.set_toggles(false, false, true);
        let cur = FakeCursor {
            accept: true,
            pos: Some((7, 8)),
            placed: Vec::new(),
        };
        core.reset(&cur);
        let seen = log.lock().unwrap().clone();
        assert_eq!(seen[..2], [Seen::Reset, Seen::SetConfine(true)]);
        assert!(matches!(seen[2], Seen::Decide(7, 8, _)));
    }

    #[test]
    fn j03_a_panic_in_the_seed_is_contained_and_counted() {
        let (mut d, log) = ScriptDecider::new(true);
        d.panic_on_decide = true;
        let (mut core, shared) = core_with(d);
        let cur = FakeCursor {
            accept: true,
            pos: Some((3, 3)),
            placed: Vec::new(),
        };
        core.reset(&cur);
        assert_eq!(shared.counters().panics, 1);
        assert_eq!(log.lock().unwrap().last(), Some(&Seen::Reset), "left reset, unseeded");
        assert!(cur.placed.is_empty());
    }

    // ------------------------------------------------------------ J07 SetCursorPos failure (PLAN T10)

    /// A decider that blocks every move above y = 0 onto the seam, over a cursor that refuses placement.
    fn refusing() -> (HookCore, Arc<HookShared>, Arc<std::sync::Mutex<Vec<Seen>>>, FakeCursor) {
        let (mut d, log) = ScriptDecider::new(true);
        d.block_above = Some((0, 0));
        let (core, shared) = core_with(d);
        let cur = FakeCursor {
            accept: false,
            pos: Some((200, 10)),
            placed: Vec::new(),
        };
        (core, shared, log, cur)
    }

    #[test]
    fn j07_a_refused_placement_lets_the_move_pass_and_counts_it_degraded() {
        let (mut core, shared, log, mut cur) = refusing();
        let outcome = core.step(&HookEvent::mouse_move(200, -30), &mut cur);
        assert_eq!(
            outcome,
            StepOutcome::CallNext,
            "fail open: a swallowed move with a failed correction would freeze the cursor"
        );
        assert_eq!(cur.placed, vec![(200, 0)], "the placement was tried once");
        assert_eq!(
            shared.counters(),
            HookCounters {
                callbacks: 1,
                panics: 0,
                degraded: 1
            }
        );
        // The engine recorded a target it could not place; it restarts from where the cursor really goes.
        let seen = log.lock().unwrap().clone();
        assert!(matches!(seen[0], Seen::Decide(200, -30, _)), "{seen:?}");
        assert_eq!(seen[1], Seen::Reset);
        assert!(
            matches!(seen[2], Seen::Decide(200, -30, _)),
            "seeded at the event point: {seen:?}"
        );
        assert_eq!(seen.len(), 3);
        assert_eq!(cur.placed.len(), 1, "the seed's own decision places nothing");
    }

    #[test]
    fn j07_an_accepted_placement_still_swallows_and_counts_nothing() {
        let (mut core, shared, _log, mut cur) = refusing();
        cur.accept = true;
        assert_eq!(
            core.step(&HookEvent::mouse_move(200, -30), &mut cur),
            StepOutcome::Swallow
        );
        assert_eq!(shared.counters().degraded, 0);
    }

    #[test]
    fn j07_each_refusal_is_counted_and_a_later_placement_works_again() {
        let (mut core, shared, _log, mut cur) = refusing();
        for _ in 0..3 {
            assert_eq!(
                core.step(&HookEvent::mouse_move(200, -30), &mut cur),
                StepOutcome::CallNext
            );
        }
        assert_eq!(shared.counters().degraded, 3);
        cur.accept = true;
        assert_eq!(
            core.step(&HookEvent::mouse_move(200, -31), &mut cur),
            StepOutcome::Swallow
        );
        assert_eq!(shared.counters().degraded, 3, "a working placement is not degraded");
        assert_eq!(cur.pos, Some((200, 0)));
    }

    // ------------------------------------------------------------ J08 panic containment (PLAN T12)

    /// A cursor whose placement panics (a bug in the platform layer).
    struct PanickingCursor;
    impl CursorPort for PanickingCursor {
        fn set_cursor_pos(&mut self, _x: i32, _y: i32) -> bool {
            panic!("cursor placement panic (test)");
        }
        fn cursor_pos(&self) -> Option<(i32, i32)> {
            Some((0, 0))
        }
    }

    /// A named way to make the scripted decider panic.
    type PanicCase = (&'static str, fn(&mut ScriptDecider));

    #[test]
    fn j08_a_panic_in_any_engine_call_of_the_step_passes_the_event() {
        let cases: [PanicCase; 3] = [
            ("needs_processing", |d| d.panic_on_needs = true),
            ("set_confine", |d| d.panic_on_set_confine = true),
            ("decide", |d| d.panic_on_decide = true),
        ];
        for (name, arm) in cases {
            let (mut d, _log) = ScriptDecider::new(true);
            d.block_above = Some((0, 0));
            arm(&mut d);
            let (mut core, shared) = core_with(d);
            shared.set_toggles(false, false, true); // flips confine, so set_confine runs in the step
            let mut cur = FakeCursor::accepting();
            assert_eq!(
                core.step(&HookEvent::mouse_move(5, -5), &mut cur),
                StepOutcome::CallNext,
                "{name}: the event passes"
            );
            assert_eq!(
                shared.counters().panics,
                if name == "set_confine" { 1 } else { 2 },
                "{name}: counted including failed seed"
            );
            assert_eq!(shared.counters().callbacks, 1, "{name}: still counted as a callback");
            assert!(cur.placed.is_empty(), "{name}: nothing placed");
        }
    }

    #[test]
    fn j08_a_panic_while_placing_the_cursor_is_contained() {
        let (mut d, log) = ScriptDecider::new(true);
        d.block_above = Some((0, 0));
        let (mut core, shared) = core_with(d);
        let outcome = core.step(&HookEvent::mouse_move(9, -9), &mut PanickingCursor);
        assert_eq!(outcome, StepOutcome::CallNext);
        assert_eq!(shared.counters().panics, 1);
        assert_eq!(
            log.lock().unwrap().last(),
            Some(&Seen::Decide(9, -9, shared.toggles())),
            "the engine is re-baselined at the event point"
        );
    }

    #[test]
    fn j08_a_panic_in_reset_or_set_engine_is_contained() {
        let (mut d, _log) = ScriptDecider::new(true);
        d.panic_on_reset = true;
        let (mut core, shared) = core_with(d);
        let cur = FakeCursor::accepting();
        core.reset(&cur);
        assert_eq!(shared.counters().panics, 1, "reset");

        let (mut d2, log2) = ScriptDecider::new(true);
        d2.panic_on_reset = true;
        core.set_engine(Box::new(d2), &cur);
        assert_eq!(shared.counters().panics, 2, "set_engine");
        assert_eq!(
            log2.lock().unwrap().first(),
            Some(&Seen::Reset),
            "the new engine is in place"
        );

        // The step still works on the engine whose reset panicked.
        let mut cur = FakeCursor::accepting();
        assert_eq!(core.step(&HookEvent::mouse_move(1, 1), &mut cur), StepOutcome::CallNext);
        assert_eq!(shared.counters().callbacks, 1);
    }

    #[test]
    fn j08_a_panic_in_the_fail_open_reseed_is_contained() {
        // J07 meets J08: the placement is refused and the re-seed panics; still one passed event.
        let (mut d, _log) = ScriptDecider::new(true);
        d.block_above = Some((0, 0));
        d.panic_on_reset = true;
        let (mut core, shared) = core_with(d);
        let mut cur = FakeCursor {
            accept: false,
            pos: None,
            placed: Vec::new(),
        };
        assert_eq!(
            core.step(&HookEvent::mouse_move(4, -4), &mut cur),
            StepOutcome::CallNext
        );
        let c = shared.counters();
        assert_eq!((c.callbacks, c.degraded, c.panics), (1, 1, 1));
    }

    #[test]
    fn s_win_set_engine_replaces_and_resets() {
        let (d1, log1) = ScriptDecider::new(true);
        let (mut core, _shared) = core_with(d1);
        let (d2, log2) = ScriptDecider::new(true);
        let cur = FakeCursor::accepting();
        core.set_engine(Box::new(d2), &cur);
        assert!(log1.lock().unwrap().is_empty(), "the old engine is dropped, not reset");
        assert_eq!(log2.lock().unwrap().first(), Some(&Seen::Reset));
    }
}
