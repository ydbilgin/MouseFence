//! The configure pipeline: `MouseGuard.Configure` (B025) and `TrayApplicationContext.Configure` (B026). The whole
//! pipeline is also diffed against the C# code over the seeded corpus (crates/fence-trace/tests/
//! engine_reference_diff.rs, `b026_reference_diff_engine_corpus_seeds_0_to_9999`).

use crate::rig::*;
use fence_engine::decide::ClassicEngine;
use fence_engine::plan::{build_detail, guard_config};
use fence_types::{Decider, Direction, MonitorInfo, Policy, Rect, Toggles};

fn mon(id: &str, r: R4, primary: bool, index: u32) -> MonitorInfo {
    MonitorInfo {
        device: format!("\\\\.\\DISPLAY{index}"),
        stable_id: id.to_string(),
        bounds: rect(r),
        primary,
        index,
    }
}

/// Synthetic desk: top over main, a right side screen and an excluded dummy beyond it (sorted and numbered the
/// way `MonitorInfo.All` returns them: Left, then Top).
fn desk() -> Vec<MonitorInfo> {
    vec![
        mon("SYN-TOP", (0, -900, 1600, 0), false, 1),
        mon("SYN-MAIN", (0, 0, 1600, 900), true, 2),
        mon("SYN-RIGHT", (1600, 0, 2600, 800), false, 3),
        mon("SYN-DUMMY", (2600, 0, 3400, 600), false, 4),
    ]
}

scenarios! {
    fn b025_guard_config_barrier_is_max_top_bottom_gates_need_max_above_min_empty_forbidden_dropped() {
        let tops = [Rect::new(0, -500, 400, -100), Rect::new(400, -300, 1000, 0)];
        let gates = [(24, 376), (500, 500), (700, 650), (424, 976)];
        let forbidden = [Rect::new(2000, 0, 2000, 600), Rect::new(2000, 0, 2600, 600), Rect::new(0, 50, 100, 40)];
        let monitors = [Rect::new(0, 0, 1000, 800)];
        let config = guard_config(&tops, &gates, &monitors, &[], &[], &forbidden, false, monitors[0]);
        assert!(config.has_top && config.barrier_y == 0, "{config:?}");
        assert_eq!(config.gates, vec![(24, 376), (424, 976)]);
        assert_eq!(config.forbidden, vec![Rect::new(2000, 0, 2600, 600)]);
        assert_eq!((config.cross_min_up, config.cross_slack), (3, 5));
        let none = guard_config(&[], &gates, &monitors, &[], &[], &[], false, monitors[0]);
        assert!(!none.has_top && none.barrier_y == 0);

        // Re-baseline: a reconfigure drops the cursor state (OnTop, HaveLast) and the safety gates; the next move
        // re-derives OnTop against the new barrier. The C# replay of this sequence: set fields, then Reset.
        let mut c = new_core();
        c.mv(1280, 0, true);
        let up = c.mv(1280, -15, true);
        assert!(up.on_top);
        c.set(|s| {
            s.barrier_y = -100;
            s.gates = vec![[24, 376]];
        });
        c.reset();
        assert!(!c.on_top() && !c.have_last());
        let below = c.mv(1280, -50, true); // below the new barrier (-100) with no baseline: not on top
        assert!(below.pass() && !below.on_top, "{below:?}");
        c.reset();
        let above = c.mv(1280, -150, true); // above the new barrier with no baseline: on top
        assert!(above.pass() && above.on_top, "{above:?}");

        let mut e = ClassicEngine::new(config.clone());
        e.set_safety_gates(vec![fence_engine::topology::SafetyGate::new(0, 10, 0, 10)]);
        let toggles = Toggles {
            gate_open: true,
            side_contain: true,
        };
        e.decide(fence_types::MoveEvent::untimed(100,100), toggles);
        assert!(e.have_last);
        e.configure(config);
        assert!(!e.have_last && !e.on_top && e.safety_gates().is_empty());
    }

    fn b026_configure_pipeline_splits_excluded_compiles_gates_routes_sides_and_thresholds() {
        let policy = Policy {
            excluded_devices: vec!["syn-dummy".to_string(), "MONITOR\\GONE".to_string()],
            up_cross_min: 9,
            up_cross_slack: 9,
            side_cross_min: 0,
            side_cross_slack: -3,
            deliberate_cross: false,
            descent_routing: true,
            ..Policy::default()
        };
        let all = desk();
        let d = build_detail(&policy, &all);
        let c = &d.config;
        // The excluded dummy is walled (case-insensitive request), the absent id is dormant, nothing is pruned.
        assert_eq!(d.report.excluded, vec!["SYN-DUMMY".to_string()]);
        assert!(!d.report.pruned);
        assert_eq!(c.forbidden, vec![Rect::new(2600, 0, 3400, 600)]);
        assert_eq!(c.monitors, vec![Rect::new(0, -900, 1600, 0), Rect::new(0, 0, 1600, 900), Rect::new(1600, 0, 2600, 800)]);
        // Tops by geometry; the default path primary -> top gets a gate inset by 24 (GateInset).
        assert_eq!(d.tops, vec!["SYN-TOP".to_string()]);
        assert!(c.has_top && c.barrier_y == 0);
        assert_eq!(c.gates, vec![(24, 1576)]);
        // Compiled: main -> top is the Automatic Up to a configured top (Push), so it yields the descent route.
        let up = c
            .transitions
            .iter()
            .find(|t| t.from_stable_id == "SYN-MAIN" && t.to_stable_id == "SYN-TOP" && t.direction == Direction::Up)
            .expect("main -> top");
        assert_eq!(up.behavior, fence_types::Behavior::Push);
        assert!(c.transitions.iter().all(|t| t.to_stable_id != "SYN-DUMMY" && t.from_stable_id != "SYN-DUMMY"));
        assert_eq!(c.descent_routes, vec![(Rect::new(0, -900, 1600, 0), Rect::new(0, 0, 1600, 900))]);
        // Side containment engages because a real screen flanks main on the right.
        assert!(c.has_sides && c.main == Rect::new(0, 0, 1600, 900));
        // Thresholds: side values clamped (1, 0); the up values never reach the core (3, 5).
        assert_eq!((c.side_cross_min, c.side_cross_slack, c.cross_min_up, c.cross_slack), (1, 0, 3, 5));
        assert!(!c.deliberate_cross && c.descent_routing);
        assert!(d.report.isolated.is_empty() && d.report.sealed.is_empty() && !d.report.has_safety_demotions);

        // The same request for the primary is refused (pruned), and the primary is never walled.
        let refuse = Policy {
            excluded_devices: vec!["SYN-MAIN".to_string()],
            ..Policy::default()
        };
        let d = build_detail(&refuse, &all);
        assert!(d.report.excluded.is_empty() && d.report.pruned && d.config.forbidden.is_empty());

        // The plan drives the engine: a deliberate push up through the gate crosses, a dummy entry is walled.
        let (config, _) = fence_engine::plan::build(&policy, &all);
        let mut c = Core::new(Spec {
            has_top: config.has_top,
            barrier_y: config.barrier_y,
            gates: config.gates.iter().map(|g| [g.0, g.1]).collect(),
            monitors: config.monitors.iter().map(|m| [m.l, m.t, m.r, m.b]).collect(),
            forbidden: config.forbidden.iter().map(|m| [m.l, m.t, m.r, m.b]).collect(),
            has_sides: config.has_sides,
            main: [config.main.l, config.main.t, config.main.r, config.main.b],
            deliberate_cross: config.deliberate_cross,
            side_cross_min: config.side_cross_min,
            side_cross_slack: config.side_cross_slack,
            ..Spec::default()
        });
        c.mv(800, 0, true);
        let cross = c.mv(800, -15, true);
        assert!(cross.pass() && cross.on_top, "{cross:?}");
        c.reset();
        c.set_last(2500, 300); // on the right side screen
        let wall = c.mv(2700, 300, true);
        assert!(wall.block() && wall.bx == 2599, "{wall:?}");
    }
}
