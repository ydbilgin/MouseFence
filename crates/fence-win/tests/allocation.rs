//! Entire real-engine hook step, with a fake cursor and no Windows hooks or input.
use fence_engine::{compile::compile, decide::ClassicEngine};
use fence_types::{CrossingModel, EngineConfig, IntentConfig, Policy, Rect, Screen};
use fence_win::hook::{CursorPort, HookCore, HookEvent, HookShared, StepOutcome};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::{Arc, atomic::Ordering};

struct Counting;
thread_local! {static TRACK:Cell<bool>=const{Cell::new(false)};static BYTES:Cell<usize>=const{Cell::new(0)};}
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let _ = TRACK.try_with(|t| {
            if t.get() {
                let _ = BYTES.try_with(|n| n.set(n.get() + l.size()));
            }
        });
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let _ = TRACK.try_with(|t| {
            if t.get() {
                let _ = BYTES.try_with(|b| b.set(b.get() + n));
            }
        });
        unsafe { System.realloc(p, l, n) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let _ = TRACK.try_with(|t| {
            if t.get() {
                let _ = BYTES.try_with(|n| n.set(n.get() + l.size()));
            }
        });
        unsafe { System.alloc_zeroed(l) }
    }
}
#[global_allocator]
static ALLOC: Counting = Counting;
struct Cursor((i32, i32));
impl CursorPort for Cursor {
    fn cursor_pos(&self) -> Option<(i32, i32)> {
        Some(self.0)
    }
    fn set_cursor_pos(&mut self, x: i32, y: i32) -> bool {
        self.0 = (x, y);
        true
    }
}

#[test]
fn j26_hook_step_allocates_zero_bytes_for_100000_events_per_model_and_trace_mode() {
    fence_win::qpc::initialize();
    // A negative control proves the allocator observes allocations on this test thread.
    TRACK.set(true);
    let control = std::hint::black_box(vec![0u8; 37]);
    TRACK.set(false);
    assert!(BYTES.get() >= 37);
    drop(control);
    for model in [
        CrossingModel::Classic,
        CrossingModel::Classic1k,
        CrossingModel::StopThenPush,
    ] {
        for trace in [false, true] {
            let screens = vec![
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
            let policy = Policy {
                intent: IntentConfig {
                    model,
                    ..IntentConfig::default()
                },
                ..Policy::default()
            };
            let engine = ClassicEngine::new(EngineConfig {
                intent: policy.intent,
                monitors: screens.iter().map(|s| s.bounds).collect(),
                transitions: compile(&policy, &screens, false).transitions,
                ..EngineConfig::default()
            });
            let shared = Arc::new(HookShared::default());
            shared.set_toggles(true, true, false);
            shared.trace_enabled.store(trace, Ordering::Release);
            let mut core = HookCore::new(Box::new(engine), shared.clone());
            let mut cursor = Cursor((500, 1));
            core.reset(&cursor);
            let mut held = 0;
            let mut passed = 0;
            BYTES.set(0);
            TRACK.set(true);
            for i in 0..100000u64 {
                // Repeat an observed edge origin, alternating drift and deliberate outward input.
                cursor.0 = (500, 0);
                let ev = HookEvent {
                    time_ms: (i / 8) as u32,
                    qpc_us: 1 + i * 125,
                    ..HookEvent::mouse_move(500, if i % 32 == 0 { -100 } else { -1 })
                };
                match core.step(&ev, &mut cursor) {
                    StepOutcome::Swallow => held += 1,
                    StepOutcome::CallNext => passed += 1,
                }
            }
            TRACK.set(false);
            let bytes = BYTES.get();
            assert_eq!(bytes, 0, "{model:?}, trace={trace}");
            assert!(held > 0 && passed > 0, "both decision paths run: {model:?}");
            assert_eq!(
                (shared.metrics().callbacks, shared.metrics().sampled_steps),
                (100000, 100000)
            );
            if trace {
                assert!(shared.metrics().trace_dropped > 0, "bounded full-ring path runs");
            }
        }
    }
}
