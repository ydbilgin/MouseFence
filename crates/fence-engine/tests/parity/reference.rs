//! Reference replay of every parity scenario: each scenario runs again (its own assertions included, on the 2.0
//! engine), and every core and helper call it recorded goes through the C# 1.1.1 engine (tools/reference-cs). The
//! exact engine (`Fixes::EXACT_1_1_1`) must agree with C# op by op and compiled transition by transition; the
//! 2.0 engine may differ only where a judged fix reported that it fired. FAILS, never skips, when the reference is
//! missing.

use crate::all_scenarios;
use crate::engine_common::{
    RustCore, compile_json, first_diff, first_differing_op, first_result_diff, hits_up_to, issue_json, rust_core,
};
use crate::rig::{LEGACY_PARITY, Recorded, take_recorded};
use fence_engine::fixes::Fixes;
use fence_trace::reference::Reference;
use serde_json::Value;

/// The first difference of the compiled graph (transitions, sealed screens, demotion flag, validator) between the
/// reference's answer and a Rust run (None for a core without a graph).
pub fn graph_diff(answer: &Value, rust: &RustCore) -> Option<String> {
    let c = rust.compiled.as_ref()?;
    let (transitions, sealed, demotions) = compile_json(c);
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

#[test]
#[ignore = "needs MF_REFERENCE_EXE (tools/reference-cs)"]
fn s_engine_reference_replays_every_parity_scenario() {
    let mut o = Reference::from_env().unwrap_or_else(|e| panic!("{e} (needs MF_REFERENCE_EXE)"));
    let (mut cores, mut ops, mut helpers) = (0usize, 0usize, 0usize);
    let mut silent = Vec::new();
    let mut fails = Vec::new();
    let mut explained = Vec::new();
    let scenarios = all_scenarios();
    for (name, run) in &scenarios {
        take_recorded();
        run();
        let recorded = take_recorded();
        if recorded.is_empty() {
            silent.push(*name);
        }
        for rec in recorded {
            match rec {
                Recorded::Core {
                    case,
                    policy,
                    results,
                    compiled,
                    validate,
                } => {
                    cores += 1;
                    ops += case.ops.len();
                    // Reproduce the original fixture mode, then independently judge the shipped ALL mode.
                    let recorded_mode = rust_core(&case, policy.as_ref(), LEGACY_PARITY);
                    assert_eq!(recorded_mode.results, results, "{name}: the recording does not replay");
                    assert_eq!(
                        recorded_mode.compiled, compiled,
                        "{name}: the recorded compile does not replay"
                    );
                    assert_eq!(
                        recorded_mode.validate, validate,
                        "{name}: the recorded validation does not replay"
                    );
                    let fixed = rust_core(&case, policy.as_ref(), Fixes::ALL);
                    let exact = rust_core(&case, policy.as_ref(), Fixes::EXACT_1_1_1);
                    let answer = o.call_typed(&case).unwrap_or_else(|e| panic!("{name}: {e}"));
                    if answer["needs_processing"] != Value::Bool(exact.needs_processing) {
                        fails.push(format!("{name}: needs_processing differs"));
                    }
                    if let Some(d) = graph_diff(&answer, &exact) {
                        fails.push(format!("{name}: exact: {d}"));
                    }
                    if let Some(d) = first_result_diff(&answer["results"], &exact.results) {
                        fails.push(format!("{name}: exact: {d}"));
                    }
                    let fixed_diff = graph_diff(&answer, &fixed)
                        .map(|d| (d, fixed.compile_hits))
                        .or_else(|| {
                            first_differing_op(&answer["results"], &fixed.results).map(|i| {
                                let d = first_result_diff(&answer["results"], &fixed.results).unwrap_or_default();
                                (d, fixed.compile_hits | hits_up_to(&fixed.op_hits, i))
                            })
                        });
                    match fixed_diff {
                        Some((d, 0)) => fails.push(format!("{name}: 2.0 differs without a judged fix: {d}")),
                        Some((d, hits)) => explained.push(format!("{name}: fix bits {hits:#04b}: {d}")),
                        None => {}
                    }
                }
                Recorded::Helper { request, rust } => {
                    helpers += 1;
                    let answer = o.call(&request).unwrap_or_else(|e| panic!("{name}: {e}"));
                    if let Some(d) = first_diff(&answer["result"], &rust, "result") {
                        fails.push(format!("{name}: helper {}: {d}", request["fn"]));
                    }
                }
            }
        }
    }
    eprintln!(
        "reference replay: {} scenarios, {cores} cores with {ops} ops, {helpers} helper calls; {} differences of the \
         exact engine or unexplained; {} 2.0 differences by a judged fix",
        scenarios.len(),
        fails.len(),
        explained.len()
    );
    for f in fails.iter().chain(&explained).take(20) {
        eprintln!("  {f}");
    }
    assert!(
        silent.is_empty(),
        "scenarios that recorded nothing for the reference: {silent:?}"
    );
    assert!(cores > 0 && scenarios.len() > 100, "the replay ran too little");
    assert!(fails.is_empty(), "{} differences from the C# reference", fails.len());
}
