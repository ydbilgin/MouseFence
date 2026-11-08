//! Track B reference diff: the Rust settings port (fence-config) and layout helpers (fence-engine `layout`)
//! against the untouched 1.1.1 C# code in tools/reference-cs. Parity row B032 and the reference half of the
//! Track B pNNN rows. OWNED BY TRACK B.
//!
//! Run (the reference built through low_run, see docs/rust2/CONVENTIONS.md):
//!   MF_REFERENCE_EXE=<...>/MouseFence.Reference.exe cargo test -p fence-trace --test config_reference_diff --offline
//!     -- --ignored --nocapture
//! Every test FAILS (never skips) when MF_REFERENCE_EXE is unset, and fails when it compared nothing.
//!
//! What is compared:
//! - 2,000 seeded settings documents (legacy v0-v3, unknown and odd enum values, absent, null, repeated and
//!   unknown members, clamps, odd hotkeys, nicknames with tabs, emoji and combining marks, structural
//!   errors) with seeded synthetic layouts: verbs `normalize`, `load_sim` and `settings_op`;
//! - the layout helpers on the same layouts: SortAndNumber, DisambiguateKeys, SanitizeExcluded,
//!   MigrateLayoutKeysToStable, MapLayout, MonitorLabel, IsArrowRotationHotkey;
//! - WinForms key names and `Keys.ToString()` texts;
//! - the .NET string rules the port copies, character by character: OrdinalIgnoreCase equality over the
//!   BMP and planes 1 and 14, compare order for random pairs, text elements, Trim and IsNullOrWhiteSpace.
//!
//! Seal checks (`sealed` in the reference answers) need Track A's boundary compiler. They never skip on
//! their own (PLAN T20): while fence-engine's `compile` is the skeleton stub the settings-document test
//! FAILS, unless the run sets the explicit opt-out `MF_SEALS_PENDING=1`. Only Track B's own gate before
//! the join sets it; the join gate must not. With the opt-out set and the compiler in, the test fails
//! too, so the opt-out cannot outlive the join. Either way the test checks that exactly
//! `SEAL_ANSWERS` seal answers were reached (compared, or pending under the opt-out).
//! Since the join (2026-10-13) each seal answer is compared twice, as in Track A's engine diff: the exact
//! 1.1.1 validator must equal the reference, and the shipped 2.0 validator (judged fix J05, sealed groups) may
//! differ only where J05 fired.

use std::collections::HashMap;
use std::fmt::Debug;

use fence_config::{self as cfg, Migration, Settings};
use fence_engine::fixes::{Fixes, HIT_J05, HIT_J15};
use fence_engine::layout::{self, KeyedMonitor};
use fence_trace::case::WireScreen;
use fence_trace::corpus::{self, SplitMix64};
use fence_trace::reference::Reference;
use fence_types::{MonitorInfo, Policy, Rect, Screen, SealIssue};
use serde_json::{Value, json};

const DOCS: u64 = 2000;
const SEED_BASE: u64 = 0xB032_0000;

// ------------------------------------------------------------------------------------------------
// Comparison bookkeeping
// ------------------------------------------------------------------------------------------------

#[derive(Default)]
struct Diff {
    compared: usize,
    mismatches: Vec<String>,
    counters: Vec<(String, usize)>,
}

impl Diff {
    fn check<T: PartialEq + Debug>(&mut self, what: impl FnOnce() -> String, rust: T, reference: T) {
        self.compared += 1;
        if rust != reference {
            self.mismatches
                .push(format!("{}: rust={rust:?} reference={reference:?}", what()));
        }
    }
    fn count(&mut self, name: &str) {
        match self.counters.iter_mut().find(|(n, _)| n == name) {
            Some(slot) => slot.1 += 1,
            None => self.counters.push((name.to_string(), 1)),
        }
    }
    fn counter(&self, name: &str) -> usize {
        self.counters.iter().find(|(n, _)| n == name).map_or(0, |(_, c)| *c)
    }
    fn finish(self, label: &str) {
        let counters: Vec<String> = self.counters.iter().map(|(n, c)| format!("{n}={c}")).collect();
        println!(
            "{label}: compared={} mismatches={} {}",
            self.compared,
            self.mismatches.len(),
            counters.join(" ")
        );
        assert!(self.compared > 0, "{label}: nothing was compared");
        if !self.mismatches.is_empty() {
            // Every mismatch goes to the log (up to 2,000 lines); the panic message repeats the first ones.
            for m in self.mismatches.iter().take(2000) {
                println!("MISMATCH {m}");
            }
            let shown: Vec<&String> = self.mismatches.iter().take(25).collect();
            panic!("{label}: {} mismatches; first ones:\n{shown:#?}", self.mismatches.len());
        }
    }
}

fn reference() -> Reference {
    Reference::from_env().expect("MF_REFERENCE_EXE must name the built reference (tools/reference-cs)")
}

/// True once Track A's boundary compiler is in (the skeleton stub compiles nothing).
fn compile_ready() -> bool {
    let screens = vec![
        Screen {
            stable_id: "a".into(),
            bounds: Rect::new(0, 0, 100, 100),
            primary: true,
            excluded: false,
        },
        Screen {
            stable_id: "b".into(),
            bounds: Rect::new(0, -100, 100, 0),
            primary: false,
            excluded: false,
        },
    ];
    !fence_engine::compile::compile(&Policy::default(), &screens, true)
        .transitions
        .is_empty()
}

/// Seal answers the 2,000-document run reaches (normalize with screens, load_sim saves, and the
/// ValidateNoSeal op on documents that parse). Fixed by the seeds; compile-independent. Measured
/// 2026-10-13 (log B-22). The 2,138 printed before counted 78 ValidateNoSeal documents that both sides
/// reject as JSON and that therefore carry no seal answer.
const SEAL_ANSWERS: usize = 2060;

/// Whether this run compares the seal answers. Fails instead of skipping (PLAN T20): the only way to run
/// without them is the explicit opt-out `MF_SEALS_PENDING=1`, and only while the compiler is the stub.
fn seal_mode() -> bool {
    let pending = std::env::var("MF_SEALS_PENDING").is_ok_and(|v| v == "1");
    match (compile_ready(), pending) {
        (true, false) => true,
        (true, true) => panic!(
            "MF_SEALS_PENDING=1 is set but fence-engine compile is real now: remove the opt-out, the seal \
             answers must be compared"
        ),
        (false, true) => {
            println!(
                "sealed: PENDING by opt-out MF_SEALS_PENDING=1 (fence-engine compile is the skeleton stub); \
                 the join gate must run without it"
            );
            false
        }
        (false, false) => panic!(
            "fence-engine compile is the skeleton stub, so the {SEAL_ANSWERS} seal answers cannot be \
             compared. Reference tests fail, never skip (PLAN T20). Before Track A's compiler is merged, set \
             MF_SEALS_PENDING=1 explicitly (Track B's own gate only)."
        ),
    }
}

/// One seal answer: compared when `seals`, else counted as pending. `rust` gives the Rust answer under the
/// chosen judged fixes with its `HIT_*` bits. Two sides, as Track A's engine diff (JUDGE-A M1, option a):
/// the exact 1.1.1 validator (`Fixes::EXACT_1_1_1`) must equal the reference on every answer, and the shipped 2.0
/// validator (`Fixes::ALL`) may differ from it only where J05 fired (`HIT_J05`); any other 2.0 difference is
/// a mismatch.
fn seal_check(
    d: &mut Diff,
    seals: bool,
    what: impl Fn() -> String,
    rust: impl Fn(Fixes) -> (Vec<SealIssue>, u16),
    reference: Value,
) {
    if !seals {
        d.count("sealed_not_compared");
        return;
    }
    let before = d.mismatches.len();
    let (exact, _) = rust(Fixes::EXACT_1_1_1);
    d.check(
        || format!("{} (exact 1.1.1)", what()),
        issues_json(&exact),
        reference.clone(),
    );
    d.count("sealed_compared");
    let (shipped, hits) = rust(Fixes::ALL);
    let shipped = issues_json(&shipped);
    if shipped != reference {
        if hits & (HIT_J05 | HIT_J15) != 0 {
            if hits & HIT_J05 != 0 {
                d.count("sealed_2_0_differs_by_j05");
            }
            if hits & HIT_J15 != 0 {
                d.count("sealed_2_0_differs_by_j15");
            }
        } else {
            d.mismatches.push(format!(
                "{} (2.0): differs from 1.1.1 with no judged fix hit: rust={shipped:?} reference={reference:?}",
                what()
            ));
        }
    }
    if d.mismatches.len() > before {
        d.count("sealed_mismatches");
    }
}

fn s(v: &Value) -> String {
    v.as_str()
        .unwrap_or_else(|| panic!("expected a string, got {v}"))
        .to_string()
}

fn opt_s(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap_or_else(|| panic!("expected an array, got {v}"))
        .iter()
        .map(s)
        .collect()
}

fn issues_json(issues: &[SealIssue]) -> Value {
    Value::Array(
        issues
            .iter()
            .map(|i| {
                json!({"id": i.stable_id, "focus_to": i.focus_to_stable_id,
                       "focus_dir": i.focus_direction.name(), "focus_width": i.focus_corridor_width})
            })
            .collect(),
    )
}

// ------------------------------------------------------------------------------------------------
// Seeded documents and layouts (synthetic only)
// ------------------------------------------------------------------------------------------------

struct Ctx {
    ids: Vec<String>,
    devices: Vec<String>,
}

const ODD_IDS: &[&str] = &[
    "",
    "  ",
    "\t",
    "\\\\.\\DISPLAY9",
    "\\\\.\\display12",
    "MONITOR\\SYN9999\\{gone}",
    "edid-\u{131}",
    "edid-I",
    "edid-\u{17F}",
    "edid-S",
    "edid-\u{212A}",
    "edid-k",
    "\u{E9}did-\u{C4}",
    "\u{C9}DID-\u{E4}",
    "x",
    "X",
    "\u{10428}",
    "\u{10400}",
];

const NAMES: &[&str] = &[
    "Example Left",
    "  Tab\tName \r\n",
    "\u{DC}st Ekran",
    "\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}\u{E9}",
    "e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}",
    "\u{1F44D}\u{1F3FD}\u{1F44D}\u{1F3FD}\u{1F44D}\u{1F3FD}\u{1F44D}\u{1F3FD}\u{1F44D}\u{1F3FD}\u{1F44D}\u{1F3FD}\u{1F44D}\u{1F3FD}\u{1F44D}\u{1F3FD}\u{1F44D}\u{1F3FD}\u{1F44D}\u{1F3FD}\u{1F44D}\u{1F3FD}\u{1F44D}\u{1F3FD}\u{1F44D}\u{1F3FD}",
    "\u{1F1F9}\u{1F1F7}\u{1F1FA}",
    "\u{915}\u{94D}\u{937}\u{93F}\u{915}\u{94D}\u{937}\u{93F}\u{915}\u{94D}\u{937}\u{93F}\u{915}\u{94D}\u{937}\u{93F}\u{915}\u{94D}\u{937}\u{93F}\u{915}\u{94D}\u{937}\u{93F}\u{915}\u{94D}\u{937}\u{93F}",
    "a\u{200D}b",
    "",
    "   ",
    "<b>&amp;</b> 'q' \"d\" +`",
    "\u{1}ctl\u{7F}",
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMN",
    "\u{2028}line\u{2029}",
    "\u{3000}wide\u{3000}",
    "\u{1100}\u{1161}\u{11A8}\u{1100}\u{1161}",
];

const THEMES: &[&str] = &[
    "auto", "en", "tr", "System", "Light", "Dark", "dark", "\u{2028}", "x<y>&'z'", "",
];

const ODD_ENUMS: &[&str] = &[
    "null",
    "true",
    "false",
    "0",
    "1",
    "2",
    "3",
    "4",
    "5",
    "99",
    "-1",
    "2.0",
    "\"\"",
    "\" Push \"",
    "\"push\"",
    "\"PUSH\"",
    "\"Surprise\"",
    "\"Up, Down\"",
    "\"Free, Push\"",
    "\"Free,Push\"",
    "\"2\"",
    "\"+3\"",
    "\" 4 \"",
    "\"R\u{131}ght\"",
    "\"Pu\u{17F}h\"",
    "\"Blocked,\"",
    "\"99999999999\"",
    "\"-0\"",
    "\"Unknown\"",
    "\"0x2\"",
];

const DEFAULT_NAMES: &[&str] = &["Unknown", "Automatic", "Free", "Push", "Blocked"];
const DIRECTION_NAMES: &[&str] = &["Up", "Down", "Left", "Right"];
const BEHAVIOR_NAMES: &[&str] = &["Free", "Push", "Blocked", "OverallFeel"];
const CONTROL_NAMES: &[&str] = &["Automatic", "Always"];

const MEMBERS: &[&str] = &[
    "SettingsVersion",
    "HotKey",
    "ModCtrl",
    "ModAlt",
    "ModShift",
    "ModWin",
    "ConfineHotKey",
    "ConfineModCtrl",
    "ConfineModAlt",
    "ConfineModShift",
    "ConfineModWin",
    "PauseHotKey",
    "PauseModCtrl",
    "PauseModAlt",
    "PauseModShift",
    "PauseModWin",
    "SideHotKey",
    "SideModCtrl",
    "SideModAlt",
    "SideModShift",
    "SideModWin",
    "StartGateClosed",
    "StartConfineOn",
    "StartPaused",
    "StartSideContainOn",
    "AutoStart",
    "DeliberateCross",
    "DescentRouting",
    "UpwardBoundaryDefault",
    "SideBoundaryDefault",
    "DownwardBoundaryDefault",
    "BoundaryRules",
    "UpCrossMin",
    "UpCrossSlack",
    "PassageInsetPx",
    "EdgeHoldbackPx",
    "SeamTolerancePx",
    "SafetyEscapes",
    "BoundaryTipSeen",
    "SideCrossMin",
    "SideCrossSlack",
    "Mode",
    "ManualMonitors",
    "ExcludedDevices",
    "MonitorNames",
    "Language",
    "Theme",
    "CrossingPathMode",
    "UpLinks",
    "Future",
    "modctrl",
];

fn js(text: &str) -> String {
    serde_json::to_string(text).expect("string to JSON")
}

/// Every UTF-16 unit as a `\uXXXX` escape.
fn js_escaped(text: &str) -> String {
    let mut out = String::from("\"");
    for unit in text.encode_utf16() {
        out.push_str(&format!("\\u{unit:04x}"));
    }
    out.push('"');
    out
}

fn jstr(rng: &mut SplitMix64, text: &str) -> String {
    if rng.chance(10) { js_escaped(text) } else { js(text) }
}

fn case_variant(rng: &mut SplitMix64, text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if rng.chance(50) {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push(c.to_ascii_lowercase());
        }
    }
    out
}

fn pick_id(rng: &mut SplitMix64, ctx: &Ctx) -> String {
    match rng.range(0, 9) {
        0..=3 => rng.pick(&ctx.ids).clone(),
        4 => rng.pick(&ctx.devices).clone(),
        5 => {
            let id = rng.pick(&ctx.ids).clone();
            case_variant(rng, &id)
        }
        6 => format!(" {} ", rng.pick(&ctx.ids)),
        7 => rng.pick(ODD_IDS).to_string(),
        _ => {
            let d = rng.pick(&ctx.devices).clone();
            case_variant(rng, &d)
        }
    }
}

fn id_value(rng: &mut SplitMix64, ctx: &Ctx) -> String {
    if rng.chance(5) {
        "null".into()
    } else {
        let id = pick_id(rng, ctx);
        jstr(rng, &id)
    }
}

fn enum_value(rng: &mut SplitMix64, names: &[&str]) -> String {
    if rng.chance(70) {
        let name = *rng.pick(names);
        let name = if rng.chance(20) {
            case_variant(rng, name)
        } else {
            name.to_string()
        };
        js(&name)
    } else if rng.chance(3) {
        rng.pick(&["{}", "[]"]).to_string()
    } else {
        rng.pick(ODD_ENUMS).to_string()
    }
}

fn int_value(rng: &mut SplitMix64) -> String {
    if rng.chance(2) {
        rng.pick(&["2147483648", "1.5", "1e2", "null", "\"3\"", "-2147483649"])
            .to_string()
    } else if rng.chance(60) {
        rng.range(-5, 250).to_string()
    } else {
        rng.pick(&[
            "0",
            "-0",
            "1",
            "200",
            "201",
            "500",
            "501",
            "2147483647",
            "-2147483648",
            "-1",
        ])
        .to_string()
    }
}

fn keys_value(rng: &mut SplitMix64) -> String {
    if rng.chance(2) {
        rng.pick(&["\"Up\"", "null", "38.0", "true"]).to_string()
    } else if rng.chance(50) {
        rng.range(0, 300).to_string()
    } else {
        rng.pick(&[
            "38",
            "39",
            "37",
            "40",
            "71",
            "80",
            "0",
            "65",
            "256",
            "65601",
            "-1",
            "-5",
            "131110",
            "65574",
            "-65536",
            "2147483647",
        ])
        .to_string()
    }
}

fn bool_value(rng: &mut SplitMix64) -> String {
    if rng.chance(1) {
        rng.pick(&["1", "\"true\"", "null"]).to_string()
    } else {
        rng.pick(&["true", "false"]).to_string()
    }
}

fn version_value(rng: &mut SplitMix64) -> String {
    if rng.chance(2) {
        rng.pick(&["\"4\"", "null", "4.0"]).to_string()
    } else {
        rng.pick(&["0", "1", "2", "3", "3", "4", "4", "4", "4", "5", "-1", "-0"])
            .to_string()
    }
}

fn id_list(rng: &mut SplitMix64, ctx: &Ctx) -> String {
    if rng.chance(10) {
        return "null".into();
    }
    if rng.chance(1) {
        return "[1]".into();
    }
    let n = rng.range(0, 4);
    let items: Vec<String> = (0..n).map(|_| id_value(rng, ctx)).collect();
    format!("[{}]", items.join(","))
}

fn rule_value(rng: &mut SplitMix64, ctx: &Ctx) -> String {
    if rng.chance(5) {
        return "null".into();
    }
    if rng.chance(1) {
        return "1".into();
    }
    let mut members: Vec<String> = Vec::new();
    if rng.chance(90) {
        members.push(format!("\"FromStableId\":{}", id_value(rng, ctx)));
    }
    if rng.chance(90) {
        members.push(format!("\"ToStableId\":{}", id_value(rng, ctx)));
    }
    if rng.chance(90) {
        members.push(format!("\"Direction\":{}", enum_value(rng, DIRECTION_NAMES)));
    }
    if rng.chance(90) {
        members.push(format!("\"Behavior\":{}", enum_value(rng, BEHAVIOR_NAMES)));
    }
    if rng.chance(90) {
        members.push(format!("\"Control\":{}", enum_value(rng, CONTROL_NAMES)));
    }
    if rng.chance(5) {
        members.push("\"Extra\":[1,{\"a\":null}]".into());
    }
    if rng.chance(3) && !members.is_empty() {
        let again = members[rng.range(0, members.len() as i64 - 1) as usize].clone();
        members.push(again);
    }
    shuffle(rng, &mut members);
    format!("{{{}}}", members.join(","))
}

fn up_link_value(rng: &mut SplitMix64, ctx: &Ctx) -> String {
    if rng.chance(10) {
        return "null".into();
    }
    let mut members: Vec<String> = Vec::new();
    if rng.chance(90) {
        members.push(format!("\"FromDevice\":{}", id_value(rng, ctx)));
    }
    if rng.chance(90) {
        members.push(format!("\"ToDevice\":{}", id_value(rng, ctx)));
    }
    format!("{{{}}}", members.join(","))
}

fn names_value(rng: &mut SplitMix64, ctx: &Ctx) -> String {
    if rng.chance(10) {
        return "null".into();
    }
    let n = rng.range(0, 5);
    let mut members: Vec<String> = Vec::new();
    for _ in 0..n {
        let key = pick_id(rng, ctx);
        let value = if rng.chance(10) {
            "null".to_string()
        } else {
            let name = *rng.pick(NAMES);
            jstr(rng, name)
        };
        members.push(format!("{}:{}", jstr(rng, &key), value));
    }
    format!("{{{}}}", members.join(","))
}

fn future_value(rng: &mut SplitMix64) -> String {
    if rng.chance(30) {
        let depth = rng.range(61, 65) as usize;
        format!("{}{}", "[".repeat(depth), "]".repeat(depth))
    } else {
        rng.pick(&[
            "{\"a\":[1,2e400,null,\"\\ud83d\\ude00\"]}",
            "\"\\ud800\"",
            "-0.0e-7",
            "[]",
            "{}",
        ])
        .to_string()
    }
}

fn shuffle<T>(rng: &mut SplitMix64, items: &mut [T]) {
    for i in (1..items.len()).rev() {
        let j = rng.range(0, i as i64) as usize;
        items.swap(i, j);
    }
}

fn member_value(name: &str, rng: &mut SplitMix64, ctx: &Ctx) -> String {
    match name {
        "SettingsVersion" => version_value(rng),
        "HotKey" | "ConfineHotKey" | "PauseHotKey" | "SideHotKey" => keys_value(rng),
        "UpwardBoundaryDefault" | "SideBoundaryDefault" | "DownwardBoundaryDefault" => enum_value(rng, DEFAULT_NAMES),
        "BoundaryRules" => {
            if rng.chance(5) {
                "null".into()
            } else {
                let n = rng.range(0, 6);
                let rules: Vec<String> = (0..n).map(|_| rule_value(rng, ctx)).collect();
                format!("[{}]", rules.join(","))
            }
        }
        "UpCrossMin" | "UpCrossSlack" | "PassageInsetPx" | "EdgeHoldbackPx" | "SeamTolerancePx" | "SideCrossMin"
        | "SideCrossSlack" => int_value(rng),
        "Mode" => rng
            .pick(&[
                "\"AutoTop\"",
                "\"Manual\"",
                "\"Manual\"",
                "\"manual\"",
                "null",
                "\"\"",
                "\"Other\"",
            ])
            .to_string(),
        "ManualMonitors" | "ExcludedDevices" => id_list(rng, ctx),
        "MonitorNames" => names_value(rng, ctx),
        "Language" | "Theme" => {
            if rng.chance(10) {
                "null".into()
            } else {
                let t = *rng.pick(THEMES);
                jstr(rng, t)
            }
        }
        "CrossingPathMode" => rng
            .pick(&[
                "\"Default\"",
                "\"Custom\"",
                "\"Custom\"",
                "\"custom\"",
                "null",
                "\"X\"",
                "\"\"",
            ])
            .to_string(),
        "UpLinks" => {
            if rng.chance(10) {
                "null".into()
            } else {
                let n = rng.range(0, 4);
                let links: Vec<String> = (0..n).map(|_| up_link_value(rng, ctx)).collect();
                format!("[{}]", links.join(","))
            }
        }
        "Future" => future_value(rng),
        _ => bool_value(rng),
    }
}

fn member_chance(name: &str) -> u64 {
    match name {
        "SettingsVersion" => 75,
        "BoundaryRules" | "MonitorNames" | "Mode" | "ManualMonitors" | "ExcludedDevices" => 50,
        "CrossingPathMode" | "UpLinks" => 35,
        "Future" | "modctrl" => 5,
        _ => 30,
    }
}

fn gen_doc(rng: &mut SplitMix64, ctx: &Ctx) -> String {
    if rng.chance(1) {
        return rng.pick(&["null", " null ", "{}", "[]", "\"x\"", "1"]).to_string();
    }
    let mut members: Vec<(String, String)> = Vec::new();
    for name in MEMBERS {
        if rng.chance(member_chance(name)) {
            let v = member_value(name, rng, ctx);
            members.push((name.to_string(), v));
        }
    }
    shuffle(rng, &mut members);
    if rng.chance(3) && !members.is_empty() {
        let (name, _) = members[rng.range(0, members.len() as i64 - 1) as usize].clone();
        let v = member_value(&name, rng, ctx);
        members.push((name, v));
    }
    let sep = if rng.chance(50) { ", " } else { "," };
    let body: Vec<String> = members.iter().map(|(n, v)| format!("{}: {v}", js(n))).collect();
    let doc = format!("{{{}}}", body.join(sep));
    if !rng.chance(4) {
        return doc;
    }
    match rng.range(0, 5) {
        0 => {
            // Truncated at a random character boundary.
            let cut: Vec<usize> = doc.char_indices().map(|(i, _)| i).filter(|&i| i > 0).collect();
            doc[..*rng.pick(&cut)].to_string()
        }
        1 => format!("{}, }}", &doc[..doc.len() - 1]),
        2 => format!("{{/*c*/{}", &doc[1..]),
        3 => format!("\u{FEFF}{doc}"),
        4 => format!("\r\n\t {doc} \r\n"),
        _ => format!("[{doc}]"),
    }
}

fn gen_layout(rng: &mut SplitMix64) -> Vec<WireScreen> {
    let mut screens = corpus::layout(rng, 6);
    if screens.len() >= 2 && rng.chance(10) {
        screens[1].id = screens[0].id.clone();
    }
    if screens.len() >= 3 && rng.chance(5) {
        screens[2].id = screens[0].id.to_lowercase();
    }
    if screens.len() >= 2 && rng.chance(4) {
        screens[1].device = screens[0].device.to_lowercase();
    }
    if screens.len() >= 2 && rng.chance(3) {
        screens[1].device = screens[0].device.clone();
    }
    shuffle(rng, &mut screens);
    screens
}

fn monitors_of(screens: &[WireScreen]) -> Vec<MonitorInfo> {
    layout::identify_monitors(
        screens
            .iter()
            .map(|w| MonitorInfo {
                device: w.device.clone(),
                stable_id: w.id.clone(),
                bounds: Rect::new(w.l, w.t, w.r, w.b),
                primary: w.primary,
                index: 0,
            })
            .collect(),
    )
}

fn ctx_of(screens: &[WireScreen], monitors: &[MonitorInfo]) -> Ctx {
    let mut ids: Vec<String> = screens.iter().map(|w| w.id.clone()).collect();
    ids.extend(monitors.iter().map(|m| m.stable_id.clone()));
    Ctx {
        ids,
        devices: screens.iter().map(|w| w.device.clone()).collect(),
    }
}

struct Case {
    seed: u64,
    screens: Vec<WireScreen>,
    monitors: Vec<MonitorInfo>,
    ctx: Ctx,
    doc: String,
    rng: SplitMix64,
}

fn case(seed: u64) -> Case {
    let mut rng = SplitMix64::new(SEED_BASE + seed);
    let screens = gen_layout(&mut rng);
    let monitors = monitors_of(&screens);
    let ctx = ctx_of(&screens, &monitors);
    let doc = gen_doc(&mut rng, &ctx);
    Case {
        seed,
        screens,
        monitors,
        ctx,
        doc,
        rng,
    }
}

// ------------------------------------------------------------------------------------------------
// Settings verbs
// ------------------------------------------------------------------------------------------------

fn texts_of(s: &Settings) -> Vec<String> {
    vec![
        s.hot_key_text(),
        s.confine_hot_key_text(),
        s.pause_hot_key_text(),
        s.side_hot_key_text(),
    ]
}

fn modifiers_of(s: &Settings) -> Vec<u64> {
    [s.hotkey(), s.confine_hotkey(), s.pause_hotkey(), s.side_hotkey()]
        .iter()
        .map(|h| h.modifiers() as u64)
        .collect()
}

fn reference_texts(v: &Value) -> Vec<String> {
    ["hot", "confine", "pause", "side"].iter().map(|k| s(&v[k])).collect()
}

/// The `normalize` verb in Rust: ReadVersion, Deserialize, Normalize, the key migration when screens are
/// given and the file is older than v4 (no try: an exception is the verb's error), Normalize, version 4.
fn compare_normalize(d: &mut Diff, o: &mut Reference, c: &Case, with_screens: bool, seals: bool) {
    let mut req = json!({"verb": "normalize", "json": c.doc});
    if with_screens {
        req["screens"] = serde_json::to_value(&c.screens).unwrap();
    }
    let seed = c.seed;
    let answer = o.call(&req);
    let loaded = cfg::read_version(&c.doc);
    let parsed = cfg::from_json(&c.doc);
    let mut settings = match (parsed, &answer) {
        (Err(_), Ok(a)) if a.get("parse_error").is_some() => {
            d.count("normalize_parse_error");
            d.check(
                || format!("seed {seed} normalize loaded_version"),
                loaded as i64,
                a["loaded_version"].as_i64().unwrap(),
            );
            return;
        }
        (Err(e), _) => {
            d.mismatches.push(format!(
                "seed {seed} normalize: rust rejects ({e}), reference {answer:?}; doc {}",
                c.doc
            ));
            return;
        }
        (Ok(v), _) => v,
    };
    cfg::normalize(&mut settings);
    if loaded < cfg::CURRENT_SETTINGS_VERSION && with_screens {
        if let Err(e) = settings.migrate_layout_keys_to_stable(&c.monitors) {
            d.count("normalize_migration_threw");
            match &answer {
                Err(msg) if msg.contains(e.csharp_exception()) => d.compared += 1,
                _ => d.mismatches.push(format!(
                    "seed {seed} normalize: rust migration fails ({e}), reference {answer:?}; doc {}",
                    c.doc
                )),
            }
            return;
        }
        settings.set_settings_version(cfg::CURRENT_SETTINGS_VERSION);
        d.count("normalize_migrated");
    }
    cfg::normalize(&mut settings);
    settings.set_settings_version(cfg::CURRENT_SETTINGS_VERSION);
    let a = match answer {
        Ok(a) => a,
        Err(msg) => {
            d.mismatches
                .push(format!("seed {seed} normalize: reference error {msg}; doc {}", c.doc));
            return;
        }
    };
    if a.get("parse_error").is_some() {
        d.mismatches.push(format!(
            "seed {seed} normalize: reference rejects ({}), rust accepts; doc {}",
            a["parse_error"], c.doc
        ));
        return;
    }
    d.count("normalize_ok");
    if settings.has_repairable_import_errors {
        d.count("normalize_repairable");
    }
    d.check(
        || format!("seed {seed} normalize loaded_version"),
        loaded as i64,
        a["loaded_version"].as_i64().unwrap(),
    );
    d.check(
        || format!("seed {seed} normalize repairable"),
        settings.has_repairable_import_errors,
        a["repairable"].as_bool().unwrap(),
    );
    d.check(
        || format!("seed {seed} normalize json; doc {}", c.doc),
        cfg::to_json(&settings),
        s(&a["json"]),
    );
    d.check(
        || format!("seed {seed} normalize texts"),
        texts_of(&settings),
        reference_texts(&a["texts"]),
    );
    let want_mods: Vec<u64> = a["modifiers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    d.check(
        || format!("seed {seed} normalize modifiers"),
        modifiers_of(&settings),
        want_mods,
    );
    if with_screens {
        seal_check(
            d,
            seals,
            || format!("seed {seed} normalize sealed"),
            |f| settings.clone().validate_no_seal_with(&c.monitors, f),
            a["sealed"].clone(),
        );
    }
}

fn migration_text(m: &Migration) -> String {
    match m {
        Migration::NotNeeded => "none".into(),
        Migration::Migrated => "save".into(),
        Migration::Failed(e) => format!("threw:{}", e.csharp_exception()),
    }
}

/// The `load_sim` verb in Rust: `parse_and_migrate`, then for a migration the in-memory half of `save`.
fn compare_load_sim(d: &mut Diff, o: &mut Reference, c: &Case, seals: bool) {
    let seed = c.seed;
    let req = json!({"verb": "load_sim", "json": c.doc, "screens": serde_json::to_value(&c.screens).unwrap()});
    let a = match o.call(&req) {
        Ok(a) => a,
        Err(msg) => {
            d.mismatches
                .push(format!("seed {seed} load_sim: reference error {msg}"));
            return;
        }
    };
    d.check(
        || format!("seed {seed} load_sim loaded_version"),
        cfg::read_version(&c.doc) as i64,
        a["loaded_version"].as_i64().unwrap(),
    );
    let parsed = match cfg::parse_and_migrate(&c.doc, &c.monitors) {
        Ok(p) => p,
        Err(e) => {
            d.count("load_parse_error");
            d.check(
                || format!("seed {seed} load_sim parse error ({e}); doc {}", c.doc),
                true,
                a.get("parse_error").is_some(),
            );
            return;
        }
    };
    if a.get("parse_error").is_some() {
        d.mismatches.push(format!(
            "seed {seed} load_sim: reference rejects ({}), rust accepts; doc {}",
            a["parse_error"], c.doc
        ));
        return;
    }
    let mut settings = parsed.settings;
    let mut migration = migration_text(&parsed.migration);
    let mut saved_json = None;
    if parsed.migration == Migration::Migrated {
        cfg::normalize(&mut settings);
        settings.set_settings_version(cfg::CURRENT_SETTINGS_VERSION);
        if settings.has_repairable_import_errors {
            migration = "refused_repairable".into();
        } else {
            saved_json = Some(cfg::to_json(&settings));
            seal_check(
                d,
                seals,
                || format!("seed {seed} load_sim sealed"),
                |f| cfg::seal_issues_with(&settings, &c.monitors, f),
                a["sealed"].clone(),
            );
        }
    }
    d.count(&format!("load_{migration}"));
    d.check(
        || format!("seed {seed} load_sim migration; doc {}", c.doc),
        migration,
        s(&a["migration"]),
    );
    d.check(
        || format!("seed {seed} load_sim saved_json"),
        saved_json,
        opt_s(&a["saved_json"]),
    );
    d.check(
        || format!("seed {seed} load_sim repairable"),
        settings.has_repairable_import_errors,
        a["repairable"].as_bool().unwrap(),
    );
    d.check(
        || format!("seed {seed} load_sim json; doc {}", c.doc),
        cfg::to_json(&settings),
        s(&a["json"]),
    );
}

const OPS: &[&str] = &[
    "AddShiftToArrowHotkeys",
    "ResetLayoutConfig",
    "DeepClone",
    "MonitorNameFor",
    "Computed",
    "ValidateNoSeal",
];

fn compare_settings_op(d: &mut Diff, o: &mut Reference, c: &mut Case, seals: bool) {
    let seed = c.seed;
    let mut op = OPS[(seed % OPS.len() as u64) as usize];
    // Under the opt-out the ValidateNoSeal documents run MonitorNameFor instead; the seal answer they
    // would have given is counted as pending once the document parses (as the compared ones are).
    let seal_pending = op == "ValidateNoSeal" && !seals;
    if seal_pending {
        op = "MonitorNameFor";
    }
    let stable_id = if c.rng.chance(10) {
        None
    } else {
        Some(pick_id(&mut c.rng, &c.ctx))
    };
    let req = json!({"verb": "settings_op", "op": op, "json": c.doc, "stable_id": stable_id,
                     "screens": serde_json::to_value(&c.screens).unwrap()});
    let answer = o.call(&req);
    let mut settings = match (cfg::from_json(&c.doc), &answer) {
        (Err(_), Err(msg)) if msg.contains("JsonException") => {
            d.count("op_parse_error");
            d.compared += 1;
            return;
        }
        (Ok(v), Ok(_)) => v,
        (r, _) => {
            d.mismatches.push(format!(
                "seed {seed} settings_op {op}: rust parse {:?}, reference {answer:?}",
                r.err()
            ));
            return;
        }
    };
    let a = answer.unwrap();
    cfg::normalize(&mut settings);
    d.count(&format!("op_{op}"));
    if seal_pending {
        d.count("sealed_not_compared");
    }
    match op {
        "AddShiftToArrowHotkeys" => {
            cfg::add_shift_to_arrow_hotkeys(&mut settings);
            d.check(
                || format!("seed {seed} op intel_defaults_applied"),
                settings.intel_defaults_applied,
                a["intel_defaults_applied"].as_bool().unwrap(),
            );
        }
        "ResetLayoutConfig" => cfg::reset_layout_config(&mut settings),
        "DeepClone" => settings = cfg::deep_clone(&settings),
        "MonitorNameFor" => {
            let name = cfg::monitor_name_for(&settings.monitor_names, stable_id.as_deref().unwrap_or(""));
            d.check(|| format!("seed {seed} op name {stable_id:?}"), name, s(&a["name"]));
        }
        "Computed" => {
            d.check(
                || format!("seed {seed} op crossing_path_mode"),
                settings.crossing_path_mode().to_string(),
                s(&a["crossing_path_mode"]),
            );
            let links: Vec<Vec<String>> = settings
                .up_links()
                .into_iter()
                .map(|l| vec![l.from_device, l.to_device])
                .collect();
            let want: Vec<Vec<String>> = a["up_links"].as_array().unwrap().iter().map(strs).collect();
            d.check(|| format!("seed {seed} op up_links"), links, want);
        }
        "ValidateNoSeal" => {
            seal_check(
                d,
                seals,
                || format!("seed {seed} op sealed"),
                |f| settings.clone().validate_no_seal_with(&c.monitors, f),
                a["sealed"].clone(),
            );
            // C# ValidateNoSeal normalizes the settings in place; the JSON below is compared after it.
            cfg::normalize(&mut settings);
        }
        _ => unreachable!(),
    }
    d.check(
        || format!("seed {seed} op {op} repairable"),
        settings.has_repairable_import_errors,
        a["repairable"].as_bool().unwrap(),
    );
    d.check(
        || format!("seed {seed} op {op} json; doc {}", c.doc),
        cfg::to_json(&settings),
        s(&a["json"]),
    );
}

#[test]
#[ignore = "needs MF_REFERENCE_EXE (tools/reference-cs)"]
fn config_reference_2000_seeded_settings_documents() {
    let mut o = reference();
    let seals = seal_mode();
    let mut d = Diff::default();
    for seed in 0..DOCS {
        let mut c = case(seed);
        let with_screens = !c.rng.chance(30);
        compare_normalize(&mut d, &mut o, &c, with_screens, seals);
        compare_load_sim(&mut d, &mut o, &c, seals);
        compare_settings_op(&mut d, &mut o, &mut c, seals);
        d.count("docs");
    }
    let compared = d.counter("sealed_compared");
    let pending = d.counter("sealed_not_compared");
    println!(
        "sealed: compared={compared} mismatches={} pending={pending} expected={SEAL_ANSWERS} \
         2.0-differs-by-J05={}",
        d.counter("sealed_mismatches"),
        d.counter("sealed_2_0_differs_by_j05")
    );
    if seals {
        assert_eq!(pending, 0, "seal answers left uncompared with the compiler in");
        assert_eq!(compared, SEAL_ANSWERS, "seal answers compared");
    } else {
        assert_eq!(compared, 0, "seal answers compared under the opt-out");
        assert_eq!(pending, SEAL_ANSWERS, "seal answers pending under the opt-out");
    }
    d.finish("config reference settings documents");
}

// ------------------------------------------------------------------------------------------------
// Layout helpers
// ------------------------------------------------------------------------------------------------

fn helper(o: &mut Reference, f: &str, args: Value) -> Result<Value, String> {
    o.call(&json!({"verb": "helper", "fn": f, "args": args}))
        .map(|v| v["result"].clone())
}

fn rect_json(r: &Rect) -> Value {
    json!([r.l, r.t, r.r, r.b])
}

#[test]
#[ignore = "needs MF_REFERENCE_EXE (tools/reference-cs)"]
fn config_reference_layout_helpers_on_2000_seeded_layouts() {
    let mut o = reference();
    let mut d = Diff::default();
    for seed in 0..DOCS {
        let mut c = case(seed);
        let rng = &mut c.rng;

        // SortAndNumber on the raw screens.
        let mut sorted: Vec<MonitorInfo> = c
            .screens
            .iter()
            .map(|w| MonitorInfo {
                device: w.device.clone(),
                stable_id: w.id.clone(),
                bounds: Rect::new(w.l, w.t, w.r, w.b),
                primary: w.primary,
                index: 0,
            })
            .collect();
        layout::sort_and_number(&mut sorted);
        let rust: Vec<(String, String, u64)> = sorted
            .iter()
            .map(|m| (m.stable_id.clone(), m.device.clone(), m.index as u64))
            .collect();
        let a = helper(&mut o, "SortAndNumber", json!({"screens": c.screens})).expect("SortAndNumber");
        let want: Vec<(String, String, u64)> = a
            .as_array()
            .unwrap()
            .iter()
            .map(|m| (s(&m["id"]), s(&m["device"]), m["index"].as_u64().unwrap()))
            .collect();
        d.check(|| format!("seed {seed} SortAndNumber"), rust, want);

        // DisambiguateKeys with shared and case-variant keys.
        let n = rng.range(1, 6) as usize;
        let mut items: Vec<KeyedMonitor> = Vec::new();
        for _ in 0..n {
            let key = if rng.chance(50) && !items.is_empty() {
                let k = items[rng.range(0, items.len() as i64 - 1) as usize].key.clone();
                if rng.chance(30) { case_variant(rng, &k) } else { k }
            } else {
                pick_id(rng, &c.ctx)
            };
            let raw = if rng.chance(50) {
                key.clone()
            } else {
                pick_id(rng, &c.ctx)
            };
            items.push(KeyedMonitor {
                key,
                left: rng.range(-2, 2) as i32 * 1000,
                top: rng.range(-1, 1) as i32 * 500,
                raw_identity_key: raw,
            });
        }
        let rust = layout::disambiguate_keys(&items);
        let args: Vec<Value> = items
            .iter()
            .map(|m| json!({"key": m.key, "left": m.left, "top": m.top, "raw": m.raw_identity_key}))
            .collect();
        let a = helper(&mut o, "DisambiguateKeys", json!({"items": args})).expect("DisambiguateKeys");
        d.check(|| format!("seed {seed} DisambiguateKeys {items:?}"), rust, strs(&a));

        // SanitizeExcluded.
        let present: Vec<String> = c.monitors.iter().map(|m| m.stable_id.clone()).collect();
        let primary = match rng.range(0, 9) {
            0 => None,
            1 => Some(pick_id(rng, &c.ctx)),
            _ => c
                .monitors
                .iter()
                .find(|m| m.primary)
                .or(c.monitors.first())
                .map(|m| m.stable_id.clone()),
        };
        let k = rng.range(0, 4);
        let requested: Vec<String> = (0..k)
            .map(|_| {
                if rng.chance(70) {
                    let p = rng.pick(&present).clone();
                    if rng.chance(30) { case_variant(rng, &p) } else { p }
                } else {
                    pick_id(rng, &c.ctx)
                }
            })
            .collect();
        let rust = layout::sanitize_excluded(&present, primary.as_deref(), &requested);
        let a = helper(
            &mut o,
            "SanitizeExcluded",
            json!({"present": present, "primary": primary, "requested": requested}),
        )
        .expect("SanitizeExcluded");
        d.check(
            || format!("seed {seed} SanitizeExcluded"),
            rust,
            (strs(&a["excluded"]), a["pruned"].as_bool().unwrap()),
        );

        // MigrateLayoutKeysToStable (the GuardCore helper): nulls, stale names, a case-variant map.
        let manual: Vec<Option<String>> = (0..rng.range(0, 4))
            .map(|_| {
                if rng.chance(10) {
                    None
                } else {
                    Some(pick_id(rng, &c.ctx))
                }
            })
            .collect();
        let links: Vec<(Option<String>, Option<String>)> = (0..rng.range(0, 4))
            .map(|_| {
                let f = if rng.chance(5) {
                    None
                } else {
                    Some(pick_id(rng, &c.ctx))
                };
                let t = if rng.chance(5) {
                    None
                } else {
                    Some(pick_id(rng, &c.ctx))
                };
                (f, t)
            })
            .collect();
        let mut map: Vec<(String, String)> = c
            .monitors
            .iter()
            .map(|m| (m.device.clone(), m.stable_id.clone()))
            .collect();
        if rng.chance(20) && !map.is_empty() {
            let (dev, _) = map[0].clone();
            let other = pick_id(rng, &c.ctx);
            map.push((case_variant(rng, &dev), other));
        }
        let manual_rust: Vec<String> = manual.iter().map(|m| m.clone().unwrap_or_default()).collect();
        let links_rust: Vec<(String, String)> = links
            .iter()
            .map(|(f, t)| (f.clone().unwrap_or_default(), t.clone().unwrap_or_default()))
            .collect();
        let (m_rust, l_rust) = layout::migrate_layout_keys_to_stable(&manual_rust, &links_rust, &map);
        let links_json: Vec<Value> = links.iter().map(|(f, t)| json!([f, t])).collect();
        let map_json: Vec<Value> = map.iter().map(|(k, v)| json!([k, v])).collect();
        let a = helper(
            &mut o,
            "MigrateLayoutKeysToStable",
            json!({"manual": manual, "links": links_json, "map": map_json}),
        )
        .expect("MigrateLayoutKeysToStable");
        let l_want: Vec<(String, String)> = a["links"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (s(&p[0]), s(&p[1])))
            .collect();
        d.check(
            || format!("seed {seed} MigrateLayoutKeysToStable"),
            (m_rust, l_rust),
            (strs(&a["manual"]), l_want),
        );

        // MapLayout: the layout, sometimes with an extreme or degenerate rectangle.
        let mut rects: Vec<Rect> = c.monitors.iter().map(|m| m.bounds).collect();
        if rng.chance(20) {
            rects.push(*rng.pick(&[
                Rect::new(-32768, 0, -32128, 480),
                Rect::new(30000, 0, 32767, 2160),
                Rect::new(5, 5, 5, 5),
                Rect::new(10, 10, 0, 0),
                Rect::new(i32::MIN / 2, i32::MIN / 2, i32::MAX / 2, i32::MAX / 2),
            ]));
        }
        let (w, h, pad) = (
            rng.range(-10, 900) as i32,
            rng.range(-10, 500) as i32,
            rng.range(-5, 60) as i32,
        );
        let rust: Vec<Value> = layout::map_layout(&rects, w, h, pad).iter().map(rect_json).collect();
        let rect_args: Vec<Value> = rects.iter().map(rect_json).collect();
        let a = helper(
            &mut o,
            "MapLayout",
            json!({"rects": rect_args, "w": w, "h": h, "pad": pad}),
        )
        .expect("MapLayout");
        d.check(
            || format!("seed {seed} MapLayout {rects:?} {w} {h} {pad}"),
            Value::Array(rust),
            a,
        );

        // MonitorLabel.
        let ordinal = rng.range(1, 9) as u32;
        let (lw, lh) = (rng.range(-10, 4000), rng.range(-10, 3000));
        let primary_flag = rng.chance(50);
        let tag = rng
            .pick(&[Some("(main)"), Some(""), None, Some("(ana)")])
            .map(str::to_string);
        let name = if rng.chance(30) {
            None
        } else {
            Some(rng.pick(NAMES).to_string())
        };
        let rust = layout::monitor_label(
            ordinal,
            lw,
            lh,
            primary_flag,
            tag.as_deref().unwrap_or(""),
            name.as_deref(),
        );
        let a = helper(
            &mut o,
            "MonitorLabel",
            json!({"ordinal": ordinal, "width": lw, "height": lh, "primary": primary_flag, "main_tag": tag, "name": name}),
        )
        .expect("MonitorLabel");
        d.check(|| format!("seed {seed} MonitorLabel"), rust, s(&a));

        // IsArrowRotationHotkey.
        let vk = rng.range(0, 300) as i32;
        let (ctrl, alt, shift, win) = (rng.chance(70), rng.chance(70), rng.chance(30), rng.chance(20));
        let rust = layout::is_arrow_rotation_hotkey(vk as u32, ctrl, alt, shift, win);
        let a = helper(
            &mut o,
            "IsArrowRotationHotkey",
            json!({"vk": vk, "ctrl": ctrl, "alt": alt, "shift": shift, "win": win}),
        )
        .expect("IsArrowRotationHotkey");
        d.check(
            || format!("seed {seed} IsArrowRotationHotkey"),
            rust,
            a.as_bool().unwrap(),
        );
        d.count("layouts");
    }
    d.finish("config reference layout helpers");
}

// ------------------------------------------------------------------------------------------------
// Keys
// ------------------------------------------------------------------------------------------------

#[test]
#[ignore = "needs MF_REFERENCE_EXE (tools/reference-cs)"]
fn config_reference_key_names_and_texts() {
    let mut o = reference();
    let mut d = Diff::default();
    let e = o.call(&json!({"verb": "keys_enum"})).expect("keys_enum");
    let names = strs(&e["names"]);
    let values: Vec<u32> = e["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap() as i32 as u32)
        .collect();
    let want: Vec<(String, u32)> = names.into_iter().zip(values).collect();
    let rust: Vec<(String, u32)> = cfg::keys::KEYS.iter().map(|(n, v)| (n.to_string(), *v)).collect();
    d.check(|| "keys_enum table".to_string(), rust, want);

    let k = o.call(&json!({"verb": "keys"})).expect("keys");
    for vk in 0..256 {
        d.check(
            || format!("keys {vk}"),
            cfg::keys_to_string(vk),
            s(&k["names"][vk.to_string()]),
        );
    }

    let mut values: Vec<i32> = (0..0x300).collect();
    for key in 0..256 {
        for m in [0x10000u32, 0x20000, 0x40000, 0x30000, 0x70000, 0xFFFF_0000, 0x8000_0000] {
            values.push((key | m) as i32);
        }
    }
    let mut rng = SplitMix64::new(0x4B45_5953);
    for _ in 0..4000 {
        values.push(rng.next_u64() as u32 as i32);
    }
    values.extend([i32::MIN, i32::MAX, -1, -5, 0xFFFF, 0x1FFFF, 65601]);
    for chunk in values.chunks(2000) {
        let a = o
            .call(&json!({"verb": "keys_text", "values": chunk}))
            .expect("keys_text");
        let texts = strs(&a["texts"]);
        for (v, t) in chunk.iter().zip(texts) {
            d.check(|| format!("keys_text {v}"), cfg::keys_to_string(*v), t);
        }
    }
    d.finish("config reference keys");
}

// ------------------------------------------------------------------------------------------------
// .NET string rules, character by character
// ------------------------------------------------------------------------------------------------

fn plane_chars(lo: u32, hi: u32) -> Vec<char> {
    (lo..=hi).filter_map(char::from_u32).collect()
}

fn ranges() -> Vec<(u32, u32)> {
    vec![(0x0, 0xFFFF), (0x10000, 0x1FFFF), (0xE0000, 0xE0FFF)]
}

#[test]
#[ignore = "needs MF_REFERENCE_EXE (tools/reference-cs)"]
fn config_reference_case_rules_whole_planes() {
    let mut o = reference();
    let mut d = Diff::default();
    for (lo, hi) in ranges() {
        let chars = plane_chars(lo, hi);
        let strings: Vec<String> = chars.iter().map(|c| c.to_string()).collect();
        let a = o
            .call(&json!({"verb": "case_groups", "strings": strings}))
            .expect("case_groups");
        let want: Vec<u64> = a["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        let mut first: HashMap<String, u64> = HashMap::new();
        for (i, (text, want_group)) in strings.iter().zip(&want).enumerate() {
            let key: String = text.chars().map(layout::ignore_case_upper).collect();
            let group = *first.entry(key).or_insert(i as u64);
            d.check(
                || format!("case group of U+{:04X}", text.chars().next().unwrap() as u32),
                group,
                *want_group,
            );
        }
        d.count("planes");
    }

    // Compare order for random pairs of tricky strings (ordinal is UTF-16 order; ignore-case is code point
    // order after the uppercase mapping).
    let pool: Vec<char> = "aAzZ_[`{~\u{7F}\u{E9}\u{C9}\u{DF}\u{1E9E}\u{131}\u{130}iI\u{17F}sS\u{212A}kK\u{3C3}\u{3C2}\u{3A3}\u{1F80}\u{1F88}\u{1FB3}\u{1FBC}\u{FF61}\u{FF41}\u{FF21}\u{E000}\u{FFFD}\u{10000}\u{10428}\u{10400}\u{1F600}\u{E0041}\u{2C65}\u{23A}\u{1C4}\u{1C5}\u{1C6}\u{345}\u{3B9}\u{399}\u{B5}\u{39C}\u{3BC}"
        .chars()
        .collect();
    let mut rng = SplitMix64::new(0xCA5E);
    let mut pairs: Vec<(String, String)> = Vec::new();
    for _ in 0..5000 {
        let make = |rng: &mut SplitMix64| -> String {
            let n = rng.range(0, 4);
            (0..n).map(|_| *rng.pick(&pool)).collect()
        };
        let a = make(&mut rng);
        let b = if rng.chance(30) {
            a.chars()
                .map(|c| {
                    if rng.chance(50) {
                        c.to_uppercase().next().unwrap_or(c)
                    } else {
                        c
                    }
                })
                .collect()
        } else {
            make(&mut rng)
        };
        pairs.push((a, b));
    }
    let pair_json: Vec<Value> = pairs.iter().map(|(a, b)| json!([a, b])).collect();
    let a = o
        .call(&json!({"verb": "case_groups", "strings": [], "pairs": pair_json}))
        .expect("case_groups pairs");
    let sign = |o: std::cmp::Ordering| o as i64;
    for (i, (x, y)) in pairs.iter().enumerate() {
        d.check(
            || format!("ignore-case compare {x:?} {y:?}"),
            sign(layout::ordinal_ignore_case_cmp(x, y)),
            a["ignore_case"][i].as_i64().unwrap(),
        );
        d.check(
            || format!("ordinal compare {x:?} {y:?}"),
            sign(layout::ordinal_cmp(x, y)),
            a["ordinal"][i].as_i64().unwrap(),
        );
    }
    d.finish("config reference case rules");
}

/// UTF-16 offsets of the text-element starts the Rust port computes.
fn starts_utf16(text: &str) -> Vec<u64> {
    let starts = cfg::text_element_starts(text);
    let mut out = Vec::with_capacity(starts.len());
    let mut units = 0u64;
    let mut si = 0;
    for (i, c) in text.char_indices() {
        if si < starts.len() && starts[si] == i {
            out.push(units);
            si += 1;
        }
        units += c.len_utf16() as u64;
    }
    out
}

fn compare_text_batch(d: &mut Diff, o: &mut Reference, values: &[String]) {
    let a = o
        .call(&json!({"verb": "text_elements", "values": values}))
        .expect("text_elements");
    for (i, v) in values.iter().enumerate() {
        let want: Vec<u64> = a["starts"][i]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_u64().unwrap())
            .collect();
        d.check(
            || {
                format!(
                    "text elements of {v:?} ({:X?})",
                    v.chars().map(|c| c as u32).collect::<Vec<_>>()
                )
            },
            starts_utf16(v),
            want,
        );
        d.check(|| format!("trim of {v:?}"), v.trim().to_string(), s(&a["trimmed"][i]));
        d.check(
            || format!("blank of {v:?}"),
            v.chars().all(char::is_whitespace),
            a["blank"][i].as_bool().unwrap(),
        );
        d.check(
            || format!("normalized name of {v:?}"),
            cfg::normalize_monitor_name(v),
            s(&a["normalized"][i]),
        );
    }
}

#[test]
#[ignore = "needs MF_REFERENCE_EXE (tools/reference-cs)"]
fn config_reference_text_elements_whole_planes() {
    let mut o = reference();
    let mut d = Diff::default();
    for (lo, hi) in ranges() {
        let chars = plane_chars(lo, hi);
        for chunk in chars.chunks(4096) {
            let mut values: Vec<String> = Vec::with_capacity(chunk.len() * 4);
            for &c in chunk {
                values.push(format!("a{c}a"));
                values.push(format!("{c}{c}"));
                values.push(format!("\u{915}\u{94D}{c}"));
                values.push(format!("{c}\u{200D}{c}"));
            }
            compare_text_batch(&mut d, &mut o, &values);
        }
        d.count("planes");
    }
    // Nicknames: the name pool and random mixes of combining marks, joiners, emoji and spaces.
    let parts: Vec<&str> = vec![
        "a",
        "e\u{301}",
        "\u{1F44D}",
        "\u{1F3FD}",
        "\u{200D}",
        "\u{1F1F9}",
        "\u{1F1F7}",
        "\u{915}",
        "\u{94D}",
        "\u{937}",
        "\u{93F}",
        " ",
        "\t",
        "\r\n",
        "\u{3000}",
        "\u{600}",
        "\u{E01}",
        "\u{E33}",
        "\u{1100}",
        "\u{1161}",
        "\u{11A8}",
        "\u{AC00}",
        "\u{FE0F}",
        "\u{E0041}",
        "\u{E007F}",
        "\u{1F3F4}",
        "\u{2028}",
    ];
    let mut rng = SplitMix64::new(0x7E47);
    let mut values: Vec<String> = NAMES.iter().map(|n| n.to_string()).collect();
    for _ in 0..3000 {
        let n = rng.range(0, 40);
        values.push((0..n).map(|_| *rng.pick(&parts)).collect());
    }
    compare_text_batch(&mut d, &mut o, &values);
    d.finish("config reference text rules");
}
