//! The per-move decision (port of C# `GuardCore.Decide` and its partial files Decision, Transitions,
//! Forbidden, Routes). Track A.
//!
//! Faithful to 1.1.1 statement for statement. Numbers follow PLAN T7: i32 coordinates with C#'s wrapping
//! (`unchecked`) arithmetic, f64 where the C# uses `double` (segment hit fractions, same IEEE operations in the
//! same order), i128 where it uses `decimal` (squared distances, exact in both). `Math.Clamp` never sees
//! `min > max` on any path (each caller guards it, as in the C#); `clamp` below does not panic either way.
//!
//! The hook path does not allocate in steady state: the candidate grids and the forbidden-region flood fill use
//! scratch vectors whose capacity is reserved when the config is set.

use crate::compile::effective_behavior;
use crate::fixes::{Fixes, HIT_J01, HIT_J06, HIT_J09, HIT_J11, HIT_J12, HIT_J13, HIT_J14, HIT_J15};
use crate::intent::{Intent, Site};
use crate::topology::SafetyGate;
use fence_types::{
    Action, Behavior, CrossingModel, Decider, Decision, Direction, EdgeState, EngineConfig, IntentConfig, MoveEvent,
    Rect, RuntimeTransition, Toggles,
};
use std::cell::Cell;

/// The 2.0.0 engine (one production engine, PLAN T1). Holds the compiled config and the last-position state.
#[derive(Clone, Debug)]
pub struct ClassicEngine {
    config: EngineConfig,
    /// C# `GuardCore.SafetyGates`. `MouseGuard.Configure` always clears them in 1.1.1 (the sole-up escape lives
    /// on the compiled transition), so `plan::build` never sets them; the decision still honours them.
    safety_gates: Vec<SafetyGate>,
    confine: bool,
    pub last_x: i32,
    pub last_y: i32,
    pub have_last: bool,
    pub on_top: bool,
    gates_enabled_for_decision: bool,
    side_contain_for_decision: bool,
    scratch: Scratch,
    fixes: Fixes,
    /// `HIT_*` bits of the judged fixes that changed the last decision.
    fix_hits: Cell<u16>,
    /// The attempted target of the decision in progress (J06 judges push intent on the move itself).
    move_target: Pt,
    /// J06 applies to the landing check of the decision in progress (a passing move over a transition).
    downstream_intent: bool,
    current_event: MoveEvent,
    intent: Intent,
    trace_origin: Option<(i32, i32)>,
    trace_result: Decision,
}

#[derive(Clone, Debug, Default)]
struct Scratch {
    xs: Vec<i32>,
    ys: Vec<i32>,
    region: Vec<usize>,
    seen: Vec<bool>,
    /// J06: the screens a passing move may still reach (at most one per transition target, plus the first).
    landing: Vec<Rect>,
}

impl Scratch {
    fn reserve_for(&mut self, forbidden: usize, transitions: usize) {
        let grid = 3 + 2 * forbidden;
        self.xs.reserve(grid.saturating_sub(self.xs.len()));
        self.ys.reserve(grid.saturating_sub(self.ys.len()));
        self.region.reserve(forbidden.saturating_sub(self.region.len()));
        self.seen.reserve(forbidden.saturating_sub(self.seen.len()));
        self.landing
            .reserve((transitions + 1).saturating_sub(self.landing.len()));
    }
}

/// The C# `Math.Clamp` result for `lo <= hi` (never panics).
#[inline]
fn clamp(v: i32, lo: i32, hi: i32) -> i32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

#[inline]
fn in_rect(r: Rect, x: i32, y: i32) -> bool {
    r.contains(x, y)
}

/// C# `RectsTouch`: closed extents intersect (overlap or a shared edge).
#[inline]
fn rects_touch(a: Rect, b: Rect) -> bool {
    a.l <= b.r && b.l <= a.r && a.t <= b.b && b.t <= a.b
}

/// C# `PreferredAxis`: the crossing axis when the target is outside the source on exactly one axis.
fn preferred_axis(source: Rect, target_x: i32, target_y: i32, region_axis: i32) -> i32 {
    let outside_x = target_x < source.l || target_x >= source.r;
    let outside_y = target_y < source.t || target_y >= source.b;
    if outside_x != outside_y {
        return if outside_x { 0 } else { 1 };
    }
    region_axis
}

/// Where the cursor ends after one decision step (C# `out bx, out by`).
#[derive(Clone, Copy, Debug, Default)]
struct Pt {
    x: i32,
    y: i32,
}

impl ClassicEngine {
    /// The 2.0 engine: the 1.1.1 decision with every judged fix (`Fixes::ALL`).
    pub fn new(config: EngineConfig) -> Self {
        Self::with_fixes(config, Fixes::ALL)
    }

    /// The exact 1.1.1 decision (no judged fix), for the reference diff.
    pub fn exact_1_1_1(config: EngineConfig) -> Self {
        Self::with_fixes(config, Fixes::EXACT_1_1_1)
    }

    pub fn with_fixes(config: EngineConfig, fixes: Fixes) -> Self {
        let intent = Intent::new(intent_config(config.intent, fixes));
        let mut scratch = Scratch::default();
        scratch.reserve_for(config.forbidden.len(), config.transitions.len());
        Self {
            config,
            safety_gates: Vec::new(),
            confine: false,
            last_x: 0,
            last_y: 0,
            have_last: false,
            on_top: false,
            gates_enabled_for_decision: false,
            side_contain_for_decision: false,
            scratch,
            fixes,
            fix_hits: Cell::new(0),
            move_target: Pt::default(),
            downstream_intent: false,
            current_event: MoveEvent::default(),
            intent,
            trace_origin: None,
            trace_result: Decision {
                action: Action::Pass,
                x: 0,
                y: 0,
            },
        }
    }

    pub fn fixes(&self) -> Fixes {
        self.fixes
    }

    /// The `fixes::HIT_*` bits of the judged fixes that changed the last decision (0 = 1.1.1 would have decided
    /// the same way).
    pub fn last_fix_hits(&self) -> u16 {
        self.fix_hits.get()
    }

    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    /// Direct access to the GuardCore fields, the way the C# tests set them (`c.DeliberateCross = false`).
    /// Production code replaces the whole config with [`ClassicEngine::configure`].
    pub fn config_mut(&mut self) -> &mut EngineConfig {
        &mut self.config
    }

    /// Replace the compiled config. Matches C# `MouseGuard.Configure`, which ends with `Reset()`.
    pub fn configure(&mut self, config: EngineConfig) {
        self.intent.configure(intent_config(config.intent, self.fixes));
        self.config = config;
        self.safety_gates.clear();
        self.scratch
            .reserve_for(self.config.forbidden.len(), self.config.transitions.len());
        self.reset();
    }

    pub fn safety_gates(&self) -> &[SafetyGate] {
        &self.safety_gates
    }

    /// C# `GuardCore.SafetyGates` (tests and the reference `core` verb only; 1.1.1 production clears them).
    pub fn set_safety_gates(&mut self, gates: Vec<SafetyGate>) {
        self.safety_gates = gates;
    }

    /// Set the last accepted position (C# test helper `SetLast`: LastX, LastY, HaveLast = true).
    pub fn set_last(&mut self, x: i32, y: i32) {
        self.last_x = x;
        self.last_y = y;
        self.have_last = true;
    }

    /// C# `GuardCore.Decide(x, y, gatesEnabled, sideContain, out bx, out by)`.
    pub fn decide_xy(&mut self, x: i32, y: i32, gates_enabled: bool, side_contain: bool) -> Decision {
        self.decide_event(
            MoveEvent::untimed(x, y),
            Toggles {
                gate_open: gates_enabled,
                side_contain,
            },
        )
    }

    pub fn decide_event(&mut self, event: MoveEvent, toggles: Toggles) -> Decision {
        self.trace_origin = self.have_last.then_some((self.last_x, self.last_y));
        self.current_event = event;
        let (dx, dy) = if self.have_last {
            (event.x.wrapping_sub(self.last_x), event.y.wrapping_sub(self.last_y))
        } else {
            (0, 0)
        };
        self.intent.record(dx, dy, &event);
        self.fix_hits.set(0);
        let (action, p) = self.decide_inner(event.x, event.y, toggles.gate_open, toggles.side_contain);
        self.trace_result = Decision { action, x: p.x, y: p.y };
        self.trace_result
    }

    fn decide_inner(&mut self, x: i32, y: i32, gates_enabled: bool, side_contain: bool) -> (Action, Pt) {
        let mut b = Pt { x, y };
        self.gates_enabled_for_decision = gates_enabled;
        self.side_contain_for_decision = side_contain;
        self.move_target = Pt { x, y };
        self.scratch
            .reserve_for(self.config.forbidden.len(), self.config.transitions.len());

        // (0) Wall-off: the cursor may never ENTER an excluded display. Clamp the attempted target back to a legal
        //     edge first, so every later rule only sees a position outside every forbidden rect. Runs even with no
        //     top monitors and no game mode. Only genuine incursions clamp, so sliding along a seam never jitters.
        if !self.config.forbidden.is_empty() && self.clamp_out_of_forbidden(&mut b) {
            return self.finish(Action::Block, x, y, b, None, false);
        }

        let c = &self.config;
        if !c.has_top && !self.confine && !c.has_sides && c.transitions.is_empty() {
            return self.finish(Action::Pass, x, y, b, None, false);
        }

        if !self.have_last {
            return self.finish(Action::Pass, x, y, b, None, false);
        }

        // Game mode: confine the cursor to the monitor it is currently on (overrides the up-barrier).
        if self.confine {
            if let Some(m) = self.try_active_monitor(self.last_x, self.last_y) {
                if x >= m.l && x < m.r && y >= m.t && y < m.b {
                    return self.finish(Action::Pass, x, y, b, None, false);
                }
                b.x = clamp(x, m.l, m.r.wrapping_sub(1));
                b.y = clamp(y, m.t, m.b.wrapping_sub(1));
                return self.finish(Action::Block, x, y, b, None, false);
            }
            return self.finish(Action::Pass, x, y, b, None, false);
        }

        // Schema-v4 directional graph: runs before the legacy BarrierY/OnTop path so Down and side rules are
        // first-class and a fast move cannot skip a blocked seam.
        let mut transition = self.match_transition(x, y, None, -1.0);
        for _ in 0..self.config.transitions.len() {
            let Some((ti, in_passage, hit)) = transition else {
                break;
            };
            let t = &self.config.transitions[ti];
            let behavior = effective_behavior(t, gates_enabled, side_contain);
            let direction = t.direction;
            let crosses = behavior == Behavior::Free
                || behavior == Behavior::Push && in_passage && self.has_transition_intent(ti, x, y);
            if crosses {
                if direction == Direction::Down
                    && let Some(g) = self.try_guide_descent(x, y)
                {
                    return self.finish(Action::Block, x, y, g, Some(ti), true);
                }
                if self.fixes.j11_segment_and_identity {
                    let next = self.match_transition(x, y, Some(ti), hit);
                    if next.is_some() {
                        self.fix_hits.set(self.fix_hits.get() | HIT_J11);
                        transition = next;
                        continue;
                    }
                }
                return self.finish(Action::Pass, x, y, b, Some(ti), true);
            }
            hold_on_source(&self.config.transitions[ti], x, y, &mut b);
            return self.finish(Action::Block, x, y, b, Some(ti), false);
        }

        // (S) Side containment: a soft, symmetric barrier on the main screen's left/right edges, origin-aware,
        //     with its own threshold (SideCrossMin/SideCrossSlack). A deliberate horizontal push always crosses.
        if self.config.has_sides {
            let c = &self.config;
            let (main_l, main_r, main_t, main_b) = (c.main.l, c.main.r, c.main.t, c.main.b);
            let (has_top, barrier_y) = (c.has_top, c.barrier_y);
            let (lx, ly) = (self.last_x, self.last_y);
            let started_on_main = lx >= main_l && lx < main_r && ly >= main_t && ly < main_b;
            let side_dx = x.wrapping_sub(lx).wrapping_abs();
            let side_dy = y.wrapping_sub(ly).wrapping_abs();
            let site = Site {
                id: usize::MAX - 1,
                minimum: c.side_cross_min,
                slack: c.side_cross_slack,
            };
            let direction = if x < lx { Direction::Left } else { Direction::Right };
            let at_edge = match direction {
                Direction::Left => lx == main_l || lx == main_r,
                _ => lx == main_r.wrapping_sub(1) || lx == main_l.wrapping_sub(1),
            };
            let side_crossing = started_on_main && (x < main_l || x >= main_r)
                || !started_on_main
                    && (lx < main_l || lx >= main_r)
                    && x >= main_l
                    && x < main_r
                    && y >= main_t
                    && y < main_b;
            let side_intent = side_crossing && self.judge_intent(site, direction, side_dx, side_dy, at_edge);

            if started_on_main && (x < main_l || x >= main_r) {
                // Direction OUT (main -> side).
                if side_contain && !side_intent {
                    b.x = clamp(x, main_l, main_r.wrapping_sub(1)); // never leave main horizontally
                    b.y = y; // keep the vertical component by default
                    // Compose the up-barrier: a diagonal that also goes above the line must not slip up at the
                    // clamped edge.
                    if has_top && b.y < barrier_y {
                        b.y = barrier_y;
                    }
                    // Y-void correction only: pull Y back onto the main band when the clamped point is on no
                    // monitor (a side screen taller than main, or a corner beyond every screen).
                    if self.try_active_monitor(b.x, b.y).is_none() {
                        b.y = clamp(b.y, main_t, main_b.wrapping_sub(1));
                    }
                    return self.finish(Action::Block, x, y, b, None, false);
                }
                // allowed: fall through so the up-barrier still governs the vertical component.
            } else if !started_on_main
                && (lx < main_l || lx >= main_r)
                && x >= main_l
                && x < main_r
                && y >= main_t
                && y < main_b
            {
                // Direction IN (side -> main): block an accidental drift back onto main.
                if side_contain && !side_intent {
                    b.x = if lx < main_l { main_l.wrapping_sub(1) } else { main_r };
                    b.y = y;
                    // Void correction: pull the point back onto the origin side screen when the held pixel is
                    // on no monitor (a gap between main and the side screen).
                    if self.try_active_monitor(b.x, b.y).is_none()
                        && let Some(origin) = self.try_active_monitor(lx, ly)
                    {
                        b.x = clamp(b.x, origin.l, origin.r.wrapping_sub(1));
                        b.y = clamp(b.y, origin.t, origin.b.wrapping_sub(1));
                    }
                    return self.finish(Action::Block, x, y, b, None, false);
                }
                // allowed: fall through (the up-barrier / OnTop logic governs from here).
            }
        }

        if !self.config.has_top {
            return self.finish(Action::Pass, x, y, b, None, false);
        }

        // (A) Already above the line -> free roam; clears once the cursor descends past the line.
        if self.on_top {
            // Descent routing (opt-in): on a real descent crossing a top's local bottom edge onto a different
            // present monitor than the one linked to that top, clamp X into the linked monitor's X range. Fires
            // on the first route whose conditions all hold against that same route.
            if self.config.descent_routing {
                for ri in 0..self.config.descent_routes.len() {
                    let (top, landing) = self.config.descent_routes[ri];
                    if landing.r <= landing.l {
                        continue; // a zero/negative-width landing would make the clamp below throw
                    }
                    let descending = y > self.last_y;
                    let origin_in_top = in_rect(top, self.last_x, self.last_y);
                    let crosses_local_edge = self.crosses_local_bottom_edge(top, x, y);
                    let outside_landing = !in_rect(landing, x, y);
                    let land_x = clamp(x, landing.l, landing.r.wrapping_sub(1));
                    let real_landing = in_rect(landing, land_x, y);
                    let wrong_monitor = self.try_active_monitor(x, y).is_some_and(|a| a != landing);
                    if descending
                        && origin_in_top
                        && crosses_local_edge
                        && outside_landing
                        && real_landing
                        && wrong_monitor
                    {
                        b.x = land_x;
                        b.y = y;
                        // Only leave "on top" at/below the global barrier (a shallow top can fire above it).
                        if b.y >= self.config.barrier_y {
                            self.on_top = false;
                        }
                        return self.finish(Action::Block, x, y, b, None, false);
                    }
                }
            }

            if y >= self.config.barrier_y {
                self.on_top = false;
            }
            return self.finish(Action::Pass, x, y, b, None, false);
        }

        // (B) On the bottom row, staying at/below the line -> normal move.
        if y >= self.config.barrier_y {
            return self.finish(Action::Pass, x, y, b, None, false);
        }

        // (C) Upward crossing attempt: honour only a deliberate push that starts and ends inside the same gate. A
        //     user gate needs the master toggle; a safety gate is honoured even when it is closed.
        let c = &self.config;
        let started_below = self.last_y >= c.barrier_y;
        let in_user_gate = gates_enabled && started_below && in_same_gate(x, self.last_x, &c.gates);
        let legacy_safety = started_below && self.in_safety_gate(x, self.last_x, self.last_y);
        if self.fixes.j11_segment_and_identity && legacy_safety && !in_user_gate {
            self.fix_hits.set(self.fix_hits.get() | HIT_J11);
        }
        let in_safety_gate = !self.fixes.j11_segment_and_identity && legacy_safety;
        let dx_abs = x.wrapping_sub(self.last_x).wrapping_abs();
        let dy_up = self.last_y.wrapping_sub(y);
        let site = Site {
            id: usize::MAX,
            minimum: c.cross_min_up,
            slack: c.cross_slack,
        };
        let intent = self.judge_intent(site, Direction::Up, dy_up, dx_abs, self.last_y == c.barrier_y);
        if (in_user_gate || in_safety_gate) && intent {
            self.on_top = true;
            return self.finish(Action::Pass, x, y, b, None, false);
        }

        // Block: clamp down to the barrier line, keep X so horizontal sliding stays smooth.
        b.x = x;
        b.y = self.config.barrier_y;
        self.finish(Action::Block, x, y, b, None, false)
    }

    /// C# `FinishDecision`: the single exit for every decision. A valid target is kept exactly. A Pass target in
    /// a desktop void resolves to the globally nearest legal pixel; a Block clamp in a void first resolves onto the
    /// usable monitor that owned the move's last legal point.
    fn finish(
        &mut self,
        mut action: Action,
        attempted_x: i32,
        attempted_y: i32,
        mut b: Pt,
        transition: Option<usize>,
        mut transition_passed: bool,
    ) -> (Action, Pt) {
        let mut final_x = if action == Action::Block { b.x } else { attempted_x };
        let mut final_y = if action == Action::Block { b.y } else { attempted_y };
        // A single report originating on a side cannot use MAIN as an intermediate permission to
        // bypass that side's default blocked upper seam. A later report observed on MAIN can use MAIN's rule.
        // Check effective behavior: the sole-exit anti-trap Push takes precedence over F12,
        // and has already passed the same deliberate-intent judge as every other Push.
        if self.fixes.j15_side_to_top_blocked && action == Action::Pass && transition_passed {
            let denied = self.config.transitions.iter().any(|edge| {
                edge.direction == Direction::Up
                    && !edge.explicit_rule
                    && edge.from.contains(self.last_x, self.last_y)
                    && edge.to.contains(final_x, final_y)
                    && effective_behavior(edge, self.gates_enabled_for_decision, self.side_contain_for_decision)
                        == Behavior::Blocked
            });
            if denied && let Some(ti) = transition {
                hold_on_source(&self.config.transitions[ti], final_x, final_y, &mut b);
                final_x = b.x;
                final_y = b.y;
                action = Action::Block;
                transition_passed = false;
                self.fix_hits.set(self.fix_hits.get() | HIT_J15);
            }
        }
        self.downstream_intent = self.fixes.j06_downstream_intent && action == Action::Pass && transition_passed;

        let unauthorized = match transition {
            Some(ti) => !self.point_is_authorized(final_x, final_y, ti, transition_passed),
            None => {
                let legacy = self.point_is_globally_authorized_from_last(final_x, final_y);
                let free = self.fixes.j12_corner_void
                    && self
                        .try_active_monitor(final_x, final_y)
                        .is_some_and(|m| !self.freely_reachable(m));
                if legacy && free {
                    self.fix_hits.set(self.fix_hits.get() | HIT_J12);
                }
                !legacy || free
            }
        };
        if !self.config.monitors.is_empty()
            && (self.try_active_monitor(final_x, final_y).is_none()
                || self.inside_forbidden(final_x, final_y)
                || unauthorized)
        {
            let mut resolved: Option<(i32, i32)> = None;

            if self.fixes.j11_segment_and_identity
                && action == Action::Pass
                && transition_passed
                && self.try_active_monitor(final_x, final_y).is_none()
                && let Some(ti) = transition
            {
                let destination = self.config.transitions[ti].to;
                resolved = self
                    .try_legal_point_on_monitor(destination, final_x, final_y, -1)
                    .map(|(x, y, _)| (x, y));
                if resolved.is_some() {
                    self.fix_hits.set(self.fix_hits.get() | HIT_J11);
                }
            }

            if action == Action::Block
                && self.have_last
                && let Some(entry) = self.try_active_monitor(self.last_x, self.last_y)
                && !self.inside_forbidden(self.last_x, self.last_y)
            {
                let entry_authorized = match transition {
                    None => true,
                    Some(ti) => self.rect_is_authorized(entry, ti, transition_passed),
                };
                if entry_authorized {
                    resolved = self
                        .try_legal_point_on_monitor(entry, final_x, final_y, -1)
                        .map(|(lx, ly, _)| (lx, ly));
                }
            }

            if resolved.is_none() {
                resolved = self.try_nearest_legal_monitor_point(final_x, final_y, transition, transition_passed);
            }

            if let Some((lx, ly)) = resolved {
                final_x = lx;
                final_y = ly;
                b.x = lx;
                b.y = ly;
                action = Action::Block;
            }
        }

        // Derive free-roam from where the cursor actually lands, never the off-screen attempted Y.
        if !self.have_last {
            self.on_top = self.config.has_top && final_y < self.config.barrier_y;
        } else if self.config.has_top {
            self.on_top = final_y < self.config.barrier_y;
        }

        self.last_x = final_x;
        self.last_y = final_y;
        self.have_last = true;
        (action, b)
    }

    // ------------------------------------------------------------------ GuardCore.cs

    fn in_safety_gate(&self, x: i32, last_x: i32, last_y: i32) -> bool {
        self.safety_gates.iter().any(|g| {
            x >= g.min && x < g.max && last_x >= g.min && last_x < g.max && last_y >= g.owner_t && last_y < g.owner_b
        })
    }

    // ------------------------------------------------------------------ GuardCore.Transitions.cs

    /// C# `MatchTransition`: the earliest crossed corridor of a transition leaving the source rect.
    fn match_transition(&self, x: i32, y: i32, previous: Option<usize>, after: f64) -> Option<(usize, bool, f64)> {
        let mut best: Option<usize> = None;
        let mut best_in_passage = false;
        let mut best_t = f64::INFINITY;
        let (lx, ly) = (self.last_x, self.last_y);
        for (i, t) in self.config.transitions.iter().enumerate() {
            if if let Some(prev) = previous {
                t.from != self.config.transitions[prev].to
            } else {
                !in_rect(t.from, lx, ly)
            } {
                continue;
            }
            let (delta, plane, origin_axis, target_axis, origin_orth, target_orth) = match t.direction {
                Direction::Up => (ly.wrapping_sub(y), t.from.t, ly, y, lx, x),
                Direction::Down => (y.wrapping_sub(ly), t.from.b, ly, y, lx, x),
                Direction::Left => (lx.wrapping_sub(x), t.from.l, lx, x, ly, y),
                Direction::Right => (x.wrapping_sub(lx), t.from.r, lx, x, ly, y),
            };
            if delta <= 0 || target_axis == origin_axis {
                continue;
            }
            let hit = f64::from(plane.wrapping_sub(origin_axis)) / f64::from(target_axis.wrapping_sub(origin_axis));
            if !(0.0..=1.0).contains(&hit) || hit <= after {
                continue;
            }
            let orth = f64::from(origin_orth) + f64::from(target_orth.wrapping_sub(origin_orth)) * hit;
            let inset = t.passage_inset_px.min(0.max(t.corridor_width().wrapping_sub(1) / 2));
            let in_passage = orth >= f64::from(t.corridor_start.wrapping_add(inset))
                && orth < f64::from(t.corridor_end.wrapping_sub(inset));
            if if self.fixes.j09_full_corridor {
                orth < f64::from(t.corridor_start) || orth >= f64::from(t.corridor_end)
            } else {
                !in_passage
            } {
                continue;
            }
            if self.fixes.j09_full_corridor && !in_passage {
                self.fix_hits.set(self.fix_hits.get() | HIT_J09);
            }
            // A gap is enforceable only when Windows establishes this exact ordered transition by landing in To.
            if t.gap_px > t.seam_tolerance_px && !in_rect(t.to, x, y) {
                continue;
            }
            // J01 (cd65f2e): the top row and left column belong to the half-open source rect, so a move that
            // ends there has not crossed the Up or Left plane (1.1.1 counted the hit at t == 1 as a crossing).
            if self.fixes.j01_source_edge_pixels && in_rect(t.from, x, y) {
                self.fix_hits.set(self.fix_hits.get() | HIT_J01);
                continue;
            }
            if self.fixes.j11_segment_and_identity
                && match t.direction {
                    Direction::Up => y >= t.from.t,
                    Direction::Down => y < t.from.b,
                    Direction::Left => x >= t.from.l,
                    Direction::Right => x < t.from.r,
                }
            {
                // The old engine considered a hit on this plane a crossing even when the target is
                // still on its inclusive source side (but outside its orthogonal extent).
                self.fix_hits.set(self.fix_hits.get() | HIT_J11);
                continue;
            }
            let better = match best {
                None => true,
                Some(bi) => {
                    let b = &self.config.transitions[bi];
                    hit < best_t
                        || hit == best_t && (t.gap_px < b.gap_px || t.gap_px == b.gap_px && t.sort_order < b.sort_order)
                }
            };
            if better {
                best = Some(i);
                best_in_passage = in_passage;
                best_t = hit;
            }
        }
        best.map(|i| (i, best_in_passage, best_t))
    }

    /// C# `HasTransitionIntent`.
    fn has_transition_intent(&mut self, ti: usize, x: i32, y: i32) -> bool {
        let t = &self.config.transitions[ti];
        let (lx, ly) = (self.last_x, self.last_y);
        let primary = match t.direction {
            Direction::Up => ly.wrapping_sub(y),
            Direction::Down => y.wrapping_sub(ly),
            Direction::Left => lx.wrapping_sub(x),
            Direction::Right => x.wrapping_sub(lx),
        };
        let orthogonal = if t.direction.is_vertical() {
            x.wrapping_sub(lx).wrapping_abs()
        } else {
            y.wrapping_sub(ly).wrapping_abs()
        };
        let site = Site {
            id: ti,
            minimum: t.push_min,
            slack: t.push_slack,
        };
        let direction = t.direction;
        let at_edge = match direction {
            Direction::Up => ly <= t.from.t.saturating_add(t.edge_holdback_px),
            Direction::Down => ly >= t.from.b.saturating_sub(1).saturating_sub(t.edge_holdback_px),
            Direction::Left => lx <= t.from.l.saturating_add(t.edge_holdback_px),
            Direction::Right => lx >= t.from.r.saturating_sub(1).saturating_sub(t.edge_holdback_px),
        };
        self.judge_intent(site, direction, primary, orthogonal, at_edge)
    }

    fn judge_intent(&mut self, site: Site, dir: Direction, primary: i32, orthogonal: i32, at_edge: bool) -> bool {
        if !self.config.deliberate_cross {
            return primary > 0;
        }
        let classic = primary >= site.minimum && orthogonal <= primary.wrapping_add(site.slack);
        let answer = self
            .intent
            .judge(site, dir, primary, orthogonal, &self.current_event, at_edge);
        if answer != classic {
            let bit = match self.config.intent.model {
                CrossingModel::Classic1k | CrossingModel::Intent => HIT_J13,
                CrossingModel::StopThenPush => HIT_J14,
                _ => 0,
            };
            self.fix_hits.set(self.fix_hits.get() | bit);
        }
        answer
    }

    pub fn estimated_rate(&self) -> f64 {
        self.intent.estimated_rate()
    }
    pub fn intent_state(&self) -> u8 {
        self.intent.state()
    }

    fn point_is_authorized(&mut self, x: i32, y: i32, ti: usize, passed: bool) -> bool {
        let (g, s) = (self.gates_enabled_for_decision, self.side_contain_for_decision);
        let t = &self.config.transitions[ti];
        if !passed {
            return in_rect(t.from, x, y);
        }
        for i in 0..t.allowed_landings_for(g, s).len() {
            let m = self.config.transitions[ti].allowed_landings_for(g, s)[i];
            if in_rect(m, x, y) && self.reached_with_intent(ti, m) {
                return true;
            }
        }
        false
    }

    fn rect_is_authorized(&mut self, monitor: Rect, ti: usize, passed: bool) -> bool {
        let t = &self.config.transitions[ti];
        if !passed {
            return monitor == t.from;
        }
        t.allowed_landings_for(self.gates_enabled_for_decision, self.side_contain_for_decision)
            .contains(&monitor)
            && self.reached_with_intent(ti, monitor)
    }

    /// J06 (PLAN F7, lesson L8), for a landing `monitor` in the allowed set of the passed transition `ti`: with the
    /// fix on and a passing move, the monitor is the transition's source or target, or reachable from the target
    /// over seams this move may cross (Free, or Push with this move's own push intent). 1.1.1 (and every other
    /// decision) accepts the whole allowed set. Reports `HIT_J06` when it refuses a monitor 1.1.1 accepts.
    fn reached_with_intent(&mut self, ti: usize, monitor: Rect) -> bool {
        let t = &self.config.transitions[ti];
        if !self.downstream_intent || monitor == t.from || monitor == t.to {
            return true;
        }
        let (g, s) = (self.gates_enabled_for_decision, self.side_contain_for_decision);
        let Pt { x, y } = self.move_target;
        // Breadth-first over screen rects; the scratch holds one entry per distinct target, so it never grows
        // past the capacity reserved for the transitions.
        let mut reached = std::mem::take(&mut self.scratch.landing);
        reached.clear();
        reached.push(t.to);
        let mut found = false;
        let mut next = 0;
        'search: while next < reached.len() {
            let at = reached[next];
            next += 1;
            for ei in 0..self.config.transitions.len() {
                let edge = &self.config.transitions[ei];
                let (from, to, behavior) = (edge.from, edge.to, effective_behavior(edge, g, s));
                if from != at || reached.contains(&to) {
                    continue;
                }
                let open = match behavior {
                    Behavior::Free => true,
                    Behavior::Push => self.has_transition_intent(ei, x, y),
                    Behavior::Blocked => false,
                };
                if !open {
                    continue;
                }
                if to == monitor {
                    found = true;
                    break 'search;
                }
                reached.push(to);
            }
        }
        self.scratch.landing = reached;
        if !found {
            self.fix_hits.set(self.fix_hits.get() | HIT_J06);
        }
        found
    }

    fn point_is_globally_authorized_from_last(&self, x: i32, y: i32) -> bool {
        match self.try_active_monitor(x, y) {
            None => true, // a void is repaired through the rect overload
            Some(m) => self.rect_is_globally_authorized_from_last(m),
        }
    }

    fn rect_is_globally_authorized_from_last(&self, monitor: Rect) -> bool {
        if self.config.transitions.is_empty() || !self.have_last {
            return true;
        }
        let Some(source) = self.try_active_monitor(self.last_x, self.last_y) else {
            return true;
        };
        if monitor == source {
            return true;
        }
        let (g, s) = (self.gates_enabled_for_decision, self.side_contain_for_decision);
        let mut has_source_transition = false;
        for edge in &self.config.transitions {
            if edge.from != source {
                continue;
            }
            has_source_transition = true;
            if effective_behavior(edge, g, s) == Behavior::Blocked {
                continue;
            }
            if edge.allowed_landings_for(g, s).contains(&monitor) {
                return true;
            }
        }
        // With no geometric boundary from the source (true diagonal/clone), there is no ordered rule to violate.
        !has_source_transition
    }

    // ------------------------------------------------------------------ GuardCore.Routes.cs

    /// C# `CrossesLocalBottomEdge`: Last -> target crosses the top's local bottom edge inside its X span.
    fn crosses_local_bottom_edge(&self, top: Rect, x: i32, y: i32) -> bool {
        let vertical_delta = y.wrapping_sub(self.last_y);
        if vertical_delta <= 0 || self.last_y >= top.b || y < top.b {
            return false;
        }
        let hit = f64::from(top.b.wrapping_sub(self.last_y)) / f64::from(vertical_delta);
        let crossing_x = f64::from(self.last_x) + f64::from(x.wrapping_sub(self.last_x)) * hit;
        crossing_x >= f64::from(top.l) && crossing_x < f64::from(top.r)
    }

    /// C# `TryGuideDescent`.
    fn try_guide_descent(&self, x: i32, y: i32) -> Option<Pt> {
        if !self.config.descent_routing {
            return None;
        }
        for &(top, landing) in &self.config.descent_routes {
            if landing.r <= landing.l {
                continue;
            }
            let descending = y > self.last_y;
            let origin_in_top = in_rect(top, self.last_x, self.last_y);
            let crosses_local_edge = self.crosses_local_bottom_edge(top, x, y);
            let outside_landing = !in_rect(landing, x, y);
            let land_x = clamp(x, landing.l, landing.r.wrapping_sub(1));
            let real_landing = in_rect(landing, land_x, y);
            let wrong_monitor = self.try_active_monitor(x, y).is_some_and(|a| a != landing);
            if !descending
                || !origin_in_top
                || !crosses_local_edge
                || !outside_landing
                || !real_landing
                || !wrong_monitor
            {
                continue;
            }
            return Some(Pt { x: land_x, y });
        }
        None
    }

    // ------------------------------------------------------------------ GuardCore.Forbidden.cs

    /// C# `TryActiveMonitor`: the containing monitor with the smallest (L, T, R, B), never list order.
    fn try_active_monitor(&self, x: i32, y: i32) -> Option<Rect> {
        let mut found: Option<Rect> = None;
        for &r in &self.config.monitors {
            if !in_rect(r, x, y) {
                continue;
            }
            if found.is_none_or(|m| r.cmp_ltrb(&m).is_lt()) {
                found = Some(r);
            }
        }
        found
    }

    /// C# `InsideForbidden`: inside a forbidden rect and on no usable monitor (usable geometry wins on overlap).
    fn inside_forbidden(&self, x: i32, y: i32) -> bool {
        if !self.config.forbidden.iter().any(|f| in_rect(*f, x, y)) {
            return false;
        }
        self.try_active_monitor(x, y).is_none()
    }

    fn region_contains(&self, region: &[usize], x: i32, y: i32) -> bool {
        region.iter().any(|&i| in_rect(self.config.forbidden[i], x, y))
    }

    /// C# `ConnectedRegion`: every forbidden rect transitively touching `seed`, flood-fill order.
    fn connected_region(&self, seed: usize, region: &mut Vec<usize>, seen: &mut Vec<bool>) {
        let f = &self.config.forbidden;
        region.clear();
        seen.clear();
        seen.resize(f.len(), false);
        region.push(seed);
        seen[seed] = true;
        let mut qi = 0;
        while qi < region.len() {
            let cur = f[region[qi]];
            for j in 0..f.len() {
                if !seen[j] && rects_touch(cur, f[j]) {
                    seen[j] = true;
                    region.push(j);
                }
            }
            qi += 1;
        }
    }

    /// C# `ComponentExtent`: the union's outer box.
    fn component_extent(&self, region: &[usize]) -> (i32, i32, i32, i32) {
        let (mut l, mut t, mut r, mut b) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for &i in region {
            let f = self.config.forbidden[i];
            l = l.min(f.l);
            t = t.min(f.t);
            r = r.max(f.r);
            b = b.max(f.b);
        }
        (l, t, r, b)
    }

    /// C# `ClampOutOfForbidden`: push a point inside the excluded-only part of a forbidden rect back out to a legal
    /// pixel, entry-side aware, treating touching forbidden rects as one region. Returns true when it moved.
    fn clamp_out_of_forbidden(&mut self, p: &mut Pt) -> bool {
        let mut region = std::mem::take(&mut self.scratch.region);
        let mut seen = std::mem::take(&mut self.scratch.seen);
        let moved = self.clamp_out_of_forbidden_with(p, &mut region, &mut seen);
        self.scratch.region = region;
        self.scratch.seen = seen;
        moved
    }

    fn clamp_out_of_forbidden_with(&mut self, p: &mut Pt, region: &mut Vec<usize>, seen: &mut Vec<bool>) -> bool {
        let mut moved = false;
        let count = self.config.forbidden.len();
        // Bounded: each pass exits one whole connected component along a fixed direction.
        for _pass in 0..=count {
            let (x, y) = (p.x, p.y);
            if !self.inside_forbidden(x, y) {
                break;
            }
            let Some(hit) = self.config.forbidden.iter().position(|f| in_rect(*f, x, y)) else {
                break;
            };
            self.connected_region(hit, region, seen);

            // Default: nearest edge of the hit rect; ties Left > Right > Top > Bottom.
            let h = self.config.forbidden[hit];
            let dl = x.wrapping_sub(h.l);
            let dr = h.r.wrapping_sub(x);
            let dt = y.wrapping_sub(h.t);
            let db = h.b.wrapping_sub(y);
            let min = dl.min(dr).min(dt.min(db));
            let (mut exit_axis, mut exit_to_low) = if min == dl {
                (0, true)
            } else if min == dr {
                (0, false)
            } else if min == dt {
                (1, true)
            } else {
                (1, false)
            };

            // Entry-side override: exit back through the region wall the move segment crosses first.
            let (reg_l, reg_t, reg_r, reg_b) = self.component_extent(region);
            let (lx, ly) = (self.last_x, self.last_y);
            let last_outside_region = self.have_last && !self.region_contains(region, lx, ly);
            if last_outside_region {
                let last_left = lx < reg_l;
                let last_right = lx >= reg_r;
                let last_above = ly < reg_t;
                let last_below = ly >= reg_b;
                let x_outside = last_left || last_right;
                let y_outside = last_above || last_below;

                let use_x = if x_outside && y_outside {
                    let wall_x = if last_left { reg_l } else { reg_r };
                    let wall_y = if last_above { reg_t } else { reg_b };
                    let denom_x = x.wrapping_sub(lx);
                    let denom_y = y.wrapping_sub(ly);
                    let t_x = if denom_x != 0 {
                        f64::from(wall_x.wrapping_sub(lx)) / f64::from(denom_x)
                    } else {
                        f64::INFINITY
                    };
                    let t_y = if denom_y != 0 {
                        f64::from(wall_y.wrapping_sub(ly)) / f64::from(denom_y)
                    } else {
                        f64::INFINITY
                    };
                    let valid_x = denom_x != 0 && (0.0..=1.0).contains(&t_x);
                    let valid_y = denom_y != 0 && (0.0..=1.0).contains(&t_y);
                    if valid_x && valid_y {
                        t_x <= t_y // smallest valid t = first crossed
                    } else if valid_x {
                        true
                    } else if valid_y {
                        false
                    } else {
                        x.wrapping_sub(lx).wrapping_abs() >= y.wrapping_sub(ly).wrapping_abs()
                    }
                } else if x_outside || y_outside {
                    x_outside
                } else {
                    exit_axis == 0 // outside a non-rectangular region hole
                };

                if use_x {
                    exit_axis = 0;
                    exit_to_low = last_left;
                } else {
                    exit_axis = 1;
                    exit_to_low = last_above;
                }
            }

            // Monitor-aware resolution: return to the usable monitor the move came from when Last is a legal
            // point outside this region, else the nearest legal usable monitor.
            if !self.config.monitors.is_empty() {
                let entry = if last_outside_region && !self.inside_forbidden(lx, ly) {
                    self.try_active_monitor(lx, ly)
                } else {
                    None
                };
                if let Some(entry_monitor) = entry {
                    let pa = preferred_axis(entry_monitor, x, y, exit_axis);
                    // Wall feel: clamp only the crossing axis; keep the entry coordinate on the orthogonal axis.
                    let desired_x = if pa == 0 { x } else { lx };
                    let desired_y = if pa == 1 { y } else { ly };
                    if let Some((ex, ey, _)) = self.try_legal_point_on_monitor(entry_monitor, desired_x, desired_y, pa)
                    {
                        p.x = ex;
                        p.y = ey;
                        return true;
                    }
                }

                if let Some((nx, ny)) = self.try_nearest_legal_monitor_point(x, y, None, false) {
                    p.x = nx;
                    p.y = ny;
                    return true;
                }
                // Malformed synthetic data can cover every usable pixel; keep the bounded geometric fallback.
            }

            // March to the far edge of the whole connected region.
            if exit_axis == 0 {
                p.x = if exit_to_low { reg_l.wrapping_sub(1) } else { reg_r };
            } else {
                p.y = if exit_to_low { reg_t.wrapping_sub(1) } else { reg_b };
            }
            moved = true;
        }
        moved
    }

    /// C# `TryLegalPointOnMonitor`: the closest legal pixel on one monitor from a finite candidate grid (desired
    /// coordinate, monitor edges, every forbidden outside edge). `preferred_axis` 0/1 minimizes the orthogonal
    /// movement first; -1 uses squared distance. Returns (x, y, squared distance of the winner).
    fn try_legal_point_on_monitor(
        &mut self,
        monitor: Rect,
        desired_x: i32,
        desired_y: i32,
        preferred_axis: i32,
    ) -> Option<(i32, i32, i128)> {
        if monitor.r <= monitor.l || monitor.b <= monitor.t {
            return None;
        }
        let mut xs = std::mem::take(&mut self.scratch.xs);
        let mut ys = std::mem::take(&mut self.scratch.ys);
        xs.clear();
        ys.clear();
        let add = |v: &mut Vec<i32>, value: i64, lo: i32, hi: i32| {
            let c = value.clamp(i64::from(lo), i64::from(hi) - 1);
            v.push(c as i32);
        };
        add(&mut xs, i64::from(desired_x), monitor.l, monitor.r);
        add(&mut xs, i64::from(monitor.l), monitor.l, monitor.r);
        add(&mut xs, i64::from(monitor.r) - 1, monitor.l, monitor.r);
        add(&mut ys, i64::from(desired_y), monitor.t, monitor.b);
        add(&mut ys, i64::from(monitor.t), monitor.t, monitor.b);
        add(&mut ys, i64::from(monitor.b) - 1, monitor.t, monitor.b);
        for f in &self.config.forbidden {
            add(&mut xs, i64::from(f.l) - 1, monitor.l, monitor.r);
            add(&mut xs, i64::from(f.r), monitor.l, monitor.r);
            add(&mut ys, i64::from(f.t) - 1, monitor.t, monitor.b);
            add(&mut ys, i64::from(f.b), monitor.t, monitor.b);
        }
        // SortedSet<int>: ascending, unique.
        xs.sort_unstable();
        xs.dedup();
        ys.sort_unstable();
        ys.dedup();

        let mut best: Option<(i32, i32)> = None;
        let mut best_distance = i128::MAX;
        let (mut best_first, mut best_second) = (i64::MAX, i64::MAX);
        for &cx in &xs {
            for &cy in &ys {
                if self.inside_forbidden(cx, cy) {
                    continue;
                }
                let dx = (i64::from(cx) - i64::from(desired_x)).abs();
                let dy = (i64::from(cy) - i64::from(desired_y)).abs();
                let distance = i128::from(dx) * i128::from(dx) + i128::from(dy) * i128::from(dy);
                let (first, second) = match preferred_axis {
                    0 => (dy, dx),
                    1 => (dx, dy),
                    _ => (0, 0),
                };
                let better = match best {
                    None => true,
                    Some((bx, by)) => {
                        if preferred_axis >= 0 {
                            first < best_first
                                || first == best_first
                                    && (second < best_second
                                        || second == best_second && (cx < bx || cx == bx && cy < by))
                        } else {
                            distance < best_distance || distance == best_distance && (cx < bx || cx == bx && cy < by)
                        }
                    }
                };
                if !better {
                    continue;
                }
                best = Some((cx, cy));
                best_distance = distance;
                best_first = first;
                best_second = second;
            }
        }
        self.scratch.xs = xs;
        self.scratch.ys = ys;
        best.map(|(x, y)| (x, y, best_distance))
    }

    /// C# `TryNearestLegalMonitorPoint`: nearest legal pixel over the authorized usable monitors; ties by monitor
    /// (L, T, R, B), then X, then Y, never list order.
    fn try_nearest_legal_monitor_point(
        &mut self,
        desired_x: i32,
        desired_y: i32,
        transition: Option<usize>,
        transition_passed: bool,
    ) -> Option<(i32, i32)> {
        let old = self.nearest_legal_monitor_point_inner(desired_x, desired_y, transition, transition_passed, false);
        if self.fixes.j12_corner_void && transition.is_none() {
            let fixed =
                self.nearest_legal_monitor_point_inner(desired_x, desired_y, transition, transition_passed, true);
            if fixed.is_some() && fixed != old {
                self.fix_hits.set(self.fix_hits.get() | HIT_J12);
            }
            return fixed.or(old);
        }
        old
    }

    fn freely_reachable(&mut self, monitor: Rect) -> bool {
        if self.config.transitions.is_empty() || !self.have_last {
            return true;
        }
        let Some(source) = self.try_active_monitor(self.last_x, self.last_y) else {
            return true;
        };
        if source == monitor {
            return true;
        }
        let mut reached = std::mem::take(&mut self.scratch.landing);
        reached.clear();
        reached.push(source);
        let mut next = 0;
        let mut source_has_transition = false;
        while next < reached.len() {
            let at = reached[next];
            next += 1;
            for edge in &self.config.transitions {
                if edge.from != at {
                    continue;
                }
                if at == source {
                    source_has_transition = true;
                }
                if effective_behavior(edge, self.gates_enabled_for_decision, self.side_contain_for_decision)
                    == Behavior::Free
                    && !reached.contains(&edge.to)
                {
                    reached.push(edge.to);
                }
            }
        }
        let found = reached.contains(&monitor) || !source_has_transition;
        self.scratch.landing = reached;
        found
    }

    fn nearest_legal_monitor_point_inner(
        &mut self,
        desired_x: i32,
        desired_y: i32,
        transition: Option<usize>,
        transition_passed: bool,
        free_only: bool,
    ) -> Option<(i32, i32)> {
        let mut best: Option<(i32, i32, i128, Rect)> = None;
        for mi in 0..self.config.monitors.len() {
            let monitor = self.config.monitors[mi];
            if free_only && !self.freely_reachable(monitor) {
                continue;
            }
            let authorized = match transition {
                Some(ti) => self.rect_is_authorized(monitor, ti, transition_passed),
                None => self.rect_is_globally_authorized_from_last(monitor),
            };
            if !authorized {
                continue;
            }
            let Some((cx, cy, distance)) = self.try_legal_point_on_monitor(monitor, desired_x, desired_y, -1) else {
                continue;
            };
            let better = match best {
                None => true,
                Some((bx, by, bd, bm)) => {
                    let rect_order = monitor.cmp_ltrb(&bm);
                    distance < bd
                        || distance == bd
                            && (rect_order.is_lt() || rect_order.is_eq() && (cx < bx || cx == bx && cy < by))
                }
            };
            if better {
                best = Some((cx, cy, distance, monitor));
            }
        }
        best.map(|(x, y, _, _)| (x, y))
    }
}

/// C# `InSameGate`: both ends inside the same user gate.
fn intent_config(mut config: IntentConfig, fixes: Fixes) -> IntentConfig {
    if matches!(config.model, CrossingModel::Classic1k | CrossingModel::Intent) && !fixes.j13_classic_1k
        || config.model == CrossingModel::StopThenPush && !fixes.j14_stop_then_push
    {
        config.model = CrossingModel::Classic;
    }
    config
}

fn in_same_gate(x: i32, last_x: i32, gates: &[(i32, i32)]) -> bool {
    gates
        .iter()
        .any(|&(min, max)| x >= min && x < max && last_x >= min && last_x < max)
}

/// C# `HoldOnSource`: clamp onto the source rect and hold back from the crossed edge.
fn hold_on_source(t: &RuntimeTransition, x: i32, y: i32, b: &mut Pt) {
    let hold_x = t
        .edge_holdback_px
        .min(0.max(t.from.r.wrapping_sub(t.from.l).wrapping_sub(1)));
    let hold_y = t
        .edge_holdback_px
        .min(0.max(t.from.b.wrapping_sub(t.from.t).wrapping_sub(1)));
    b.x = clamp(x, t.from.l, t.from.r.wrapping_sub(1));
    b.y = clamp(y, t.from.t, t.from.b.wrapping_sub(1));
    match t.direction {
        Direction::Up => b.y = t.from.t.wrapping_add(hold_y),
        Direction::Down => b.y = t.from.b.wrapping_sub(1).wrapping_sub(hold_y),
        Direction::Left => b.x = t.from.l.wrapping_add(hold_x),
        Direction::Right => b.x = t.from.r.wrapping_sub(1).wrapping_sub(hold_x),
    }
}

impl Decider for ClassicEngine {
    fn raw_motion(&mut self, dx: i32, dy: i32, buttons: u16, qpc_us: u64) {
        self.intent.raw(dx, dy, buttons, qpc_us);
    }
    fn edge_state(&self) -> EdgeState {
        let mut state = EdgeState {
            features: self.intent.features(),
            estimated_rate: self.estimated_rate(),
            model_state: self.intent_state(),
            ..EdgeState::default()
        };
        let Some((ox, oy)) = self.trace_origin else {
            return state;
        };
        let source = self.try_active_monitor(ox, oy);
        let landing = self.try_active_monitor(self.trace_result.x, self.trace_result.y);
        let attempted = self.try_active_monitor(self.current_event.x, self.current_event.y);
        for t in &self.config.transitions {
            if effective_behavior(t, self.gates_enabled_for_decision, self.side_contain_for_decision) == Behavior::Free
            {
                continue;
            }
            let plane = match t.direction {
                Direction::Up => t.from.t,
                Direction::Down => t.from.b,
                Direction::Left => t.from.l,
                Direction::Right => t.from.r,
            };
            for (x, y) in [(ox, oy), (self.current_event.x, self.current_event.y)] {
                let (axis, orth) = if t.direction.is_vertical() { (y, x) } else { (x, y) };
                if (i64::from(axis) - i64::from(plane)).abs() <= 64
                    && i64::from(orth) >= i64::from(t.corridor_start) - 64
                    && i64::from(orth) < i64::from(t.corridor_end) + 64
                {
                    state.near = true;
                }
            }
            if source.is_some()
                && landing.is_some()
                && source != landing
                && trace_segment_exits(t, (ox, oy), (self.trace_result.x, self.trace_result.y))
            {
                state.crossed = true;
            }
            if source.is_some()
                && attempted.is_some()
                && source != attempted
                && trace_segment_exits(t, (ox, oy), (self.current_event.x, self.current_event.y))
            {
                state.attempt_crossed = true;
            }
        }
        state
    }
    fn set_last(&mut self, x: i32, y: i32) {
        ClassicEngine::set_last(self, x, y);
    }

    fn observe(&mut self, x: i32, y: i32) {
        if self.fixes.j10_observed_origin {
            self.set_last(x, y);
            self.on_top = self.config.has_top && y < self.config.barrier_y;
        }
    }

    fn needs_processing(&self) -> bool {
        let c = &self.config;
        c.has_top || self.confine || !c.forbidden.is_empty() || c.has_sides || !c.transitions.is_empty()
    }

    fn decide(&mut self, event: MoveEvent, toggles: Toggles) -> Decision {
        self.decide_event(event, toggles)
    }

    fn reset(&mut self) {
        self.have_last = false;
        self.on_top = false;
        self.intent.reset();
    }

    fn set_confine(&mut self, on: bool) {
        self.confine = on;
    }

    fn confine(&self) -> bool {
        self.confine
    }
}

/// A segment must actually occupy the source rectangle before leaving this directed corridor.
/// This also recognizes guarded second seams after a Free first seam, without flagging unrelated exits.
fn trace_segment_exits(t: &RuntimeTransition, origin: (i32, i32), target: (i32, i32)) -> bool {
    if t.gap_px > t.seam_tolerance_px && !t.to.contains(target.0, target.1) {
        return false;
    }
    let beyond = match t.direction {
        Direction::Up => target.1 < t.from.t,
        Direction::Down => target.1 >= t.from.b,
        Direction::Left => target.0 < t.from.l,
        Direction::Right => target.0 >= t.from.r,
    };
    if !beyond {
        return false;
    }
    let (axis, orth, delta, orth_delta, plane) = if t.direction.is_vertical() {
        (
            origin.1,
            origin.0,
            i64::from(target.1) - i64::from(origin.1),
            i64::from(target.0) - i64::from(origin.0),
            if t.direction == Direction::Up {
                t.from.t
            } else {
                t.from.b
            },
        )
    } else {
        (
            origin.0,
            origin.1,
            i64::from(target.0) - i64::from(origin.0),
            i64::from(target.1) - i64::from(origin.1),
            if t.direction == Direction::Left {
                t.from.l
            } else {
                t.from.r
            },
        )
    };
    if delta == 0 {
        return false;
    }
    let hit = (i64::from(plane) - i64::from(axis)) as f64 / delta as f64;
    if !(0.0..=1.0).contains(&hit) {
        return false;
    }
    let orth = orth as f64 + orth_delta as f64 * hit;
    if orth < t.corridor_start as f64 || orth >= t.corridor_end as f64 {
        return false;
    }
    let (mut enter, mut exit) = (f64::NEG_INFINITY, f64::INFINITY);
    for (start, end, lo, hi) in [
        (origin.0, target.0, t.from.l, t.from.r),
        (origin.1, target.1, t.from.t, t.from.b),
    ] {
        let d = i64::from(end) - i64::from(start);
        if d == 0 {
            if start < lo || start >= hi {
                return false;
            }
            continue;
        }
        let a = (i64::from(lo) - i64::from(start)) as f64 / d as f64;
        let b = (i64::from(hi) - i64::from(start)) as f64 / d as f64;
        enter = enter.max(a.min(b));
        exit = exit.min(a.max(b));
    }
    enter < exit && hit <= exit && (hit > enter.max(0.0) || hit == 0.0 && t.from.contains(origin.0, origin.1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fence_types::Rect;

    #[test]
    fn s_engine_needs_processing_matches_csharp_rule() {
        // C# GuardCore.NeedsProcessing: HasTop || Confine || Forbidden.Count > 0 || HasSides || Transitions.Length > 0.
        let mut e = ClassicEngine::new(EngineConfig::default());
        assert!(!e.needs_processing());
        e.set_confine(true);
        assert!(e.needs_processing());
        e.set_confine(false);
        e.configure(EngineConfig {
            forbidden: vec![Rect::new(0, 0, 10, 10)],
            ..EngineConfig::default()
        });
        assert!(e.needs_processing());
        e.configure(EngineConfig {
            has_sides: true,
            ..EngineConfig::default()
        });
        assert!(e.needs_processing());
        e.configure(EngineConfig {
            has_top: true,
            ..EngineConfig::default()
        });
        assert!(e.needs_processing());
    }

    #[test]
    fn s_engine_configure_resets_state() {
        let mut e = ClassicEngine::new(EngineConfig::default());
        e.have_last = true;
        e.on_top = true;
        e.configure(EngineConfig::default());
        assert!(!e.have_last && !e.on_top);
    }

    #[test]
    fn s_engine_clamp_never_panics() {
        assert_eq!(clamp(5, 0, 9), 5);
        assert_eq!(clamp(-5, 0, 9), 0);
        assert_eq!(clamp(50, 0, 9), 9);
        // min > max: C# throws; the engine never builds such a call, and the helper must not panic either.
        let _ = clamp(5, 9, 0);
    }
}
