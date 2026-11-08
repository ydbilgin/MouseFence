//! Shared helpers of the engine reference-diff tests (Track A): run a `run` or `core` case through the Rust engine
//! with exactly the reference's semantics (crates/fence-trace/src/case.rs) and render the Rust plan in the reference's
//! JSON shape, so the two answers compare as plain JSON values.
#![allow(dead_code)]

use fence_engine::compile::{compile_with, validate_no_seal_with};
use fence_engine::decide::ClassicEngine;
use fence_engine::fixes::Fixes;
use fence_engine::plan::{PlanDetail, build_detail_with};
use fence_engine::topology::SafetyGate;
use fence_trace::case::{CoreCase, CoreOp, Op, OpResult, RunCase, WireCore, WireGraphScreen};
use fence_trace::corpus::monitors_like_csharp;
use fence_types::{
    Action, Behavior, CompileResult, Control, Decider, EngineConfig, MonitorInfo, Policy, Rect, RuntimeTransition,
    Screen, SealIssue,
};
use serde_json::{Value, json};

pub fn r(rect: Rect) -> Value {
    json!([rect.l, rect.t, rect.r, rect.b])
}

pub fn rect_of(a: [i32; 4]) -> Rect {
    Rect::new(a[0], a[1], a[2], a[3])
}

fn behavior_name(b: Behavior) -> &'static str {
    match b {
        Behavior::Free => "Free",
        Behavior::Push => "Push",
        Behavior::Blocked => "Blocked",
    }
}

fn control_name(c: Control) -> &'static str {
    match c {
        Control::Automatic => "Automatic",
        Control::Always => "Always",
    }
}

/// The reference's `T(RuntimeTransition)` JSON.
pub fn transition_json(t: &RuntimeTransition) -> Value {
    json!({
        "from": t.from_stable_id, "to": t.to_stable_id, "dir": t.direction.name(),
        "from_rect": r(t.from), "to_rect": r(t.to),
        "cs": t.corridor_start, "ce": t.corridor_end, "gap": t.gap_px, "tol": t.seam_tolerance_px,
        "behavior": behavior_name(t.behavior), "control": control_name(t.control),
        "push_min": t.push_min, "push_slack": t.push_slack, "inset": t.passage_inset_px,
        "holdback": t.edge_holdback_px, "explicit": t.explicit_rule,
        "sole_up": t.sole_up_safety_escape, "demoted": t.safety_demoted, "sort": t.sort_order,
        "landings": t.allowed_landings.iter().map(|set| set.iter().map(|x| r(*x)).collect::<Vec<_>>()).collect::<Vec<_>>(),
    })
}

/// The reference's `Issue(BoundaryValidationIssue)` JSON.
pub fn issue_json(i: &SealIssue) -> Value {
    json!({"id": i.stable_id, "focus_to": i.focus_to_stable_id, "focus_dir": i.focus_direction.name(),
           "focus_width": i.focus_corridor_width})
}

/// The reference's `plan` JSON for a Rust plan.
pub fn plan_json(d: &PlanDetail, all: &[MonitorInfo]) -> Value {
    let c = &d.config;
    json!({
        "monitors": all.iter().map(|m| json!({"id": m.stable_id, "device": m.device, "index": m.index,
                                              "primary": m.primary, "rect": r(m.bounds)})).collect::<Vec<_>>(),
        "excluded": d.report.excluded,
        "pruned": d.report.pruned,
        "tops": d.tops,
        "has_top": c.has_top,
        "barrier_y": c.barrier_y,
        "gates": c.gates.iter().map(|g| json!([g.0, g.1])).collect::<Vec<_>>(),
        "usable": c.monitors.iter().map(|m| r(*m)).collect::<Vec<_>>(),
        "transitions": c.transitions.iter().map(transition_json).collect::<Vec<_>>(),
        "sealed": d.report.sealed.iter().map(issue_json).collect::<Vec<_>>(),
        "has_safety_demotions": d.report.has_safety_demotions,
        "routes": c.descent_routes.iter().map(|(t, l)| json!([r(*t), r(*l)])).collect::<Vec<_>>(),
        "forbidden": c.forbidden.iter().map(|f| r(*f)).collect::<Vec<_>>(),
        "has_sides": c.has_sides,
        "main": r(c.main),
        "isolated": d.report.isolated,
        "deliberate_cross": c.deliberate_cross,
        "descent_routing": c.descent_routing,
        "side_cross_min": c.side_cross_min,
        "side_cross_slack": c.side_cross_slack,
        "cross_min_up": c.cross_min_up,
        "cross_slack": c.cross_slack,
    })
}

pub fn compile_json(c: &CompileResult) -> (Value, Value, Value) {
    (
        Value::Array(c.transitions.iter().map(transition_json).collect()),
        Value::Array(c.sealed_screens.iter().map(issue_json).collect()),
        Value::Bool(c.has_safety_demotions()),
    )
}

/// Counts of what a set of runs exercised (printed by the corpus tests).
#[derive(Default, Debug, Clone, Copy)]
pub struct Tally {
    pub cases: usize,
    pub ops: usize,
    pub moves: usize,
    pub blocks: usize,
    pub passes: usize,
    pub unprocessed: usize,
}

impl Tally {
    pub fn add(&mut self, results: &[OpResult]) {
        self.cases += 1;
        for r in results {
            self.ops += 1;
            match r.act.as_str() {
                "Block" => {
                    self.moves += 1;
                    self.blocks += 1
                }
                "Pass" => {
                    self.moves += 1;
                    self.passes += 1
                }
                "Unprocessed" => {
                    self.moves += 1;
                    self.unprocessed += 1
                }
                _ => {}
            }
        }
    }
}

pub struct RustRun {
    pub monitors: Vec<MonitorInfo>,
    pub detail: PlanDetail,
    pub plan: Value,
    pub results: Vec<OpResult>,
    /// `fixes::HIT_*` bits of the compile behind the plan.
    pub plan_hits: u16,
    /// `fixes::HIT_*` bits of each op's decision (0 for ops that decide nothing).
    pub op_hits: Vec<u16>,
}

/// The plan with the given judged fixes, and the fix bits that changed it.
pub fn plan_with(policy: &Policy, monitors: &[MonitorInfo], fixes: Fixes) -> (PlanDetail, u16) {
    build_detail_with(policy, monitors, fixes)
}

/// The reference's `run` semantics over the Rust engine (`plan::build_detail` + `ClassicEngine`), with the given
/// judged fixes (`Fixes::EXACT_1_1_1` must match the reference exactly).
pub fn rust_run(case: &RunCase, policy: &Policy, fixes: Fixes) -> RustRun {
    let monitors = monitors_like_csharp(&case.screens);
    let (detail, plan_hits) = plan_with(policy, &monitors, fixes);
    let plan = plan_json(&detail, &monitors);
    let mut e = ClassicEngine::with_fixes(detail.config.clone(), fixes);
    let mut gate = case.toggles.gate_open;
    let mut side = case.toggles.side_contain;
    e.set_confine(case.toggles.confine);
    let mut running = true;
    let mut results = Vec::with_capacity(case.ops.len());
    let mut op_hits = Vec::with_capacity(case.ops.len());
    for (i, op) in case.ops.iter().enumerate() {
        let (mut act, mut x, mut y) = ("Ok", 0, 0);
        let mut hits = 0;
        match *op {
            Op::Move { x: tx, y: ty } => {
                (x, y) = (tx, ty);
                if !running || !e.needs_processing() {
                    act = "Unprocessed";
                } else {
                    let d = e.decide_xy(tx, ty, gate, side);
                    hits = e.last_fix_hits();
                    if d.action == Action::Block {
                        act = "Block";
                        (x, y) = (d.x, d.y);
                    } else {
                        act = "Pass";
                        assert_eq!((d.x, d.y), (tx, ty), "{}: a Pass must report its own target", case.id);
                    }
                }
            }
            Op::Gate { on } => gate = on,
            Op::Side { on } => side = on,
            Op::Confine { on } => e.set_confine(on),
            Op::Pause => running = false,
            Op::Resume => {
                if !running {
                    running = true;
                    e.reset();
                }
            }
            Op::Reset => e.reset(),
        }
        op_hits.push(hits);
        results.push(OpResult {
            i,
            act: act.to_string(),
            x,
            y,
            on_top: e.on_top,
            have_last: e.have_last,
        });
    }
    RustRun {
        monitors,
        detail,
        plan,
        results,
        plan_hits,
        op_hits,
    }
}

pub fn engine_config_of(c: &WireCore) -> (EngineConfig, Vec<SafetyGate>) {
    (
        EngineConfig {
            intent: Default::default(),
            has_top: c.has_top,
            barrier_y: c.barrier_y,
            gates: c.gates.iter().map(|g| (g[0], g[1])).collect(),
            monitors: c.monitors.iter().map(|m| rect_of(*m)).collect(),
            transitions: Vec::new(),
            descent_routes: c
                .descent_routes
                .iter()
                .map(|p| (rect_of(p[0]), rect_of(p[1])))
                .collect(),
            forbidden: c.forbidden.iter().map(|f| rect_of(*f)).collect(),
            has_sides: c.has_sides,
            main: rect_of(c.main),
            deliberate_cross: c.deliberate_cross,
            descent_routing: c.descent_routing,
            cross_min_up: c.cross_min_up,
            cross_slack: c.cross_slack,
            side_cross_min: c.side_cross_min,
            side_cross_slack: c.side_cross_slack,
        },
        c.safety_gates
            .iter()
            .map(|g| SafetyGate::new(g[0], g[1], g[2], g[3]))
            .collect(),
    )
}

pub fn screens_of(graph: &[WireGraphScreen]) -> Vec<Screen> {
    graph
        .iter()
        .map(|s| Screen {
            stable_id: s.id.clone(),
            bounds: rect_of(s.rect),
            primary: s.primary,
            excluded: s.excluded,
        })
        .collect()
}

pub struct RustCore {
    pub compiled: Option<CompileResult>,
    /// `validate_no_seal` over the graph input (the reference's `validate`).
    pub validate: Option<Vec<SealIssue>>,
    pub needs_processing: bool,
    pub results: Vec<OpResult>,
    /// `fixes::HIT_*` bits of the compile and the validation.
    pub compile_hits: u16,
    /// `fixes::HIT_*` bits of each op's decision.
    pub op_hits: Vec<u16>,
}

/// The compiler and the validator with the given judged fixes, and the fix bits that changed them.
pub fn compile_with_fixes(policy: &Policy, screens: &[Screen], safety: bool, fixes: Fixes) -> (CompileResult, u16) {
    compile_with(policy, screens, safety, fixes)
}

pub fn validate_with_fixes(policy: &Policy, screens: &[Screen], fixes: Fixes) -> (Vec<SealIssue>, u16) {
    validate_no_seal_with(policy, screens, fixes)
}

/// The reference's `core` semantics over the Rust engine with the given judged fixes. `policy` is the graph's
/// settings as a policy (the JSON in the case is built from it with `policy_to_settings_json`).
pub fn rust_core(case: &CoreCase, policy: Option<&Policy>, fixes: Fixes) -> RustCore {
    let (mut config, safety) = engine_config_of(&case.core);
    let default = Policy::default();
    let p = policy.unwrap_or(&default);
    let mut compile_hits = 0;
    let compiled = case.graph.as_ref().map(|g| {
        let (c, hits) = compile_with_fixes(p, &screens_of(&g.screens), g.safety, fixes);
        compile_hits |= hits;
        c
    });
    let validate = case.graph.as_ref().map(|g| {
        let (v, hits) = validate_with_fixes(p, &screens_of(&g.screens), fixes);
        compile_hits |= hits;
        v
    });
    if let Some(c) = &compiled {
        config.transitions = c.transitions.clone();
    }
    let mut e = ClassicEngine::with_fixes(config, fixes);
    e.set_safety_gates(safety);
    e.set_confine(case.core.confine);
    e.reset();
    let needs_processing = e.needs_processing();
    let mut results = Vec::with_capacity(case.ops.len());
    let mut op_hits = Vec::with_capacity(case.ops.len());
    for (i, op) in case.ops.iter().enumerate() {
        let (mut act, mut x, mut y) = ("Ok", 0, 0);
        let mut hits = 0;
        match *op {
            CoreOp::Move {
                x: tx,
                y: ty,
                gate,
                side,
            } => {
                let d = e.decide_xy(tx, ty, gate, side);
                hits = e.last_fix_hits();
                act = if d.action == Action::Block { "Block" } else { "Pass" };
                (x, y) = (d.x, d.y);
            }
            CoreOp::SetLast { x, y } => e.set_last(x, y),
            CoreOp::Confine { on } => e.set_confine(on),
            CoreOp::Reset => e.reset(),
            CoreOp::Set { ref core } => {
                let (fields, safety) = engine_config_of(core);
                let transitions = std::mem::take(&mut e.config_mut().transitions);
                *e.config_mut() = EngineConfig { transitions, ..fields };
                e.set_safety_gates(safety);
                e.set_confine(core.confine);
            }
        }
        op_hits.push(hits);
        results.push(OpResult {
            i,
            act: act.to_string(),
            x,
            y,
            on_top: e.on_top,
            have_last: e.have_last,
        });
    }
    RustCore {
        compiled,
        validate,
        needs_processing,
        results,
        compile_hits,
        op_hits,
    }
}

/// Index of the first op whose result differs (None when equal; the shorter length when one side is longer).
pub fn first_differing_op(reference: &Value, rust: &[OpResult]) -> Option<usize> {
    let reference: Vec<OpResult> = serde_json::from_value(reference.clone()).ok()?;
    reference
        .iter()
        .zip(rust)
        .position(|(o, r)| o != r)
        .or_else(|| (reference.len() != rust.len()).then(|| reference.len().min(rust.len())))
}

/// The fix bits fired at or before op `i` (the fixes that can explain a difference first seen at op `i`).
pub fn hits_up_to(op_hits: &[u16], i: usize) -> u16 {
    op_hits.iter().take(i + 1).fold(0, |a, h| a | h)
}

/// First differing path between two JSON values ("" when equal).
pub fn first_diff(a: &Value, b: &Value, path: &str) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, va) in x {
                match y.get(k) {
                    None => return Some(format!("{path}.{k}: missing on the Rust side")),
                    Some(vb) => {
                        if let Some(d) = first_diff(va, vb, &format!("{path}.{k}")) {
                            return Some(d);
                        }
                    }
                }
            }
            y.keys()
                .find(|k| !x.contains_key(*k))
                .map(|k| format!("{path}.{k}: missing on the reference side"))
        }
        (Value::Array(x), Value::Array(y)) => {
            for (i, (va, vb)) in x.iter().zip(y).enumerate() {
                if let Some(d) = first_diff(va, vb, &format!("{path}[{i}]")) {
                    return Some(d);
                }
            }
            (x.len() != y.len()).then(|| format!("{path}: reference has {} items, Rust {}", x.len(), y.len()))
        }
        _ => (a != b).then(|| format!("{path}: reference {a} vs Rust {b}")),
    }
}

/// Compare the per-op results; returns the first difference.
pub fn first_result_diff(reference: &Value, rust: &[OpResult]) -> Option<String> {
    let reference: Vec<OpResult> = match serde_json::from_value(reference.clone()) {
        Ok(v) => v,
        Err(e) => return Some(format!("reference results do not parse: {e}")),
    };
    for (o, r) in reference.iter().zip(rust) {
        if o != r {
            return Some(format!("op {}: reference {:?} vs Rust {:?}", o.i, o, r));
        }
    }
    (reference.len() != rust.len()).then(|| format!("reference {} results, Rust {}", reference.len(), rust.len()))
}
