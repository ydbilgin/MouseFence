//! Judged differences from 1.1.1 (`J` rows): each fix lands in its own commit after exact parity, is on in the
//! 2.0 engine (`Fixes::ALL`) and off in the exact engine (`Fixes::EXACT_1_1_1`). Each row has a unit test of the
//! fixed behaviour, a test that the exact engine still reproduces 1.1.1, and an ignored reference test that shows
//! the C# engine deciding the 1.1.1 way on the same scenario.

use crate::engine_common::{first_differing_op, first_result_diff, hits_up_to, rust_core};
use crate::reference::graph_diff;
use crate::rig::*;
use fence_engine::compile::{compile_with, validate_no_seal_with};
use fence_engine::decide::ClassicEngine;
use fence_engine::fixes::{Fixes, HIT_J01, HIT_J05, HIT_J06};
use fence_trace::case::WireGraphScreen;
use fence_trace::reference::Reference;
use fence_types::{
    Action, Behavior, BoundaryDefault, CompileResult, Control, Direction, Policy, RuleBehavior, RuntimeTransition,
    SealIssue,
};

/// Replay the cores recorded since the last drain through the reference: the exact engine must agree (compiled
/// graph and every op), the 2.0 engine must differ, in its compiled graph or first at an op, where one of
/// `expected_hits` fired.
fn reference_shows_the_judged_difference(expected_hits: u16) {
    let mut o = Reference::from_env().unwrap_or_else(|e| panic!("{e} (needs MF_REFERENCE_EXE)"));
    let mut differing = 0;
    for rec in take_recorded() {
        let Recorded::Core { case, policy, .. } = rec else {
            continue;
        };
        let answer = o.call_typed(&case).expect("reference answers");
        let exact = rust_core(&case, policy.as_ref(), Fixes::EXACT_1_1_1);
        assert_eq!(graph_diff(&answer, &exact), None, "exact compile vs 1.1.1");
        assert_eq!(
            first_result_diff(&answer["results"], &exact.results),
            None,
            "exact engine vs 1.1.1"
        );
        let fixed = rust_core(&case, policy.as_ref(), LEGACY_PARITY);
        if let Some(d) = graph_diff(&answer, &fixed) {
            assert!(
                fixed.compile_hits & expected_hits != 0,
                "compile difference without the fix: {d}"
            );
            differing += 1;
        } else if let Some(i) = first_differing_op(&answer["results"], &fixed.results) {
            assert!(
                (fixed.compile_hits | hits_up_to(&fixed.op_hits, i)) & expected_hits != 0,
                "difference at op {i} without the fix"
            );
            differing += 1;
        }
    }
    assert!(
        differing > 0,
        "the scenario does not show the judged difference against the reference"
    );
}

// ---------------------------------------------------------------- J01 (cd65f2e)

/// cd65f2e: main with a screen above and one to the left, both seams Blocked, holdback 7.
fn j01_core() -> Core {
    let s = Policy {
        passage_inset_px: 0,
        edge_holdback_px: 7,
        safety_escapes: false,
        upward_default: BoundaryDefault::Blocked,
        side_default: BoundaryDefault::Blocked,
        ..Policy::default()
    };
    let screens = graph_screens(&[
        ("main", 0, 0, 100, 100, true),
        ("top", 0, -100, 100, 0, false),
        ("left", -100, 0, 0, 100, false),
    ]);
    graph_core(&s, &screens, false, 0)
}

type Pt = (i32, i32);

/// (edge target, crossing target, expected hold-back point) for the Up and the Left seam.
const J01_CASES: [(Pt, Pt, Pt); 2] = [((50, 0), (50, -1), (50, 7)), ((0, 50), (-1, 50), (7, 50))];

#[test]
fn j01_reaching_the_source_top_row_or_left_column_passes_without_holdback() {
    for (edge, crossing, held) in J01_CASES {
        let mut c = j01_core();
        c.mv(50, 50, true);
        let r = c.mv(edge.0, edge.1, true);
        assert!(r.pass() && (r.bx, r.by) == edge, "edge {edge:?}: {r:?}");
        assert_ne!(
            c.e.last_fix_hits() & HIT_J01,
            0,
            "the fix reports that it changed the decision"
        );
        // Crossing the seam itself still blocks and holds back by 7 px.
        let x = c.mv(crossing.0, crossing.1, true);
        assert!(x.block() && (x.bx, x.by) == held, "crossing {crossing:?}: {x:?}");
    }
}

#[test]
fn j01_exact_engine_still_holds_back_at_the_source_edge_pixel_like_1_1_1() {
    for (edge, _, held) in J01_CASES {
        let c = j01_core();
        let mut e = ClassicEngine::exact_1_1_1(c.e.config().clone());
        e.decide_xy(50, 50, true, true);
        let d = e.decide_xy(edge.0, edge.1, true, true);
        assert!(d.action == Action::Block && (d.x, d.y) == held, "edge {edge:?}: {d:?}");
        assert_eq!(e.last_fix_hits(), 0);
    }
}

#[test]
#[ignore = "needs MF_REFERENCE_EXE (tools/reference-cs)"]
fn j01_reference_1_1_1_blocks_the_source_edge_pixel_and_2_0_passes_it() {
    take_recorded();
    for (edge, crossing, _) in J01_CASES {
        let mut c = j01_core();
        c.mv(50, 50, true);
        c.mv(edge.0, edge.1, true);
        c.mv(crossing.0, crossing.1, true);
    }
    reference_shows_the_judged_difference(HIT_J01);
}

// ---------------------------------------------------------------- J05 (PLAN F6)

/// The primary `main` with two screens in a row to its right: `a` and `b` are joined by Free seams both ways, and
/// the only way out of the pair (`a` -> `main`, Left) is Blocked. Each of `a` and `b` has a usable exit (to the
/// other), so 1.1.1 finds nothing sealed, yet a cursor that enters the pair can never come back to main.
fn j05_pair() -> (Policy, Vec<WireGraphScreen>) {
    let policy = Policy {
        rules: vec![rule(
            "a",
            "main",
            Direction::Left,
            RuleBehavior::Blocked,
            Control::Always,
        )],
        ..side_free_inset0()
    };
    let screens = graph_screens(&[
        ("main", 0, 0, 100, 100, true),
        ("a", 100, 0, 200, 100, false),
        ("b", 200, 0, 300, 100, false),
    ]);
    (policy, screens)
}

/// `main` | `a` | `b` with every side seam Blocked except `main` -> `a` (Free); main is half as tall as `a`, so
/// `a`'s widest exit is `a` -> `b`. 1.1.1 seals `a` and `b` and safety demotes `a` -> `b` and `b` -> `a`. That
/// joins them into a pair whose only way out (`a` -> `main`) is still Blocked: one round of demotion is not
/// enough.
fn j05_cascade() -> (Policy, Vec<WireGraphScreen>) {
    let policy = Policy {
        side_default: BoundaryDefault::Blocked,
        passage_inset_px: 0,
        rules: vec![rule("main", "a", Direction::Right, RuleBehavior::Free, Control::Always)],
        ..Policy::default()
    };
    let screens = graph_screens(&[
        ("main", 0, 50, 100, 100, true),
        ("a", 100, 0, 200, 100, false),
        ("b", 200, 0, 300, 100, false),
    ]);
    (policy, screens)
}

fn seal(id: &str, to: &str, direction: Direction, width: i32) -> SealIssue {
    SealIssue {
        stable_id: id.to_string(),
        focus_to_stable_id: to.to_string(),
        focus_direction: direction,
        focus_corridor_width: width,
    }
}

fn exit<'a>(c: &'a CompileResult, from: &str, to: &str) -> &'a RuntimeTransition {
    c.transitions
        .iter()
        .find(|t| t.from_stable_id == from && t.to_stable_id == to)
        .unwrap_or_else(|| panic!("no transition {from} -> {to}"))
}

#[test]
fn j05_two_screens_whose_every_way_out_is_blocked_are_reported_as_one_sealed_group() {
    let (policy, screens) = j05_pair();
    assert_eq!(
        validate_rec(&policy, &screens),
        vec![seal("a", "main", Direction::Left, 100)],
        "one issue for the group, named after the owner of its widest way out"
    );
    let (fixed, hits) = validate_no_seal_with(&policy, &screens_of(&screens), Fixes::ALL);
    assert_eq!(fixed.len(), 1);
    assert_eq!(hits, HIT_J05, "the fix reports that it changed the validation");
}

#[test]
fn j05_geometry_safety_demotes_the_widest_way_out_of_a_sealed_group() {
    let (policy, screens) = j05_pair();
    let c = compile_rec(&policy, &screens, true);
    let out = exit(&c, "a", "main");
    assert!(out.safety_demoted && out.behavior == Behavior::Push, "{out:?}");
    assert!(c.has_safety_demotions());
    // The seams inside the group are untouched.
    assert!(!exit(&c, "a", "b").safety_demoted && !exit(&c, "b", "a").safety_demoted);
}

#[test]
fn j05_geometry_safety_repeats_until_no_group_is_sealed() {
    let (policy, screens) = j05_cascade();
    let (c, hits) = compile_with(&policy, &screens_of(&screens), true, Fixes::ALL);
    assert_eq!(
        c.sealed_screens,
        vec![
            seal("a", "b", Direction::Right, 100),
            seal("b", "a", Direction::Left, 100)
        ],
        "the first check is the 1.1.1 one: two single screens"
    );
    for (from, to) in [("a", "b"), ("b", "a"), ("a", "main")] {
        let t = exit(&c, from, to);
        assert!(
            t.safety_demoted && t.behavior == Behavior::Push,
            "{from} -> {to}: {t:?}"
        );
    }
    assert_eq!(hits, HIT_J05, "the second round is the fix");
    assert_eq!(
        compile_rec(&policy, &screens, true),
        c,
        "the production compile is the 2.0 one"
    );
}

#[test]
fn j05_exact_engine_checks_single_screens_and_demotes_once_like_1_1_1() {
    let (policy, screens) = j05_pair();
    let input = screens_of(&screens);
    assert_eq!(validate_no_seal_with(&policy, &input, Fixes::EXACT_1_1_1), (vec![], 0));
    let (c, hits) = compile_with(&policy, &input, true, Fixes::EXACT_1_1_1);
    let out = exit(&c, "a", "main");
    assert!(!out.safety_demoted && out.behavior == Behavior::Blocked, "{out:?}");
    assert_eq!(hits, 0);

    let (policy, screens) = j05_cascade();
    let (c, hits) = compile_with(&policy, &screens_of(&screens), true, Fixes::EXACT_1_1_1);
    let stuck = exit(&c, "a", "main");
    assert!(
        !stuck.safety_demoted && stuck.behavior == Behavior::Blocked,
        "{stuck:?}"
    );
    assert_eq!(c.sealed_screens.len(), 2);
    assert_eq!(hits, 0);
}

/// Confining the cursor to the primary's group is the fence's purpose, so such a group is never sealed: the closed
/// Up gate keeps main and its side neighbour off the top screen (the P207 desk), and an explicit one-way rule
/// keeps the primary and its neighbour off a third screen (the P199 desk). Both compile exactly as in 1.1.1.
#[test]
fn j05_a_group_that_holds_the_primary_is_never_sealed() {
    let mut up_closed = Policy {
        upward_default: BoundaryDefault::Blocked,
        passage_inset_px: 0,
        ..Policy::default()
    };
    up_closed.rules.push(rule(
        "bottom",
        "top",
        Direction::Up,
        RuleBehavior::Free,
        Control::Automatic,
    ));
    let desk = graph_screens(&[
        ("top", 0, -100, 100, 0, false),
        ("bottom", 0, 0, 100, 100, true),
        ("side", 100, 0, 200, 100, false),
    ]);
    let mut one_way = side_free_inset0();
    one_way
        .rules
        .push(rule("b", "c", Direction::Right, RuleBehavior::Blocked, Control::Always));
    let row = graph_screens(&[
        ("a", 0, 0, 100, 100, true),
        ("b", 100, 0, 200, 100, false),
        ("c", 200, 0, 300, 100, false),
    ]);
    for (policy, screens) in [(up_closed, desk), (one_way, row)] {
        let input = screens_of(&screens);
        let (fixed, hits) = compile_with(&policy, &input, true, Fixes::ALL);
        let (exact, _) = compile_with(&policy, &input, true, Fixes::EXACT_1_1_1);
        assert_eq!(hits, 0);
        assert_eq!(fixed, exact);
        assert!(!fixed.has_safety_demotions() && fixed.sealed_screens.is_empty());
    }
}

#[test]
#[ignore = "needs MF_REFERENCE_EXE (tools/reference-cs)"]
fn j05_reference_1_1_1_misses_the_sealed_group_and_2_0_reports_and_opens_it() {
    take_recorded();
    let (policy, screens) = j05_pair();
    validate_rec(&policy, &screens);
    compile_rec(&policy, &screens, true);
    reference_shows_the_judged_difference(HIT_J05);
    let (policy, screens) = j05_cascade();
    compile_rec(&policy, &screens, true);
    reference_shows_the_judged_difference(HIT_J05);
}

// ---------------------------------------------------------------- J06 (PLAN F7, lesson L8)

/// The primary `main`, `side` to its right and `top` above `side` (diagonal to main, so main has no seam to it).
/// main -> side is Free (side default Free); side -> top is the Automatic Up seam to a top screen, so Push while
/// the Up gate is open. The landing set of main -> side holds `top` (reachable over a seam that is not Blocked).
fn j06_core() -> Core {
    let s = Policy {
        side_default: BoundaryDefault::Free,
        upward_default: BoundaryDefault::Automatic,
        passage_inset_px: 0,
        safety_escapes: false,
        ..Policy::default()
    };
    let screens = graph_screens(&[
        ("main", 0, 0, 100, 100, true),
        ("side", 100, 0, 200, 100, false),
        ("top", 100, -100, 200, 0, false),
    ]);
    graph_core(&s, &screens, true, 0)
}

/// From (99, 2) on main to (150, -1) on top: one event crosses the Free side seam near main's top and ends three
/// pixels up, past side's Push Up seam, with 51 px sideways and 3 px up (no push intent for an Up seam: the
/// sideways part exceeds the up part plus the slack). Two hops away.
const J06_FROM: (i32, i32) = (99, 2);
const J06_SHALLOW: (i32, i32) = (150, -1);

#[test]
fn j06_a_shallow_diagonal_over_a_free_seam_no_longer_lands_behind_a_push_seam_two_hops_away() {
    let mut c = j06_core();
    c.mvs(J06_FROM.0, J06_FROM.1, true, false);
    let r = c.mvs(J06_SHALLOW.0, J06_SHALLOW.1, true, false);
    assert!(
        r.block() && (r.bx, r.by) == (150, 0) && !r.on_top,
        "held on side's top row: {r:?}"
    );
    assert_ne!(
        c.e.last_fix_hits() & HIT_J06,
        0,
        "the fix reports that it refused the landing"
    );
    // A deliberate push up from side still crosses its Push seam.
    let up = c.mvs(150, -20, true, false);
    assert!(up.pass() && up.on_top, "{up:?}");
}

#[test]
fn j06_a_move_with_push_intent_still_reaches_the_screen_two_hops_away() {
    // Steep enough for the Up seam (60 px up, 51 px sideways): the landing behind it is authorized, as P209.
    let mut c = j06_core();
    c.mvs(99, 50, true, false);
    let r = c.mvs(150, -10, true, false);
    assert!(r.pass() && (r.bx, r.by) == (150, -10) && r.on_top, "{r:?}");
    assert_eq!(c.e.last_fix_hits(), 0);
    // A Free chain needs no intent (P200), and a closed Up gate still keeps the move off top (P209).
    let mut c = j06_core();
    c.mvs(99, 50, false, false);
    let closed = c.mvs(150, -10, false, false);
    assert!(closed.block() && !closed.on_top, "{closed:?}");
}

#[test]
fn j06_exact_engine_lands_two_hops_away_without_intent_like_1_1_1() {
    let c = j06_core();
    let mut e = ClassicEngine::exact_1_1_1(c.e.config().clone());
    e.decide_xy(J06_FROM.0, J06_FROM.1, true, false);
    let d = e.decide_xy(J06_SHALLOW.0, J06_SHALLOW.1, true, false);
    assert!(
        d.action == Action::Pass && (d.x, d.y) == J06_SHALLOW && e.on_top,
        "1.1.1 lands on top: {d:?}"
    );
    assert_eq!(e.last_fix_hits(), 0);
}

#[test]
#[ignore = "needs MF_REFERENCE_EXE (tools/reference-cs)"]
fn j06_reference_1_1_1_lands_two_hops_away_and_2_0_holds_the_cursor_on_side() {
    take_recorded();
    let mut c = j06_core();
    c.mvs(J06_FROM.0, J06_FROM.1, true, false);
    c.mvs(J06_SHALLOW.0, J06_SHALLOW.1, true, false);
    c.mvs(150, -20, true, false);
    drop(c);
    reference_shows_the_judged_difference(HIT_J06);
}
