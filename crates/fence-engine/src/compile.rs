//! Boundary compiler (port of C# `Core/BoundaryGraph.cs`). Track A.
//!
//! A pure rectangle compiler and save/import validator: no Win32 calls and no display mutation. All geometry,
//! rule and default work happens here, off the hook thread; `decide` only intersects a movement segment with
//! the sorted result.

use crate::fixes::{Fixes, HIT_J05, HIT_J15};
use crate::ids::{eq_ignore_case, is_blank, ordinal_cmp, same};
use crate::topology::anti_trap;
use fence_types::{
    Behavior, BoundaryDefault, BoundaryRule, CompileResult, Control, Direction, MonitorInfo, Policy, Rect,
    RuleBehavior, RuntimeTransition, Screen, SealIssue, TopMode,
};
use std::cmp::Ordering;

/// C# `BoundaryGraph.Compile(settings, screens, applyGeometrySafety)` with every judged fix (MouseFence 2.0).
pub fn compile(policy: &Policy, screens: &[Screen], apply_geometry_safety: bool) -> CompileResult {
    compile_with(policy, screens, apply_geometry_safety, Fixes::ALL).0
}

/// C# `BoundaryGraph.Compile` with the chosen judged fixes (`Fixes::EXACT_1_1_1` is 1.1.1 exactly), plus the
/// `fixes::HIT_*` bits of the fixes that changed the result.
pub fn compile_with(
    policy: &Policy,
    screens: &[Screen],
    apply_geometry_safety: bool,
    fixes: Fixes,
) -> (CompileResult, u16) {
    let usable: Vec<&Screen> = screens
        .iter()
        .filter(|s| !s.excluded && s.bounds.is_valid() && !is_blank(&s.stable_id))
        .collect();
    let candidates = build_candidates(&usable);
    let mut transitions: Vec<RuntimeTransition> = Vec::with_capacity(candidates.len());
    let mut default_hits = 0;

    for c in &candidates {
        let from = usable[c.from];
        let to = usable[c.to];
        let rule = find_rule(&policy.rules, &from.stable_id, &to.stable_id, c.direction);
        let behavior = match rule {
            None if fixes.j15_side_to_top_blocked
                && c.direction == Direction::Up
                && !from.primary
                && policy.upward_default == BoundaryDefault::Automatic
                && is_configured_top(policy, to) =>
            {
                default_hits |= HIT_J15;
                Behavior::Blocked
            }
            None => resolve_default(
                policy,
                c.direction,
                from.primary || to.primary,
                c.gap,
                is_configured_top(policy, to),
            ),
            Some(r) => match r.behavior {
                RuleBehavior::OverallFeel | RuleBehavior::Push => Behavior::Push,
                RuleBehavior::Free => Behavior::Free,
                RuleBehavior::Blocked => Behavior::Blocked,
            },
        };
        let control = rule.map_or(Control::Automatic, |r| r.control);
        let side = matches!(c.direction, Direction::Left | Direction::Right);
        transitions.push(RuntimeTransition {
            from_stable_id: from.stable_id.clone(),
            to_stable_id: to.stable_id.clone(),
            direction: c.direction,
            from: from.bounds,
            to: to.bounds,
            corridor_start: c.start,
            corridor_end: c.end,
            gap_px: c.gap,
            seam_tolerance_px: policy.seam_tolerance_px,
            behavior,
            control,
            push_min: if side {
                policy.side_cross_min
            } else {
                policy.up_cross_min
            },
            push_slack: if side {
                policy.side_cross_slack
            } else {
                policy.up_cross_slack
            },
            passage_inset_px: policy.passage_inset_px,
            edge_holdback_px: policy.edge_holdback_px,
            explicit_rule: rule.is_some(),
            sole_up_safety_escape: false,
            safety_demoted: false,
            sort_order: 0,
            allowed_landings: Default::default(),
        });
    }

    // C# List.Sort is unstable, but the key (orientation, corridor, from, to, direction) is unique per transition.
    transitions.sort_by(compare_transition);
    for (i, t) in transitions.iter_mut().enumerate() {
        t.sort_order = i as i32;
    }

    // F12 precedence: the Automatic side-to-top default is Blocked, except that a
    // sole Up exit retains the existing anti-trap Push. Mark it before geometry
    // safety, so the escape also survives compile(false) and a closed live gate.
    mark_legacy_sole_up_escapes(policy, &usable, &mut transitions);

    let groups = fixes.j05_group_seal;
    let sealed_before_safety = find_sealed(&usable, &transitions, None, groups);
    let mut hits = group_hits(&sealed_before_safety) | default_hits;
    if apply_geometry_safety && policy.safety_escapes {
        // Explicit all-blocked configurations are rejected by validate_no_seal before persistence. This first pass
        // is the later-geometry backstop; the second catches a graph that becomes sealed only when Automatic Up
        // resolves Blocked at runtime. Side protection ON is the restrictive side state.
        hits |= demote_until_open(&usable, &mut transitions, &sealed_before_safety, None, groups);
        let sealed_with_up_closed = find_sealed(&usable, &transitions, Some((false, true)), groups);
        hits |= group_hits(&sealed_with_up_closed);
        hits |= demote_until_open(
            &usable,
            &mut transitions,
            &sealed_with_up_closed,
            Some((false, true)),
            groups,
        );
    }

    build_allowed_landing_sets(&usable, &mut transitions);
    let result = CompileResult {
        transitions,
        sealed_screens: sealed_before_safety.into_iter().map(|s| s.issue).collect(),
    };
    (result, hits)
}

/// C# `BoundaryGraph.ValidateNoSeal` with every judged fix (MouseFence 2.0): compile without geometry safety and
/// return the sealed screens (with J05, the sealed groups).
pub fn validate_no_seal(policy: &Policy, screens: &[Screen]) -> Vec<SealIssue> {
    validate_no_seal_with(policy, screens, Fixes::ALL).0
}

/// C# `BoundaryGraph.ValidateNoSeal` with the chosen judged fixes (`Fixes::EXACT_1_1_1` is 1.1.1 exactly, the
/// form a validator diff against the C# reference needs), plus the `fixes::HIT_*` bits of the fixes that changed it.
pub fn validate_no_seal_with(policy: &Policy, screens: &[Screen], fixes: Fixes) -> (Vec<SealIssue>, u16) {
    let (compiled, hits) = compile_with(policy, screens, false, fixes);
    (compiled.sealed_screens, hits)
}

/// C# `GuardCore.EffectiveBehavior(t, gatesEnabled, sideContain)`.
///
/// Compile-time safety exits still require deliberate Push intent, but the live Up/side toggles may never close
/// them. This safety precedence also overrides F12's Automatic side-to-top Blocked default:
/// the only way out remains Push, never Free, so incidental contact cannot escape.
/// With another exit, the side-to-top default remains Blocked. `Always` rules ignore the toggles;
/// `Automatic` Up is Blocked while the Up gate is closed, and
/// `Automatic` side rules are Free while side protection is off.
pub fn effective_behavior(t: &RuntimeTransition, gates_enabled: bool, side_contain: bool) -> Behavior {
    if t.sole_up_safety_escape || t.safety_demoted {
        return if t.behavior == Behavior::Free {
            Behavior::Free
        } else {
            Behavior::Push
        };
    }
    if t.control == Control::Always {
        return t.behavior;
    }
    match t.direction {
        Direction::Up if !gates_enabled => Behavior::Blocked,
        Direction::Left | Direction::Right if !side_contain => Behavior::Free,
        _ => t.behavior,
    }
}

/// C# `BoundaryGraph.FromMonitors(monitors, excluded)`; `excluded` holds stable ids (case-sensitive set in C#).
pub fn screens_from_monitors(monitors: &[MonitorInfo], excluded: &[String]) -> Vec<Screen> {
    monitors
        .iter()
        .map(|m| Screen {
            stable_id: m.stable_id.clone(),
            bounds: m.bounds,
            primary: m.primary,
            excluded: excluded.iter().any(|e| e == &m.stable_id),
        })
        .collect()
}

/// C# `BoundaryGraph.FromMonitors(monitors, excluded)` with an `OrdinalIgnoreCase` excluded set, the form
/// `Settings.ValidateNoSeal` uses.
pub fn screens_from_monitors_ignore_case(monitors: &[MonitorInfo], excluded: &[String]) -> Vec<Screen> {
    monitors
        .iter()
        .map(|m| Screen {
            stable_id: m.stable_id.clone(),
            bounds: m.bounds,
            primary: m.primary,
            excluded: excluded.iter().any(|e| eq_ignore_case(e, &m.stable_id)),
        })
        .collect()
}

struct Candidate {
    from: usize,
    to: usize,
    direction: Direction,
    start: i32,
    end: i32,
    gap: i32,
}

/// The last valid rule with this trimmed, case-insensitive (from, to, direction) identity (C# NormalizeRuleMap:
/// last-valid-wins; blank ids and from == to are invalid).
fn find_rule<'a>(rules: &'a [BoundaryRule], from: &str, to: &str, direction: Direction) -> Option<&'a BoundaryRule> {
    rules.iter().rev().find(|r| {
        !is_blank(&r.from_stable_id)
            && !is_blank(&r.to_stable_id)
            && !same(&r.from_stable_id, &r.to_stable_id)
            && r.direction == direction
            && same(&r.from_stable_id, from)
            && same(&r.to_stable_id, to)
    })
}

/// C# `IsConfiguredTop`: Manual mode uses the manual list (case-insensitive), AutoTop uses `b <= 0`.
fn is_configured_top(policy: &Policy, screen: &Screen) -> bool {
    match policy.mode {
        TopMode::Manual => policy
            .manual_monitors
            .iter()
            .any(|m| eq_ignore_case(m, &screen.stable_id)),
        TopMode::AutoTop => screen.bounds.b <= 0,
    }
}

fn mark_legacy_sole_up_escapes(policy: &Policy, screens: &[&Screen], transitions: &mut [RuntimeTransition]) {
    if !policy.safety_escapes || screens.is_empty() {
        return;
    }
    let top_idx: Vec<usize> = (0..screens.len())
        .filter(|&i| is_configured_top(policy, screens[i]))
        .collect();
    let rects: Vec<Rect> = screens.iter().map(|s| s.bounds).collect();
    let (_, gates) = anti_trap(&rects, &top_idx);
    for t in transitions.iter_mut().filter(|t| t.direction == Direction::Up) {
        t.sole_up_safety_escape = gates.iter().any(|g| {
            g.owner_t == t.from.t && g.owner_b == t.from.b && g.min.max(t.corridor_start) < g.max.min(t.corridor_end)
        });
    }
}

fn resolve_default(
    policy: &Policy,
    direction: Direction,
    touches_primary: bool,
    gap: i32,
    automatic_upper: bool,
) -> Behavior {
    let value = match direction {
        Direction::Up => policy.upward_default,
        Direction::Down => policy.downward_default,
        Direction::Left | Direction::Right => policy.side_default,
    };
    match value {
        BoundaryDefault::Free => Behavior::Free,
        BoundaryDefault::Push => Behavior::Push,
        BoundaryDefault::Blocked => Behavior::Blocked,
        BoundaryDefault::Automatic => match direction {
            Direction::Up if automatic_upper => Behavior::Push,
            Direction::Up | Direction::Down => Behavior::Free,
            _ if touches_primary && gap <= policy.seam_tolerance_px => Behavior::Push,
            _ => Behavior::Free,
        },
    }
}

fn build_candidates(screens: &[&Screen]) -> Vec<Candidate> {
    let mut result = Vec::new();
    for i in 0..screens.len() {
        for j in 0..screens.len() {
            if i == j {
                continue;
            }
            let a = screens[i].bounds;
            let b = screens[j].bounds;
            let (direction, vertical, overlap_start, overlap_end, gap);
            if b.b <= a.t && a.r.min(b.r) > a.l.max(b.l) {
                direction = Direction::Up;
                vertical = true;
                overlap_start = a.l.max(b.l);
                overlap_end = a.r.min(b.r);
                gap = a.t.wrapping_sub(b.b);
            } else if b.t >= a.b && a.r.min(b.r) > a.l.max(b.l) {
                direction = Direction::Down;
                vertical = true;
                overlap_start = a.l.max(b.l);
                overlap_end = a.r.min(b.r);
                gap = b.t.wrapping_sub(a.b);
            } else if b.r <= a.l && a.b.min(b.b) > a.t.max(b.t) {
                direction = Direction::Left;
                vertical = false;
                overlap_start = a.t.max(b.t);
                overlap_end = a.b.min(b.b);
                gap = a.l.wrapping_sub(b.r);
            } else if b.l >= a.r && a.b.min(b.b) > a.t.max(b.t) {
                direction = Direction::Right;
                vertical = false;
                overlap_start = a.t.max(b.t);
                overlap_end = a.b.min(b.b);
                gap = b.l.wrapping_sub(a.r);
            } else {
                continue; // overlaps/clones and true diagonals have no geometric boundary
            }

            let mut spans: Vec<(i32, i32)> = vec![(overlap_start, overlap_end)];
            for (k, third) in screens.iter().enumerate() {
                if k == i || k == j {
                    continue;
                }
                let c = third.bounds;
                let between = if vertical {
                    c.b > a.b.min(b.b) && c.t < a.t.max(b.t)
                } else {
                    c.r > a.r.min(b.r) && c.l < a.l.max(b.l)
                };
                if !between {
                    continue;
                }
                let (cut_start, cut_end) = if vertical { (c.l, c.r) } else { (c.t, c.b) };
                spans = subtract(&spans, cut_start, cut_end);
                if spans.is_empty() {
                    break;
                }
            }
            for &(start, end) in spans.iter().filter(|s| s.1 > s.0) {
                result.push(Candidate {
                    from: i,
                    to: j,
                    direction,
                    start,
                    end,
                    gap: gap.max(0),
                });
            }
        }
    }
    result
}

fn subtract(spans: &[(i32, i32)], cut_start: i32, cut_end: i32) -> Vec<(i32, i32)> {
    let mut result = Vec::with_capacity(spans.len() + 1);
    for &(start, end) in spans {
        if cut_end <= start || cut_start >= end {
            result.push((start, end));
            continue;
        }
        if cut_start > start {
            result.push((start, cut_start.min(end)));
        }
        if cut_end < end {
            result.push((cut_end.max(start), end));
        }
    }
    result
}

/// The widest exit of a list of transitions, then the compiler's stable order (C# `OrderByDescending(width)
/// .ThenBy(SortOrder).First()`).
fn widest<'a>(exits: impl Iterator<Item = (usize, &'a RuntimeTransition)>) -> Option<usize> {
    let mut best: Option<(usize, &RuntimeTransition)> = None;
    for (i, t) in exits {
        let better = match best {
            None => true,
            Some((_, b)) => {
                t.corridor_width() > b.corridor_width()
                    || t.corridor_width() == b.corridor_width() && t.sort_order < b.sort_order
            }
        };
        if better {
            best = Some((i, t));
        }
    }
    best.map(|(i, _)| i)
}

/// One sealed screen (1.1.1) or sealed group (J05) and the exit geometry safety demotes for it.
struct Sealed {
    issue: SealIssue,
    /// Index of the focus exit in the transitions (the widest way out, then compiler order).
    exit: usize,
    /// A group of two or more screens (only found with J05).
    group: bool,
    /// Index of the group's first screen (report order).
    order: usize,
}

fn group_hits(sealed: &[Sealed]) -> u16 {
    if sealed.iter().any(|s| s.group) { HIT_J05 } else { 0 }
}

/// Whether an exit is usable: `runtime` = `Some((gates_enabled, side_contain))` judges it by its live effective
/// behaviour, `None` by its compiled behaviour (Free or Push is usable).
fn usable_exit(t: &RuntimeTransition, runtime: Option<(bool, bool)>) -> bool {
    match runtime {
        Some((g, s)) => effective_behavior(t, g, s) != Behavior::Blocked,
        None => matches!(t.behavior, Behavior::Free | Behavior::Push),
    }
}

/// C# `FindSealed`: every screen with exits but no usable exit, in screen order. With `groups` (J05) also every
/// sealed group of two or more screens (see `sealed_groups`), the list ordered by each group's first screen.
fn find_sealed(
    screens: &[&Screen],
    transitions: &[RuntimeTransition],
    runtime: Option<(bool, bool)>,
    groups: bool,
) -> Vec<Sealed> {
    let mut sealed = Vec::new();
    if screens.len() <= 1 {
        return sealed;
    }
    for (order, screen) in screens.iter().enumerate() {
        let exits = || {
            transitions
                .iter()
                .enumerate()
                .filter(|(_, t)| same(&t.from_stable_id, &screen.stable_id))
        };
        if exits().any(|(_, t)| usable_exit(t, runtime)) {
            continue;
        }
        let Some(exit) = widest(exits()) else { continue }; // no exits at all
        sealed.push(Sealed {
            issue: seal_issue(&screen.stable_id, &transitions[exit]),
            exit,
            group: false,
            order,
        });
    }
    if groups {
        sealed.extend(sealed_groups(screens, transitions, runtime));
        sealed.sort_by_key(|s| s.order); // stable; a sealed single screen is never in a group
    }
    sealed
}

fn seal_issue(stable_id: &str, focus: &RuntimeTransition) -> SealIssue {
    SealIssue {
        stable_id: stable_id.to_string(),
        focus_to_stable_id: focus.to_stable_id.clone(),
        focus_direction: focus.direction,
        focus_corridor_width: focus.corridor_width(),
    }
}

/// J05: the sealed groups of two or more screens. Screens are nodes; a usable exit from screen A's id to screen
/// B's id is an edge A -> B. A strongly connected group (every member reaches every other) that no usable exit
/// leaves traps the cursor once it is inside, even though each member has a usable exit (to another member), which
/// is all the 1.1.1 check asks.
///
/// Not sealed: a group that holds the primary screen, because keeping the cursor in the primary's group is what
/// the fence is for (the closed Up gate keeps the whole bottom row from the top screens, an explicit one-way
/// rule keeps main and its neighbour off a third screen; parity rows P199, P207 and B026 pin both, and a seal
/// check that demoted those exits would open every Up gate on a desk with a top screen). A group with no exit to
/// a screen outside it at all (the whole desk, or an island with no seam to the rest) is not sealed either, like
/// a single screen without exits. Single screens keep the 1.1.1 check unchanged (the primary included).
///
/// The issue is named after the owner of the group's widest way out (then compiler order); that exit is the one
/// geometry safety demotes.
fn sealed_groups(screens: &[&Screen], transitions: &[RuntimeTransition], runtime: Option<(bool, bool)>) -> Vec<Sealed> {
    let n = screens.len();
    let mut edge = vec![vec![false; n]; n];
    for t in transitions.iter().filter(|t| usable_exit(t, runtime)) {
        for (a, sa) in screens.iter().enumerate() {
            if !same(&t.from_stable_id, &sa.stable_id) {
                continue;
            }
            for (b, sb) in screens.iter().enumerate() {
                if same(&t.to_stable_id, &sb.stable_id) {
                    edge[a][b] = true;
                }
            }
        }
    }
    // reach[a][b]: b can be reached from a over usable exits (a reaches itself).
    let reach: Vec<Vec<bool>> = (0..n)
        .map(|start| {
            let mut seen = vec![false; n];
            seen[start] = true;
            let mut stack = vec![start];
            while let Some(a) = stack.pop() {
                for (b, &linked) in edge[a].iter().enumerate() {
                    if linked && !seen[b] {
                        seen[b] = true;
                        stack.push(b);
                    }
                }
            }
            seen
        })
        .collect();

    let mut sealed = Vec::new();
    let mut placed = vec![false; n];
    for first in 0..n {
        if placed[first] {
            continue;
        }
        let members: Vec<usize> = (0..n).filter(|&b| reach[first][b] && reach[b][first]).collect();
        for &m in &members {
            placed[m] = true;
        }
        let closed = members
            .iter()
            .all(|&a| (0..n).all(|b| !edge[a][b] || members.contains(&b)));
        if members.len() < 2 || !closed || members.iter().any(|&m| screens[m].primary) {
            continue;
        }
        let is_member = |id: &str| members.iter().any(|&m| same(id, &screens[m].stable_id));
        let ways_out = transitions
            .iter()
            .enumerate()
            .filter(|(_, t)| is_member(&t.from_stable_id) && !is_member(&t.to_stable_id));
        let Some(exit) = widest(ways_out) else { continue };
        let focus = &transitions[exit];
        sealed.push(Sealed {
            issue: seal_issue(&focus.from_stable_id, focus),
            exit,
            group: true,
            order: first,
        });
    }
    sealed
}

/// C# `DemoteWidestExits`: one deterministic exit per sealed screen (widest corridor, then compiler order) becomes
/// at least Push and is flagged. (For a single screen the stored focus exit is the exit 1.1.1 recomputes here: the
/// widest of the same list, and demotion changes no width or order.)
fn demote_widest_exits(sealed: &[Sealed], transitions: &mut [RuntimeTransition]) {
    for s in sealed {
        let t = &mut transitions[s.exit];
        if t.behavior == Behavior::Blocked {
            t.behavior = Behavior::Push;
        }
        t.safety_demoted = true;
    }
}

/// Demote the exits of `sealed`. With J05 the check then repeats in the same mode until nothing is sealed: a
/// demoted exit can lead into another screen whose own demoted exit leads back, which leaves the pair sealed.
/// Each round turns at least one unusable way out into a usable one (a demoted exit is never Blocked), so the
/// rounds end within the number of transitions. Returns `HIT_J05` when a round after the first found more.
fn demote_until_open(
    screens: &[&Screen],
    transitions: &mut [RuntimeTransition],
    sealed: &[Sealed],
    runtime: Option<(bool, bool)>,
    groups: bool,
) -> u16 {
    demote_widest_exits(sealed, transitions);
    if !groups {
        return 0;
    }
    let mut hits = 0;
    for _ in 0..transitions.len() {
        let again = find_sealed(screens, transitions, runtime, true);
        if again.is_empty() {
            break;
        }
        hits = HIT_J05;
        demote_widest_exits(&again, transitions);
    }
    hits
}

fn build_allowed_landing_sets(screens: &[&Screen], transitions: &mut [RuntimeTransition]) {
    for i in 0..transitions.len() {
        let sets = [
            allowed_landing_set(screens, transitions, i, true, true),
            allowed_landing_set(screens, transitions, i, true, false),
            allowed_landing_set(screens, transitions, i, false, true),
            allowed_landing_set(screens, transitions, i, false, false),
        ];
        transitions[i].allowed_landings = sets;
    }
}

/// C# `BuildAllowedLandingSet`: every screen reachable from the transition's target over edges that are not
/// Blocked in this toggle state, minus screens whose own transition from the source is Blocked.
fn allowed_landing_set(
    screens: &[&Screen],
    transitions: &[RuntimeTransition],
    index: usize,
    gates_enabled: bool,
    side_contain: bool,
) -> Vec<Rect> {
    let transition = &transitions[index];
    if effective_behavior(transition, gates_enabled, side_contain) == Behavior::Blocked {
        return vec![transition.from];
    }
    // HashSet<string>(OrdinalIgnoreCase): case-insensitive, untrimmed. `insert` is HashSet.Add.
    fn insert<'a>(seen: &mut Vec<&'a str>, id: &'a str) -> bool {
        if seen.iter().any(|s| eq_ignore_case(s, id)) {
            return false;
        }
        seen.push(id);
        true
    }
    let mut seen: Vec<&str> = Vec::new();
    insert(&mut seen, &transition.from_stable_id);
    insert(&mut seen, &transition.to_stable_id);
    let mut queue: std::collections::VecDeque<&str> = std::collections::VecDeque::new();
    queue.push_back(&transition.to_stable_id);
    while let Some(from) = queue.pop_front() {
        for edge in transitions {
            if same(&edge.from_stable_id, from)
                && effective_behavior(edge, gates_enabled, side_contain) != Behavior::Blocked
                && insert(&mut seen, &edge.to_stable_id)
            {
                queue.push_back(&edge.to_stable_id);
            }
        }
    }
    // A transitive alternate route must never authorize a correction directly onto a monitor whose ordered
    // transition from this source is blocked in the current live toggle state.
    for blocked in transitions.iter().filter(|t| {
        same(&t.from_stable_id, &transition.from_stable_id)
            && effective_behavior(t, gates_enabled, side_contain) == Behavior::Blocked
    }) {
        seen.retain(|s| !eq_ignore_case(s, &blocked.to_stable_id));
    }
    screens
        .iter()
        .filter(|s| seen.iter().any(|k| eq_ignore_case(k, &s.stable_id)))
        .map(|s| s.bounds)
        .collect()
}

fn compare_transition(a: &RuntimeTransition, b: &RuntimeTransition) -> Ordering {
    let orientation = |d: Direction| if d.is_vertical() { 0 } else { 1 };
    orientation(a.direction)
        .cmp(&orientation(b.direction))
        .then(a.corridor_start.cmp(&b.corridor_start))
        .then(a.corridor_end.cmp(&b.corridor_end))
        .then_with(|| ordinal_cmp(&a.from_stable_id, &b.from_stable_id))
        .then_with(|| ordinal_cmp(&a.to_stable_id, &b.to_stable_id))
        .then(a.direction.csharp_ordinal().cmp(&b.direction.csharp_ordinal()))
}
