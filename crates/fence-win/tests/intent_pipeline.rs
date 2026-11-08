//! Exercise the production batching, hook decision and fixed trace queue with a fake cursor.
use fence_engine::{compile::compile, decide::ClassicEngine};
use fence_types::*;
use fence_win::hook::{CursorPort, HookCore, HookEvent, HookShared, StepOutcome};
use std::sync::{Arc, atomic::Ordering};

struct Cursor {
    pos: (i32, i32),
}
impl CursorPort for Cursor {
    fn cursor_pos(&self) -> Option<(i32, i32)> {
        Some(self.pos)
    }
    fn set_cursor_pos(&mut self, x: i32, y: i32) -> bool {
        self.pos = (x, y);
        true
    }
}

#[test]
fn batched_device_counts_drive_a_light_crossing_and_publish_features_and_outcomes() {
    for hz in [125u64, 1000, 8000] {
        for drag in [false, true] {
            let screens = [
                Screen {
                    stable_id: "main".into(),
                    bounds: Rect::new(0, 0, 1000, 1000),
                    primary: true,
                    excluded: false,
                },
                Screen {
                    stable_id: "side".into(),
                    bounds: Rect::new(1000, 0, 2000, 1000),
                    primary: false,
                    excluded: false,
                },
            ];
            let policy = Policy {
                intent: IntentConfig {
                    model: CrossingModel::Intent,
                    ..Default::default()
                },
                side_default: BoundaryDefault::Push,
                side_cross_min: 12,
                ..Default::default()
            };
            let engine = ClassicEngine::new(EngineConfig {
                intent: policy.intent,
                monitors: screens.iter().map(|s| s.bounds).collect(),
                transitions: compile(&policy, &screens, false).transitions,
                ..Default::default()
            });
            let shared = Arc::new(HookShared::default());
            shared.set_toggles(true, true, false);
            shared.trace_enabled.store(true, Ordering::Release);
            let mut hook = HookCore::new(Box::new(engine), shared.clone());
            let mut cursor = Cursor { pos: (500, 500) };
            let mut frac = 0u64;
            let mut latency = None;
            for i in 0..hz * 400 / 1000 {
                let now = 1_000_000 + i * 1_000_000 / hz;
                frac += 800;
                let dx = (frac / hz) as i32;
                frac %= hz;
                // Raw arrives after the previous hook event in Windows; this update is one report behind.
                hook.raw_mouse(dx, 0, if drag && i == 0 { 1 } else { 0 }, now);
                cursor.pos = if now < 1_200_000 { (500, 500) } else { (999, 500) };
                let ev = HookEvent {
                    n_code: 0,
                    message: 0x200,
                    x: cursor.pos.0 + dx.max(1),
                    y: 500,
                    flags: 0,
                    time_ms: (now / 1000) as u32,
                    qpc_us: now + 1,
                };
                let outcome = hook.step(&ev, &mut cursor);
                if now >= 1_200_000 && outcome == StepOutcome::CallNext {
                    latency = Some((now - 1_200_000) / 1000);
                    break;
                }
            }
            assert!(latency.is_some_and(|ms| ms <= 100), "{hz}/{drag}: {latency:?}");
            let mut records = Vec::new();
            shared.trace.drain(&mut records);
            assert!(records.iter().any(|r| r.action == 3 && r.features[10] > 0));
            assert!(records.iter().any(|r| r.action == 1 && r.features[7] != 0));
            let cross = records
                .iter()
                .find(|r| r.action == 2)
                .expect("actual guarded crossing recorded");
            assert_eq!(cross.features[7], 6);
            assert_eq!(cross.features[6] != 0, drag);
            assert_eq!(
                (
                    shared.metrics().panics,
                    shared.metrics().degraded,
                    shared.metrics().trace_dropped
                ),
                (0, 0, 0)
            );
            eprintln!(
                "PIPELINE hz={hz} drag={drag} latency_ms={} features={:?}",
                latency.unwrap(),
                cross.features
            );
        }
    }
}
