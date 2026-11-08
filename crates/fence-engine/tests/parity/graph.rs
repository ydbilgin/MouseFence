//! The schema-v4 boundary graph: compiler, validator and the compiled engine (tests/Tests.cs:2470-2800).

use crate::rig::*;
use fence_engine::compile::screens_from_monitors;
use fence_engine::plan::{runtime_excluded, validate_settings_no_seal};
use fence_types::{
    Behavior, BoundaryDefault, Control, Direction, MonitorInfo, Policy, Rect, RuleBehavior, RuntimeTransition, TopMode,
};

fn up(from: &str, to: &str) -> impl Fn(&&RuntimeTransition) -> bool {
    let (from, to) = (from.to_string(), to.to_string());
    move |t| t.from_stable_id == from && t.to_stable_id == to && t.direction == Direction::Up
}

fn reversed(screens: &[fence_trace::case::WireGraphScreen]) -> Vec<fence_trace::case::WireGraphScreen> {
    screens.iter().rev().cloned().collect()
}

/// tests/Tests.cs:2505 settings: a/b blocked both ways (Always).
fn sealed_pair() -> (Policy, Vec<fence_trace::case::WireGraphScreen>) {
    let mut s = side_free_inset0();
    s.rules
        .push(rule("a", "b", Direction::Right, RuleBehavior::Blocked, Control::Always));
    s.rules
        .push(rule("b", "a", Direction::Left, RuleBehavior::Blocked, Control::Always));
    (
        s,
        graph_screens(&[("a", 0, 0, 100, 100, true), ("b", 100, 0, 200, 100, false)]),
    )
}

/// tests/Tests.cs:2518: three screens in a row.
fn three_in_a_row() -> Vec<fence_trace::case::WireGraphScreen> {
    graph_screens(&[
        ("a", 0, 0, 100, 100, true),
        ("b", 100, 0, 200, 100, false),
        ("c", 200, 0, 300, 100, false),
    ])
}

/// tests/Tests.cs:2696: the isolated lower screen whose only exit is Up.
fn isolated_lower(safety: bool) -> (Policy, Vec<fence_trace::case::WireGraphScreen>) {
    let s = Policy {
        passage_inset_px: 0,
        safety_escapes: safety,
        ..Policy::default()
    };
    (
        s,
        graph_screens(&[
            ("top", 0, -100, 100, 0, false),
            ("isolated-lower", 0, 0, 100, 100, true),
        ]),
    )
}

/// tests/Tests.cs:2788: the imported settings (SettingsVersion 4, SafetyEscapes off, the primary requested as
/// excluded, both seams blocked).
fn imported() -> Policy {
    Policy {
        safety_escapes: false,
        excluded_devices: vec!["primary".to_string()],
        rules: vec![
            rule(
                "primary",
                "side",
                Direction::Right,
                RuleBehavior::Blocked,
                Control::Always,
            ),
            rule(
                "side",
                "primary",
                Direction::Left,
                RuleBehavior::Blocked,
                Control::Always,
            ),
        ],
        ..Policy::default()
    }
}

scenarios! {
    fn p196_m1_compiler_third_screen_occlusion_splits_a_corridor_and_sort_is_input_order_independent() {
        let s = side_free_inset0();
        let screens = graph_screens(&[
            ("bottom", 0, 200, 300, 300, true),
            ("top", 0, 0, 300, 100, false),
            ("occluder", 100, 100, 200, 200, false),
        ]);
        let ca = compile_rec(&s, &screens, true);
        let cb = compile_rec(&s, &reversed(&screens), true);
        let a: Vec<(i32, i32)> =
            ca.transitions.iter().filter(up("bottom", "top")).map(|t| (t.corridor_start, t.corridor_end)).collect();
        let b: Vec<(i32, i32)> =
            cb.transitions.iter().filter(up("bottom", "top")).map(|t| (t.corridor_start, t.corridor_end)).collect();
        assert!(a == vec![(0, 100), (200, 300)] && a == b, "a={a:?} b={b:?}");
    }

    fn p197_m1_validator_an_authored_zero_exit_configuration_is_rejected_deterministically() {
        let (s, screens) = sealed_pair();
        let issues = validate_rec(&s, &screens);
        assert!(issues.len() == 2 && issues[0].focus_corridor_width == 100, "issues={issues:?}");
    }

    fn p198_m1_compiler_geometry_seal_backstop_demotes_widest_exits_to_push_and_flags_them() {
        let (s, screens) = sealed_pair();
        let healed = compile_rec(&s, &screens, true);
        let demoted: Vec<&RuntimeTransition> = healed.transitions.iter().filter(|t| t.safety_demoted).collect();
        assert!(
            healed.has_safety_demotions() && demoted.len() == 2 && demoted.iter().all(|t| t.behavior == Behavior::Push),
            "{demoted:?}"
        );
    }

    fn p199_m1_engine_authorized_landing_stops_a_fast_multi_seam_teleport_before_a_blocked_second_transition() {
        let mut s = side_free_inset0();
        s.rules.push(rule("b", "c", Direction::Right, RuleBehavior::Blocked, Control::Always));
        let mut c = graph_core(&s, &three_in_a_row(), false, 0);
        c.mvs(90, 50, true, false);
        let r = c.mvs(250, 50, true, false); // one event crosses two seams
        assert!(r.block() && r.bx == 199 && r.by == 50, "{r:?}");
    }

    fn p200_m1_engine_fast_multi_seam_move_reaches_a_transitively_authorized_free_monitor() {
        let s = side_free_inset0();
        let mut c = graph_core(&s, &three_in_a_row(), false, 0);
        c.mvs(90, 50, true, false);
        let r = c.mvs(250, 50, true, false);
        assert!(r.pass() && r.bx == 250, "{r:?}");
    }

    fn p201_m1_engine_compiled_free_crossing_into_void_is_corrected_only_onto_an_authorized_real_screen() {
        let s = side_free_inset0();
        let screens = graph_screens(&[("a", 0, 0, 100, 100, true), ("b", 100, 0, 200, 100, false)]);
        let mut c = graph_core(&s, &screens, false, 0);
        c.mvs(90, 50, true, false);
        let r = c.mvs(250, -40, true, false); // crosses the seam, then aims at a desktop void
        assert!(r.block() && screens.iter().any(|m| in_rect(bounds(m), r.bx, r.by)), "{r:?}");
    }

    fn p202_m1_engine_failed_blocked_push_holds_back_on_source_and_never_lands_on_the_blocked_monitor() {
        let mut s = Policy {
            edge_holdback_px: 5,
            safety_escapes: false,
            ..side_free_inset0()
        };
        s.rules.push(rule("a", "dummy", Direction::Right, RuleBehavior::Blocked, Control::Always));
        let screens = graph_screens(&[("a", 0, 0, 100, 100, true), ("dummy", 100, 0, 200, 100, false)]);
        let mut c = graph_core(&s, &screens, false, 0);
        c.mvs(90, 22, true, true);
        let r = c.mvs(180, 78, true, true); // the eased target changes height substantially
        assert!(r.block() && r.bx == 94 && r.by == 78 && !in_rect(bounds(&screens[1]), r.bx, r.by), "{r:?}");
    }

    fn p203_m1_compiler_excluded_screens_are_absent_from_transitions_and_every_allowed_landing_set() {
        let s = side_free_inset0();
        let mut screens = graph_screens(&[("a", 0, 0, 100, 100, true), ("dummy", 100, 0, 200, 100, false)]);
        screens[1].excluded = true;
        let compiled = compile_rec(&s, &screens, true);
        let dummy = rect(bounds(&screens[1]));
        assert!(
            compiled.transitions.is_empty()
                && compiled.transitions.iter().all(|t| t.allowed_landings.iter().flatten().all(|r| *r != dummy)),
            "{:?}",
            compiled.transitions
        );
    }

    fn p204_m1_engine_down_blocked_runs_before_on_top_legacy_descent_and_state_follows_the_source_landing() {
        let mut s = Policy {
            downward_default: BoundaryDefault::Free,
            passage_inset_px: 0,
            safety_escapes: false,
            ..Policy::default()
        };
        s.rules.push(rule("top", "bottom", Direction::Down, RuleBehavior::Blocked, Control::Always));
        let screens = graph_screens(&[("top", 0, -100, 100, 0, false), ("bottom", 0, 0, 100, 100, true)]);
        let mut c = graph_core(&s, &screens, true, 0);
        c.mv(50, -10, true);
        let r = c.mv(50, 20, true);
        assert!(r.block() && r.by == -1 && r.on_top, "{r:?}");
    }

    fn p205_m1_engine_compiled_down_free_applies_global_descent_guidance_after_transition_resolution() {
        let s = Policy {
            downward_default: BoundaryDefault::Free,
            passage_inset_px: 0,
            ..Policy::default()
        };
        let screens = graph_screens(&[
            ("top", 0, -100, 200, 0, false),
            ("landing", 0, 0, 100, 100, true),
            ("wrong", 100, 0, 200, 100, false),
        ]);
        let mut c = graph_core(&s, &screens, true, 0);
        let route = [screens[0].rect, screens[1].rect];
        c.set(|w| {
            w.descent_routing = true;
            w.descent_routes.push(route);
        });
        c.mv(150, -10, true);
        let r = c.mv(150, 10, true);
        assert!(r.block() && r.bx == 99 && r.by == 10 && !r.on_top, "{r:?}");
    }

    fn p206_m1_descent_routing_fast_diagonal_uses_the_local_edge_intersection_after_endpoint_leaves_the_top_x_span() {
        let s = Policy {
            downward_default: BoundaryDefault::Free,
            passage_inset_px: 0,
            ..Policy::default()
        };
        let screens = graph_screens(&[
            ("top", 0, -100, 200, 0, false),
            ("landing", 0, 0, 100, 100, true),
            ("wrong", 100, 0, 300, 100, false),
        ]);
        let mut c = graph_core(&s, &screens, true, 0);
        let route = [screens[0].rect, screens[1].rect];
        c.set(|w| {
            w.descent_routing = true;
            w.descent_routes.push(route);
        });
        c.mv(50, -50, true);
        let r = c.mv(250, 50, true);
        assert!(r.block() && r.bx == 99 && r.by == 50 && !r.on_top, "{r:?}");
    }

    fn p207_m1_engine_automatic_follows_the_live_up_switch_while_always_ignores_it() {
        let mut automatic = Policy {
            upward_default: BoundaryDefault::Blocked,
            passage_inset_px: 0,
            ..Policy::default()
        };
        automatic.rules.push(rule("bottom", "top", Direction::Up, RuleBehavior::Free, Control::Automatic));
        // A side neighbour keeps bottom non-isolated, so this stays a pure Automatic-vs-Always toggle test.
        let screens = graph_screens(&[
            ("top", 0, -100, 100, 0, false),
            ("bottom", 0, 0, 100, 100, true),
            ("side", 100, 0, 200, 100, false),
        ]);
        let mut c = graph_core(&automatic, &screens, true, 0);
        c.mv(50, 10, false);
        let blocked = c.mv(50, -10, false);
        automatic.rules[0].control = Control::Always;
        let mut c = graph_core(&automatic, &screens, true, 0);
        c.mv(50, 10, false);
        let allowed = c.mv(50, -10, false);
        assert!(blocked.block() && allowed.pass(), "automatic={blocked:?} always={allowed:?}");
    }

    fn p208_m1_engine_automatic_blocked_side_rule_becomes_end_to_end_free_only_while_side_protection_is_off() {
        let s = Policy {
            side_default: BoundaryDefault::Blocked,
            passage_inset_px: 0,
            safety_escapes: false,
            ..Policy::default()
        };
        let screens = graph_screens(&[("a", 0, 0, 100, 100, true), ("b", 100, 0, 200, 100, false)]);
        let mut c = graph_core(&s, &screens, false, 0);
        c.mvs(50, 50, true, false);
        let off = c.mvs(150, 50, true, false);
        let mut c = graph_core(&s, &screens, false, 0);
        c.mvs(50, 50, true, true);
        let on = c.mvs(150, 50, true, true);
        assert!(
            off.pass() && off.bx == 150 && on.block() && in_rect(bounds(&screens[0]), on.bx, on.by),
            "off={off:?} on={on:?}"
        );
    }

    fn p209_m1_engine_transitive_authorized_landing_honors_the_live_up_toggle_on_downstream_edges() {
        let s = Policy {
            side_default: BoundaryDefault::Free,
            upward_default: BoundaryDefault::Automatic,
            passage_inset_px: 0,
            safety_escapes: false,
            ..Policy::default()
        };
        let screens = graph_screens(&[
            ("a", 0, 0, 100, 100, true),
            ("b", 100, 0, 200, 100, false),
            ("top", 100, -100, 200, 0, false),
        ]);
        let mut c = graph_core(&s, &screens, true, 0);
        c.mvs(50, 50, false, false);
        let closed = c.mvs(150, -50, false, false);
        let mut c = graph_core(&s, &screens, true, 0);
        c.mvs(50, 50, true, false);
        let open = c.mvs(150, -50, true, false);
        assert!(
            closed.block()
                && in_rect(bounds(&screens[1]), closed.bx, closed.by)
                && open.pass()
                && in_rect(bounds(&screens[2]), open.bx, open.by),
            "closed={closed:?} open={open:?}"
        );
    }

    fn p210_m1_identity_dormancy_absent_ids_stay_inert_persist_and_reactivate_only_on_exact_identity_return() {
        // Engine half of the row: the policy keeps the dormant rule (it is data, never pruned by the compiler),
        // the rule is inert while its ids are absent and active again when exactly those ids return. The JSON
        // round trip of the settings file is fence-config's (Track B) settings tests.
        let mut dormant = Policy {
            safety_escapes: false,
            ..Policy::default()
        };
        dormant.rules.push(rule("dock-old-a", "dock-old-b", Direction::Right, RuleBehavior::Blocked, Control::Always));
        let live_other = graph_screens(&[("live-a", 0, 0, 100, 100, true), ("live-b", 100, 0, 200, 100, false)]);
        let inert = compile_rec(&dormant, &live_other, true).transitions.iter().all(|t| !t.explicit_rule);
        let restored = dormant.clone();
        let old_returns = graph_screens(&[("dock-old-a", 0, 0, 100, 100, true), ("dock-old-b", 100, 0, 200, 100, false)]);
        let restored_rule = compile_rec(&restored, &old_returns, true)
            .transitions
            .iter()
            .any(|t| t.explicit_rule && t.behavior == Behavior::Blocked);
        assert!(inert && restored_rule && restored.rules.len() == 1, "inert={inert} restored={restored_rule}");
    }

    fn p211_m1_gap_semantics_void_attempt_stays_source_authorized_os_landing_on_to_activates_the_saved_gap_rule() {
        let mut s = Policy {
            seam_tolerance_px: 2,
            safety_escapes: false,
            ..side_free_inset0()
        };
        s.rules.push(rule("a", "b", Direction::Right, RuleBehavior::Blocked, Control::Always));
        let screens = graph_screens(&[("a", 0, 0, 100, 100, true), ("b", 200, 0, 300, 100, false)]);
        let mut c = graph_core(&s, &screens, false, 0);
        c.mvs(90, 50, true, false);
        let into_gap = c.mvs(150, 50, true, false);
        let mut c = graph_core(&s, &screens, false, 0);
        c.mvs(90, 50, true, false);
        let established = c.mvs(220, 50, true, false);
        assert!(
            into_gap.block() && into_gap.bx == 99 && established.block() && established.bx == 99,
            "gap={into_gap:?} established={established:?}"
        );
    }

    fn p212_m1_simultaneous_intersection_tie_true_corner_fires_no_edge_authorized_repair_is_deterministic() {
        let s = Policy {
            side_default: BoundaryDefault::Blocked,
            upward_default: BoundaryDefault::Blocked,
            passage_inset_px: 0,
            safety_escapes: false,
            ..Policy::default()
        };
        let screens = graph_screens(&[
            ("a", 0, 0, 100, 100, true),
            ("up", 0, -100, 100, 0, false),
            ("right", 100, 0, 200, 100, false),
            ("diagonal", 100, -100, 200, 0, false),
        ]);
        let mut c1 = graph_core(&s, &screens, false, 0);
        c1.mvs(50, 50, true, true);
        let r1 = c1.mvs(150, -50, true, true);
        let mut c2 = graph_core(&s, &reversed(&screens), false, 0);
        c2.mvs(50, 50, true, true);
        let r2 = c2.mvs(150, -50, true, true);
        let no_diagonal_edge = compile_rec(&s, &screens, true)
            .transitions
            .iter()
            .all(|t| t.to_stable_id != "diagonal" || t.from_stable_id != "a");
        assert!(
            no_diagonal_edge
                && r1.block()
                && r2.block()
                && r1.bx == r2.bx
                && r1.by == r2.by
                && in_rect(bounds(&screens[0]), r1.bx, r1.by),
            "first={r1:?} second={r2:?}"
        );
    }

    fn p213_m1_schema_a_newly_empty_v4_manual_upper_group_is_literal_none_automatic_up_resolves_free() {
        let s = Policy {
            mode: TopMode::Manual,
            upward_default: BoundaryDefault::Automatic,
            passage_inset_px: 0,
            ..Policy::default()
        };
        let screens = graph_screens(&[("top", 0, -100, 100, 0, false), ("main", 0, 0, 100, 100, true)]);
        let compiled = compile_rec(&s, &screens, true);
        let ups: Vec<&RuntimeTransition> =
            compiled.transitions.iter().filter(|t| t.from_stable_id == "main" && t.direction == Direction::Up).collect();
        assert!(ups.len() == 1 && ups[0].behavior == Behavior::Free, "{ups:?}");
    }

    fn p214_m1_anti_trap_sole_up_exit_remains_a_deliberate_push_while_the_normal_up_gate_is_closed() {
        let (s, screens) = isolated_lower(true);
        let compiled = compile_rec(&s, &screens, true);
        let exits: Vec<&RuntimeTransition> = compiled.transitions.iter().filter(up("isolated-lower", "top")).collect();
        let mut c = graph_core(&s, &screens, true, 0);
        c.mv(50, 50, false);
        let escaped = c.mv(50, -10, false);
        assert!(
            exits.len() == 1
                && exits[0].sole_up_safety_escape
                && escaped.pass()
                && escaped.by == -10
                && in_rect(bounds(&screens[0]), escaped.bx, escaped.by),
            "{exits:?} {escaped:?}"
        );
    }

    fn p215_m1_anti_trap_safety_escapes_off_permits_the_expert_runtime_seal_and_validator_remains_total() {
        let (s, screens) = isolated_lower(false);
        let mut c = graph_core(&s, &screens, true, 0);
        c.mv(50, 50, false);
        let sealed_by_expert = c.mv(50, -10, false);
        // "Remains total": the validator returns (it cannot throw in Rust; a panic would fail this test).
        let validation = validate_rec(&s, &screens);
        assert!(sealed_by_expert.block() && sealed_by_expert.by == 0, "{sealed_by_expert:?} issues={validation:?}");
    }

    fn p216_m1_anti_trap_geometry_demoted_up_exit_remains_push_while_the_ordinary_up_toggle_is_closed() {
        let s = Policy {
            upward_default: BoundaryDefault::Blocked,
            side_default: BoundaryDefault::Blocked,
            passage_inset_px: 0,
            safety_escapes: true,
            ..Policy::default()
        };
        let screens = graph_screens(&[
            ("top", 0, -100, 200, 0, false),
            ("lower", 0, 0, 200, 100, true),
            ("side", 200, 0, 250, 100, false),
        ]);
        let compiled = compile_rec(&s, &screens, true);
        let exits: Vec<&RuntimeTransition> = compiled.transitions.iter().filter(up("lower", "top")).collect();
        let mut c = graph_core(&s, &screens, true, 0);
        c.mv(100, 50, false);
        let escaped = c.mv(100, -20, false);
        assert!(
            exits.len() == 1
                && exits[0].safety_demoted
                && !exits[0].sole_up_safety_escape
                && escaped.pass()
                && in_rect(bounds(&screens[0]), escaped.bx, escaped.by),
            "{exits:?} {escaped:?}"
        );
    }

    fn p217_m1_anti_trap_live_up_closure_cannot_seal_an_otherwise_free_automatic_exit() {
        let mut s = Policy {
            upward_default: BoundaryDefault::Blocked,
            side_default: BoundaryDefault::Blocked,
            passage_inset_px: 0,
            safety_escapes: true,
            ..Policy::default()
        };
        s.rules.push(rule("lower", "top", Direction::Up, RuleBehavior::Free, Control::Automatic));
        s.rules.push(rule("lower", "side", Direction::Right, RuleBehavior::Blocked, Control::Always));
        let screens = graph_screens(&[
            ("top", 0, -100, 200, 0, false),
            ("lower", 0, 0, 200, 100, true),
            ("side", 200, 0, 250, 100, false),
        ]);
        let compiled = compile_rec(&s, &screens, true);
        let exits: Vec<&RuntimeTransition> = compiled.transitions.iter().filter(up("lower", "top")).collect();
        let mut c = graph_core(&s, &screens, true, 0);
        c.mv(100, 50, false);
        let escaped = c.mv(100, -20, false);
        assert!(
            exits.len() == 1
                && exits[0].safety_demoted
                && !exits[0].sole_up_safety_escape
                && escaped.pass()
                && in_rect(bounds(&screens[0]), escaped.bx, escaped.by),
            "{exits:?} {escaped:?}"
        );
    }

    fn p219_m1_validator_imported_primary_exclusion_is_sanitized_identically_to_runtime_topology() {
        let imported = imported();
        let monitors = vec![
            MonitorInfo {
                device: String::new(),
                stable_id: "primary".into(),
                bounds: Rect::new(0, 0, 100, 100),
                primary: true,
                index: 0,
            },
            MonitorInfo {
                device: String::new(),
                stable_id: "side".into(),
                bounds: Rect::new(100, 0, 200, 100),
                primary: false,
                index: 0,
            },
        ];
        let (runtime_ex, _) = runtime_excluded(&imported, &monitors);
        let runtime_screens = screens_from_monitors(&monitors, &runtime_ex);
        // The runtime half goes through the recorded compiler too (C# BoundaryGraph.ValidateNoSeal(FromMonitors)).
        let wire: Vec<fence_trace::case::WireGraphScreen> = runtime_screens
            .iter()
            .map(|s| fence_trace::case::WireGraphScreen {
                id: s.stable_id.clone(),
                rect: [s.bounds.l, s.bounds.t, s.bounds.r, s.bounds.b],
                primary: s.primary,
                excluded: s.excluded,
            })
            .collect();
        let runtime_issues = validate_rec(&imported, &wire);
        let validator_issues = validate_settings_no_seal(&imported, &monitors);
        let ids = |v: &[fence_types::SealIssue]| v.iter().map(|i| i.stable_id.clone()).collect::<Vec<_>>();
        assert!(
            runtime_ex.is_empty() && ids(&validator_issues) == ids(&runtime_issues) && validator_issues.len() == 2,
            "excluded={runtime_ex:?} validator={:?} runtime={:?}",
            ids(&validator_issues),
            ids(&runtime_issues)
        );
    }
}
