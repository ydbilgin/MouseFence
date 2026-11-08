use fence_engine::{compile::compile, decide::ClassicEngine};
use fence_types::{
    Action, Behavior, BoundaryRule, Control, Direction, EngineConfig, Policy, Rect, RuleBehavior, Screen,
};

fn screen(id: &str, bounds: Rect, primary: bool) -> Screen {
    Screen {
        stable_id: id.into(),
        bounds,
        primary,
        excluded: false,
    }
}

fn engine(policy: &Policy, screens: &[Screen]) -> ClassicEngine {
    ClassicEngine::new(EngineConfig {
        monitors: screens.iter().map(|s| s.bounds).collect(),
        transitions: compile(policy, screens, false).transitions,
        ..EngineConfig::default()
    })
}

#[test]
fn j09_full_corridor_edge_contract_all_directions_negative_coordinates() {
    for direction in [Direction::Up, Direction::Down, Direction::Left, Direction::Right] {
        for behavior in [RuleBehavior::Free, RuleBehavior::Push, RuleBehavior::Blocked] {
            for inset in [0, 24, 500] {
                for orth in [-99, -50, -1] {
                    for distance in [1, 10] {
                        let vertical = direction.is_vertical();
                        let source = match direction {
                            Direction::Up => Rect::new(-100, 0, 0, 100),
                            Direction::Left => Rect::new(0, -100, 100, 0),
                            _ => Rect::new(-100, -100, 0, 0),
                        };
                        let target = match direction {
                            Direction::Up | Direction::Left => Rect::new(-100, -100, 0, 0),
                            Direction::Down => Rect::new(-100, 0, 0, 100),
                            Direction::Right => Rect::new(0, -100, 100, 0),
                        };
                        let screens = [screen("SYN-A", source, true), screen("SYN-B", target, false)];
                        let policy = Policy {
                            safety_escapes: false,
                            passage_inset_px: inset,
                            rules: vec![BoundaryRule {
                                from_stable_id: "SYN-A".into(),
                                to_stable_id: "SYN-B".into(),
                                direction,
                                behavior,
                                control: Control::Always,
                            }],
                            ..Policy::default()
                        };
                        let mut e = engine(&policy, &screens);
                        let oa = if matches!(direction, Direction::Up | Direction::Left) {
                            0
                        } else {
                            -1
                        };
                        let ta = oa
                            + if matches!(direction, Direction::Up | Direction::Left) {
                                -distance
                            } else {
                                distance
                            };
                        e.set_last(if vertical { orth } else { oa }, if vertical { oa } else { orth });
                        let d = e.decide_xy(
                            if vertical { orth } else { ta },
                            if vertical { ta } else { orth },
                            true,
                            true,
                        );
                        let cross = behavior == RuleBehavior::Free
                            || behavior == RuleBehavior::Push && distance == 10 && (inset == 0 || orth == -50);
                        assert_eq!(
                            d.action == Action::Pass,
                            cross,
                            "{direction:?}/{behavior:?}/{inset}/{orth}/{distance}: {d:?}"
                        );
                        assert!(if cross { target } else { source }.contains(d.x, d.y));
                        assert!(e.config().transitions.iter().any(|t| t.from == source
                            && t.behavior
                                == match behavior {
                                    RuleBehavior::Free => Behavior::Free,
                                    RuleBehavior::Blocked => Behavior::Blocked,
                                    _ => Behavior::Push,
                                }));
                    }
                }
            }
        }
    }
}

#[test]
fn j11_actual_segment_cannot_bypass_blocked_second_or_third_seam() {
    use fence_types::BoundaryDefault;
    for direction in [Direction::Down, Direction::Up, Direction::Left, Direction::Right] {
        let rotate = |x: i32, y: i32| match direction {
            Direction::Up => (-x, -y),
            Direction::Left => (-y, x),
            Direction::Right => (y, -x),
            _ => (x, y),
        };
        let rect = |l, t, r, b| {
            let (x, y) = rotate(l, t);
            let (u, v) = rotate(r, b);
            Rect::new(x.min(u), y.min(v), x.max(u), y.max(v))
        };
        let screens = [
            screen("SYN-A", rect(0, 0, 100, 100), true),
            screen("SYN-B", rect(100, 0, 200, 100), false),
            screen("SYN-E", rect(200, 0, 300, 100), false),
            screen("SYN-C", rect(100, 100, 200, 200), false),
            screen("SYN-F", rect(200, 100, 300, 200), false),
        ];
        let side = match direction {
            Direction::Up => Direction::Left,
            Direction::Left => Direction::Down,
            Direction::Right => Direction::Up,
            _ => Direction::Right,
        };
        for behavior in [RuleBehavior::Blocked, RuleBehavior::Push, RuleBehavior::Free] {
            let mut policy = Policy {
                upward_default: BoundaryDefault::Free,
                downward_default: BoundaryDefault::Free,
                side_default: BoundaryDefault::Free,
                passage_inset_px: 0,
                edge_holdback_px: 5,
                safety_escapes: false,
                up_cross_min: 200,
                side_cross_min: 200,
                rules: vec![
                    BoundaryRule {
                        from_stable_id: "SYN-B".into(),
                        to_stable_id: "SYN-C".into(),
                        direction,
                        behavior,
                        control: Control::Always,
                    },
                    BoundaryRule {
                        from_stable_id: "SYN-A".into(),
                        to_stable_id: "SYN-B".into(),
                        direction: side,
                        behavior: RuleBehavior::Free,
                        control: Control::Always,
                    },
                ],
                ..Policy::default()
            };
            for toggles in [(true, true), (false, false)] {
                let mut e = engine(&policy, &screens);
                let (x, y) = rotate(90, 10);
                e.set_last(x, y);
                let (x, y) = rotate(150, 150);
                let d = e.decide_xy(x, y, toggles.0, toggles.1);
                assert_eq!(
                    d.action == Action::Pass,
                    behavior == RuleBehavior::Free,
                    "{direction:?}/{behavior:?}: {d:?}"
                );
                assert!(
                    if behavior == RuleBehavior::Free {
                        screens[3].bounds
                    } else {
                        screens[1].bounds
                    }
                    .contains(d.x, d.y),
                    "{direction:?}/{behavior:?}/{toggles:?}: {d:?}"
                );
            }
            if behavior == RuleBehavior::Free {
                policy.rules.push(BoundaryRule {
                    from_stable_id: "SYN-C".into(),
                    to_stable_id: "SYN-F".into(),
                    direction: side,
                    behavior: RuleBehavior::Blocked,
                    control: Control::Always,
                });
                let mut e = engine(&policy, &screens);
                let (x, y) = rotate(90, 10);
                e.set_last(x, y);
                let (x, y) = rotate(250, 150);
                let d = e.decide_xy(x, y, true, true);
                assert_eq!(d.action, Action::Block);
                assert!(screens[3].bounds.contains(d.x, d.y), "{direction:?}: {d:?}");
            }
        }
    }
}

#[test]
fn j11_identity_tracks_durable_instances_and_keeps_legacy_keys_dormant() {
    use fence_engine::{
        fixes::Fixes,
        layout::{KeyedMonitor, canonical_native_identity, disambiguate_keys_with},
    };
    let make = |left, instance: &str| KeyedMonitor {
        key: "SYN-SHARED".into(),
        left,
        top: 0,
        raw_identity_key: instance.into(),
    };
    let keys = |list: &[KeyedMonitor]| disambiguate_keys_with(list, Fixes::ALL, "SYN-SESSION");
    let initial = vec![make(0, "instance-a"), make(100, "instance-b")];
    let before = keys(&initial);
    assert_eq!(before, keys(&[make(100, "instance-a"), make(0, "instance-b")]));
    assert_eq!(before[1], keys(&[make(-100, "instance-b")])[0]);
    assert_ne!(before[0], before[1]);
    assert!(before.iter().all(|s| s != "SYN-SHARED#0" && s != "SYN-SHARED#1"));
    let reverse = keys(&[initial[1].clone(), initial[0].clone()]);
    assert_eq!(before[0], reverse[1]);
    assert_eq!(before[1], reverse[0]);
    assert_eq!(keys(&[KeyedMonitor::new("SYN-UNIQUE", 0, 0)])[0], "SYN-UNIQUE");
    let ambiguous = [make(0, "SYN-SHARED"), make(100, "SYN-SHARED")];
    let first = keys(&ambiguous);
    assert_ne!(first[0], first[1]);
    assert!(first.iter().all(|s| s.contains("#ambiguous:")));
    assert_eq!(first, keys(&ambiguous));
    let next = keys(&[make(100, "SYN-SHARED"), make(200, "SYN-SHARED")]);
    assert!(!first.iter().any(|s| next.contains(s)));
    let restart = disambiguate_keys_with(&ambiguous, Fixes::ALL, "SYN-NEXT-SESSION");
    assert!(!first.iter().any(|s| restart.contains(s)));
    assert_eq!(
        canonical_native_identity(r"\\?\DISPLAY#PANEL#INSTANCE#{class-guid}"),
        r"MONITOR\PANEL\INSTANCE"
    );
}

#[test]
fn j11_void_endpoint_stays_on_the_last_actually_entered_monitor() {
    use fence_types::BoundaryDefault;
    let screens = [
        screen("SYN-A", Rect::new(0, 0, 100, 100), true),
        screen("SYN-B", Rect::new(100, 0, 200, 100), false),
        screen("SYN-E", Rect::new(200, 0, 300, 100), false),
        screen("SYN-C", Rect::new(100, 100, 130, 200), false),
        screen("SYN-F", Rect::new(200, 100, 300, 200), false),
    ];
    let policy = Policy {
        upward_default: BoundaryDefault::Free,
        side_default: BoundaryDefault::Free,
        downward_default: BoundaryDefault::Free,
        passage_inset_px: 0,
        safety_escapes: false,
        rules: vec![BoundaryRule {
            from_stable_id: "SYN-B".into(),
            to_stable_id: "SYN-C".into(),
            direction: Direction::Down,
            behavior: RuleBehavior::Blocked,
            control: Control::Always,
        }],
        ..Policy::default()
    };
    let mut e = engine(&policy, &screens);
    e.set_last(90, 10);
    let d = e.decide_xy(160, 150, true, true);
    assert_eq!((d.action, d.x, d.y), (Action::Block, 160, 99));
}

#[test]
fn j12_bottom_corner_void_slides_cannot_hop_across_push_or_blocked_seams() {
    let screens = [
        screen("SYN-TOP", Rect::new(-421, -1440, 3019, 0), false),
        screen("SYN-LEFT", Rect::new(-1920, 0, 0, 1080), false),
        screen("SYN-MAIN", Rect::new(0, 0, 2560, 1440), true),
        screen("SYN-RIGHT", Rect::new(2560, 0, 4480, 1080), false),
    ];
    let policy = Policy {
        passage_inset_px: 24,
        ..Policy::default()
    };
    for (source, corners) in [(screens[1].bounds, [-1920, -1]), (screens[3].bounds, [2560, 4479])] {
        for corner in corners {
            for dx in -2..=2 {
                for dy in 1..=2 {
                    let mut e = engine(&policy, &screens);
                    e.set_last(corner, 1079);
                    let d = e.decide_xy(corner + dx, 1079 + dy, true, true);
                    assert_eq!(d.action, Action::Block, "corner={corner},delta=({dx},{dy}): {d:?}");
                    assert!(source.contains(d.x, d.y), "corner={corner},delta=({dx},{dy}): {d:?}");
                }
            }
        }
    }
}
