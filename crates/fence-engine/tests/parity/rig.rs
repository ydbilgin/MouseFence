//! The 1.1.1 test helpers (tests/Tests.cs: `New`, `NewGates`, `NewConfine`, `NewSafety`, `NewForbidden`,
//! `NewMonitorAwareWall`, `NewSides`, `GraphScreens`, `GraphCore`, `SetLast`, `Move`, `InRect`,
//! `IsLegalWallResult`) over the Rust engine.
//!
//! Every core records how it was built and every op with its result. When the core is dropped the recording goes
//! to a per-thread sink; the ignored reference replay (`reference.rs`) runs each scenario, drains the sink and sends
//! each recording through the C# engine (`core` verb), so every scenario is proven twice: by its own assertions
//! on the Rust engine and op by op against 1.1.1.

use std::cell::RefCell;

use fence_engine::compile::{compile_with, validate_no_seal_with};
use fence_engine::decide::ClassicEngine;
use fence_engine::fixes::Fixes;
use fence_engine::topology::SafetyGate;
use fence_trace::case::{CoreCase, CoreOp, OpResult, WireCore, WireGraph, WireGraphScreen};
use fence_trace::corpus::policy_to_settings_json;
use fence_types::{
    Action, BoundaryDefault, BoundaryRule, CompileResult, Control, Decider, Direction, EngineConfig, Policy, Rect,
    RuleBehavior, Screen, SealIssue,
};
use serde_json::{Value, json};

/// GuardCore fields (C# `new GuardCore { ... }`); `Spec::default()` is `new GuardCore()`.
pub type Spec = WireCore;
pub type R4 = (i32, i32, i32, i32);

/// Preserve the original parity fixtures, including their retired mutable safety-gate scaffold.
/// The new keeper suites exercise the shipped ALL mode; the corpus diffs also run ALL independently.
pub const LEGACY_PARITY: Fixes = Fixes {
    j09_full_corridor: false,
    j10_observed_origin: false,
    j11_segment_and_identity: false,
    j12_corner_void: false,
    j13_classic_1k: false,
    j14_stop_then_push: false,
    j15_side_to_top_blocked: false,
    ..Fixes::ALL
};

pub fn rect(r: R4) -> Rect {
    Rect::new(r.0, r.1, r.2, r.3)
}

pub fn arr(r: R4) -> [i32; 4] {
    [r.0, r.1, r.2, r.3]
}

pub fn rects(rs: &[R4]) -> Vec<[i32; 4]> {
    rs.iter().map(|r| arr(*r)).collect()
}

/// C# `InRect`.
pub fn in_rect(r: R4, x: i32, y: i32) -> bool {
    rect(r).contains(x, y)
}

fn r4(a: [i32; 4]) -> R4 {
    (a[0], a[1], a[2], a[3])
}

/// The C# `Move` tuple `(act, by, onTop, bx)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct M {
    pub act: Action,
    pub by: i32,
    pub on_top: bool,
    pub bx: i32,
}

impl M {
    pub fn block(&self) -> bool {
        self.act == Action::Block
    }
    pub fn pass(&self) -> bool {
        self.act == Action::Pass
    }
}

/// One recorded core or helper call, replayed against the reference.
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)] // test-only records, a few hundred per run
pub enum Recorded {
    Core {
        case: CoreCase,
        policy: Option<Policy>,
        results: Vec<OpResult>,
        compiled: Option<CompileResult>,
        validate: Option<Vec<SealIssue>>,
    },
    /// A `helper` verb request and the Rust answer (the reference's `result`).
    Helper { request: Value, rust: Value },
}

thread_local! {
    static SINK: RefCell<Vec<Recorded>> = const { RefCell::new(Vec::new()) };
}

pub fn record(r: Recorded) {
    SINK.with(|s| s.borrow_mut().push(r));
}

pub fn take_recorded() -> Vec<Recorded> {
    SINK.with(|s| std::mem::take(&mut *s.borrow_mut()))
}

fn engine_config(spec: &Spec) -> EngineConfig {
    EngineConfig {
        intent: Default::default(),
        has_top: spec.has_top,
        barrier_y: spec.barrier_y,
        gates: spec.gates.iter().map(|g| (g[0], g[1])).collect(),
        monitors: spec.monitors.iter().map(|m| rect(r4(*m))).collect(),
        transitions: Vec::new(),
        descent_routes: spec
            .descent_routes
            .iter()
            .map(|p| (rect(r4(p[0])), rect(r4(p[1]))))
            .collect(),
        forbidden: spec.forbidden.iter().map(|f| rect(r4(*f))).collect(),
        has_sides: spec.has_sides,
        main: rect(r4(spec.main)),
        deliberate_cross: spec.deliberate_cross,
        descent_routing: spec.descent_routing,
        cross_min_up: spec.cross_min_up,
        cross_slack: spec.cross_slack,
        side_cross_min: spec.side_cross_min,
        side_cross_slack: spec.side_cross_slack,
    }
}

fn safety_gates(spec: &Spec) -> Vec<SafetyGate> {
    spec.safety_gates
        .iter()
        .map(|g| SafetyGate::new(g[0], g[1], g[2], g[3]))
        .collect()
}

/// A GuardCore under test.
pub struct Core {
    pub e: ClassicEngine,
    spec: Spec,
    case: CoreCase,
    policy: Option<Policy>,
    results: Vec<OpResult>,
    compiled: Option<CompileResult>,
    validate: Option<Vec<SealIssue>>,
}

impl Core {
    /// `new GuardCore { ... }` (followed by `Reset()`, which a fresh core does not need).
    pub fn new(spec: Spec) -> Self {
        let mut e = ClassicEngine::with_fixes(engine_config(&spec), LEGACY_PARITY);
        e.set_safety_gates(safety_gates(&spec));
        e.set_confine(spec.confine);
        let case = CoreCase::new("scenario", spec.clone(), Vec::new());
        Self {
            e,
            spec,
            case,
            policy: None,
            results: Vec::new(),
            compiled: None,
            validate: None,
        }
    }

    /// A core whose transitions come from `compile(policy, screens, safety)` (the reference `graph` input).
    pub fn with_graph(spec: Spec, policy: &Policy, screens: &[WireGraphScreen], safety: bool) -> Self {
        let mut c = Self::new(spec);
        let input = screens_of(screens);
        let compiled = compile_with(policy, &input, safety, LEGACY_PARITY).0;
        c.e.config_mut().transitions = compiled.transitions.clone();
        c.validate = Some(validate_no_seal_with(policy, &input, LEGACY_PARITY).0);
        c.compiled = Some(compiled);
        c.policy = Some(policy.clone());
        c.case.graph = Some(WireGraph {
            settings: Some(policy_to_settings_json(policy)),
            screens: screens.to_vec(),
            safety,
        });
        c
    }

    pub fn compiled(&self) -> &CompileResult {
        self.compiled.as_ref().expect("a graph core")
    }

    fn push(&mut self, op: CoreOp, act: &str, x: i32, y: i32) {
        self.case.ops.push(op);
        self.results.push(OpResult {
            i: self.results.len(),
            act: act.to_string(),
            x,
            y,
            on_top: self.e.on_top,
            have_last: self.e.have_last,
        });
    }

    /// C# `Move(c, x, y, gate, side)`.
    pub fn mvs(&mut self, x: i32, y: i32, gate: bool, side: bool) -> M {
        let d = self.e.decide_xy(x, y, gate, side);
        let act = if d.action == Action::Block { "Block" } else { "Pass" };
        self.push(CoreOp::Move { x, y, gate, side }, act, d.x, d.y);
        M {
            act: d.action,
            by: d.y,
            on_top: self.e.on_top,
            bx: d.x,
        }
    }

    /// C# `Move(c, x, y, gate)` (side defaults to true).
    pub fn mv(&mut self, x: i32, y: i32, gate: bool) -> M {
        self.mvs(x, y, gate, true)
    }

    /// C# `SetLast(c, x, y)`.
    pub fn set_last(&mut self, x: i32, y: i32) {
        self.e.set_last(x, y);
        self.push(CoreOp::SetLast { x, y }, "Ok", 0, 0);
    }

    /// C# `c.Reset()`.
    pub fn reset(&mut self) {
        self.e.reset();
        self.push(CoreOp::Reset, "Ok", 0, 0);
    }

    /// C# field assignments between moves (`c.DescentRouting = true; c.DescentRoutes.Add(...)`), no Reset.
    pub fn set(&mut self, f: impl FnOnce(&mut Spec)) {
        f(&mut self.spec);
        let transitions = std::mem::take(&mut self.e.config_mut().transitions);
        *self.e.config_mut() = EngineConfig {
            transitions,
            ..engine_config(&self.spec)
        };
        self.e.set_safety_gates(safety_gates(&self.spec));
        self.e.set_confine(self.spec.confine);
        self.push(
            CoreOp::Set {
                core: Box::new(self.spec.clone()),
            },
            "Ok",
            0,
            0,
        );
    }

    pub fn needs_processing(&self) -> bool {
        self.e.needs_processing()
    }

    pub fn on_top(&self) -> bool {
        self.e.on_top
    }

    pub fn have_last(&self) -> bool {
        self.e.have_last
    }

    /// C# `IsLegalWallResult(c, x, y)`: on a usable monitor and outside every forbidden rect.
    pub fn legal(&self, x: i32, y: i32) -> bool {
        self.spec.monitors.iter().any(|m| in_rect(r4(*m), x, y))
            && !self.spec.forbidden.iter().any(|f| in_rect(r4(*f), x, y))
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        let mut case = self.case.clone();
        case.id = format!("scenario-core-{}", self.results.len());
        record(Recorded::Core {
            case,
            policy: self.policy.clone(),
            results: std::mem::take(&mut self.results),
            compiled: self.compiled.take(),
            validate: self.validate.take(),
        });
    }
}

// ---------------------------------------------------------------- C# helper constructors

/// C# `New()`: HasTop, BarrierY 0, one gate (24, 2536).
pub fn new_core() -> Core {
    Core::new(Spec {
        has_top: true,
        barrier_y: 0,
        gates: vec![[24, 2536]],
        ..Spec::default()
    })
}

/// C# `NewGates(...)`.
pub fn new_gates(gates: &[(i32, i32)]) -> Core {
    Core::new(Spec {
        has_top: true,
        barrier_y: 0,
        gates: gates.iter().map(|g| [g.0, g.1]).collect(),
        ..Spec::default()
    })
}

/// C# `NewConfine(...)`.
pub fn new_confine(mons: &[R4]) -> Core {
    Core::new(Spec {
        has_top: false,
        confine: true,
        monitors: rects(mons),
        ..Spec::default()
    })
}

/// C# `NewSafety(...)`.
pub fn new_safety(safety: &[(i32, i32, i32, i32)]) -> Core {
    Core::new(Spec {
        has_top: true,
        barrier_y: 0,
        safety_gates: safety.iter().map(|g| [g.0, g.1, g.2, g.3]).collect(),
        ..Spec::default()
    })
}

/// C# `NewForbidden(...)`.
pub fn new_forbidden(forbidden: &[R4]) -> Core {
    Core::new(Spec {
        has_top: false,
        confine: false,
        forbidden: rects(forbidden),
        ..Spec::default()
    })
}

/// C# `NewMonitorAwareWall(monitors, forbidden...)`.
pub fn new_monitor_aware_wall(monitors: &[R4], forbidden: &[R4]) -> Core {
    Core::new(Spec {
        has_top: false,
        confine: false,
        forbidden: rects(forbidden),
        monitors: rects(monitors),
        ..Spec::default()
    })
}

/// C# `NewSides()`: synthetic main (0,0,2560,1440) with side screens, upper gate (24, 2536).
pub fn new_sides() -> Core {
    Core::new(Spec {
        has_top: true,
        barrier_y: 0,
        gates: vec![[24, 2536]],
        has_sides: true,
        main: [0, 0, 2560, 1440],
        monitors: rects(&[
            (0, 0, 2560, 1440),
            (-1500, 0, 0, 1050),
            (2560, 0, 4300, 1050),
            (-600, -1000, 2900, 0),
        ]),
        ..Spec::default()
    })
}

/// C# `GraphScreens(("id", l, t, r, b, primary), ...)`.
pub fn graph_screens(items: &[(&str, i32, i32, i32, i32, bool)]) -> Vec<WireGraphScreen> {
    items
        .iter()
        .map(|&(id, l, t, r, b, primary)| WireGraphScreen {
            id: id.to_string(),
            rect: [l, t, r, b],
            primary,
            excluded: false,
        })
        .collect()
}

pub fn screens_of(items: &[WireGraphScreen]) -> Vec<Screen> {
    items
        .iter()
        .map(|s| Screen {
            stable_id: s.id.clone(),
            bounds: rect(r4(s.rect)),
            primary: s.primary,
            excluded: s.excluded,
        })
        .collect()
}

pub fn bounds(s: &WireGraphScreen) -> R4 {
    r4(s.rect)
}

/// C# `GraphCore(settings, screens, hasTop, barrierY)`: compiled transitions (geometry safety on) and the
/// non-excluded rects as monitors.
pub fn graph_core(policy: &Policy, screens: &[WireGraphScreen], has_top: bool, barrier_y: i32) -> Core {
    let spec = Spec {
        has_top,
        barrier_y,
        monitors: screens.iter().filter(|s| !s.excluded).map(|s| s.rect).collect(),
        ..Spec::default()
    };
    Core::with_graph(spec, policy, screens, true)
}

/// `BoundaryGraph.Compile(settings, screens, safety)` with the call recorded for the reference replay.
pub fn compile_rec(policy: &Policy, screens: &[WireGraphScreen], safety: bool) -> CompileResult {
    let c = Core::with_graph(Spec::default(), policy, screens, safety);
    c.compiled().clone()
}

/// `BoundaryGraph.ValidateNoSeal(settings, screens)` with the call recorded for the reference replay.
pub fn validate_rec(policy: &Policy, screens: &[WireGraphScreen]) -> Vec<SealIssue> {
    let c = Core::with_graph(Spec::default(), policy, screens, true);
    c.validate.clone().expect("graph core validates")
}

// ---------------------------------------------------------------- pure helpers, recorded as `helper` requests

/// C# `GuardCore.AntiTrap(mons, topIdx)`.
pub fn anti_trap_rec(mons: &[R4], top_idx: &[usize]) -> (Vec<usize>, Vec<SafetyGate>) {
    let rs: Vec<Rect> = mons.iter().map(|m| rect(*m)).collect();
    let (warn, gates) = fence_engine::topology::anti_trap(&rs, top_idx);
    record(Recorded::Helper {
        request: json!({"verb": "helper", "fn": "AntiTrap", "args": {"rects": rects(mons), "top_idx": top_idx}}),
        rust: json!({"warn": warn, "gates": gates.iter().map(|g| [g.min, g.max, g.owner_t, g.owner_b]).collect::<Vec<_>>()}),
    });
    (warn, gates)
}

/// C# `GuardCore.IsTop(m)` (reference verb `is_top`).
pub fn is_top_rec(m: R4) -> bool {
    let v = fence_engine::topology::is_top(rect(m));
    record(Recorded::Helper {
        request: json!({"verb": "is_top", "rect": arr(m)}),
        rust: json!(v),
    });
    v
}

/// C# `GuardCore.Adjacent(a, b, d)`.
pub fn adjacent_rec(a: R4, b: R4, d: Direction) -> bool {
    let v = fence_engine::topology::adjacent(rect(a), rect(b), d);
    record(Recorded::Helper {
        request: json!({"verb": "helper", "fn": "Adjacent", "args": {"a": arr(a), "b": arr(b), "dir": d.name()}}),
        rust: json!(v),
    });
    v
}

/// C# `GuardCore.DeriveRoutes(links, deviceRects, topDevices)`.
pub fn derive_routes_rec(links: &[(&str, &str)], device_rects: &[(&str, R4)], tops: &[&str]) -> Vec<(R4, R4)> {
    let l: Vec<(String, String)> = links.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    let r: Vec<(String, Rect)> = device_rects.iter().map(|(k, v)| (k.to_string(), rect(*v))).collect();
    let t: Vec<String> = tops.iter().map(|s| s.to_string()).collect();
    let routes = fence_engine::routes::derive_routes(&l, &r, &t);
    let out: Vec<(R4, R4)> = routes
        .iter()
        .map(|(a, b)| ((a.l, a.t, a.r, a.b), (b.l, b.t, b.r, b.b)))
        .collect();
    record(Recorded::Helper {
        request: json!({"verb": "helper", "fn": "DeriveRoutes", "args": {
            "links": links.iter().map(|(a, b)| [a, b]).collect::<Vec<_>>(),
            "rects": device_rects.iter().map(|(k, v)| json!([k, arr(*v)])).collect::<Vec<_>>(),
            "tops": tops,
        }}),
        rust: json!(out.iter().map(|(a, b)| [arr(*a), arr(*b)]).collect::<Vec<_>>()),
    });
    out
}

/// A saved rule (C# `new BoundaryRule { ... }`).
pub fn rule(from: &str, to: &str, direction: Direction, behavior: RuleBehavior, control: Control) -> BoundaryRule {
    BoundaryRule {
        from_stable_id: from.to_string(),
        to_stable_id: to.to_string(),
        direction,
        behavior,
        control,
    }
}

/// `new Settings { SideBoundaryDefault = Free, PassageInsetPx = 0 }`, the most common graph settings.
pub fn side_free_inset0() -> Policy {
    Policy {
        side_default: BoundaryDefault::Free,
        passage_inset_px: 0,
        ..Policy::default()
    }
}
