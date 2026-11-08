//! Test double for the engine and settings model (Tracks A and B are stubs until the join).
//!
//! `FakeBackend` is a reduced, faithful port of the C# pieces the window calls: `BoundaryGraph.Compile` (candidates,
//! rules, defaults, ordering, sealing and the safety demotion; without the legacy sole-up escape and the landing
//! sets, which the window never reads), `Settings.Normalize` (names, clamps, rule validation and order, import
//! errors; v4 documents only), `NormalizeMonitorName`, `GuardCore.SanitizeExcluded`, `GuardCore.MapLayout`,
//! `MonitorInfo.SortAndNumber`, `IsTop`, the WinForms key names the tests use, and a JSON form with the C# property
//! names. The `s_join_*` tests run the same scenarios against `RealBackend` (since the join, 2026-10-13).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use fence_config::{Settings, Stored, StoredRule};
use fence_types::{
    Behavior, BoundaryDefault, CompileResult, Control, Direction, MonitorInfo, Rect, RuleBehavior, RuntimeTransition,
    SealIssue,
};
use serde_json::{Map, Value, json};

use crate::backend::Backend;
use crate::boundary::same;
use crate::editor::Dialogs;

#[derive(Clone, Copy, Debug, Default)]
pub struct FakeBackend;

/// True when `MF_UI_REAL_BACKEND=1`: [`fake`] then hands out the real backend.
pub fn real_backend_on() -> bool {
    std::env::var("MF_UI_REAL_BACKEND").is_ok_and(|v| v == "1")
}

pub fn fake() -> Rc<dyn Backend> {
    // MF_UI_REAL_BACKEND=1 runs every window test against the real fence-config and fence-engine instead
    // (JUDGE-D J2: the fake re-implements the compiler and seal check and can drift from them).
    if real_backend_on() {
        return Rc::new(crate::backend::RealBackend);
    }
    Rc::new(FakeBackend)
}

pub fn mon(id: &str, index: u32, primary: bool, l: i32, t: i32, r: i32, b: i32) -> MonitorInfo {
    MonitorInfo {
        device: String::new(),
        stable_id: id.into(),
        bounds: Rect::new(l, t, r, b),
        primary,
        index,
    }
}

pub fn rule(from: &str, to: &str, d: Direction, b: RuleBehavior, c: Control) -> StoredRule {
    StoredRule {
        from_stable_id: from.into(),
        to_stable_id: to.into(),
        direction: Stored::Known(d),
        behavior: Stored::Known(b),
        control: Stored::Known(c),
    }
}

pub fn names(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

struct Candidate {
    from: usize,
    to: usize,
    direction: Direction,
    start: i32,
    end: i32,
    gap: i32,
}

fn known_default(v: &Stored<BoundaryDefault>) -> Option<BoundaryDefault> {
    match v {
        Stored::Known(x) => Some(*x),
        Stored::Unknown => None,
    }
}

/// C# `BoundaryGraph.Resolve`.
fn resolve_default(
    value: Option<BoundaryDefault>,
    direction: Direction,
    touches_primary: bool,
    gap: i32,
    seam: i32,
    automatic_upper: bool,
) -> Behavior {
    match value {
        Some(BoundaryDefault::Free) => Behavior::Free,
        Some(BoundaryDefault::Push) => Behavior::Push,
        Some(BoundaryDefault::Blocked) => Behavior::Blocked,
        Some(BoundaryDefault::Automatic) => match direction {
            Direction::Up if automatic_upper => Behavior::Push,
            Direction::Up | Direction::Down => Behavior::Free,
            _ if touches_primary && gap <= seam => Behavior::Push,
            _ => Behavior::Free,
        },
        None => Behavior::Blocked,
    }
}

fn subtract(spans: Vec<(i32, i32)>, cut_start: i32, cut_end: i32) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for (s, e) in spans {
        if cut_end <= s || cut_start >= e {
            out.push((s, e));
            continue;
        }
        if cut_start > s {
            out.push((s, cut_start.min(e)));
        }
        if cut_end < e {
            out.push((cut_end.max(s), e));
        }
    }
    out
}

struct Usable {
    id: String,
    bounds: Rect,
    primary: bool,
}

fn build_candidates(screens: &[Usable]) -> Vec<Candidate> {
    let mut out = Vec::new();
    for i in 0..screens.len() {
        for j in 0..screens.len() {
            if i == j {
                continue;
            }
            let (a, b) = (screens[i].bounds, screens[j].bounds);
            let (direction, start, end, gap, vertical) = if b.b <= a.t && a.r.min(b.r) > a.l.max(b.l) {
                (Direction::Up, a.l.max(b.l), a.r.min(b.r), a.t - b.b, true)
            } else if b.t >= a.b && a.r.min(b.r) > a.l.max(b.l) {
                (Direction::Down, a.l.max(b.l), a.r.min(b.r), b.t - a.b, true)
            } else if b.r <= a.l && a.b.min(b.b) > a.t.max(b.t) {
                (Direction::Left, a.t.max(b.t), a.b.min(b.b), a.l - b.r, false)
            } else if b.l >= a.r && a.b.min(b.b) > a.t.max(b.t) {
                (Direction::Right, a.t.max(b.t), a.b.min(b.b), b.l - a.r, false)
            } else {
                continue;
            };
            let mut spans = vec![(start, end)];
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
                let (cs, ce) = if vertical { (c.l, c.r) } else { (c.t, c.b) };
                spans = subtract(spans, cs, ce);
                if spans.is_empty() {
                    break;
                }
            }
            for (s, e) in spans.into_iter().filter(|(s, e)| e > s) {
                out.push(Candidate {
                    from: i,
                    to: j,
                    direction,
                    start: s,
                    end: e,
                    gap: gap.max(0),
                });
            }
        }
    }
    out
}

fn effective(t: &RuntimeTransition, gates: bool, side: bool) -> Behavior {
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
        Direction::Up if !gates => Behavior::Blocked,
        Direction::Left | Direction::Right if !side => Behavior::Free,
        _ => t.behavior,
    }
}

fn find_sealed(screens: &[Usable], ts: &[RuntimeTransition], runtime: Option<(bool, bool)>) -> Vec<SealIssue> {
    let mut issues = Vec::new();
    if screens.len() <= 1 {
        return issues;
    }
    for s in screens {
        let exits: Vec<&RuntimeTransition> = ts
            .iter()
            .filter(|t| same(t.from_stable_id.trim(), s.id.trim()))
            .collect();
        let usable = exits.iter().any(|t| match runtime {
            Some((g, sd)) => effective(t, g, sd) != Behavior::Blocked,
            None => matches!(t.behavior, Behavior::Free | Behavior::Push),
        });
        if exits.is_empty() || usable {
            continue;
        }
        let focus = exits
            .iter()
            .max_by(|a, b| {
                a.corridor_width()
                    .cmp(&b.corridor_width())
                    .then(b.sort_order.cmp(&a.sort_order))
            })
            .expect("non-empty");
        issues.push(SealIssue {
            stable_id: s.id.clone(),
            focus_to_stable_id: focus.to_stable_id.clone(),
            focus_direction: focus.direction,
            focus_corridor_width: focus.corridor_width(),
        });
    }
    issues
}

fn demote(issues: &[SealIssue], ts: &mut [RuntimeTransition]) {
    for issue in issues {
        let best = (0..ts.len())
            .filter(|&i| same(ts[i].from_stable_id.trim(), issue.stable_id.trim()))
            .max_by(|&a, &b| {
                ts[a]
                    .corridor_width()
                    .cmp(&ts[b].corridor_width())
                    .then(ts[b].sort_order.cmp(&ts[a].sort_order))
            });
        if let Some(i) = best {
            if ts[i].behavior == Behavior::Blocked {
                ts[i].behavior = Behavior::Push;
            }
            ts[i].safety_demoted = true;
        }
    }
}

fn rule_key(from: &str, to: &str, d: Direction) -> String {
    format!(
        "{}\n{}\n{}",
        from.trim().to_lowercase(),
        to.trim().to_lowercase(),
        d.name()
    )
}

/// The fake compile (C# `BoundaryGraph.Compile(settings, FromMonitors(monitors, excluded), safety)`).
pub fn fake_compile(settings: &Settings, monitors: &[MonitorInfo], excluded: &[String], safety: bool) -> CompileResult {
    let usable: Vec<Usable> = monitors
        .iter()
        .filter(|m| {
            !excluded.iter().any(|e| same(e, &m.stable_id)) && m.bounds.is_valid() && !m.stable_id.trim().is_empty()
        })
        .map(|m| Usable {
            id: m.stable_id.clone(),
            bounds: m.bounds,
            primary: m.primary,
        })
        .collect();
    let mut rules: Vec<(String, RuleBehavior, Control)> = Vec::new();
    for r in &settings.boundary_rules {
        let (Stored::Known(d), Stored::Known(b), Stored::Known(c)) = (&r.direction, &r.behavior, &r.control) else {
            continue;
        };
        if r.from_stable_id.trim().is_empty()
            || r.to_stable_id.trim().is_empty()
            || same(&r.from_stable_id, &r.to_stable_id)
        {
            continue;
        }
        let key = rule_key(&r.from_stable_id, &r.to_stable_id, *d);
        rules.retain(|(k, _, _)| *k != key);
        rules.push((key, *b, *c));
    }
    let manual = settings.mode == "Manual";
    let is_top = |s: &Usable| {
        if manual {
            settings.manual_monitors.iter().any(|x| same(x, &s.id))
        } else {
            s.bounds.b <= 0
        }
    };
    let mut ts: Vec<RuntimeTransition> = Vec::new();
    for c in build_candidates(&usable) {
        let (from, to) = (&usable[c.from], &usable[c.to]);
        let found = rules
            .iter()
            .find(|(k, _, _)| *k == rule_key(&from.id, &to.id, c.direction));
        let behavior = match found {
            None => {
                let value = match c.direction {
                    Direction::Up => known_default(&settings.upward_boundary_default),
                    Direction::Down => known_default(&settings.downward_boundary_default),
                    _ => known_default(&settings.side_boundary_default),
                };
                resolve_default(
                    value,
                    c.direction,
                    from.primary || to.primary,
                    c.gap,
                    settings.seam_tolerance_px,
                    is_top(to),
                )
            }
            Some((_, RuleBehavior::OverallFeel, _)) | Some((_, RuleBehavior::Push, _)) => Behavior::Push,
            Some((_, RuleBehavior::Free, _)) => Behavior::Free,
            Some((_, RuleBehavior::Blocked, _)) => Behavior::Blocked,
        };
        let side = matches!(c.direction, Direction::Left | Direction::Right);
        ts.push(RuntimeTransition {
            from_stable_id: from.id.clone(),
            to_stable_id: to.id.clone(),
            direction: c.direction,
            from: from.bounds,
            to: to.bounds,
            corridor_start: c.start,
            corridor_end: c.end,
            gap_px: c.gap,
            seam_tolerance_px: settings.seam_tolerance_px,
            behavior,
            control: found.map_or(Control::Automatic, |(_, _, c)| *c),
            push_min: if side {
                settings.side_cross_min
            } else {
                settings.up_cross_min
            },
            push_slack: if side {
                settings.side_cross_slack
            } else {
                settings.up_cross_slack
            },
            passage_inset_px: settings.passage_inset_px,
            edge_holdback_px: settings.edge_holdback_px,
            explicit_rule: found.is_some(),
            sole_up_safety_escape: false,
            safety_demoted: false,
            sort_order: 0,
            allowed_landings: Default::default(),
        });
    }
    ts.sort_by(|a, b| {
        let o = |d: Direction| if d.is_vertical() { 0 } else { 1 };
        o(a.direction)
            .cmp(&o(b.direction))
            .then(a.corridor_start.cmp(&b.corridor_start))
            .then(a.corridor_end.cmp(&b.corridor_end))
            .then(a.from_stable_id.cmp(&b.from_stable_id))
            .then(a.to_stable_id.cmp(&b.to_stable_id))
            .then(a.direction.csharp_ordinal().cmp(&b.direction.csharp_ordinal()))
    });
    for (i, t) in ts.iter_mut().enumerate() {
        t.sort_order = i as i32;
    }
    let sealed = find_sealed(&usable, &ts, None);
    if safety && settings.safety_escapes {
        demote(&sealed, &mut ts);
        let again = find_sealed(&usable, &ts, Some((false, true)));
        demote(&again, &mut ts);
    }
    CompileResult {
        transitions: ts,
        sealed_screens: sealed,
    }
}

fn is_combining(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
}

/// C# `Settings.NormalizeMonitorName`.
pub fn fake_normalize_name(value: &str) -> String {
    let clean: String = value
        .chars()
        .map(|c| if matches!(c, '\t' | '\r' | '\n') { ' ' } else { c })
        .collect();
    let clean = clean.trim();
    let mut elements = 0;
    for (i, c) in clean.char_indices() {
        if !is_combining(c) {
            if elements == 24 {
                return clean[..i].to_string();
            }
            elements += 1;
        }
    }
    clean.to_string()
}

fn key_name(vk: u32) -> String {
    match vk {
        0x41..=0x5A => char::from_u32(vk).expect("ascii").to_string(),
        0x30..=0x39 => format!("D{}", vk - 0x30),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        0x25 => "Left".into(),
        0x26 => "Up".into(),
        0x27 => "Right".into(),
        0x28 => "Down".into(),
        0x20 => "Space".into(),
        0x2E => "Delete".into(),
        0x2D => "Insert".into(),
        0x24 => "Home".into(),
        0x23 => "End".into(),
        0x21 => "PageUp".into(),
        0x22 => "Next".into(),
        0xBD => "OemMinus".into(),
        0xBB => "Oemplus".into(),
        _ => vk.to_string(),
    }
}

impl Backend for FakeBackend {
    fn compile(&self, settings: &Settings, monitors: &[MonitorInfo]) -> CompileResult {
        fake_compile(settings, monitors, &settings.excluded_devices, true)
    }

    fn validate_no_seal(&self, settings: &mut Settings, monitors: &[MonitorInfo]) -> Vec<SealIssue> {
        self.normalize(settings);
        let excluded = crate::backend::sanitized_excluded(self, settings, monitors);
        fake_compile(settings, monitors, &excluded, false).sealed_screens
    }

    fn normalize(&self, s: &mut Settings) {
        let mut pairs = std::mem::take(&mut s.monitor_names);
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        for (k, v) in pairs {
            let (k, v) = (k.trim().to_string(), fake_normalize_name(&v));
            if !k.is_empty() && !v.is_empty() && !s.monitor_names.iter().any(|(x, _)| same(x, &k)) {
                s.monitor_names.push((k, v));
            }
        }
        if s.settings_version < 4 {
            s.settings_version = 4;
        }
        s.up_cross_min = s.up_cross_min.clamp(1, 200);
        s.up_cross_slack = s.up_cross_slack.clamp(0, 200);
        s.side_cross_min = s.side_cross_min.clamp(1, 200);
        s.side_cross_slack = s.side_cross_slack.clamp(0, 200);
        s.passage_inset_px = s.passage_inset_px.clamp(0, 500);
        s.edge_holdback_px = s.edge_holdback_px.clamp(0, 200);
        s.seam_tolerance_px = s.seam_tolerance_px.clamp(0, 200);
        let mut repairable = [
            &s.upward_boundary_default,
            &s.side_boundary_default,
            &s.downward_boundary_default,
        ]
        .iter()
        .any(|v| **v == Stored::Unknown);
        let mut valid: Vec<StoredRule> = Vec::new();
        let mut invalid: Vec<StoredRule> = Vec::new();
        for mut r in std::mem::take(&mut s.boundary_rules) {
            r.from_stable_id = r.from_stable_id.trim().to_string();
            r.to_stable_id = r.to_stable_id.trim().to_string();
            let enum_invalid =
                r.direction == Stored::Unknown || r.behavior == Stored::Unknown || r.control == Stored::Unknown;
            if enum_invalid
                || r.from_stable_id.is_empty()
                || r.to_stable_id.is_empty()
                || same(&r.from_stable_id, &r.to_stable_id)
            {
                repairable = true;
                invalid.push(r);
                continue;
            }
            let key = |x: &StoredRule| {
                (
                    x.from_stable_id.to_lowercase(),
                    x.to_stable_id.to_lowercase(),
                    x.direction.clone(),
                )
            };
            let k = key(&r);
            if let Some(slot) = valid.iter_mut().find(|x| key(x) == k) {
                *slot = r;
            } else {
                valid.push(r);
            }
        }
        let ord = |d: &Stored<Direction>| match d {
            Stored::Known(d) => d.csharp_ordinal(),
            Stored::Unknown => 0,
        };
        valid.sort_by(|a, b| {
            a.from_stable_id
                .cmp(&b.from_stable_id)
                .then(a.to_stable_id.cmp(&b.to_stable_id))
                .then(ord(&a.direction).cmp(&ord(&b.direction)))
        });
        valid.extend(invalid);
        s.boundary_rules = valid;
        s.has_repairable_import_errors = repairable;
        if s.mode != "AutoTop" && s.mode != "Manual" {
            s.mode = "AutoTop".into();
        }
    }

    fn to_json(&self, s: &Settings) -> String {
        let mut s = s.clone();
        self.normalize(&mut s);
        serde_json::to_string_pretty(&to_value(&s)).expect("json")
    }

    fn parse_json(&self, text: &str) -> Result<Settings, String> {
        let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        from_value(&v)
    }

    fn migrate_layout_keys_to_stable(&self, settings: &mut Settings, _monitors: &[MonitorInfo]) -> Result<(), String> {
        self.normalize(settings);
        Ok(())
    }

    fn sanitize_excluded(&self, present: &[String], primary: Option<&str>, requested: &[String]) -> Vec<String> {
        let asked: Vec<String> = present
            .iter()
            .filter(|k| requested.iter().any(|r| same(r, k)))
            .cloned()
            .collect();
        let mut excluded: Vec<String> = asked.into_iter().filter(|k| Some(k.as_str()) != primary).collect();
        if excluded.len() >= present.len() && !excluded.is_empty() {
            excluded.pop();
        }
        excluded
    }

    fn map_layout(&self, rects: &[Rect], target_w: i32, target_h: i32, pad: i32) -> Vec<Rect> {
        if rects.is_empty() {
            return Vec::new();
        }
        let (tw, th) = (target_w.max(0), target_h.max(0));
        let pad = pad.clamp(0, tw.min(th) / 2);
        let (uw, uh) = ((tw - pad * 2).max(0), (th - pad * 2).max(0));
        let min_l = rects.iter().map(|r| r.l).min().expect("non-empty");
        let min_t = rects.iter().map(|r| r.t).min().expect("non-empty");
        let max_r = rects.iter().map(|r| r.r).max().expect("non-empty");
        let max_b = rects.iter().map(|r| r.b).max().expect("non-empty");
        let (vw, vh) = (max_r as i64 - min_l as i64, max_b as i64 - min_t as i64);
        if uw == 0 || uh == 0 || vw <= 0 || vh <= 0 {
            return rects.iter().map(|_| Rect::new(pad, pad, pad, pad)).collect();
        }
        let scale = (uw as f64 / vw as f64).min(uh as f64 / vh as f64);
        let (sw, sh) = (vw as f64 * scale, vh as f64 * scale);
        let (ox, oy) = (pad as f64 + (uw as f64 - sw) / 2.0, pad as f64 + (uh as f64 - sh) / 2.0);
        let edge = |v: f64| v.round() as i32;
        rects
            .iter()
            .map(|r| {
                let mut l = edge(ox + (r.l as i64 - min_l as i64) as f64 * scale).clamp(pad, tw - pad);
                let mut t = edge(oy + (r.t as i64 - min_t as i64) as f64 * scale).clamp(pad, th - pad);
                let mut rr = edge(ox + (r.r as i64 - min_l as i64) as f64 * scale).clamp(pad, tw - pad);
                let mut bb = edge(oy + (r.b as i64 - min_t as i64) as f64 * scale).clamp(pad, th - pad);
                let (min_w, min_h) = (12.min(uw), 12.min(uh));
                if r.r > r.l && rr - l < min_w {
                    let centre = (l + rr) / 2;
                    l = (centre - min_w / 2).clamp(pad, tw - pad - min_w);
                    rr = l + min_w;
                }
                if r.b > r.t && bb - t < min_h {
                    let centre = (t + bb) / 2;
                    t = (centre - min_h / 2).clamp(pad, th - pad - min_h);
                    bb = t + min_h;
                }
                Rect::new(l, t, rr, bb)
            })
            .collect()
    }

    fn normalize_monitor_name(&self, value: &str) -> String {
        fake_normalize_name(value)
    }

    fn key_name(&self, vk: u32) -> String {
        key_name(vk)
    }

    fn is_top(&self, bounds: Rect) -> bool {
        bounds.b <= 0
    }

    fn sort_and_number(&self, monitors: &mut [MonitorInfo]) {
        monitors.sort_by(|a, b| {
            a.bounds
                .l
                .cmp(&b.bounds.l)
                .then(a.bounds.t.cmp(&b.bounds.t))
                .then(a.stable_id.to_lowercase().cmp(&b.stable_id.to_lowercase()))
                .then(a.stable_id.cmp(&b.stable_id))
                .then(a.device.to_lowercase().cmp(&b.device.to_lowercase()))
                .then(a.device.cmp(&b.device))
                .then(a.bounds.r.cmp(&b.bounds.r))
                .then(a.bounds.b.cmp(&b.bounds.b))
        });
        for (i, m) in monitors.iter_mut().enumerate() {
            m.index = i as u32 + 1;
        }
    }
}

fn stored_name<T: Copy>(v: &Stored<T>, name: impl Fn(T) -> &'static str) -> Value {
    match v {
        Stored::Known(x) => json!(name(*x)),
        Stored::Unknown => json!("Unknown"),
    }
}

fn default_name(d: BoundaryDefault) -> &'static str {
    match d {
        BoundaryDefault::Automatic => "Automatic",
        BoundaryDefault::Free => "Free",
        BoundaryDefault::Push => "Push",
        BoundaryDefault::Blocked => "Blocked",
    }
}

fn behavior_name(b: RuleBehavior) -> &'static str {
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

fn to_value(s: &Settings) -> Value {
    let rules: Vec<Value> = s
        .boundary_rules
        .iter()
        .map(|r| {
            json!({
                "FromStableId": r.from_stable_id, "ToStableId": r.to_stable_id,
                "Direction": stored_name(&r.direction, Direction::name),
                "Behavior": stored_name(&r.behavior, behavior_name),
                "Control": stored_name(&r.control, control_name),
            })
        })
        .collect();
    let mut names = Map::new();
    for (k, v) in &s.monitor_names {
        names.insert(k.clone(), json!(v));
    }
    let mut o = Map::new();
    let mut put = |k: &str, v: Value| {
        o.insert(k.to_string(), v);
    };
    put("SettingsVersion", json!(s.settings_version));
    put("HotKey", json!(s.hot_key));
    put("ModCtrl", json!(s.mod_ctrl));
    put("ModAlt", json!(s.mod_alt));
    put("ModShift", json!(s.mod_shift));
    put("ModWin", json!(s.mod_win));
    put("ConfineHotKey", json!(s.confine_hot_key));
    put("PauseHotKey", json!(s.pause_hot_key));
    put("SideHotKey", json!(s.side_hot_key));
    put("StartGateClosed", json!(s.start_gate_closed));
    put("StartConfineOn", json!(s.start_confine_on));
    put("StartPaused", json!(s.start_paused));
    put("StartSideContainOn", json!(s.start_side_contain_on));
    put("AutoStart", json!(s.auto_start));
    put("DeliberateCross", json!(s.deliberate_cross));
    put("DescentRouting", json!(s.descent_routing));
    put(
        "UpwardBoundaryDefault",
        stored_name(&s.upward_boundary_default, default_name),
    );
    put(
        "SideBoundaryDefault",
        stored_name(&s.side_boundary_default, default_name),
    );
    put(
        "DownwardBoundaryDefault",
        stored_name(&s.downward_boundary_default, default_name),
    );
    put("BoundaryRules", Value::Array(rules));
    put("UpCrossMin", json!(s.up_cross_min));
    put("UpCrossSlack", json!(s.up_cross_slack));
    put("PassageInsetPx", json!(s.passage_inset_px));
    put("EdgeHoldbackPx", json!(s.edge_holdback_px));
    put("SeamTolerancePx", json!(s.seam_tolerance_px));
    put("SafetyEscapes", json!(s.safety_escapes));
    put("BoundaryTipSeen", json!(s.boundary_tip_seen));
    put("SideCrossMin", json!(s.side_cross_min));
    put("SideCrossSlack", json!(s.side_cross_slack));
    put("Mode", json!(s.mode));
    put("ManualMonitors", json!(s.manual_monitors));
    put("ExcludedDevices", json!(s.excluded_devices));
    put("MonitorNames", Value::Object(names));
    put("Language", json!(s.language));
    put("Theme", json!(s.theme));
    Value::Object(o)
}

fn parse_enum<T: Copy>(v: Option<&Value>, all: &[T], name: impl Fn(T) -> &'static str, default: T) -> Stored<T> {
    match v.and_then(Value::as_str) {
        None => Stored::Known(default),
        Some(text) => all
            .iter()
            .copied()
            .find(|x| name(*x).eq_ignore_ascii_case(text))
            .map_or(Stored::Unknown, Stored::Known),
    }
}

fn from_value(v: &Value) -> Result<Settings, String> {
    let o = v.as_object().ok_or("not an object")?;
    let mut s = Settings::default();
    let int = |k: &str, d: i32| o.get(k).and_then(Value::as_i64).map_or(d, |n| n as i32);
    let uint = |k: &str, d: u32| o.get(k).and_then(Value::as_u64).map_or(d, |n| n as u32);
    let boolean = |k: &str, d: bool| o.get(k).and_then(Value::as_bool).unwrap_or(d);
    let text = |k: &str, d: &str| o.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
    let list = |k: &str| -> Vec<String> {
        o.get(k).and_then(Value::as_array).map_or_else(Vec::new, |a| {
            a.iter().filter_map(|x| x.as_str().map(String::from)).collect()
        })
    };
    let defaults = [
        BoundaryDefault::Automatic,
        BoundaryDefault::Free,
        BoundaryDefault::Push,
        BoundaryDefault::Blocked,
    ];
    s.settings_version = int("SettingsVersion", 0);
    s.hot_key = uint("HotKey", s.hot_key);
    s.mod_ctrl = boolean("ModCtrl", s.mod_ctrl);
    s.mod_alt = boolean("ModAlt", s.mod_alt);
    s.mod_shift = boolean("ModShift", s.mod_shift);
    s.mod_win = boolean("ModWin", s.mod_win);
    s.confine_hot_key = uint("ConfineHotKey", s.confine_hot_key);
    s.pause_hot_key = uint("PauseHotKey", s.pause_hot_key);
    s.side_hot_key = uint("SideHotKey", s.side_hot_key);
    s.start_gate_closed = boolean("StartGateClosed", s.start_gate_closed);
    s.start_confine_on = boolean("StartConfineOn", s.start_confine_on);
    s.start_paused = boolean("StartPaused", s.start_paused);
    s.start_side_contain_on = boolean("StartSideContainOn", s.start_side_contain_on);
    s.auto_start = boolean("AutoStart", s.auto_start);
    s.deliberate_cross = boolean("DeliberateCross", s.deliberate_cross);
    s.descent_routing = boolean("DescentRouting", s.descent_routing);
    s.upward_boundary_default = parse_enum(
        o.get("UpwardBoundaryDefault"),
        &defaults,
        default_name,
        BoundaryDefault::Automatic,
    );
    s.side_boundary_default = parse_enum(
        o.get("SideBoundaryDefault"),
        &defaults,
        default_name,
        BoundaryDefault::Automatic,
    );
    s.downward_boundary_default = parse_enum(
        o.get("DownwardBoundaryDefault"),
        &defaults,
        default_name,
        BoundaryDefault::Free,
    );
    let dirs = [Direction::Up, Direction::Down, Direction::Left, Direction::Right];
    let behaviors = [
        RuleBehavior::Free,
        RuleBehavior::Push,
        RuleBehavior::Blocked,
        RuleBehavior::OverallFeel,
    ];
    s.boundary_rules = o
        .get("BoundaryRules")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|r| StoredRule {
                    from_stable_id: r.get("FromStableId").and_then(Value::as_str).unwrap_or("").into(),
                    to_stable_id: r.get("ToStableId").and_then(Value::as_str).unwrap_or("").into(),
                    direction: parse_enum(r.get("Direction"), &dirs, Direction::name, Direction::Up),
                    behavior: parse_enum(r.get("Behavior"), &behaviors, behavior_name, RuleBehavior::Free),
                    control: parse_enum(
                        r.get("Control"),
                        &[Control::Automatic, Control::Always],
                        control_name,
                        Control::Automatic,
                    ),
                })
                .collect()
        })
        .unwrap_or_default();
    s.up_cross_min = int("UpCrossMin", s.up_cross_min);
    s.up_cross_slack = int("UpCrossSlack", s.up_cross_slack);
    s.passage_inset_px = int("PassageInsetPx", s.passage_inset_px);
    s.edge_holdback_px = int("EdgeHoldbackPx", s.edge_holdback_px);
    s.seam_tolerance_px = int("SeamTolerancePx", s.seam_tolerance_px);
    s.safety_escapes = boolean("SafetyEscapes", s.safety_escapes);
    s.boundary_tip_seen = boolean("BoundaryTipSeen", s.boundary_tip_seen);
    s.side_cross_min = int("SideCrossMin", s.side_cross_min);
    s.side_cross_slack = int("SideCrossSlack", s.side_cross_slack);
    s.mode = text("Mode", &s.mode);
    s.manual_monitors = list("ManualMonitors");
    s.excluded_devices = list("ExcludedDevices");
    s.monitor_names = o
        .get("MonitorNames")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                .collect()
        })
        .unwrap_or_default();
    s.language = text("Language", &s.language);
    s.theme = text("Theme", &s.theme);
    Ok(s)
}

/// Dialogs for tests: fixed paths, a counted confirmation (C# `RestorePathPickerForTests`/`RestoreConfirmForTests`).
#[derive(Default)]
pub struct FakeDialogs {
    pub backup_path: Option<PathBuf>,
    pub restore_path: Option<PathBuf>,
    pub confirm: bool,
    pub confirmations: Rc<RefCell<u32>>,
    pub backup_picks: Rc<RefCell<u32>>,
}

impl Dialogs for FakeDialogs {
    fn pick_backup_path(&mut self, _title: &str, _filter: &str) -> Option<PathBuf> {
        *self.backup_picks.borrow_mut() += 1;
        self.backup_path.clone()
    }
    fn pick_restore_path(&mut self, _title: &str, _filter: &str) -> Option<PathBuf> {
        self.restore_path.clone()
    }
    fn confirm_restore(&mut self, _text: &str, _caption: &str) -> bool {
        *self.confirmations.borrow_mut() += 1;
        self.confirm
    }
}

/// A unique temp file path (deleted by the caller's guard).
pub struct TempFile(pub PathBuf);

impl TempFile {
    pub fn new(tag: &str) -> Self {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!("MouseFence-ui-{tag}-{}-{n}.json", std::process::id())))
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s_ui_fake_backend_compiles_the_csharp_defaults() {
        let ms = [
            mon("left", 1, true, 0, 0, 100, 100),
            mon("right", 2, false, 100, 0, 200, 100),
        ];
        let r = fake_compile(&Settings::default(), &ms, &[], true);
        assert_eq!(r.transitions.len(), 2);
        assert!(
            r.transitions.iter().all(|t| t.behavior == Behavior::Push),
            "side Automatic touching Main is Push"
        );
        let top = [
            mon("top", 1, false, 0, -100, 100, 0),
            mon("main", 2, true, 0, 0, 100, 100),
        ];
        let r = fake_compile(&Settings::default(), &top, &[], true);
        let up = r.transitions.iter().find(|t| t.direction == Direction::Up).expect("up");
        let down = r
            .transitions
            .iter()
            .find(|t| t.direction == Direction::Down)
            .expect("down");
        assert_eq!((up.behavior, down.behavior), (Behavior::Push, Behavior::Free));
    }

    #[test]
    fn s_ui_fake_json_round_trips_and_flags_unknown_enums() {
        let b = FakeBackend;
        let s = Settings {
            up_cross_min: 20,
            boundary_rules: vec![rule("a", "b", Direction::Up, RuleBehavior::Push, Control::Always)],
            monitor_names: names(&[("a", "Desk")]),
            ..Settings::default()
        };
        let mut back = b.parse_json(&b.to_json(&s)).expect("parse");
        b.normalize(&mut back);
        assert_eq!(back.up_cross_min, 20);
        assert_eq!(back.boundary_rules, s.boundary_rules);
        assert_eq!(back.monitor_names, s.monitor_names);
        let mut odd = b
            .parse_json(r#"{"SettingsVersion":4,"UpwardBoundaryDefault":"Surprise"}"#)
            .expect("parse");
        b.normalize(&mut odd);
        assert!(odd.has_repairable_import_errors);
        assert_eq!(fake_normalize_name("  BenQ\r\nDesk \t"), "BenQ  Desk");
        let long: String = std::iter::repeat_n("e\u{301}", 30).collect();
        assert_eq!(fake_normalize_name(&long).chars().filter(|c| *c == 'e').count(), 24);
    }
}
