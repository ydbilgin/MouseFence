//! Deterministic device-count replays. No live hook, cursor movement or wall clock.
use fence_engine::{compile::compile, decide::ClassicEngine};
use fence_types::*;

const SCREENS: [Rect; 4] = [
    Rect::new(-421, -1440, 3019, 0),
    Rect::new(-1920, 0, 0, 1080),
    Rect::new(0, 0, 2560, 1440),
    Rect::new(2560, 0, 4480, 1080),
];
const SEAMS: [(usize, usize, Direction, (i32, i32)); 7] = [
    (2, 0, Direction::Up, (1200, 0)),
    (1, 0, Direction::Up, (-200, 0)),
    (3, 0, Direction::Up, (2800, 0)),
    (2, 1, Direction::Left, (0, 500)),
    (1, 2, Direction::Right, (-1, 500)),
    (2, 3, Direction::Right, (2559, 500)),
    (3, 2, Direction::Left, (2560, 500)),
];
const SWITCHES: Toggles = Toggles {
    gate_open: true,
    side_contain: true,
};

fn vector(dir: Direction, p: i32, o: i32) -> (i32, i32) {
    match dir {
        Direction::Up => (o, -p),
        Direction::Down => (o, p),
        Direction::Left => (-p, o),
        Direction::Right => (p, o),
    }
}
fn engine(seam: usize, model: CrossingModel, ease: u8, automatic: bool) -> ClassicEngine {
    let (from, to, dir, _) = SEAMS[seam];
    let screens: Vec<_> = SCREENS
        .iter()
        .enumerate()
        .map(|(i, b)| Screen {
            stable_id: format!("S{i}"),
            bounds: *b,
            primary: i == 2,
            excluded: false,
        })
        .collect();
    let mut policy = Policy {
        intent: IntentConfig {
            model,
            crossing_ease: ease,
            ..Default::default()
        },
        up_cross_min: 12,
        side_cross_min: 12,
        ..Default::default()
    };
    if !automatic {
        policy.rules.push(BoundaryRule {
            from_stable_id: format!("S{from}"),
            to_stable_id: format!("S{to}"),
            direction: dir,
            behavior: RuleBehavior::Push,
            control: Control::Always,
        });
    }
    ClassicEngine::new(EngineConfig {
        intent: policy.intent,
        monitors: SCREENS.to_vec(),
        transitions: compile(&policy, &screens, false).transitions,
        ..Default::default()
    })
}

#[derive(Clone, Copy, Debug)]
enum Trace {
    Steady,
    Accelerate,
    Drag,
    Drift,
    Graze,
    Decelerate,
    Flick,
    Corner,
    Rest,
}

fn motion(trace: Trace, ms: f64) -> (f64, f64, u16) {
    match trace {
        Trace::Steady => (0.8, 0.08, 0),
        Trace::Accelerate => (0.25 + ms.min(240.0) / 150.0, 0.04, 0),
        Trace::Drag => (0.3, 0.02, 1),
        Trace::Drift => (0.04, 0.9, 0),
        Trace::Graze => (1.0, 3.0, 0),
        Trace::Decelerate | Trace::Corner => ((3.0 * (220.0 - ms) / 100.0).clamp(0.0, 3.0), 0.0, 0),
        Trace::Flick => (if (190.0..212.0).contains(&ms) { 30.0 } else { 0.0 }, 0.0, 0),
        Trace::Rest => (0.0, 0.0, 0),
    }
}

// Contacts begin at t=200ms. Raw may arrive up to 8ms behind the hook; scaling affects only pixels.
#[allow(clippy::too_many_arguments)] // Independent replay axes are printed in the evidence corpus.
fn replay(
    seam: usize,
    trace: Trace,
    hz: u64,
    ease: u8,
    lag: u64,
    scale: i32,
    model: CrossingModel,
    automatic: bool,
    confine: bool,
) -> (Option<u64>, [i32; 12]) {
    let mut e = engine(seam, model, ease, automatic);
    e.set_confine(confine);
    let (from, to, dir, edge) = SEAMS[seam];
    let center = (
        (SCREENS[from].l + SCREENS[from].r) / 2,
        (SCREENS[from].t + SCREENS[from].b) / 2,
    );
    let mut fractions = (0.0, 0.0);
    let mut result = None;
    let mut features = [0; 12];
    for i in 1..=hz * 600 / 1000 {
        let now = 1_000_000 + i * 1_000_000 / hz;
        let ms = i as f64 * 1000.0 / hz as f64;
        let (p, o, buttons) = motion(trace, ms);
        fractions.0 += p * 1000.0 / hz as f64;
        fractions.1 += o * 1000.0 / hz as f64;
        let (pc, oc) = (fractions.0.floor() as i32, fractions.1.floor() as i32);
        fractions.0 -= f64::from(pc);
        fractions.1 -= f64::from(oc);
        let (rx, ry) = vector(dir, pc, oc);
        e.raw_motion(rx, ry, buttons, now - lag);
        let origin = if ms < 200.0 { center } else { edge };
        e.set_last(origin.0, origin.1);
        // OS cursor scaling deliberately differs from the raw device counts.
        let (dx, dy) = vector(dir, (pc * scale).max(1), oc * scale);
        let d = e.decide_event(
            MoveEvent {
                x: origin.0 + dx,
                y: origin.1 + dy,
                time_ms: (now / 1000) as u32,
                qpc_us: now,
            },
            SWITCHES,
        );
        features = e.edge_state().features;
        if ms >= 200.0 && SCREENS[to].contains(d.x, d.y) {
            result = Some((now - 1_200_000) / 1000);
            break;
        }
    }
    (result, features)
}

#[test]
fn deliberate_crossings_pass_in_100ms_across_rates_scales_directions_and_dragging() {
    let mut count = 0;
    let mut max = 0;
    eprintln!("trace,seam,hz,ease,raw_lag_us,scale,contact_latency_ms");
    for seam in 0..7 {
        for hz in [125, 1000, 2000, 4000, 8000] {
            for ease in 1..=5 {
                for trace in [Trace::Steady, Trace::Accelerate, Trace::Drag] {
                    for lag in [0, 8000] {
                        for scale in [1, 2] {
                            let (latency, f) =
                                replay(seam, trace, hz, ease, lag, scale, CrossingModel::Intent, false, false);
                            let latency = latency.unwrap_or_else(|| {
                                panic!("held {trace:?} seam={seam} hz={hz} ease={ease} lag={lag} features={f:?}")
                            });
                            assert!(latency <= 100, "late {latency}ms {trace:?}/{seam}/{hz}/{ease}");
                            max = max.max(latency);
                            count += 1;
                            eprintln!("{trace:?},{seam},{hz},{ease},{lag},{scale},{latency}");
                        }
                    }
                }
            }
        }
    }
    eprintln!("WANTED cases={count} max_contact_latency_ms={max}");
}

#[test]
fn drift_grazing_deceleration_corner_overshoot_and_game_flicks_remain_blocked() {
    let mut count = 0;
    for seam in 0..7 {
        for hz in [125, 1000, 2000, 4000, 8000] {
            for ease in 1..=5 {
                for trace in [
                    Trace::Drift,
                    Trace::Graze,
                    Trace::Decelerate,
                    Trace::Corner,
                    Trace::Flick,
                    Trace::Rest,
                ] {
                    let (latency, f) = replay(seam, trace, hz, ease, 0, 2, CrossingModel::Intent, false, false);
                    assert!(
                        latency.is_none(),
                        "unwanted {trace:?} {seam}/{hz}/{ease}: {latency:?} {f:?}"
                    );
                    count += 1;
                }
            }
        }
    }
    eprintln!("UNWANTED cases={count} accidental_passes=0");
}

#[test]
fn automatic_side_to_top_slips_stay_blocked_and_game_lock_overrides_intent() {
    for seam in [1, 2] {
        for trace in [Trace::Steady, Trace::Accelerate, Trace::Flick] {
            assert!(
                replay(seam, trace, 1000, 5, 0, 2, CrossingModel::Intent, true, false)
                    .0
                    .is_none()
            );
        }
    }
    for seam in 0..7 {
        assert!(
            replay(seam, Trace::Steady, 1000, 5, 0, 2, CrossingModel::Intent, false, true)
                .0
                .is_none()
        );
    }
}

#[test]
fn default_is_lighter_than_installed_classic1k_min12_for_identical_slow_screen_reports() {
    for seam in 0..7 {
        assert!(
            replay(seam, Trace::Steady, 1000, 3, 0, 1, CrossingModel::Intent, false, false)
                .0
                .unwrap()
                <= 100
        );
        assert!(
            replay(
                seam,
                Trace::Steady,
                1000,
                3,
                0,
                1,
                CrossingModel::Classic1k,
                false,
                false
            )
            .0
            .is_none()
        );
    }
}

#[test]
fn missing_stale_raw_and_passage_from_waiting_alone_cannot_authorize() {
    use fence_engine::intent_detector::Detector;
    let mut d = Detector::default();
    assert!(!d.judge(1, Direction::Right, 1_000_000, 5));
    for i in 0..40 {
        d.raw(10, 0, 0, 1_000_000 + i * 5000);
    }
    assert!(!d.judge(1, Direction::Right, 1_200_000, 5));
    assert!(!d.judge(1, Direction::Right, 1_240_000, 5));
    assert_eq!(d.features()[7], 1);
}

#[test]
fn f12_sole_upper_exit_keeps_anti_trap_with_intent_at_every_ease_and_gate_state() {
    let screens = [
        Screen {
            stable_id: "main".into(),
            bounds: Rect::new(0, 0, 1000, 1000),
            primary: true,
            excluded: false,
        },
        Screen {
            stable_id: "top".into(),
            bounds: Rect::new(-1000, -1000, 1000, 0),
            primary: false,
            excluded: false,
        },
        Screen {
            stable_id: "side".into(),
            bounds: Rect::new(-1000, 0, -100, 1000),
            primary: false,
            excluded: false,
        },
    ];
    for geometry in [false, true] {
        for gate_open in [false, true] {
            for ease in 1..=5 {
                let policy = Policy {
                    intent: IntentConfig {
                        model: CrossingModel::Intent,
                        crossing_ease: ease,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let mut e = ClassicEngine::new(EngineConfig {
                    intent: policy.intent,
                    monitors: screens.iter().map(|s| s.bounds).collect(),
                    transitions: compile(&policy, &screens, geometry).transitions,
                    ..Default::default()
                });
                let toggles = Toggles {
                    gate_open,
                    side_contain: true,
                };
                e.set_last(-500, 0);
                assert_eq!(
                    e.decide_event(
                        MoveEvent {
                            x: -500,
                            y: -1,
                            time_ms: 1,
                            qpc_us: 1000
                        },
                        toggles
                    )
                    .action,
                    Action::Block
                );
                e.reset();
                let mut crossed = None;
                for ms in 1..=300 {
                    let now = 1_000_000 + ms * 1000;
                    e.raw_motion(0, -1, 0, now);
                    e.set_last(-500, if ms < 200 { 500 } else { 0 });
                    let d = e.decide_event(
                        MoveEvent {
                            x: -500,
                            y: if ms < 200 { 499 } else { -1 },
                            time_ms: (now / 1000) as u32,
                            qpc_us: now,
                        },
                        toggles,
                    );
                    if ms >= 200 && screens[1].bounds.contains(d.x, d.y) {
                        crossed = Some(ms - 200);
                        break;
                    }
                }
                assert!(
                    crossed.is_some_and(|ms| ms <= 100),
                    "{geometry}/{gate_open}/{ease}: {crossed:?}"
                );
            }
        }
    }
}

#[test]
fn renewed_push_escapes_a_rejected_arrival_and_reversal_cannot_reuse_pressure() {
    use fence_engine::intent_detector::Detector;
    for ease in 1..=5 {
        // The 2.0.2 steady path (reason 9) is switched off to test the 2.0.1 renewed path alone.
        let mut d = Detector::default().with_steady_push_ms(-1);
        // No approach: first contact is deliberately rejected.
        d.raw(1, 0, 0, 1_000_000);
        assert!(!d.judge(1, Direction::Right, 1_000_000, ease));
        let mut passed = None;
        for ms in 1..=100 {
            d.raw(1, 0, 0, 1_000_000 + ms * 1000);
            if d.judge(1, Direction::Right, 1_000_000 + ms * 1000, ease) {
                passed = Some(ms);
                break;
            }
        }
        assert!(
            passed.is_some_and(|ms| ms <= 100),
            "ease={ease} features={:?}",
            d.features()
        );
        assert_eq!(d.features()[7], 7);
        let now = 1_000_000 + passed.unwrap() * 1000;
        d.raw(-2, 0, 0, now + 1000);
        assert!(!d.judge(1, Direction::Right, now + 1000, ease));
        d.raw(2, 0, 0, now + 2000);
        assert!(!d.judge(1, Direction::Right, now + 2000, ease));
        assert!(!d.judge(2, Direction::Right, now + 3000, ease));
        d.begin_move(); // previous move judged
        d.begin_move(); // one interior move without a crossing attempt
        assert!(!d.judge(1, Direction::Right, now + 4000, ease));
        assert_eq!(d.features()[5], 0);
    }
}
