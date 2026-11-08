//! Reference smoke tests (skeleton). Run with the reference built and MF_REFERENCE_EXE set:
//!   cargo test -p fence-trace --test reference_smoke -- --ignored
//! They FAIL (never skip) when MF_REFERENCE_EXE is unset, so a parity run cannot pass with zero reference cases.

use fence_trace::case::{Op, RunCase, WireToggles, screen};
use fence_trace::reference::Reference;
use serde_json::{Value, json};

fn three_screens() -> Vec<fence_trace::case::WireScreen> {
    // Synthetic: main 1920x1080 at the origin, an equal screen above it, an equal screen to its right.
    vec![
        screen("MONITOR\\SYN1000\\{main}", "\\\\.\\DISPLAY1", 0, 0, 1920, 1080, true),
        screen("MONITOR\\SYN1001\\{top}", "\\\\.\\DISPLAY2", 0, -1080, 1920, 0, false),
        screen(
            "MONITOR\\SYN1002\\{right}",
            "\\\\.\\DISPLAY3",
            1920,
            0,
            3840,
            1080,
            false,
        ),
    ]
}

fn act(results: &Value, i: usize) -> (String, i64, i64) {
    let r = &results[i];
    (
        r["act"].as_str().unwrap_or("?").to_string(),
        r["x"].as_i64().unwrap_or(i64::MIN),
        r["y"].as_i64().unwrap_or(i64::MIN),
    )
}

#[test]
#[ignore = "needs MF_REFERENCE_EXE (tools/reference-cs)"]
fn s_reference_run_up_gate_closed_then_open() {
    let mut reference = Reference::from_env().expect("reference");
    let case = RunCase {
        toggles: WireToggles {
            gate_open: false,
            side_contain: true,
            confine: false,
        },
        ..RunCase::new(
            "smoke-1",
            three_screens(),
            vec![
                Op::Move { x: 960, y: 540 },
                Op::Move { x: 960, y: 2 },
                Op::Move { x: 960, y: -5 },
                Op::Gate { on: true },
                Op::Move { x: 960, y: -5 },
                Op::Move { x: 970, y: -20 },
            ],
        )
    };
    let out = reference.call_typed(&case).expect("run");
    assert_eq!(out["id"], "smoke-1");
    let plan = &out["plan"];
    assert_eq!(plan["has_top"], true);
    assert_eq!(plan["barrier_y"], 0);
    assert_eq!(plan["has_sides"], true);
    assert_eq!(plan["isolated"].as_array().map(|a| a.len()), Some(0));
    // main->top Up, top->main Down, main->right Right, right->main Left.
    assert_eq!(plan["transitions"].as_array().map(|a| a.len()), Some(4), "{plan}");
    let results = &out["results"];
    assert_eq!(
        act(results, 0),
        ("Pass".into(), 960, 540),
        "first move after reset passes"
    );
    assert_eq!(act(results, 1), ("Pass".into(), 960, 2));
    assert_eq!(
        act(results, 2),
        ("Block".into(), 960, 0),
        "Up closed: held on main's top row"
    );
    assert_eq!(results[3]["act"], "Ok");
    assert_eq!(
        act(results, 4),
        ("Pass".into(), 960, -5),
        "Up open: a 5 px push crosses"
    );
    assert_eq!(results[4]["on_top"], true);
    assert_eq!(act(results, 5), ("Pass".into(), 970, -20), "free roam on top");
}

#[test]
#[ignore = "needs MF_REFERENCE_EXE (tools/reference-cs)"]
fn s_reference_pause_and_needs_processing_are_hook_level() {
    let mut reference = Reference::from_env().expect("reference");
    // One screen and no game mode: NeedsProcessing is false, so the hook never calls Decide.
    let single = vec![screen(
        "MONITOR\\SYN2000\\{solo}",
        "\\\\.\\DISPLAY1",
        0,
        0,
        1920,
        1080,
        true,
    )];
    let case = RunCase::new(
        "smoke-2",
        single,
        vec![
            Op::Move { x: 5, y: 5 },
            Op::Confine { on: true },
            Op::Move { x: 6, y: 6 },
            Op::Pause,
            Op::Move { x: 7, y: 7 },
        ],
    );
    let out = reference.call_typed(&case).expect("run");
    let results = &out["results"];
    assert_eq!(results[0]["act"], "Unprocessed");
    assert_eq!(
        results[2]["act"], "Pass",
        "game mode turns processing on; first move passes"
    );
    assert_eq!(results[2]["have_last"], true);
    assert_eq!(results[4]["act"], "Unprocessed", "paused: the hook is gone");
}

#[test]
#[ignore = "needs MF_REFERENCE_EXE (tools/reference-cs)"]
fn s_reference_keys_normalize_and_helpers_answer() {
    let mut reference = Reference::from_env().expect("reference");
    let keys = reference.call(&json!({"verb": "keys"})).expect("keys");
    assert_eq!(keys["names"]["38"], "Up");
    assert_eq!(keys["names"]["71"], "G");
    assert_eq!(keys["names"]["39"], "Right");

    let norm = reference
        .call(&json!({"verb": "normalize", "json": "{\"SettingsVersion\": 4}"}))
        .expect("normalize");
    let text = norm["json"].as_str().expect("json text");
    assert!(text.contains("\"SettingsVersion\": 4"), "{text}");
    assert!(
        text.contains("\"HotKey\": 38"),
        "WinForms Keys serialize as numbers: {text}"
    );
    assert_eq!(norm["texts"]["hot"], "Ctrl+Alt+Up");
    assert_eq!(norm["repairable"], false);

    let bad = reference
        .call(&json!({"verb": "normalize", "json": "{not json"}))
        .expect("normalize bad");
    assert!(bad.get("parse_error").is_some(), "{bad}");

    let label = reference
        .call(&json!({"verb": "helper", "fn": "MonitorLabel", "args": {"ordinal": 2, "width": 1920, "height": 1080, "primary": true, "main_tag": "(main)", "name": "Desk"}}))
        .expect("helper");
    assert_eq!(label["result"], "2  •  Desk  •  1920x1080  (main)");
}
