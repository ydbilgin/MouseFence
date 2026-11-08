//! Synthetic physical-report corpus; no hook, clock, cursor or recorded user input.
use fence_engine::{
    compile::compile,
    decide::ClassicEngine,
    intent::{Intent, Site},
};
use fence_types::{
    Action, Behavior, BoundaryDefault, BoundaryRule, Control, CrossingModel, Direction, EngineConfig, IntentConfig,
    MoveEvent, Policy, Rect, RuleBehavior, Screen, Toggles,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;
thread_local! {static TRACK:Cell<bool>=const{Cell::new(false)};static BYTES:Cell<usize>=const{Cell::new(0)};}
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = TRACK.try_with(|track| {
            if track.get() {
                let _ = BYTES.try_with(|n| n.set(n.get() + layout.size()));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) }
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, new: usize) -> *mut u8 {
        let _ = TRACK.try_with(|track| {
            if track.get() {
                let _ = BYTES.try_with(|n| n.set(n.get() + new));
            }
        });
        unsafe { System.realloc(p, layout, new) }
    }
}
#[global_allocator]
static ALLOC: Counting = Counting;

const RECTS: [Rect; 4] = [
    Rect::new(-421, -1440, 3019, 0),
    Rect::new(-1920, 0, 0, 1080),
    Rect::new(0, 0, 2560, 1440),
    Rect::new(2560, 0, 4480, 1080),
];
const IDS: [&str; 4] = ["SYN-TOP", "SYN-LEFT", "SYN-MAIN", "SYN-RIGHT"];
const SEAMS: [(usize, usize, Direction, (i32, i32)); 7] = [
    (2, 0, Direction::Up, (1200, 0)),
    (1, 0, Direction::Up, (-200, 0)),
    (3, 0, Direction::Up, (2800, 0)),
    (2, 1, Direction::Left, (0, 500)),
    (1, 2, Direction::Right, (-1, 500)),
    (2, 3, Direction::Right, (2559, 500)),
    (3, 2, Direction::Left, (2560, 500)),
];
const TOGGLES: Toggles = Toggles {
    gate_open: true,
    side_contain: true,
};

fn vector(dir: Direction, p: i32, orth: i32) -> (i32, i32) {
    match dir {
        Direction::Up => (orth, -p),
        Direction::Down => (orth, p),
        Direction::Left => (-p, orth),
        Direction::Right => (p, orth),
    }
}

fn engine(seam: usize, model: CrossingModel, behavior: RuleBehavior) -> ClassicEngine {
    let (from, to, dir, _) = SEAMS[seam];
    let screens: Vec<_> = RECTS
        .iter()
        .enumerate()
        .map(|(i, bounds)| Screen {
            stable_id: IDS[i].into(),
            bounds: *bounds,
            primary: i == 2,
            excluded: false,
        })
        .collect();
    let policy = Policy {
        intent: IntentConfig {
            model,
            ..IntentConfig::default()
        },
        passage_inset_px: 24,
        safety_escapes: false,
        upward_default: BoundaryDefault::Blocked,
        side_default: BoundaryDefault::Blocked,
        downward_default: BoundaryDefault::Free,
        rules: vec![BoundaryRule {
            from_stable_id: IDS[from].into(),
            to_stable_id: IDS[to].into(),
            direction: dir,
            behavior,
            control: Control::Always,
        }],
        ..Policy::default()
    };
    ClassicEngine::new(EngineConfig {
        intent: policy.intent,
        monitors: RECTS.to_vec(),
        transitions: compile(&policy, &screens, false).transitions,
        ..EngineConfig::default()
    })
}

fn report(index: u64, hz: u64, bunched: bool, x: i32, y: i32) -> MoveEvent {
    let group = (hz / 1000).clamp(2, 8);
    let qpc_index = if bunched { index / group * group } else { index };
    MoveEvent {
        x,
        y,
        time_ms: (index * 1000 / hz) as u32,
        qpc_us: qpc_index * 1_000_000 / hz,
    }
}

fn delta(index: u64, speed: u64, hz: u64) -> i32 {
    (((index + 1) * speed / hz) - (index * speed / hz)) as i32
}

fn response(seam: usize, model: CrossingModel, hz: u64, speed: u64, bunched: bool, missing_zero: bool) -> bool {
    let (from, to, dir, edge) = SEAMS[seam];
    let mut e = engine(seam, model, RuleBehavior::Push);
    let center = ((RECTS[from].l + RECTS[from].r) / 2, (RECTS[from].t + RECTS[from].b) / 2);
    let warm = hz * 350 / 1000;
    for index in 0..warm {
        let p = delta(index, speed, hz);
        if missing_zero && p == 0 {
            continue;
        }
        let (dx, dy) = vector(dir, p, 0);
        e.set_last(center.0, center.1);
        e.decide_event(report(index, hz, bunched, center.0 + dx, center.1 + dy), TOGGLES);
    }
    for index in warm..warm + hz * 8 / 1000 {
        let p = delta(index, speed, hz);
        if missing_zero && p == 0 {
            continue;
        }
        let (dx, dy) = vector(dir, p, 0);
        e.set_last(edge.0, edge.1);
        let d = e.decide_event(report(index, hz, bunched, edge.0 + dx, edge.1 + dy), TOGGLES);
        if RECTS[to].contains(d.x, d.y) {
            return true;
        }
    }
    false
}

#[test]
fn j13_classic_1k_constant_speed_matches_classic_1000hz_within_one_step() {
    eprintln!("seam,rate,bunched,classic1000_threshold_px_s,classic1k_threshold_px_s");
    for seam in 0..7 {
        let base = (1000..=20000)
            .step_by(500)
            .find(|speed| response(seam, CrossingModel::Classic, 1000, *speed, false, false))
            .unwrap();
        for hz in [1000, 2000, 4000, 8000] {
            for bunched in [false, true] {
                let threshold = (1000..=20000)
                    .step_by(500)
                    .find(|speed| response(seam, CrossingModel::Classic1k, hz, *speed, bunched, false))
                    .unwrap();
                eprintln!("{seam},{hz},{bunched},{base},{threshold}");
                assert!(
                    base.abs_diff(threshold) <= 500,
                    "seam={seam},hz={hz},bunched={bunched}: {base} vs {threshold}"
                );
                for speed in (1000..=20000).step_by(500) {
                    let actual = response(seam, CrossingModel::Classic1k, hz, speed, bunched, false);
                    assert!(
                        actual == (speed >= base) || base.abs_diff(speed) <= 500,
                        "seam={seam},hz={hz},speed={speed},bunched={bunched}"
                    );
                }
            }
        }
    }
}

#[test]
fn j13_slow_motion_never_crosses_even_with_missing_zero_delta_reports() {
    for seam in 0..7 {
        for hz in [1000, 2000, 4000, 8000] {
            for speed in [1000, 1250, 1500] {
                for bunched in [false, true] {
                    for missing in [false, true] {
                        assert!(
                            !response(seam, CrossingModel::Classic1k, hz, speed, bunched, missing),
                            "seam={seam},hz={hz},speed={speed},bunched={bunched},missing={missing}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn j14_decelerating_arrivals_and_rest_hold_then_a_continued_push_crosses_in_250ms() {
    let mut arrivals = 0;
    for (seam, &(_, to, dir, edge)) in SEAMS.iter().enumerate() {
        for hz in [1000u64, 2000, 4000, 8000] {
            for bunched in [false, true] {
                for speed in (1000..=5500).step_by(500) {
                    for decel_ms in [25u64, 50, 75] {
                        let mut e = engine(seam, CrossingModel::StopThenPush, RuleBehavior::Push);
                        let n = hz * decel_ms / 1000;
                        let mut subpixel = 0.0;
                        let mut crossed = false;
                        for i in 0..n {
                            let velocity = speed as f64 * (1.0 - i as f64 / n as f64);
                            subpixel += velocity / hz as f64;
                            let p = subpixel.floor() as i32;
                            subpixel -= p as f64;
                            let (dx, dy) = vector(dir, p, 0);
                            e.set_last(edge.0, edge.1);
                            let d = e.decide_event(report(i, hz, bunched, edge.0 + dx, edge.1 + dy), TOGGLES);
                            crossed |= RECTS[to].contains(d.x, d.y);
                        }
                        for i in n..n + hz * 2 {
                            e.set_last(edge.0, edge.1);
                            let d = e.decide_event(report(i, hz, bunched, edge.0, edge.1), TOGGLES);
                            crossed |= RECTS[to].contains(d.x, d.y);
                        }
                        assert!(
                            !crossed,
                            "arrival seam={seam},hz={hz},speed={speed},decel={decel_ms},bunched={bunched}"
                        );
                        arrivals += 1;
                        let start = n + hz * 2;
                        let mut crossing = None;
                        for i in start..start + hz / 4 {
                            let (dx, dy) = vector(dir, delta(i, 4000, hz), 0);
                            e.set_last(edge.0, edge.1);
                            let d = e.decide_event(report(i, hz, bunched, edge.0 + dx, edge.1 + dy), TOGGLES);
                            if RECTS[to].contains(d.x, d.y) {
                                crossing = Some((i - start) * 1_000_000 / hz);
                                break;
                            }
                        }
                        assert!(
                            crossing.is_some(),
                            "continued push seam={seam},hz={hz},bunched={bunched}"
                        );
                    }
                }
            }
        }
    }
    eprintln!("StopThenPush: {arrivals} arrival+2000ms-rest cases held; continued pushes all crossed within250ms");
}

#[test]
fn j13_j14_blocked_seams_hold_at_every_speed_down_is_free_and_diagonals_obey_slack() {
    for model in [CrossingModel::Classic1k, CrossingModel::StopThenPush] {
        for (seam, &(_, to, dir, edge)) in SEAMS.iter().enumerate() {
            for hz in [1000u64, 2000, 4000, 8000] {
                for speed in [1000, 1500, 3000, 20000, 50000] {
                    let mut e = engine(seam, model, RuleBehavior::Blocked);
                    for i in 0..hz / 4 {
                        let (dx, dy) = vector(dir, delta(i, speed, hz), 0);
                        e.set_last(edge.0, edge.1);
                        let d = e.decide_event(report(i, hz, true, edge.0 + dx, edge.1 + dy), TOGGLES);
                        assert!(!RECTS[to].contains(d.x, d.y));
                    }
                }
            }
            let mut e = engine(seam, model, RuleBehavior::Push);
            for i in 0..500 {
                let (dx, dy) = vector(dir, 3, 20);
                e.set_last(edge.0, edge.1);
                let d = e.decide_event(report(i, 1000, false, edge.0 + dx, edge.1 + dy), TOGGLES);
                assert!(!RECTS[to].contains(d.x, d.y), "diagonal {model:?}/{seam}");
            }
            let mut e = engine(0, model, RuleBehavior::Push);
            assert!(
                e.config()
                    .transitions
                    .iter()
                    .any(|t| t.from == RECTS[0] && t.direction == Direction::Down && t.behavior == Behavior::Free)
            );
            e.set_last(1200, -1);
            let d = e.decide_event(
                MoveEvent {
                    x: 1200,
                    y: 10,
                    time_ms: 1,
                    qpc_us: 1000,
                },
                TOGGLES,
            );
            assert_eq!(d.action, Action::Pass);
            assert!(RECTS[2].contains(d.x, d.y));
        }
    }
}

#[test]
fn j13_j14_judge_allocates_zero_bytes_over_100000_warmed_reports() {
    for model in [
        CrossingModel::Intent,
        CrossingModel::Classic,
        CrossingModel::Classic1k,
        CrossingModel::StopThenPush,
    ] {
        let mut judge = Intent::new(IntentConfig {
            model,
            ..IntentConfig::default()
        });
        let site = Site {
            id: 1,
            minimum: 3,
            slack: 5,
        };
        for i in 0..3000 {
            let ev = report(i, 8000, true, 0, 0);
            judge.record(1, 0, &ev);
            judge.judge(site, Direction::Right, 1, 0, &ev, true);
        }
        BYTES.with(|n| n.set(0));
        TRACK.with(|track| track.set(true));
        for i in 3000..103000 {
            let ev = report(i, 8000, true, 0, 0);
            judge.raw(1, 0, 0, ev.qpc_us);
            judge.record(1, 0, &ev);
            std::hint::black_box(judge.judge(site, Direction::Right, 1, 0, &ev, true));
        }
        TRACK.with(|track| track.set(false));
        assert_eq!(BYTES.with(Cell::get), 0, "{model:?}");
    }
}

#[test]
fn j13_classic_time_is_ignored_by_the_exact_1_1_1_adapter() {
    let config = engine(0, CrossingModel::Classic, RuleBehavior::Push).config().clone();
    let mut timed = ClassicEngine::exact_1_1_1(config.clone());
    let mut untimed = ClassicEngine::exact_1_1_1(config);
    for i in 0..10000 {
        let x = (i * 37) % 2560;
        let y = (i * 13) % 100 - 50;
        let a = timed.decide_event(
            MoveEvent {
                x,
                y,
                time_ms: (i * 71) as u32,
                qpc_us: (i * 131) as u64,
            },
            TOGGLES,
        );
        let b = untimed.decide_xy(x, y, true, true);
        assert_eq!(a, b);
    }
}

#[test]
fn j14_legacy_side_and_up_sites_accumulate_independently_only_on_crossing_attempts() {
    for up in [false, true] {
        let mut e = ClassicEngine::new(EngineConfig {
            has_top: true,
            has_sides: true,
            barrier_y: 0,
            main: Rect::new(0, 0, 100, 100),
            gates: vec![(0, 100)],
            monitors: vec![
                Rect::new(0, 0, 100, 100),
                Rect::new(-100, 0, 0, 100),
                Rect::new(0, -100, 100, 0),
            ],
            intent: IntentConfig {
                model: CrossingModel::StopThenPush,
                ..IntentConfig::default()
            },
            ..EngineConfig::default()
        });
        let (origin, target) = if up { ((50, 0), (50, -4)) } else { ((0, 50), (-4, 50)) };
        let mut crossed = false;
        for ms in 1..=200 {
            e.set_last(origin.0, origin.1);
            let d = e.decide_event(
                MoveEvent {
                    x: target.0,
                    y: target.1,
                    time_ms: ms,
                    qpc_us: ms as u64 * 1000,
                },
                TOGGLES,
            );
            if d.action == Action::Pass {
                crossed = true;
                break;
            }
        }
        assert!(
            crossed,
            "legacy{} push did not accumulate",
            if up { "up" } else { "side" }
        );
    }
}

#[test]
fn j14_one_pixel_inward_preserves_pressure_but_two_pixels_resets_it() {
    for inward in [1, 2] {
        let mut model = Intent::new(IntentConfig {
            model: CrossingModel::StopThenPush,
            ..IntentConfig::default()
        });
        let site = Site {
            id: 1,
            minimum: 3,
            slack: 5,
        };
        let ev = |ms| MoveEvent {
            time_ms: ms,
            qpc_us: ms as u64 * 1000,
            ..MoveEvent::default()
        };
        model.record(100, 0, &ev(0));
        assert!(!model.judge(site, Direction::Right, 100, 0, &ev(0), false));
        for ms in 75..104 {
            model.record(4, 0, &ev(ms));
            assert!(!model.judge(site, Direction::Right, 4, 0, &ev(ms), true));
        }
        model.record(-inward, 0, &ev(104));
        model.record(4 + inward, 0, &ev(105));
        assert!(!model.judge(site, Direction::Right, 4 + inward, 0, &ev(105), false));
        model.record(4, 0, &ev(106));
        assert_eq!(model.judge(site, Direction::Right, 4, 0, &ev(106), true), inward == 1);
    }
}
