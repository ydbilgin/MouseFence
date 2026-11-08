//! Steady-push escape (2.0.2). Pure detector replays: no hook, cursor or wall clock.
use fence_engine::intent_detector::{Detector, STEADY_PUSH_MS};
use fence_types::Direction;

const CONTACT_US: u64 = 1_200_000;

/// Outward counts per ms (normal, tangent) at `ms` after the trace start; contact is at 200ms.
type Shape<'a> = &'a dyn Fn(f64) -> (f64, f64);

struct Run {
    passed_after_contact_ms: Option<u64>,
    reason: i32,
}

/// Drives one direction. Raw reports arrive every 1/hz, `lag_us` behind the hook event.
fn run(shape: Shape, hz: u64, ease: u8, lag_us: u64, steady_ms: i32, dir: Direction, until_ms: u64) -> Run {
    let mut d = Detector::default().with_steady_push_ms(steady_ms);
    let mut acc = (0.0f64, 0.0f64);
    let mut reason = 0;
    for i in 1..=hz * until_ms / 1000 {
        let now = 1_000_000 + i * 1_000_000 / hz;
        let ms = (now - 1_000_000) as f64 / 1000.0;
        let (p, o) = shape(ms);
        acc.0 += p * 1000.0 / hz as f64;
        acc.1 += o * 1000.0 / hz as f64;
        let (pc, oc) = (acc.0.trunc() as i32, acc.1.trunc() as i32);
        acc.0 -= f64::from(pc);
        acc.1 -= f64::from(oc);
        let (dx, dy) = match dir {
            Direction::Up => (oc, -pc),
            Direction::Down => (oc, pc),
            Direction::Left => (-pc, oc),
            Direction::Right => (pc, oc),
        };
        d.raw(dx, dy, 0, now.saturating_sub(lag_us).max(1_000_001));
        d.begin_move();
        if now >= CONTACT_US {
            if d.judge(7, dir, now, ease) {
                return Run {
                    passed_after_contact_ms: Some((now - CONTACT_US) / 1000),
                    reason: d.features()[7],
                };
            }
            reason = d.features()[7];
        }
    }
    Run {
        passed_after_contact_ms: None,
        reason,
    }
}

const DIRS: [Direction; 4] = [Direction::Up, Direction::Down, Direction::Left, Direction::Right];
const RATES: [u64; 4] = [125, 1000, 4000, 8000];

// A hand that brakes as it reaches the edge, then keeps pushing at an even pace.
// 2.0.1 judged the braking arrival as "uncertain" and the push as too fast (above 12 counts/ms).
fn braked_then_push(speed: f64) -> impl Fn(f64) -> (f64, f64) {
    move |ms| {
        if ms < 160.0 {
            (speed * 3.0, 0.0)
        } else if ms < 200.0 {
            (speed * 0.6, 0.0)
        } else {
            (speed, speed * 0.1)
        }
    }
}

#[test]
fn steady_push_crosses_after_its_hold_time_at_every_ease_rate_direction_and_speed() {
    let mut cases = 0;
    let mut worst = 0;
    for speed in [1.0, 4.0, 8.0, 16.0, 25.0] {
        for ease in 1..=5u8 {
            for hz in RATES {
                for lag in [0, 8000] {
                    for dir in DIRS {
                        let shape = braked_then_push(speed);
                        let r = run(&shape, hz, ease, lag, 0, dir, 600);
                        let need = STEADY_PUSH_MS[usize::from(ease - 1)];
                        let at = r.passed_after_contact_ms.unwrap_or_else(|| {
                            panic!(
                                "held speed={speed} ease={ease} hz={hz} lag={lag} {dir:?} reason={}",
                                r.reason
                            )
                        });
                        assert!(
                            at <= need + 30,
                            "late {at}ms speed={speed} ease={ease} hz={hz} lag={lag}"
                        );
                        worst = worst.max(at);
                        cases += 1;
                    }
                }
            }
        }
    }
    eprintln!("STEADY cases={cases} worst_contact_latency_ms={worst}");
}

#[test]
fn the_braked_push_was_held_by_2_0_1_and_is_released_by_the_steady_path() {
    for speed in [16.0, 25.0] {
        let push = braked_then_push(speed);
        let shape: Shape = &push;
        for ease in 1..=5u8 {
            let old = run(shape, 1000, ease, 0, -1, Direction::Up, 600);
            assert!(old.passed_after_contact_ms.is_none(), "2.0.1 held this ease={ease}");
            let new = run(shape, 1000, ease, 0, 0, Direction::Up, 600);
            assert_eq!(new.reason, 9, "ease={ease}");
        }
    }
}

// Overshoots while aiming near an edge: the hand is braking when it reaches the edge.
fn overshoot(v0: f64, stop_ms: f64, ms: f64) -> (f64, f64) {
    if ms < 200.0 {
        (v0, 0.0)
    } else {
        ((v0 * (1.0 - (ms - 200.0) / stop_ms)).max(0.0), 0.0)
    }
}

#[test]
fn braking_overshoots_stops_tremor_grazes_and_game_swipes_stay_held_at_every_ease() {
    let shapes: [(&str, Shape); 9] = [
        ("overshoot 3/ms stops in 60ms", &|ms| overshoot(3.0, 60.0, ms)),
        ("overshoot 8/ms stops in 80ms", &|ms| overshoot(8.0, 80.0, ms)),
        ("overshoot 16/ms stops in 100ms", &|ms| overshoot(16.0, 100.0, ms)),
        ("overshoot 6/ms stops in 120ms", &|ms| overshoot(6.0, 120.0, ms)),
        ("push 40ms then rest", &|ms| (if ms < 240.0 { 5.0 } else { 0.0 }, 0.0)),
        ("graze along the edge", &|_| (1.0, 3.0)),
        ("game swipe 40/ms", &|_| (40.0, 0.0)),
        ("game swipe 60/ms", &|_| (60.0, 2.0)),
        ("rest at the edge", &|ms| (if ms < 200.0 { 3.0 } else { 0.0 }, 0.0)),
    ];
    // The steady path must never be the thing that lets one of these through. The 2.0.1 quick
    // path (reasons 6/7) already passes a few slow overshoots at 125 Hz; that older behaviour is
    // counted and printed, not hidden, and the new rule must not add to it.
    let mut cases = 0;
    let mut quick_leaks = Vec::new();
    let mut failures = Vec::new();
    for (name, shape) in shapes {
        for ease in 1..=5u8 {
            for hz in RATES {
                for dir in DIRS {
                    let old = run(shape, hz, ease, 0, -1, dir, 600);
                    let new = run(shape, hz, ease, 0, 0, dir, 600);
                    cases += 1;
                    let Some(at) = new.passed_after_contact_ms else {
                        continue;
                    };
                    if new.reason == 9 || old.passed_after_contact_ms != Some(at) {
                        failures.push(format!(
                            "{name} ease={ease} hz={hz} {dir:?}: {at}ms reason={}",
                            new.reason
                        ));
                    } else {
                        quick_leaks.push(format!("{name} ease={ease} hz={hz} {dir:?}"));
                    }
                }
            }
        }
    }
    eprintln!(
        "HELD cases={cases} steady_path_passes={} pre_existing_2_0_1_quick_path_passes={}",
        failures.len(),
        quick_leaks.len()
    );
    for leak in &quick_leaks {
        eprintln!("  2.0.1 quick path already passes: {leak}");
    }
    assert!(failures.is_empty(), "steady path let accidents through: {failures:#?}");
}

#[test]
fn a_two_count_reversal_restarts_the_hold_clock() {
    // Pushes steadily but jerks back every 30ms: never the 40ms of uninterrupted push ease 5 needs.
    let mut d = Detector::default();
    let mut passed = false;
    for ms in 0..400u64 {
        let now = 1_000_000 + ms * 1000;
        let dx = if ms >= 200 && ms % 30 == 29 { -2 } else { 4 };
        d.raw(dx, 0, 0, now);
        d.begin_move();
        if ms >= 200 {
            passed |= d.judge(1, Direction::Right, now, 5) && d.features()[7] == 9;
        }
    }
    assert!(!passed);
}

#[test]
fn explicit_hold_time_and_off_switch_are_obeyed() {
    let push = braked_then_push(16.0);
    let shape: Shape = &push;
    let fast = run(shape, 1000, 3, 0, 40, Direction::Left, 600);
    let slow = run(shape, 1000, 3, 0, 200, Direction::Left, 600);
    let off = run(shape, 1000, 3, 0, -1, Direction::Left, 600);
    let (f, s) = (
        fast.passed_after_contact_ms.unwrap(),
        slow.passed_after_contact_ms.unwrap(),
    );
    assert!((40..=70).contains(&f), "{f}");
    assert!((200..=230).contains(&s), "{s}");
    assert!(off.passed_after_contact_ms.is_none());
    // Values below 30ms are clamped to 30ms, so a stray 1 cannot remove the guard.
    let tiny = run(shape, 1000, 3, 0, 1, Direction::Left, 600);
    assert!(tiny.passed_after_contact_ms.unwrap() >= 30);
}
