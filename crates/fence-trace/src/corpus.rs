//! Seeded synthetic corpus: layouts, settings variations and move streams (no outside crates, no real desk
//! geometry). Every generator is a pure function of its seed, so a failing case is reproduced from its seed.
//!
//! Track A extends this module (more motion shapes, settings variations, corpus sizes); the existing
//! functions keep their output for a given seed (tests pin it), so recorded seeds stay valid.

use crate::case::{Op, RunCase, WireScreen, WireToggles};

/// SplitMix64 (Steele, Lea, Flood 2014): tiny, fast, good enough for test corpora.
#[derive(Clone, Debug)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in `[lo, hi]` (inclusive). `lo <= hi` required.
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        let span = (hi - lo + 1) as u64;
        lo + (self.next_u64() % span) as i64
    }
    pub fn chance(&mut self, percent: u64) -> bool {
        self.next_u64() % 100 < percent
    }
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next_u64() % items.len() as u64) as usize]
    }
}

/// Common panel sizes in physical pixels, including portrait and a small dummy.
const SIZES: &[(i32, i32)] = &[
    (1920, 1080),
    (2560, 1440),
    (3440, 1440),
    (1280, 1024),
    (1080, 1920),
    (1600, 900),
    (3840, 2160),
    (800, 600),
];

fn overlaps(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

/// A layout of `1..=max_screens` screens. The first is primary at the origin (Windows puts the primary's
/// top-left at 0,0); each next screen is attached to a random side of a random earlier screen, with a
/// random offset along that side and a gap of 0-3 px, never overlapping. Ids are synthetic.
pub fn layout(rng: &mut SplitMix64, max_screens: usize) -> Vec<WireScreen> {
    let n = rng.range(1, max_screens.max(1) as i64) as usize;
    let (w0, h0) = *rng.pick(SIZES);
    let mut rects = vec![(0, 0, w0, h0)];
    let mut attempts = 0;
    while rects.len() < n && attempts < 200 {
        attempts += 1;
        let base = *rng.pick(&rects);
        let (w, h) = *rng.pick(SIZES);
        let gap = if rng.chance(70) { 0 } else { rng.range(1, 3) as i32 };
        let side = rng.range(0, 3);
        let r = match side {
            0 => {
                // above
                let x = rng.range((base.0 - w + 1) as i64, (base.2 - 1) as i64) as i32;
                (x, base.1 - gap - h, x + w, base.1 - gap)
            }
            1 => {
                // below
                let x = rng.range((base.0 - w + 1) as i64, (base.2 - 1) as i64) as i32;
                (x, base.3 + gap, x + w, base.3 + gap + h)
            }
            2 => {
                // left
                let y = rng.range((base.1 - h + 1) as i64, (base.3 - 1) as i64) as i32;
                (base.0 - gap - w, y, base.0 - gap, y + h)
            }
            _ => {
                // right
                let y = rng.range((base.1 - h + 1) as i64, (base.3 - 1) as i64) as i32;
                (base.2 + gap, y, base.2 + gap + w, y + h)
            }
        };
        if rects.iter().all(|&o| !overlaps(o, r)) {
            rects.push(r);
        }
    }
    rects
        .iter()
        .enumerate()
        .map(|(i, &(l, t, r, b))| WireScreen {
            id: format!("MONITOR\\SYN{:04X}\\{{synthetic-{i}}}", 0x1000 + i),
            device: format!("\\\\.\\DISPLAY{}", i + 1),
            l,
            t,
            r,
            b,
            primary: i == 0,
        })
        .collect()
}

/// A move stream aimed at seams: from a point near a random edge of a random screen, steps of 1-12 px
/// toward and along the edge (slow drift, sliding, jitter), plus occasional flicks of up to 400 px and
/// occasional toggle, pause, resume and reconfigure ops.
pub fn ops(rng: &mut SplitMix64, screens: &[WireScreen], len: usize) -> Vec<Op> {
    let mut out = Vec::with_capacity(len);
    if screens.is_empty() {
        return out;
    }
    let s = rng.pick(screens).clone();
    let mut x = rng.range(s.l as i64, (s.r - 1) as i64) as i32;
    let mut y = rng.range(s.t as i64, (s.b - 1) as i64) as i32;
    // Start near one edge of the screen.
    match rng.range(0, 3) {
        0 => y = s.t + rng.range(0, 6) as i32,
        1 => y = s.b - 1 - rng.range(0, 6) as i32,
        2 => x = s.l + rng.range(0, 6) as i32,
        _ => x = s.r - 1 - rng.range(0, 6) as i32,
    }
    let (mut dx, mut dy) = (rng.range(-6, 6) as i32, rng.range(-6, 6) as i32);
    for _ in 0..len {
        let roll = rng.range(0, 99);
        if roll < 3 {
            out.push(Op::Gate { on: rng.chance(50) });
            continue;
        }
        if roll < 5 {
            out.push(Op::Side { on: rng.chance(50) });
            continue;
        }
        if roll < 6 {
            out.push(Op::Confine { on: rng.chance(30) });
            continue;
        }
        if roll == 6 {
            out.push(if rng.chance(50) { Op::Pause } else { Op::Resume });
            continue;
        }
        if roll == 7 {
            out.push(Op::Reset);
            continue;
        }
        if roll < 14 {
            // flick
            x += rng.range(-400, 400) as i32;
            y += rng.range(-400, 400) as i32;
        } else {
            if rng.chance(20) {
                dx = rng.range(-12, 12) as i32;
                dy = rng.range(-12, 12) as i32;
            }
            x += dx;
            y += dy;
        }
        out.push(Op::Move { x, y });
    }
    out
}

/// One complete random `run` case. `settings` is left null (defaults); Track A adds settings variations.
pub fn run_case(seed: u64, max_screens: usize, len: usize) -> RunCase {
    let mut rng = SplitMix64::new(seed);
    let screens = layout(&mut rng, max_screens);
    let ops = ops(&mut rng, &screens, len);
    let toggles = WireToggles {
        gate_open: rng.chance(50),
        side_contain: rng.chance(70),
        confine: rng.chance(5),
    };
    RunCase {
        toggles,
        ..RunCase::new(format!("seed-{seed}"), screens, ops)
    }
}

// ================================================================ Track A: engine corpus
//
// Everything below is additive: the generators above keep their output for every seed. The engine corpus
// draws its own random stream from `seed` (salted), so `run_case(seed)` and `engine_case(seed)` share the
// layout and the first op stream of a seed but nothing else.

use fence_types::{
    BoundaryDefault, BoundaryRule, Control, Direction, MonitorInfo, Policy, Rect, RuleBehavior, TopMode,
};
use std::cmp::Ordering;

use crate::case::{CoreCase, CoreOp, WireCore, WireGraph, WireGraphScreen};

const ENGINE_SALT: u64 = 0xA5A5_0E1E_C0DE_0001;
const CORE_SALT: u64 = 0xC0DE_F0BB_1D0E_0002;

/// One engine corpus case: the reference request (settings as the schema-v4 JSON 1.x loads) and the same settings
/// as the engine-facing `Policy` (already normalized: in-range numbers, valid rules in 1.x's saved order).
#[derive(Clone, Debug, PartialEq)]
pub struct EngineCase {
    pub run: RunCase,
    pub policy: Policy,
}

fn upper_char(c: char) -> char {
    let mut it = c.to_uppercase();
    match (it.next(), it.next()) {
        (Some(u), None) => u,
        _ => c,
    }
}

/// `StringComparer.OrdinalIgnoreCase.Compare`.
fn cmp_ignore_case(a: &str, b: &str) -> Ordering {
    let fold = |s: &str| -> Vec<u16> {
        let mut buf = [0u16; 2];
        s.chars()
            .flat_map(|c| upper_char(c).encode_utf16(&mut buf).to_vec())
            .collect()
    };
    fold(a).cmp(&fold(b))
}

/// `StringComparer.Ordinal.Compare` (UTF-16 code units).
fn cmp_ordinal(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// C# `MonitorInfo.All()` without Win32: `SortAndNumber` (Left, Top, StableId ignore-case then ordinal, Device
/// ignore-case then ordinal, Right, Bottom; index = position + 1), then `DisambiguateKeys` (a raw key shared by
/// 2+ monitors, compared ignore-case, gets `#0`, `#1`, ... by Left, Top, ordinal raw identity `id\ndevice`).
pub fn monitors_like_csharp(screens: &[WireScreen]) -> Vec<MonitorInfo> {
    let mut list: Vec<MonitorInfo> = screens
        .iter()
        .map(|s| MonitorInfo {
            device: s.device.clone(),
            stable_id: s.id.clone(),
            bounds: Rect::new(s.l, s.t, s.r, s.b),
            primary: s.primary,
            index: 0,
        })
        .collect();
    list.sort_by(|a, b| {
        a.bounds
            .l
            .cmp(&b.bounds.l)
            .then(a.bounds.t.cmp(&b.bounds.t))
            .then_with(|| cmp_ignore_case(&a.stable_id, &b.stable_id))
            .then_with(|| cmp_ordinal(&a.stable_id, &b.stable_id))
            .then_with(|| cmp_ignore_case(&a.device, &b.device))
            .then_with(|| cmp_ordinal(&a.device, &b.device))
            .then(a.bounds.r.cmp(&b.bounds.r))
            .then(a.bounds.b.cmp(&b.bounds.b))
    });
    for (i, m) in list.iter_mut().enumerate() {
        m.index = i as u32 + 1;
    }
    let raw: Vec<String> = list.iter().map(|m| m.stable_id.clone()).collect();
    let mut done = vec![false; list.len()];
    for i in 0..list.len() {
        if done[i] {
            continue;
        }
        let mut members: Vec<usize> = (i..list.len())
            .filter(|&j| !done[j] && cmp_ignore_case(&raw[i], &raw[j]) == Ordering::Equal)
            .collect();
        for &j in &members {
            done[j] = true;
        }
        if members.len() < 2 {
            continue;
        }
        let identity = |j: usize| format!("{}\n{}", raw[j], list[j].device);
        members.sort_by(|&a, &b| {
            list[a]
                .bounds
                .l
                .cmp(&list[b].bounds.l)
                .then(list[a].bounds.t.cmp(&list[b].bounds.t))
                .then_with(|| cmp_ordinal(&identity(a), &identity(b)))
        });
        for (ord, &j) in members.iter().enumerate() {
            list[j].stable_id = format!("{}#{ord}", raw[j]);
        }
    }
    list
}

fn default_name(d: BoundaryDefault) -> &'static str {
    match d {
        BoundaryDefault::Automatic => "Automatic",
        BoundaryDefault::Free => "Free",
        BoundaryDefault::Push => "Push",
        BoundaryDefault::Blocked => "Blocked",
    }
}

fn rule_behavior_name(b: RuleBehavior) -> &'static str {
    match b {
        RuleBehavior::Free => "Free",
        RuleBehavior::Push => "Push",
        RuleBehavior::Blocked => "Blocked",
        RuleBehavior::OverallFeel => "OverallFeel",
    }
}

fn control_name(c: Control) -> &'static str {
    match c {
        Control::Automatic => "Automatic",
        Control::Always => "Always",
    }
}

/// The schema-v4 settings JSON object (C# property names) holding exactly this policy. 1.x's `Normalize` leaves
/// it unchanged when the policy is in range (numbers within the C# clamps, valid rules deduplicated and in
/// saved order).
pub fn policy_to_settings_json(p: &Policy) -> serde_json::Value {
    let rules: Vec<serde_json::Value> = p
        .rules
        .iter()
        .map(|r| {
            serde_json::json!({
                "FromStableId": r.from_stable_id,
                "ToStableId": r.to_stable_id,
                "Direction": r.direction.name(),
                "Behavior": rule_behavior_name(r.behavior),
                "Control": control_name(r.control),
            })
        })
        .collect();
    serde_json::json!({
        "SettingsVersion": 4,
        "UpwardBoundaryDefault": default_name(p.upward_default),
        "SideBoundaryDefault": default_name(p.side_default),
        "DownwardBoundaryDefault": default_name(p.downward_default),
        "BoundaryRules": rules,
        "UpCrossMin": p.up_cross_min,
        "UpCrossSlack": p.up_cross_slack,
        "SideCrossMin": p.side_cross_min,
        "SideCrossSlack": p.side_cross_slack,
        "PassageInsetPx": p.passage_inset_px,
        "EdgeHoldbackPx": p.edge_holdback_px,
        "SeamTolerancePx": p.seam_tolerance_px,
        "SafetyEscapes": p.safety_escapes,
        "DeliberateCross": p.deliberate_cross,
        "DescentRouting": p.descent_routing,
        "Mode": match p.mode { TopMode::AutoTop => "AutoTop", TopMode::Manual => "Manual" },
        "ManualMonitors": p.manual_monitors,
        "ExcludedDevices": p.excluded_devices,
    })
}

const DIRECTIONS: [Direction; 4] = [Direction::Up, Direction::Down, Direction::Left, Direction::Right];

fn pick_default(rng: &mut SplitMix64, automatic_percent: u64, fallback: BoundaryDefault) -> BoundaryDefault {
    if rng.chance(automatic_percent) {
        return fallback;
    }
    *rng.pick(&[
        BoundaryDefault::Automatic,
        BoundaryDefault::Free,
        BoundaryDefault::Push,
        BoundaryDefault::Blocked,
    ])
}

/// An id that refers to a screen of the layout: as written, or (sometimes) with another letter case, which
/// the C# matches case-insensitively in rules, manual tops and exclusions.
fn layout_id(rng: &mut SplitMix64, screens: &[WireScreen], case_flip_percent: u64) -> String {
    let id = rng.pick(screens).id.clone();
    if rng.chance(case_flip_percent) {
        id.to_lowercase()
    } else {
        id
    }
}

/// A random in-range policy over the layout's ids (rules, manual tops, exclusions, defaults, numbers).
/// Rules are valid and deduplicated by their case-insensitive identity, in 1.x's saved order (from, to
/// ordinal, then direction), so the JSON and the policy describe the same normalized settings.
pub fn policy_variant(rng: &mut SplitMix64, screens: &[WireScreen]) -> Policy {
    let mut p = Policy {
        upward_default: pick_default(rng, 45, BoundaryDefault::Automatic),
        side_default: pick_default(rng, 45, BoundaryDefault::Automatic),
        downward_default: pick_default(rng, 55, BoundaryDefault::Free),
        ..Policy::default()
    };
    if rng.chance(50) {
        p.up_cross_min = rng.range(1, 12) as i32;
        p.up_cross_slack = rng.range(0, 12) as i32;
    }
    if rng.chance(50) {
        p.side_cross_min = rng.range(1, 12) as i32;
        p.side_cross_slack = rng.range(0, 12) as i32;
    }
    let any_inset = rng.range(1, 80) as i32;
    p.passage_inset_px = *rng.pick(&[24, 24, 0, 0, any_inset, 500]);
    p.edge_holdback_px = if rng.chance(60) { 0 } else { rng.range(1, 12) as i32 };
    p.seam_tolerance_px = if rng.chance(60) { 2 } else { rng.range(0, 5) as i32 };
    p.safety_escapes = rng.chance(80);
    p.deliberate_cross = rng.chance(85);
    p.descent_routing = rng.chance(35);

    if rng.chance(25) {
        p.mode = TopMode::Manual;
        for _ in 0..rng.range(0, 2) {
            // Configure matches manual tops exactly, the compiler case-insensitively: flips probe both.
            let id = layout_id(rng, screens, 10);
            if !p.manual_monitors.contains(&id) {
                p.manual_monitors.push(id);
            }
        }
    }
    if rng.chance(25) {
        for _ in 0..rng.range(1, 2) {
            let id = if rng.chance(15) {
                "MONITOR\\GONE\\{absent}".to_string()
            } else {
                layout_id(rng, screens, 20)
            };
            if !p.excluded_devices.iter().any(|e| e.eq_ignore_ascii_case(&id)) {
                p.excluded_devices.push(id);
            }
        }
    }

    let mut rules: Vec<BoundaryRule> = Vec::new();
    if screens.len() >= 2 && rng.chance(55) {
        for _ in 0..rng.range(1, 6) {
            let from = if rng.chance(8) {
                "MONITOR\\DOCK\\{dormant}".to_string()
            } else {
                layout_id(rng, screens, 10)
            };
            let to = layout_id(rng, screens, 10);
            if from.eq_ignore_ascii_case(&to) {
                continue;
            }
            let rule = BoundaryRule {
                from_stable_id: from,
                to_stable_id: to,
                direction: *rng.pick(&DIRECTIONS),
                behavior: *rng.pick(&[
                    RuleBehavior::Free,
                    RuleBehavior::Push,
                    RuleBehavior::Blocked,
                    RuleBehavior::Blocked,
                    RuleBehavior::OverallFeel,
                ]),
                control: if rng.chance(70) {
                    Control::Automatic
                } else {
                    Control::Always
                },
            };
            // Last valid wins for the same case-insensitive identity.
            rules.retain(|r| {
                !(r.from_stable_id.eq_ignore_ascii_case(&rule.from_stable_id)
                    && r.to_stable_id.eq_ignore_ascii_case(&rule.to_stable_id)
                    && r.direction == rule.direction)
            });
            rules.push(rule);
        }
    }
    rules.sort_by(|a, b| {
        cmp_ordinal(&a.from_stable_id, &b.from_stable_id)
            .then_with(|| cmp_ordinal(&a.to_stable_id, &b.to_stable_id))
            .then(a.direction.csharp_ordinal().cmp(&b.direction.csharp_ordinal()))
    });
    p.rules = rules;
    p
}

/// Seam-aimed motion for the engine corpus: slow drifts, deliberate pushes, slides, edge approaches that then
/// cross, flicks onto other screens and into desktop voids, plus the live switches (gate, side, game mode),
/// pause/resume and reconfigure.
pub fn ops_engine(rng: &mut SplitMix64, screens: &[WireScreen], len: usize) -> Vec<Op> {
    let mut out = Vec::with_capacity(len);
    if screens.is_empty() {
        return out;
    }
    let (min_l, min_t, max_r, max_b) = screens.iter().fold((i32::MAX, i32::MAX, i32::MIN, i32::MIN), |a, s| {
        (a.0.min(s.l), a.1.min(s.t), a.2.max(s.r), a.3.max(s.b))
    });
    let s = rng.pick(screens).clone();
    let mut x = rng.range(s.l as i64, (s.r - 1) as i64) as i32;
    let mut y = rng.range(s.t as i64, (s.b - 1) as i64) as i32;
    let (mut dx, mut dy) = (rng.range(-6, 6) as i32, rng.range(-6, 6) as i32);
    let mut pending_cross: Option<(i32, i32)> = None;
    while out.len() < len {
        if let Some((cx, cy)) = pending_cross.take() {
            x += cx;
            y += cy;
            out.push(Op::Move { x, y });
            continue;
        }
        let roll = rng.range(0, 99);
        match roll {
            0..=2 => out.push(Op::Gate { on: rng.chance(50) }),
            3..=4 => out.push(Op::Side { on: rng.chance(50) }),
            5 => out.push(Op::Confine { on: rng.chance(35) }),
            6 => out.push(if rng.chance(50) { Op::Pause } else { Op::Resume }),
            7 => out.push(Op::Reset),
            8..=17 => {
                // Flick: onto a screen near one of its edges, or anywhere around the desktop (voids included).
                if rng.chance(60) {
                    let t = rng.pick(screens).clone();
                    x = rng.range(t.l as i64, (t.r - 1) as i64) as i32;
                    y = rng.range(t.t as i64, (t.b - 1) as i64) as i32;
                    match rng.range(0, 3) {
                        0 => y = t.t + rng.range(0, 8) as i32,
                        1 => y = t.b - 1 - rng.range(0, 8) as i32,
                        2 => x = t.l + rng.range(0, 8) as i32,
                        _ => x = t.r - 1 - rng.range(0, 8) as i32,
                    }
                } else {
                    x = rng.range(min_l as i64 - 200, max_r as i64 + 200) as i32;
                    y = rng.range(min_t as i64 - 200, max_b as i64 + 200) as i32;
                }
                out.push(Op::Move { x, y });
            }
            18..=29 => {
                // Edge approach: stand 0-3 px inside an edge of a screen, then cross it with a push or a drift.
                let t = rng.pick(screens).clone();
                x = rng.range(t.l as i64, (t.r - 1) as i64) as i32;
                y = rng.range(t.t as i64, (t.b - 1) as i64) as i32;
                let depth = rng.range(0, 3) as i32;
                let step = if rng.chance(50) {
                    rng.range(1, 2) as i32
                } else {
                    rng.range(3, 40) as i32
                };
                let orth = rng.range(-6, 6) as i32;
                let cross = match rng.range(0, 3) {
                    0 => {
                        y = t.t + depth;
                        (orth, -step - depth)
                    }
                    1 => {
                        y = t.b - 1 - depth;
                        (orth, step + depth)
                    }
                    2 => {
                        x = t.l + depth;
                        (-step - depth, orth)
                    }
                    _ => {
                        x = t.r - 1 - depth;
                        (step + depth, orth)
                    }
                };
                out.push(Op::Move { x, y });
                pending_cross = Some(cross);
            }
            30..=44 => {
                // Deliberate push along one axis with a little wobble.
                let step = rng.range(3, 40) as i32;
                let wobble = rng.range(-5, 5) as i32;
                match rng.range(0, 3) {
                    0 => (dx, dy) = (wobble, -step),
                    1 => (dx, dy) = (wobble, step),
                    2 => (dx, dy) = (-step, wobble),
                    _ => (dx, dy) = (step, wobble),
                }
                x += dx;
                y += dy;
                out.push(Op::Move { x, y });
            }
            45..=59 => {
                // Slow drift (below every push threshold).
                let (sx, sy) = (rng.range(-2, 2) as i32, rng.range(-2, 2) as i32);
                x += sx;
                y += sy;
                out.push(Op::Move { x, y });
            }
            _ => {
                if rng.chance(20) {
                    dx = rng.range(-12, 12) as i32;
                    dy = rng.range(-12, 12) as i32;
                }
                x += dx;
                y += dy;
                out.push(Op::Move { x, y });
            }
        }
    }
    out
}

/// One engine corpus case: a layout of up to `max_screens` synthetic screens (the `layout` generator), a
/// settings variation (null settings, i.e. `new Settings()`, in 15% of cases), random starting switches and
/// `len` ops of `ops_engine`.
pub fn engine_case(seed: u64, max_screens: usize, len: usize) -> EngineCase {
    let mut rng = SplitMix64::new(seed);
    let screens = layout(&mut rng, max_screens);
    let mut rng = SplitMix64::new(seed ^ ENGINE_SALT);
    let (settings, policy) = if rng.chance(15) {
        (None, Policy::default())
    } else {
        let p = policy_variant(&mut rng, &screens);
        (Some(policy_to_settings_json(&p)), p)
    };
    let toggles = WireToggles {
        gate_open: rng.chance(50),
        side_contain: rng.chance(70),
        confine: rng.chance(5),
    };
    let ops = ops_engine(&mut rng, &screens, len);
    EngineCase {
        run: RunCase {
            settings,
            toggles,
            ..RunCase::new(format!("engine-seed-{seed}"), screens, ops)
        },
        policy,
    }
}

fn rect_arr(r: (i32, i32, i32, i32)) -> [i32; 4] {
    [r.0, r.1, r.2, r.3]
}

/// A forbidden-heavy GuardCore built field by field (the `core` verb): the corpus layout as usable monitors,
/// excluded screens or arbitrary rects (adjacent chains, clones of usable screens, rects across voids) as
/// forbidden, a random barrier with user and safety gates, side containment around the primary, descent
/// routes, game mode, and moves with random live switches. Exercises the wall-off resolution far beyond what
/// Configure can produce (it never makes overlapping usable/forbidden geometry).
pub fn core_case(seed: u64, max_screens: usize, len: usize) -> CoreCase {
    let mut rng = SplitMix64::new(seed ^ CORE_SALT);
    let screens = layout(&mut rng, max_screens);
    let rects: Vec<(i32, i32, i32, i32)> = screens.iter().map(|s| (s.l, s.t, s.r, s.b)).collect();
    let mut core = WireCore::default();

    // Usable monitors: all screens, minus some that become forbidden (never all of them).
    let mut usable = Vec::new();
    let mut forbidden = Vec::new();
    for (i, r) in rects.iter().enumerate() {
        if i > 0 && rng.chance(25) {
            forbidden.push(rect_arr(*r));
        } else {
            usable.push(rect_arr(*r));
        }
    }
    let (min_l, min_t, max_r, max_b) = rects.iter().fold((i32::MAX, i32::MAX, i32::MIN, i32::MIN), |a, r| {
        (a.0.min(r.0), a.1.min(r.1), a.2.max(r.2), a.3.max(r.3))
    });
    for _ in 0..rng.range(0, 3) {
        match rng.range(0, 3) {
            0 => {
                // a clone of a usable screen (usable geometry wins)
                let u = *rng.pick(&usable);
                forbidden.push(u);
            }
            1 => {
                // a chain of 1-3 adjacent dummies to the right of a random screen
                let r = *rng.pick(&rects);
                let (w, h) = (rng.range(50, 900) as i32, rng.range(50, 900) as i32);
                let y0 = rng.range(r.1 as i64 - h as i64 / 2, r.3 as i64) as i32;
                let mut x0 = r.2;
                for _ in 0..rng.range(1, 3) {
                    forbidden.push([x0, y0, x0 + w, y0 + h]);
                    x0 += w;
                }
            }
            2 => {
                let l = rng.range(min_l as i64 - 300, max_r as i64) as i32;
                let t = rng.range(min_t as i64 - 300, max_b as i64) as i32;
                forbidden.push([l, t, l + rng.range(1, 1200) as i32, t + rng.range(1, 1200) as i32]);
            }
            _ => {
                // an empty rect (never forbids anything)
                let l = rng.range(min_l as i64, max_r as i64) as i32;
                forbidden.push([l, 0, l, 100]);
            }
        }
    }
    if rng.chance(5) {
        usable.clear(); // geometric fallback only
    }
    core.monitors = usable;
    core.forbidden = forbidden;

    if rng.chance(60) {
        core.has_top = true;
        core.barrier_y = if rng.chance(70) { 0 } else { rng.range(-400, 400) as i32 };
        for _ in 0..rng.range(0, 2) {
            let a = rng.range(min_l as i64, max_r as i64) as i32;
            core.gates.push([a, a + rng.range(-20, 900) as i32]);
        }
        if rng.chance(30) {
            let r = *rng.pick(&rects);
            core.safety_gates.push([r.0, r.0 + rng.range(10, 600) as i32, r.1, r.3]);
        }
    }
    if rng.chance(40) {
        core.has_sides = true;
        core.main = rect_arr(rects[0]);
    }
    core.deliberate_cross = rng.chance(85);
    core.side_cross_min = rng.range(1, 10) as i32;
    core.side_cross_slack = rng.range(0, 8) as i32;
    if rng.chance(15) {
        core.cross_min_up = rng.range(1, 10) as i32;
        core.cross_slack = rng.range(0, 10) as i32;
    }
    if rects.len() >= 2 && rng.chance(30) {
        core.descent_routing = rng.chance(80);
        for _ in 0..rng.range(1, 2) {
            let top = *rng.pick(&rects);
            let landing = *rng.pick(&rects);
            core.descent_routes.push([rect_arr(top), rect_arr(landing)]);
        }
    }
    core.confine = rng.chance(8);
    let ops = core_ops(&mut rng, &screens, len);
    CoreCase::new(format!("core-seed-{seed}"), core, ops)
}

/// `ops_engine` as `core` ops: the live switches ride on each move; pause/resume do not exist on a bare core.
fn core_ops(rng: &mut SplitMix64, screens: &[WireScreen], len: usize) -> Vec<CoreOp> {
    let mut ops = Vec::with_capacity(len);
    let walk = ops_engine(rng, screens, len);
    let mut gate = rng.chance(50);
    let mut side = rng.chance(60);
    for op in walk {
        let next = match op {
            Op::Move { x, y } => CoreOp::Move { x, y, gate, side },
            Op::Gate { on } => {
                gate = on;
                continue;
            }
            Op::Side { on } => {
                side = on;
                continue;
            }
            Op::Confine { on } => CoreOp::Confine { on },
            Op::Pause | Op::Resume => continue,
            Op::Reset => CoreOp::Reset,
        };
        ops.push(next);
    }
    if rng.chance(30) {
        let s = rng.pick(screens);
        ops.insert(0, CoreOp::SetLast { x: s.l + 1, y: s.t + 1 });
    }
    ops
}

const GRAPH_SALT: u64 = 0x6EA9_C0DE_5EED_0003;

/// The 1.1.1 tests' `GraphCore` path at random: `BoundaryGraph.Compile(raw settings, screens, safety)` over a
/// corpus layout with some screens excluded, in a shuffled input order and with geometry safety on or off;
/// the core gets the compiled transitions and the non-excluded rects, plus (sometimes) a top barrier, side
/// containment, descent routes and walled excluded screens. Returns the case and the settings as a policy.
pub fn core_graph_case(seed: u64, max_screens: usize, len: usize) -> (CoreCase, Policy) {
    let mut rng = SplitMix64::new(seed ^ GRAPH_SALT);
    let screens = layout(&mut rng, max_screens);
    let policy = if rng.chance(10) {
        Policy::default()
    } else {
        policy_variant(&mut rng, &screens)
    };
    let monitors = monitors_like_csharp(&screens);
    let rect = |m: &MonitorInfo| [m.bounds.l, m.bounds.t, m.bounds.r, m.bounds.b];
    let mut graph: Vec<WireGraphScreen> = monitors
        .iter()
        .map(|m| WireGraphScreen {
            id: m.stable_id.clone(),
            rect: rect(m),
            primary: m.primary,
            excluded: rng.chance(12),
        })
        .collect();
    if graph.len() > 1 {
        match rng.range(0, 3) {
            0 => graph.reverse(),
            1 => {
                let k = rng.range(1, graph.len() as i64 - 1) as usize;
                graph.rotate_left(k);
            }
            _ => {}
        }
    }
    let safety = rng.chance(75);

    let mut core = WireCore {
        monitors: graph.iter().filter(|s| !s.excluded).map(|s| s.rect).collect(),
        ..WireCore::default()
    };
    if rng.chance(50) {
        // The Configure-style barrier: the tops by geometry, their maximum bottom.
        let tops: Vec<&MonitorInfo> = monitors.iter().filter(|m| m.bounds.b <= 0 && m.bounds.t < 0).collect();
        core.has_top = !tops.is_empty() || rng.chance(30);
        core.barrier_y = tops.iter().map(|m| m.bounds.b).max().unwrap_or(0);
        if rng.chance(50) {
            core.gates.push([24, rng.range(25, 3000) as i32]);
        }
    }
    if rng.chance(40) {
        core.has_sides = true;
        core.main = rect(&monitors[0]);
        if let Some(p) = monitors.iter().find(|m| m.primary) {
            core.main = rect(p);
        }
    }
    if rng.chance(40) {
        core.forbidden = graph.iter().filter(|s| s.excluded).map(|s| s.rect).collect();
    }
    if monitors.len() >= 2 && rng.chance(25) {
        core.descent_routing = true;
        let top = rect(rng.pick(&monitors));
        let landing = rect(rng.pick(&monitors));
        core.descent_routes.push([top, landing]);
    }
    core.deliberate_cross = policy.deliberate_cross;
    core.side_cross_min = policy.side_cross_min.max(1);
    core.side_cross_slack = policy.side_cross_slack.max(0);
    core.confine = rng.chance(5);
    let ops = core_ops(&mut rng, &screens, len);
    let case = CoreCase {
        graph: Some(WireGraph {
            settings: Some(policy_to_settings_json(&policy)),
            screens: graph,
            safety,
        }),
        ..CoreCase::new(format!("graph-seed-{seed}"), core, ops)
    };
    (case, policy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s_gen_engine_case_is_pinned_and_seed_stable() {
        assert_eq!(engine_case(7, 6, 64), engine_case(7, 6, 64));
        assert_ne!(engine_case(7, 6, 64), engine_case(8, 6, 64));
        assert_eq!(core_case(7, 6, 64), core_case(7, 6, 64));
        // The engine corpus shares its layout with run_case for the same seed.
        assert_eq!(engine_case(42, 6, 64).run.screens, run_case(42, 6, 64).screens);
    }

    #[test]
    fn s_gen_engine_corpus_reaches_every_settings_branch() {
        let (mut manual, mut excluded, mut rules, mut nulls, mut routing, mut blocked_up) = (0, 0, 0, 0, 0, 0);
        for seed in 0..2_000u64 {
            let c = engine_case(seed, 6, 64);
            assert_eq!(c.run.ops.len(), 64);
            match &c.run.settings {
                None => nulls += 1,
                Some(v) => assert_eq!(v, &policy_to_settings_json(&c.policy)),
            }
            manual += usize::from(c.policy.mode == TopMode::Manual);
            excluded += usize::from(!c.policy.excluded_devices.is_empty());
            rules += usize::from(!c.policy.rules.is_empty());
            routing += usize::from(c.policy.descent_routing);
            blocked_up += usize::from(c.policy.upward_default == BoundaryDefault::Blocked);
            for r in &c.policy.rules {
                assert!(!r.from_stable_id.eq_ignore_ascii_case(&r.to_stable_id));
            }
        }
        for (name, n) in [
            ("manual", manual),
            ("excluded", excluded),
            ("rules", rules),
            ("null settings", nulls),
            ("descent routing", routing),
            ("custom up links", blocked_up),
        ] {
            assert!(n > 100, "{name}: only {n} of 2000 cases");
        }
    }

    #[test]
    fn s_gen_monitors_like_csharp_sort_and_disambiguate() {
        let screens = vec![
            screen_for_test("B", "\\\\.\\DISPLAY2", 100, 0),
            screen_for_test("dup", "\\\\.\\DISPLAY3", 300, 0),
            screen_for_test("a", "\\\\.\\DISPLAY1", 0, 0),
            screen_for_test("DUP", "\\\\.\\DISPLAY4", 200, 0),
        ];
        let m = monitors_like_csharp(&screens);
        let got: Vec<(&str, u32)> = m.iter().map(|m| (m.stable_id.as_str(), m.index)).collect();
        assert_eq!(got, vec![("a", 1), ("B", 2), ("DUP#0", 3), ("dup#1", 4)]);
    }

    fn screen_for_test(id: &str, device: &str, l: i32, t: i32) -> WireScreen {
        WireScreen {
            id: id.into(),
            device: device.into(),
            l,
            t,
            r: l + 100,
            b: t + 100,
            primary: l == 0,
        }
    }

    #[test]
    fn s_gen_splitmix_is_pinned() {
        // Pinned outputs: changing the generator would silently invalidate recorded failing seeds.
        let mut r = SplitMix64::new(0);
        assert_eq!(r.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(r.next_u64(), 0x6E78_9E6A_A1B9_65F4);
    }

    #[test]
    fn s_gen_same_seed_same_case() {
        assert_eq!(run_case(42, 6, 200), run_case(42, 6, 200));
        assert_ne!(run_case(42, 6, 200), run_case(43, 6, 200));
    }

    #[test]
    fn s_gen_layouts_never_overlap_and_primary_is_at_origin() {
        let mut checked = 0;
        for seed in 0..2_000u64 {
            let mut rng = SplitMix64::new(seed);
            let screens = layout(&mut rng, 6);
            assert!(!screens.is_empty());
            assert!(screens[0].primary && screens[0].l == 0 && screens[0].t == 0);
            assert_eq!(screens.iter().filter(|s| s.primary).count(), 1);
            for (i, a) in screens.iter().enumerate() {
                assert!(a.r > a.l && a.b > a.t);
                for b in &screens[i + 1..] {
                    assert!(!overlaps((a.l, a.t, a.r, a.b), (b.l, b.t, b.r, b.b)), "seed {seed}");
                }
            }
            checked += 1;
        }
        assert_eq!(checked, 2_000);
    }

    #[test]
    fn s_gen_corpus_reaches_multi_screen_and_every_op_kind() {
        let mut multi = 0;
        let mut kinds = [0usize; 7];
        for seed in 0..500u64 {
            let c = run_case(seed, 6, 300);
            if c.screens.len() >= 3 {
                multi += 1;
            }
            for op in &c.ops {
                let k = match op {
                    Op::Move { .. } => 0,
                    Op::Gate { .. } => 1,
                    Op::Side { .. } => 2,
                    Op::Confine { .. } => 3,
                    Op::Pause => 4,
                    Op::Resume => 5,
                    Op::Reset => 6,
                };
                kinds[k] += 1;
            }
        }
        assert!(multi > 100, "only {multi} cases with 3+ screens");
        assert!(kinds.iter().all(|&k| k > 0), "{kinds:?}");
    }
}
