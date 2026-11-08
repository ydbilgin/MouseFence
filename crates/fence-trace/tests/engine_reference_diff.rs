//! Engine reference diff (Track A): the seeded corpora through the C# 1.1.1 engine (tools/reference-cs) and through
//! fence-engine, compared plan field by plan field and op by op, in two modes per case:
//! - exact (`Fixes::EXACT_1_1_1`): every case must agree with 1.1.1, no exception;
//! - 2.0 (`Fixes::ALL`, what ships): a case may differ only where a judged fix (parity.tsv J rows) reported that
//!   it changed the compile or a decision at or before the first differing op; every such case is counted per fix.
//!
//! Run: `MF_REFERENCE_EXE=<reference exe> cargo test -p fence-trace --offline -- --ignored` (through low_run). The
//! tests FAIL, never skip, when the reference is missing. Each failure names its seed; replay one seed with
//! `MF_ENGINE_SEED=<n>`.

mod engine_common;

use std::collections::BTreeMap;

use engine_common::{
    Tally, compile_json, first_diff, first_differing_op, first_result_diff, hits_up_to, issue_json, rust_core, rust_run,
};
use fence_engine::fixes::Fixes;
use fence_trace::case::OpResult;
use fence_trace::corpus::{core_case, core_graph_case, engine_case};
use fence_trace::reference::Reference;
use serde_json::{Value, json};

const IGNORE: &str = "needs MF_REFERENCE_EXE (tools/reference-cs)";

fn reference() -> Reference {
    Reference::from_env().unwrap_or_else(|e| panic!("{e} ({IGNORE})"))
}

fn seeds(default: std::ops::Range<u64>) -> Vec<u64> {
    match std::env::var("MF_ENGINE_SEED").ok().and_then(|s| s.parse().ok()) {
        Some(one) => vec![one],
        None => default.collect(),
    }
}

/// One corpus run: exact-mode failures, 2.0-mode unexplained failures, 2.0 differences per fix bit set.
#[derive(Default)]
struct Outcome {
    tally: Tally,
    exact_fails: Vec<String>,
    fixed_fails: Vec<String>,
    explained: BTreeMap<u16, usize>,
}

impl Outcome {
    /// A 2.0-mode difference `d` with the fix bits `hits` that may explain it.
    fn fixed_diff(&mut self, seed: u64, d: String, hits: u16) {
        if hits == 0 {
            self.fixed_fails.push(format!("seed {seed}: {d} (no judged fix fired)"));
        } else {
            *self.explained.entry(hits).or_default() += 1;
        }
    }

    fn report(&self, name: &str) {
        let explained: Vec<String> = self
            .explained
            .iter()
            .map(|(bits, n)| format!("{} x{n}", fix_names(*bits)))
            .collect();
        eprintln!(
            "{name}: {} cases, {} ops ({} moves: {} Pass, {} Block, {} Unprocessed); exact 1.1.1: {} differing cases; \
             2.0 fixes: {} cases differ by a judged fix [{}], {} unexplained",
            self.tally.cases,
            self.tally.ops,
            self.tally.moves,
            self.tally.passes,
            self.tally.blocks,
            self.tally.unprocessed,
            self.exact_fails.len(),
            self.explained.values().sum::<usize>(),
            explained.join(", "),
            self.fixed_fails.len()
        );
        for f in self.exact_fails.iter().chain(&self.fixed_fails).take(12) {
            eprintln!("  {f}");
        }
        assert!(self.tally.cases > 0, "{name}: zero cases ran");
        assert!(
            self.exact_fails.is_empty(),
            "{name}: {} cases of the exact engine differ from the C# reference; first: {}",
            self.exact_fails.len(),
            self.exact_fails[0]
        );
        assert!(
            self.fixed_fails.is_empty(),
            "{name}: {} cases of the 2.0 engine differ without a judged fix; first: {}",
            self.fixed_fails.len(),
            self.fixed_fails[0]
        );
    }
}

fn fix_names(bits: u16) -> String {
    use fence_engine::fixes::{HIT_J01, HIT_J05, HIT_J06, HIT_J09, HIT_J11, HIT_J12, HIT_J13, HIT_J14, HIT_J15};
    let names: Vec<&str> = [
        (HIT_J01, "J01"),
        (HIT_J05, "J05"),
        (HIT_J06, "J06"),
        (HIT_J09, "J09"),
        (HIT_J11, "J11"),
        (HIT_J12, "J12"),
        (HIT_J13, "J13"),
        (HIT_J14, "J14"),
        (HIT_J15, "J15"),
    ]
    .iter()
    .filter(|(b, _)| bits & b != 0)
    .map(|(_, n)| *n)
    .collect();
    names.join("+")
}

/// The comparison itself is not vacuous: a changed field, a missing key, a changed op and a shorter answer
/// are all reported.
#[test]
fn s_trace_engine_diff_reports_every_kind_of_difference() {
    let a = json!({"x": [1, 2, {"k": true}], "y": "s"});
    assert_eq!(first_diff(&a, &a.clone(), "plan"), None);
    let changed = json!({"x": [1, 2, {"k": false}], "y": "s"});
    assert!(first_diff(&a, &changed, "plan").unwrap().contains("plan.x[2].k"));
    assert!(
        first_diff(&a, &json!({"x": [1, 2, {"k": true}]}), "plan")
            .unwrap()
            .contains("missing on the Rust side")
    );
    assert!(
        first_diff(&a, &json!({"x": [1, 2], "y": "s"}), "plan")
            .unwrap()
            .contains("reference has 3 items, Rust 2")
    );
    let op = |i: usize, act: &str, x: i32| OpResult {
        i,
        act: act.to_string(),
        x,
        y: 0,
        on_top: false,
        have_last: true,
    };
    let reference = serde_json::to_value(vec![op(0, "Pass", 5), op(1, "Block", 7)]).unwrap();
    assert_eq!(
        first_result_diff(&reference, &[op(0, "Pass", 5), op(1, "Block", 7)]),
        None
    );
    assert_eq!(
        first_differing_op(&reference, &[op(0, "Pass", 5), op(1, "Block", 7)]),
        None
    );
    assert!(
        first_result_diff(&reference, &[op(0, "Pass", 5), op(1, "Block", 8)])
            .unwrap()
            .starts_with("op 1")
    );
    assert_eq!(
        first_differing_op(&reference, &[op(0, "Pass", 5), op(1, "Block", 8)]),
        Some(1)
    );
    assert!(
        first_result_diff(&reference, &[op(0, "Pass", 5)])
            .unwrap()
            .contains("2 results")
    );
    assert_eq!(first_differing_op(&reference, &[op(0, "Pass", 5)]), Some(1));
    assert!(
        first_result_diff(&Value::Null, &[]).is_some(),
        "a missing results array is a difference"
    );
    assert_eq!(hits_up_to(&[0, 4, 0, 1], 0), 0);
    assert_eq!(hits_up_to(&[0, 4, 0, 1], 2), 4);
    assert_eq!(hits_up_to(&[0, 4, 0, 1], 3), 5);
}

/// B026: the Configure pipeline and every decision of the `run` corpus (up to 6 screens, 64 ops, settings
/// variations: defaults, numbers, insets, Manual tops, exclusions incl. absent and case-flipped ids, rules)
/// agree with C# 1.1.1 for seeds 0..9999; the 2.0 engine differs only where a judged fix fired.
#[test]
#[ignore = "needs MF_REFERENCE_EXE (tools/reference-cs)"]
fn b026_reference_diff_engine_corpus_seeds_0_to_9999() {
    let mut o = reference();
    let mut out = Outcome::default();
    for seed in seeds(0..10_000) {
        let case = engine_case(seed, 6, 64);
        let answer = o.call_typed(&case.run).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        let exact = rust_run(&case.run, &case.policy, Fixes::EXACT_1_1_1);
        out.tally.add(&exact.results);
        if let Some(d) = first_diff(&answer["plan"], &exact.plan, "plan") {
            out.exact_fails.push(format!("seed {seed}: {d}"));
        } else if let Some(d) = first_result_diff(&answer["results"], &exact.results) {
            out.exact_fails.push(format!("seed {seed}: {d}"));
        }
        let fixed = rust_run(&case.run, &case.policy, Fixes::ALL);
        if let Some(d) = first_diff(&answer["plan"], &fixed.plan, "plan") {
            out.fixed_diff(seed, d, fixed.plan_hits);
        } else if let Some(i) = first_differing_op(&answer["results"], &fixed.results) {
            let d = first_result_diff(&answer["results"], &fixed.results).unwrap_or_default();
            out.fixed_diff(seed, d, fixed.plan_hits | hits_up_to(&fixed.op_hits, i));
        }
    }
    out.report("engine corpus");
}

/// The forbidden-heavy GuardCore corpus (built field by field, the way the 1.1.1 tests build cores): wall-off
/// regions, overlapping clones, safety gates, side containment, descent routes, game mode, seeds 0..4999.
#[test]
#[ignore = "needs MF_REFERENCE_EXE (tools/reference-cs)"]
fn engine_reference_diff_core_corpus_seeds_0_to_4999() {
    let mut o = reference();
    let mut out = Outcome::default();
    for seed in seeds(0..5_000) {
        let case = core_case(seed, 6, 64);
        let answer = o.call_typed(&case).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        let exact = rust_core(&case, None, Fixes::EXACT_1_1_1);
        out.tally.add(&exact.results);
        if answer["needs_processing"] != Value::Bool(exact.needs_processing) {
            out.exact_fails.push(format!("seed {seed}: needs_processing differs"));
        } else if let Some(d) = first_result_diff(&answer["results"], &exact.results) {
            out.exact_fails.push(format!("seed {seed}: {d}"));
        }
        let fixed = rust_core(&case, None, Fixes::ALL);
        if let Some(i) = first_differing_op(&answer["results"], &fixed.results) {
            let d = first_result_diff(&answer["results"], &fixed.results).unwrap_or_default();
            out.fixed_diff(seed, d, hits_up_to(&fixed.op_hits, i));
        }
    }
    out.report("core corpus");
}

/// Compiled-graph differences of one mode (transitions, sealed screens, demotion flag, validator).
fn graph_diff(answer: &Value, rust: &engine_common::RustCore) -> Option<String> {
    let compiled = rust.compiled.as_ref().expect("graph cases compile");
    let (transitions, sealed, demotions) = compile_json(compiled);
    let validate = Value::Array(
        rust.validate
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(issue_json)
            .collect(),
    );
    first_diff(&answer["transitions"], &transitions, "transitions")
        .or_else(|| first_diff(&answer["sealed"], &sealed, "sealed"))
        .or_else(|| first_diff(&answer["has_safety_demotions"], &demotions, "has_safety_demotions"))
        .or_else(|| first_diff(&answer["validate"], &validate, "validate"))
}

/// The compiler over raw settings with excluded screens and both geometry-safety modes (the tests'
/// `GraphCore` path), plus decisions over the compiled transitions, seeds 0..4999.
#[test]
#[ignore = "needs MF_REFERENCE_EXE (tools/reference-cs)"]
fn engine_reference_diff_compiled_graph_corpus_seeds_0_to_4999() {
    let mut o = reference();
    let mut out = Outcome::default();
    for seed in seeds(0..5_000) {
        let (case, policy) = core_graph_case(seed, 6, 64);
        let answer = o.call_typed(&case).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        let exact = rust_core(&case, Some(&policy), Fixes::EXACT_1_1_1);
        out.tally.add(&exact.results);
        if let Some(d) = graph_diff(&answer, &exact) {
            out.exact_fails.push(format!("seed {seed}: {d}"));
        } else if let Some(d) = first_result_diff(&answer["results"], &exact.results) {
            out.exact_fails.push(format!("seed {seed}: {d}"));
        }
        let fixed = rust_core(&case, Some(&policy), Fixes::ALL);
        if let Some(d) = graph_diff(&answer, &fixed) {
            out.fixed_diff(seed, d, fixed.compile_hits);
        } else if let Some(i) = first_differing_op(&answer["results"], &fixed.results) {
            let d = first_result_diff(&answer["results"], &fixed.results).unwrap_or_default();
            out.fixed_diff(seed, d, fixed.compile_hits | hits_up_to(&fixed.op_hits, i));
        }
    }
    out.report("compiled graph corpus");
}
