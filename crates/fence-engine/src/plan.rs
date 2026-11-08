//! Plan builder: the pure part of C# `TrayApplicationContext.Configure` plus `MouseGuard.Configure`
//! (Ui/TrayApplicationContext.cs:134-247, Platform/MouseGuard.cs:84-110). Track A.
//!
//! Input: the policy (already normalized, as C# `Configure` starts with `_settings.Normalize()`; fence-config
//! owns normalization) and the live monitors already sorted and numbered (C# `MonitorInfo.All`, which runs
//! `SortAndNumber` then `DisambiguateStableIds`). Output: the engine config and the report the tray turns into
//! balloons (pruned exclusion, geometry-seal demotion, isolated screens).
//!
//! Details that parity depends on (from the C# code, kept):
//! - gates use `GATE_INSET = 24`: `inset = min(24, max(0, (hi - lo) / 4))`; MouseGuard keeps only `max > min`;
//! - `barrier_y` = max bottom of the top screens; `has_top` = any top screen;
//! - `cross_min_up`/`cross_slack` stay 3/5 (never read from settings in 1.1.1);
//! - `side_cross_min = max(1, s)`, `side_cross_slack = max(0, s)`;
//! - descent routes come from compiled non-Blocked Up transitions (distinct pairs, compile order);
//! - the custom up-links (`Settings.UpLinks`) are the Up rules whose behaviour is `OverallFeel`;
//! - id matching is exact everywhere except the requested exclusions (case-insensitive) and the compiler.

use crate::compile::{compile_with, screens_from_monitors, screens_from_monitors_ignore_case, validate_no_seal_with};
use crate::fixes::Fixes;
// C# GuardCore.SanitizeExcluded: one implementation, Track B's (join 2026-10-13; it was a private copy here).
use crate::layout::sanitize_excluded;
use crate::routes::derive_routes;
use crate::topology::{adjacent, anti_trap, is_top};
use fence_types::{
    Behavior, BoundaryDefault, Direction, EngineConfig, MonitorInfo, PlanReport, Policy, Rect, RuleBehavior, SealIssue,
    TopMode,
};

/// C# `TrayApplicationContext.GateInset`.
pub const GATE_INSET: i32 = 24;

/// Everything one configure pass produces, including the intermediate values the reference reports (tops, the
/// compiled transitions live in `config.transitions`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanDetail {
    pub config: EngineConfig,
    pub report: PlanReport,
    /// Stable ids of the top monitors, in monitor order.
    pub tops: Vec<String>,
}

/// Build the engine config and the tray report for one (re)configure.
pub fn build(policy: &Policy, monitors: &[MonitorInfo]) -> (EngineConfig, PlanReport) {
    let d = build_detail(policy, monitors);
    (d.config, d.report)
}

/// [`build`] with the intermediate values (for the reference diff and the tray's labels).
pub fn build_detail(policy: &Policy, all_monitors: &[MonitorInfo]) -> PlanDetail {
    build_detail_with(policy, all_monitors, Fixes::ALL).0
}

/// [`build_detail`] with the chosen judged fixes (`Fixes::EXACT_1_1_1` is 1.1.1 exactly), plus the
/// `fixes::HIT_*` bits of the fixes that changed the compile behind it.
pub fn build_detail_with(policy: &Policy, all_monitors: &[MonitorInfo], fixes: Fixes) -> (PlanDetail, u16) {
    // Wall-off: split the live monitors into the excluded set (forbidden rects) and the topology set. The
    // primary is never walled and one usable monitor always remains (GuardCore.SanitizeExcluded).
    let primary_stable = all_monitors
        .iter()
        .find(|m| m.primary)
        .or_else(|| all_monitors.first())
        .map(|m| m.stable_id.as_str());
    let present: Vec<String> = all_monitors.iter().map(|m| m.stable_id.clone()).collect();
    let (excluded_stable, pruned) = sanitize_excluded(&present, primary_stable, &policy.excluded_devices);
    let is_excluded = |m: &MonitorInfo| excluded_stable.iter().any(|e| e == &m.stable_id);
    let excluded_monitors: Vec<&MonitorInfo> = all_monitors.iter().filter(|m| is_excluded(m)).collect();
    let monitors: Vec<&MonitorInfo> = all_monitors.iter().filter(|m| !is_excluded(m)).collect();

    let tops: Vec<&MonitorInfo> = match policy.mode {
        TopMode::Manual => monitors
            .iter()
            .copied()
            .filter(|m| policy.manual_monitors.iter().any(|k| k == &m.stable_id))
            .collect(),
        TopMode::AutoTop => monitors.iter().copied().filter(|m| is_top(m.bounds)).collect(),
    };
    let in_top_set = |id: &str| tops.iter().any(|t| t.stable_id == id);
    let primary = monitors
        .iter()
        .copied()
        .find(|m| m.primary)
        .or_else(|| monitors.first().copied());

    // Default paths are primary -> every top; custom paths (Up default Blocked) are the explicit up-links.
    let links: Vec<(String, String)> = if policy.upward_default == BoundaryDefault::Blocked {
        up_links(policy)
    } else {
        match primary {
            Some(p) => tops
                .iter()
                .map(|t| (p.stable_id.clone(), t.stable_id.clone()))
                .collect(),
            None => Vec::new(),
        }
    };

    let by_stable = |id: &str| monitors.iter().copied().find(|m| m.stable_id == id);
    let mut gates: Vec<(i32, i32)> = Vec::new();
    for (from_id, to_id) in &links {
        if !in_top_set(to_id) {
            continue; // target must be a top monitor
        }
        let Some(from) = by_stable(from_id) else { continue };
        let Some(to) = by_stable(to_id) else { continue };
        let lo = from.bounds.l.max(to.bounds.l);
        let hi = from.bounds.r.min(to.bounds.r);
        if hi <= lo {
            continue; // no horizontal overlap
        }
        let inset = GATE_INSET.min(0.max(hi.wrapping_sub(lo) / 4));
        gates.push((lo.wrapping_add(inset), hi.wrapping_sub(inset)));
    }

    // Anti-trap over the topology set: isolated screens (warn). The gates part is carried by the compiler.
    let rects: Vec<Rect> = monitors.iter().map(|m| m.bounds).collect();
    let top_idx: Vec<usize> = (0..monitors.len())
        .filter(|&i| in_top_set(&monitors[i].stable_id))
        .collect();
    let (warn, _) = anti_trap(&rects, &top_idx);

    let owned: Vec<MonitorInfo> = monitors.iter().map(|m| (*m).clone()).collect();
    let (compiled, hits) = compile_with(policy, &screens_from_monitors(&owned, &[]), true, fixes);

    // Descent routes from the compiled, non-Blocked Up transitions (distinct pairs in compile order).
    let device_rects: Vec<(String, Rect)> = monitors.iter().map(|m| (m.stable_id.clone(), m.bounds)).collect();
    let mut compiled_up_links: Vec<(String, String)> = Vec::new();
    for t in &compiled.transitions {
        if t.direction != Direction::Up || t.behavior == Behavior::Blocked {
            continue;
        }
        let pair = (t.from_stable_id.clone(), t.to_stable_id.clone());
        if !compiled_up_links.contains(&pair) {
            compiled_up_links.push(pair);
        }
    }
    let top_ids: Vec<String> = tops.iter().map(|t| t.stable_id.clone()).collect();
    let routes = derive_routes(&compiled_up_links, &device_rects, &top_ids);

    // Side containment engages only when a real side screen flanks main left/right.
    let main = primary;
    let has_sides = main.is_some_and(|main| {
        monitors.iter().any(|m| {
            m.stable_id != main.stable_id
                && (adjacent(main.bounds, m.bounds, Direction::Left)
                    || adjacent(main.bounds, m.bounds, Direction::Right))
        })
    });

    let top_rects: Vec<Rect> = tops.iter().map(|t| t.bounds).collect();
    let forbidden: Vec<Rect> = excluded_monitors.iter().map(|m| m.bounds).collect();
    let config = EngineConfig {
        intent: policy.intent,
        // ---- back in TrayApplicationContext.Configure
        deliberate_cross: policy.deliberate_cross,
        descent_routing: policy.descent_routing,
        side_cross_min: policy.side_cross_min.max(1),
        side_cross_slack: policy.side_cross_slack.max(0),
        ..guard_config(
            &top_rects,
            &gates,
            &rects,
            &compiled.transitions,
            &routes,
            &forbidden,
            has_sides,
            main.map_or(Rect::default(), |m| m.bounds),
        )
    };

    let report = PlanReport {
        has_safety_demotions: compiled.has_safety_demotions(),
        pruned,
        isolated: warn.iter().map(|&i| monitors[i].stable_id.clone()).collect(),
        isolated_indices: warn.iter().map(|&i| monitors[i].index).collect(),
        sealed: compiled.sealed_screens,
        excluded: excluded_monitors.iter().map(|m| m.stable_id.clone()).collect(),
    };
    let detail = PlanDetail {
        config,
        report,
        tops: top_ids,
    };
    (detail, hits)
}

/// C# `MouseGuard.Configure(blocked, gates, allMonitors, transitions, descentRoutes, forbidden, hasSides, main)`
/// (Platform/MouseGuard.cs:84-110): `barrier_y` is the maximum bottom of the blocked tops (0 without tops), only
/// gates with `max > min` are kept and empty forbidden rects are dropped. The fields the tray sets afterwards
/// (`deliberate_cross`, `descent_routing`, the side thresholds) keep the `EngineConfig` defaults here; `cross_min_up`
/// and `cross_slack` are never set from settings in 1.1.1. Applying the config to an engine with
/// `ClassicEngine::configure` clears the safety gates and resets the cursor state, as the C# does.
#[allow(clippy::too_many_arguments)]
pub fn guard_config(
    blocked: &[Rect],
    gates: &[(i32, i32)],
    all_monitors: &[Rect],
    transitions: &[fence_types::RuntimeTransition],
    descent_routes: &[(Rect, Rect)],
    forbidden: &[Rect],
    has_sides: bool,
    main: Rect,
) -> EngineConfig {
    let has_top = !blocked.is_empty();
    EngineConfig {
        has_top,
        barrier_y: if has_top {
            blocked.iter().map(|r| r.b).max().unwrap_or(0)
        } else {
            0
        },
        gates: gates.iter().copied().filter(|g| g.1 > g.0).collect(),
        monitors: all_monitors.to_vec(),
        transitions: transitions.to_vec(),
        descent_routes: descent_routes.to_vec(),
        forbidden: forbidden.iter().copied().filter(|r| r.r > r.l && r.b > r.t).collect(),
        has_sides,
        main,
        ..EngineConfig::default()
    }
}

/// The exclusion split of `Configure` (and of C# `Settings.SanitizedExcluded`): the present monitors the policy
/// asks to exclude (case-insensitive), minus the primary, never all of them; `pruned` when a present request was
/// refused. Returns the excluded stable ids in monitor order.
pub fn runtime_excluded(policy: &Policy, monitors: &[MonitorInfo]) -> (Vec<String>, bool) {
    let primary = monitors
        .iter()
        .find(|m| m.primary)
        .or_else(|| monitors.first())
        .map(|m| m.stable_id.as_str());
    let present: Vec<String> = monitors.iter().map(|m| m.stable_id.clone()).collect();
    sanitize_excluded(&present, primary, &policy.excluded_devices)
}

/// C# `Settings.ValidateNoSeal(monitors)` (the save/import validator) for a normalized policy: the runtime
/// exclusion split, then `BoundaryGraph.ValidateNoSeal` over the live monitors.
/// With every judged fix (2.0; J05 also reports sealed groups of screens).
pub fn validate_settings_no_seal(policy: &Policy, monitors: &[MonitorInfo]) -> Vec<SealIssue> {
    validate_settings_no_seal_with(policy, monitors, Fixes::ALL).0
}

/// [`validate_settings_no_seal`] with the chosen judged fixes (`Fixes::EXACT_1_1_1` for a validator diff against
/// the C# reference), plus the `fixes::HIT_*` bits of the fixes that changed the result.
pub fn validate_settings_no_seal_with(
    policy: &Policy,
    monitors: &[MonitorInfo],
    fixes: Fixes,
) -> (Vec<SealIssue>, u16) {
    let (excluded, _) = runtime_excluded(policy, monitors);
    validate_no_seal_with(policy, &screens_from_monitors_ignore_case(monitors, &excluded), fixes)
}

/// C# `Settings.UpLinks`: the Up rules with behaviour `OverallFeel`, as (from, to), in rule order.
pub fn up_links(policy: &Policy) -> Vec<(String, String)> {
    policy
        .rules
        .iter()
        .filter(|r| r.direction == Direction::Up && r.behavior == RuleBehavior::OverallFeel)
        .map(|r| (r.from_stable_id.clone(), r.to_stable_id.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s_engine_plan_sanitize_matches_csharp_rules() {
        let present = vec!["MAIN".to_string(), "DUMMY".to_string()];
        let (ex, pruned) = sanitize_excluded(&present, Some("MAIN"), &["main".into(), "dummy".into()]);
        assert_eq!(
            (ex, pruned),
            (vec!["DUMMY".to_string()], true),
            "primary refused, case-insensitive request"
        );
        let only = vec!["ONLY".to_string()];
        assert_eq!(
            sanitize_excluded(&only, Some("ELSEWHERE"), &["ONLY".into()]),
            (vec![], true)
        );
        assert_eq!(
            sanitize_excluded(&present, Some("MAIN"), &["DUMMY".into(), "GONE".into()]),
            (vec!["DUMMY".to_string()], false),
            "absent ids are dormant, not a prune"
        );
    }
}
