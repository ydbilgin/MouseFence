use fence_engine::{
    compile::{compile, effective_behavior, validate_no_seal},
    decide::ClassicEngine,
};
use fence_types::{
    Action, Behavior, BoundaryRule, Control, CrossingModel, Direction, EngineConfig, IntentConfig, MoveEvent, Policy,
    Rect, RuleBehavior, Screen, Toggles,
};

fn screens() -> Vec<Screen> {
    [
        ("SYN-TOP", Rect::new(-421, -1440, 3019, 0), false),
        ("SYN-LEFT", Rect::new(-1920, 0, 0, 1080), false),
        ("SYN-MAIN", Rect::new(0, 0, 2560, 1440), true),
        ("SYN-RIGHT", Rect::new(2560, 0, 4480, 1080), false),
    ]
    .into_iter()
    .map(|(id, bounds, primary)| Screen {
        stable_id: id.into(),
        bounds,
        primary,
        excluded: false,
    })
    .collect()
}

fn engine(policy: &Policy) -> ClassicEngine {
    let screens = screens();
    ClassicEngine::new(EngineConfig {
        intent: policy.intent,
        monitors: screens.iter().map(|s| s.bounds).collect(),
        transitions: compile(policy, &screens, true).transitions,
        ..EngineConfig::default()
    })
}

fn isolated_screens() -> Vec<Screen> {
    [
        ("SYN-MAIN", Rect::new(0, 0, 1000, 1000), true),
        ("SYN-TOP", Rect::new(-1000, -1000, 1000, 0), false),
        ("SYN-SIDE", Rect::new(-1000, 0, -100, 1000), false),
    ]
    .into_iter()
    .map(|(id, bounds, primary)| Screen {
        stable_id: id.into(),
        bounds,
        primary,
        excluded: false,
    })
    .collect()
}

#[test]
fn j15_sole_exit_safety_push_overrides_default_and_gate_but_never_incidental_contact() {
    let screens = isolated_screens();
    for geometry in [false, true] {
        for gate_open in [false, true] {
            for model in [
                CrossingModel::Classic,
                CrossingModel::Classic1k,
                CrossingModel::StopThenPush,
            ] {
                let policy = Policy {
                    intent: IntentConfig {
                        model,
                        ..IntentConfig::default()
                    },
                    ..Policy::default()
                };
                let compiled = compile(&policy, &screens, geometry);
                let seam = compiled
                    .transitions
                    .iter()
                    .find(|t| t.from_stable_id == "SYN-SIDE" && t.to_stable_id == "SYN-TOP")
                    .unwrap();
                assert!(seam.sole_up_safety_escape);
                assert_eq!(effective_behavior(seam, gate_open, true), Behavior::Push);
                let mut e = ClassicEngine::new(EngineConfig {
                    intent: policy.intent,
                    monitors: screens.iter().map(|s| s.bounds).collect(),
                    transitions: compiled.transitions,
                    ..EngineConfig::default()
                });
                e.set_last(-500, 1);
                // Approach, a one-pixel edge nudge and then rest are not deliberate pressure.
                for (i, y) in [0, -1, 0, 0, 0].into_iter().enumerate() {
                    let d = e.decide_event(
                        MoveEvent {
                            x: -500,
                            y,
                            time_ms: i as u32,
                            qpc_us: i as u64 * 1000,
                        },
                        Toggles {
                            gate_open,
                            side_contain: true,
                        },
                    );
                    assert!(
                        screens[2].bounds.contains(d.x, d.y),
                        "incidental/{geometry}/{gate_open}/{model:?}: {d:?}"
                    );
                }
                let mut crossed = false;
                for i in 1..=250 {
                    let d = e.decide_event(
                        MoveEvent {
                            x: -500,
                            y: -10,
                            time_ms: 5 + i,
                            qpc_us: u64::from(5 + i) * 1000,
                        },
                        Toggles {
                            gate_open,
                            side_contain: true,
                        },
                    );
                    if screens[1].bounds.contains(d.x, d.y) {
                        crossed = true;
                        break;
                    }
                }
                assert!(crossed, "deliberate/{geometry}/{gate_open}/{model:?}");
            }
        }
    }
}

#[test]
fn j15_non_sole_side_exit_keeps_top_blocked_with_safety_and_geometry_on_or_off() {
    for safety_escapes in [false, true] {
        for geometry in [false, true] {
            let policy = Policy {
                safety_escapes,
                ..Policy::default()
            };
            let compiled = compile(&policy, &screens(), geometry);
            for seam in compiled.transitions.iter().filter(|t| {
                ["SYN-LEFT", "SYN-RIGHT"].contains(&t.from_stable_id.as_str()) && t.to_stable_id == "SYN-TOP"
            }) {
                assert!(!seam.sole_up_safety_escape && !seam.safety_demoted);
                for gate in [false, true] {
                    assert_eq!(effective_behavior(seam, gate, true), Behavior::Blocked);
                }
            }
        }
    }
}

#[test]
fn j15_disabling_safety_explicitly_keeps_the_isolated_side_blocked() {
    let policy = Policy {
        safety_escapes: false,
        ..Policy::default()
    };
    for geometry in [false, true] {
        let compiled = compile(&policy, &isolated_screens(), geometry);
        let seam = compiled
            .transitions
            .iter()
            .find(|t| t.from_stable_id == "SYN-SIDE" && t.to_stable_id == "SYN-TOP")
            .unwrap();
        assert!(!seam.sole_up_safety_escape && !seam.safety_demoted);
        for gate in [false, true] {
            assert_eq!(effective_behavior(seam, gate, true), Behavior::Blocked);
        }
    }
}

#[test]
fn j15_default_side_to_top_is_blocked_at_all_rates_speeds_and_corridor_ends() {
    let screens = screens();
    let policy = Policy::default();
    let compiled = compile(&policy, &screens, true);
    for source in ["SYN-LEFT", "SYN-RIGHT"] {
        let seam = compiled
            .transitions
            .iter()
            .find(|t| t.from_stable_id == source && t.to_stable_id == "SYN-TOP")
            .unwrap();
        assert_eq!(seam.behavior, Behavior::Blocked);
        for gate in [false, true] {
            assert_eq!(effective_behavior(seam, gate, true), Behavior::Blocked);
        }
        for hz in [1000u64, 8000] {
            for speed in (1000u64..=50000).step_by(500) {
                for x in [
                    seam.corridor_start,
                    seam.corridor_start + 1,
                    (seam.corridor_start + seam.corridor_end) / 2,
                    seam.corridor_end - 2,
                    seam.corridor_end - 1,
                ] {
                    let mut e = engine(&policy);
                    e.set_last(x, 0);
                    let dy = speed.div_ceil(hz) as i32;
                    let d = e.decide_event(
                        MoveEvent {
                            x,
                            y: -dy,
                            time_ms: 100,
                            qpc_us: 100000,
                        },
                        Toggles {
                            gate_open: true,
                            side_contain: true,
                        },
                    );
                    assert_eq!(d.action, Action::Block, "{source}/{hz}/{speed}/{x}: {d:?}");
                    assert!(seam.from.contains(d.x, d.y));
                }
            }
        }
    }
    assert!(
        validate_no_seal(&policy, &screens).is_empty(),
        "the primary still reaches top and both sides"
    );
    let main_up = compiled
        .transitions
        .iter()
        .find(|t| t.from_stable_id == "SYN-MAIN" && t.to_stable_id == "SYN-TOP")
        .unwrap();
    assert_eq!(main_up.behavior, Behavior::Push);
}

#[test]
fn j15_explicit_side_to_top_rule_wins_and_real_two_hop_main_path_keeps_its_own_rules() {
    for (source, x) in [("SYN-LEFT", -200), ("SYN-RIGHT", 2800)] {
        let policy = Policy {
            intent: IntentConfig {
                model: CrossingModel::Classic,
                ..IntentConfig::default()
            },
            rules: vec![BoundaryRule {
                from_stable_id: source.into(),
                to_stable_id: "SYN-TOP".into(),
                direction: Direction::Up,
                behavior: RuleBehavior::Free,
                control: Control::Always,
            }],
            ..Policy::default()
        };
        let mut e = engine(&policy);
        e.set_last(x, 0);
        let d = e.decide_xy(x, -10, true, true);
        assert_eq!(d.action, Action::Pass);
        assert!(screens()[0].bounds.contains(d.x, d.y));
    }
    // An actual LEFT->MAIN->TOP segment is governed by both actual seams; no graph shortcut.
    let mut policy = Policy::default();
    policy.rules.push(BoundaryRule {
        from_stable_id: "SYN-MAIN".into(),
        to_stable_id: "SYN-TOP".into(),
        direction: Direction::Up,
        behavior: RuleBehavior::Blocked,
        control: Control::Always,
    });
    for (origin, target) in [
        ((-1, 10), (100, -100)),
        ((2560, 10), (2450, -100)),
        ((-1, 1079), (-1, 1081)),
        ((2560, 1079), (2560, 1081)),
    ] {
        let mut e = engine(&policy);
        e.set_last(origin.0, origin.1);
        let d = e.decide_xy(target.0, target.1, true, true);
        assert_eq!(d.action, Action::Block);
        assert!(!screens()[0].bounds.contains(d.x, d.y));
    }
}

#[test]
fn j15_one_move_cannot_bypass_the_side_top_default_via_main() {
    for (origin, target) in [((-1, 30), (50, -20)), ((2560, 30), (2509, -20))] {
        let mut e = engine(&Policy::default());
        e.set_last(origin.0, origin.1);
        let d = e.decide_event(
            MoveEvent {
                x: target.0,
                y: target.1,
                time_ms: 1,
                qpc_us: 1000,
            },
            Toggles {
                gate_open: true,
                side_contain: true,
            },
        );
        assert!(
            !screens()[0].bounds.contains(d.x, d.y),
            "a side-origin event may not land on top through MAIN: {d:?}"
        );
    }
}
