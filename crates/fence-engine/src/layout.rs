//! Identity and layout helpers (port of C# `Core/GuardCore.Layout.cs` and the pure parts of
//! `Core/MonitorInfo.cs`). OWNED BY TRACK B (the only Track B file inside fence-engine).
//!
//! Every function here is pure. The .NET string rules these helpers depend on (`StringComparer.Ordinal`,
//! `StringComparer.OrdinalIgnoreCase`, `string.Trim`) are reproduced at the top of the file and checked
//! against the C# reference character by character (crates/fence-trace/tests/config_reference_diff.rs).
//!
//! Which comparer each 1.1.1 call uses (kept exactly):
//! - `SanitizeExcluded`: the requested set is OrdinalIgnoreCase (both 1.x callers build it so); the primary
//!   test is ordinal (`k != primaryKey`).
//! - `DisambiguateKeys`: groups OrdinalIgnoreCase; suffix order Left, Top, then ordinal raw identity.
//! - `MigrateLayoutKeysToStable`: the device map is OrdinalIgnoreCase; duplicate checks are ordinal.
//! - `SortAndNumber`: StableId and Device compare OrdinalIgnoreCase, then ordinal.

use std::cmp::Ordering;

use fence_types::{MonitorInfo, Rect};

// ------------------------------------------------------------------------------------------------
// .NET string rules
// ------------------------------------------------------------------------------------------------

/// C# `StringComparer.Ordinal.Compare`: UTF-16 code unit order (a supplementary character sorts before
/// U+E000..U+FFFF, unlike Rust's `str` order).
pub fn ordinal_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// The uppercase form .NET's OrdinalIgnoreCase compares for one character: the Unicode simple uppercase
/// mapping, except that a non-ASCII character never maps to an ASCII one (U+0131 and U+017F stay
/// themselves) and characters added to Unicode after .NET 9's tables stay themselves.
pub fn ignore_case_upper(c: char) -> char {
    if c.is_ascii() {
        return c.to_ascii_uppercase();
    }
    let cp = c as u32;
    // Greek letters with ypogegrammeni: the full mapping has two characters; the simple mapping is one.
    let greek = match cp {
        0x1F80..=0x1F87 | 0x1F90..=0x1F97 | 0x1FA0..=0x1FA7 => Some(cp + 8),
        0x1FB3 => Some(0x1FBC),
        0x1FC3 => Some(0x1FCC),
        0x1FF3 => Some(0x1FFC),
        _ => None,
    };
    if let Some(g) = greek {
        return char::from_u32(g).unwrap_or(c);
    }
    if NOT_IN_DOTNET_UPPER.iter().any(|&(lo, hi)| (lo..=hi).contains(&cp)) {
        return c;
    }
    let mut up = c.to_uppercase();
    match (up.next(), up.next()) {
        (Some(u), None) if !u.is_ascii() => u,
        _ => c,
    }
}

/// Characters whose uppercase mapping is newer than the Unicode tables of .NET 9 (Unicode 16 and 17: Latin
/// additions, Garay, Beria Erfe and others), as inclusive ranges: .NET leaves them unchanged. Found and
/// checked by the whole-plane sweep against the reference (crates/fence-trace/tests/config_reference_diff.rs,
/// 2026-10-13).
const NOT_IN_DOTNET_UPPER: &[(u32, u32)] = &[
    (0x019B, 0x019B),
    (0x0264, 0x0264),
    (0x1C8A, 0x1C8A),
    (0xA7CD, 0xA7CD),
    (0xA7CF, 0xA7CF),
    (0xA7D3, 0xA7D3),
    (0xA7D5, 0xA7D5),
    (0xA7DB, 0xA7DB),
    (0xA7DD, 0xA7DD),
    (0x10D70, 0x10D85),
    (0x16EBB, 0x16ED3),
];

/// C# `StringComparer.OrdinalIgnoreCase.Compare` (code points after [`ignore_case_upper`], then length).
pub fn ordinal_ignore_case_cmp(a: &str, b: &str) -> Ordering {
    a.chars().map(ignore_case_upper).cmp(b.chars().map(ignore_case_upper))
}

/// C# `string.Equals(a, b, StringComparison.OrdinalIgnoreCase)`.
pub fn ordinal_ignore_case_eq(a: &str, b: &str) -> bool {
    ordinal_ignore_case_cmp(a, b) == Ordering::Equal
}

/// C# `key.StartsWith(@"\\.\DISPLAY", StringComparison.OrdinalIgnoreCase)` (GuardCore.LooksLikeGdiDisplayName).
pub fn looks_like_gdi_display_name(key: &str) -> bool {
    const PREFIX: &[u8] = br"\\.\DISPLAY";
    key.len() >= PREFIX.len() && key.as_bytes()[..PREFIX.len()].eq_ignore_ascii_case(PREFIX)
}

// ------------------------------------------------------------------------------------------------
// GuardCore.Layout.cs
// ------------------------------------------------------------------------------------------------

/// C# `GuardCore.SanitizeExcluded(presentKeys, primaryKey, requested, out pruned)` -> (excluded, pruned).
/// The primary is never excluded and at least one usable monitor remains; absent ids stay dormant.
/// `requested` matches OrdinalIgnoreCase, as both 1.x callers (Configure, Settings.SanitizedExcluded) build
/// their set; the primary test is ordinal.
pub fn sanitize_excluded(
    present_keys: &[String],
    primary_key: Option<&str>,
    requested: &[String],
) -> (Vec<String>, bool) {
    let asked: Vec<&String> = present_keys
        .iter()
        .filter(|k| requested.iter().any(|r| ordinal_ignore_case_eq(r, k)))
        .collect();
    let mut excluded: Vec<String> = asked
        .iter()
        .filter(|k| primary_key != Some(k.as_str()))
        .map(|k| (*k).clone())
        .collect();
    // Never leave zero usable monitors: if the request would wall off every present screen, keep the last.
    if excluded.len() >= present_keys.len() && !excluded.is_empty() {
        excluded.pop();
    }
    let pruned = excluded.len() != asked.len();
    (excluded, pruned)
}

/// One monitor for key disambiguation: raw key, Left, Top, raw identity key (C# 4-tuple overload).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyedMonitor {
    pub key: String,
    pub left: i32,
    pub top: i32,
    pub raw_identity_key: String,
}

impl KeyedMonitor {
    /// The C# 3-tuple overload: the raw identity is the key itself.
    pub fn new(key: &str, left: i32, top: i32) -> Self {
        Self {
            key: key.to_string(),
            left,
            top,
            raw_identity_key: key.to_string(),
        }
    }
}

/// C# `GuardCore.DisambiguateKeys`: a raw key shared by 2+ monitors gets `#0`, `#1`, ... by Left, Top, then
/// ordinal raw identity; a unique key stays unchanged. Output is in input order.
pub fn disambiguate_keys(monitors: &[KeyedMonitor]) -> Vec<String> {
    let mut result = vec![String::new(); monitors.len()];
    // GroupBy(Key, OrdinalIgnoreCase): groups in order of first appearance, members in input order.
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (i, m) in monitors.iter().enumerate() {
        match groups
            .iter_mut()
            .find(|g| ordinal_ignore_case_eq(&monitors[g[0]].key, &m.key))
        {
            Some(g) => g.push(i),
            None => groups.push(vec![i]),
        }
    }
    for mut members in groups {
        if members.len() < 2 {
            result[members[0]] = monitors[members[0]].key.clone();
            continue;
        }
        // OrderBy(Left).ThenBy(Top).ThenBy(RawIdentityKey, Ordinal): a stable sort.
        members.sort_by(|&a, &b| {
            let (x, y) = (&monitors[a], &monitors[b]);
            x.left
                .cmp(&y.left)
                .then(x.top.cmp(&y.top))
                .then_with(|| ordinal_cmp(&x.raw_identity_key, &y.raw_identity_key))
        });
        for (ord, i) in members.into_iter().enumerate() {
            result[i] = format!("{}#{}", monitors[i].key, ord);
        }
    }
    result
}

/// F03: stable instance-qualified keys, with explicitly ephemeral ambiguous identities.
/// `session` is supplied by the native adapter; the engine reads no clock or process state.
pub fn disambiguate_keys_with(monitors: &[KeyedMonitor], fixes: crate::fixes::Fixes, session: &str) -> Vec<String> {
    if !fixes.j11_segment_and_identity {
        return disambiguate_keys(monitors);
    }
    let mut topology: Vec<String> = monitors
        .iter()
        .map(|m| format!("{}\t{}\t{}\t{}", m.key, m.raw_identity_key, m.left, m.top))
        .collect();
    topology.sort_by(|a, b| ordinal_cmp(a, b));
    // The full, unambiguous topology encoding avoids any hash collision in these runtime-only keys.
    let topology = hex_identity(topology.join("\n").as_bytes());
    monitors
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let instance = if m.raw_identity_key.trim().is_empty() {
                &m.key
            } else {
                &m.raw_identity_key
            };
            let count = monitors
                .iter()
                .filter(|other| {
                    let other_instance = if other.raw_identity_key.trim().is_empty() {
                        &other.key
                    } else {
                        &other.raw_identity_key
                    };
                    ordinal_ignore_case_eq(&other.key, &m.key) && ordinal_ignore_case_eq(other_instance, instance)
                })
                .count();
            if count > 1 {
                format!("{}#ambiguous:{session}:{topology}:{i}", m.key)
            } else if ordinal_ignore_case_eq(instance, &m.key) {
                m.key.clone()
            } else {
                format!(
                    "{}#instance:{}",
                    m.key,
                    hex_identity(instance.chars().map(ignore_case_upper).collect::<String>().as_bytes())
                )
            }
        })
        .collect()
}

fn hex_identity(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02X}");
    }
    out
}

pub fn canonical_native_identity(value: &str) -> String {
    let mut key = value.trim().chars().map(ignore_case_upper).collect::<String>();
    if key.starts_with(r"\\?\") {
        key = key[4..].to_string();
    }
    if let Some(at) = key.rfind("#{") {
        key.truncate(at);
    }
    key = key.replace('#', r"\");
    if let Some(tail) = key.strip_prefix(r"DISPLAY\") {
        format!(r"MONITOR\{tail}")
    } else {
        key
    }
}

/// C# `GuardCore.MigrateLayoutKeysToStable(manualKeys, links, deviceToStable)`. `device_to_stable` is read as
/// an OrdinalIgnoreCase dictionary (a later pair for the same device wins, like the C# indexer).
/// Present `\\.\DISPLAYn` names convert, stale ones drop, already-stable keys stay (dormant when absent).
pub fn migrate_layout_keys_to_stable(
    manual_keys: &[String],
    links: &[(String, String)],
    device_to_stable: &[(String, String)],
) -> (Vec<String>, Vec<(String, String)>) {
    let convert = |key: &str| -> Option<String> {
        if key.chars().all(char::is_whitespace) {
            return None; // string.IsNullOrWhiteSpace
        }
        if let Some((_, stable)) = device_to_stable
            .iter()
            .rev()
            .find(|(device, _)| ordinal_ignore_case_eq(device, key))
        {
            return Some(stable.clone());
        }
        if looks_like_gdi_display_name(key) {
            None
        } else {
            Some(key.to_string())
        }
    };

    let mut manual: Vec<String> = Vec::new();
    for key in manual_keys {
        if let Some(stable) = convert(key)
            && !manual.contains(&stable)
        {
            manual.push(stable);
        }
    }

    let mut migrated: Vec<(String, String)> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for (from, to) in links {
        let (Some(f), Some(t)) = (convert(from), convert(to)) else {
            continue;
        };
        let sig = format!("{f}\n{t}");
        if !seen.contains(&sig) {
            seen.push(sig);
            migrated.push((f, t));
        }
    }
    (manual, migrated)
}

/// C# `GuardCore.ResetLayoutConfig()`: ("AutoTop", no manual picks, no links).
pub fn reset_layout_config() -> (String, Vec<String>, Vec<(String, String)>) {
    ("AutoTop".to_string(), Vec::new(), Vec::new())
}

/// C# `GuardCore.IsArrowRotationHotkey(vk, ctrl, alt, shift, win)`: exactly Ctrl+Alt+Arrow (the Intel
/// screen-rotation chord). Left 0x25, Up 0x26, Right 0x27, Down 0x28.
pub fn is_arrow_rotation_hotkey(vk: u32, ctrl: bool, alt: bool, shift: bool, win: bool) -> bool {
    ctrl && alt && !shift && !win && (0x25..=0x28).contains(&vk)
}

/// C# `GuardCore.MapLayout(rects, targetW, targetH, pad)`: one common scale, centred, 12x12 minimum footprint.
/// Same double arithmetic and `Math.Round(.., AwayFromZero)` (= `f64::round`) as the C#.
pub fn map_layout(rects: &[Rect], target_w: i32, target_h: i32, pad: i32) -> Vec<Rect> {
    let mut result = Vec::with_capacity(rects.len());
    if rects.is_empty() {
        return result;
    }
    let target_w = target_w.max(0);
    let target_h = target_h.max(0);
    let pad = pad.clamp(0, target_w.min(target_h) / 2);
    let usable_w = (target_w - pad * 2).max(0);
    let usable_h = (target_h - pad * 2).max(0);
    let min_l = rects.iter().map(|r| r.l).min().unwrap_or(0);
    let min_t = rects.iter().map(|r| r.t).min().unwrap_or(0);
    let max_r = rects.iter().map(|r| r.r).max().unwrap_or(0);
    let max_b = rects.iter().map(|r| r.b).max().unwrap_or(0);
    let virtual_w = max_r as i64 - min_l as i64;
    let virtual_h = max_b as i64 - min_t as i64;

    if usable_w == 0 || usable_h == 0 || virtual_w <= 0 || virtual_h <= 0 {
        result.extend(rects.iter().map(|_| Rect::new(pad, pad, pad, pad)));
        return result;
    }

    let scale = (usable_w as f64 / virtual_w as f64).min(usable_h as f64 / virtual_h as f64);
    let scaled_w = virtual_w as f64 * scale;
    let scaled_h = virtual_h as f64 * scale;
    let offset_x = pad as f64 + (usable_w as f64 - scaled_w) / 2.0;
    let offset_y = pad as f64 + (usable_h as f64 - scaled_h) / 2.0;
    let edge = |value: f64| value.round() as i32;

    for r in rects {
        let mut l = edge(offset_x + (r.l as i64 - min_l as i64) as f64 * scale);
        let mut t = edge(offset_y + (r.t as i64 - min_t as i64) as f64 * scale);
        let mut rr = edge(offset_x + (r.r as i64 - min_l as i64) as f64 * scale);
        let mut bb = edge(offset_y + (r.b as i64 - min_t as i64) as f64 * scale);
        l = l.clamp(pad, target_w - pad);
        t = t.clamp(pad, target_h - pad);
        rr = rr.clamp(pad, target_w - pad);
        bb = bb.clamp(pad, target_h - pad);

        // A-FIX-1: a tiny dummy at the far end of a huge span keeps a visible 12x12 footprint.
        let min_w = usable_w.min(12);
        let min_h = usable_h.min(12);
        if r.r > r.l && rr - l < min_w {
            let centre = (l + rr) / 2;
            l = (centre - min_w / 2).clamp(pad, target_w - pad - min_w);
            rr = l + min_w;
        }
        if r.b > r.t && bb - t < min_h {
            let centre = (t + bb) / 2;
            t = (centre - min_h / 2).clamp(pad, target_h - pad - min_h);
            bb = t + min_h;
        }
        result.push(Rect::new(l, t, rr, bb));
    }
    result
}

/// C# `GuardCore.MonitorLabel(ordinal, width, height, primary, mainTag, monitorName)`:
/// `"2  •  Name  •  1920x1080  (main)"`; a blank name and an empty tag are left out.
pub fn monitor_label(
    ordinal: u32,
    width: i64,
    height: i64,
    primary: bool,
    main_tag: &str,
    monitor_name: Option<&str>,
) -> String {
    let name = match monitor_name.map(str::trim) {
        Some(n) if !n.is_empty() => format!("  •  {n}"),
        _ => String::new(),
    };
    let suffix = if primary && !main_tag.is_empty() {
        format!("  {main_tag}")
    } else {
        String::new()
    };
    format!("{ordinal}{name}  •  {width}x{height}{suffix}")
}

// ------------------------------------------------------------------------------------------------
// MonitorInfo.cs (pure parts)
// ------------------------------------------------------------------------------------------------

/// C# `MonitorInfo.SortAndNumber`: sort by Left, Top, StableId (ignore-case, then ordinal), Device (same),
/// Right, Bottom; then set `index = i + 1`. (C# `List.Sort` is not stable; the comparer only ties for two
/// monitors equal in every compared field, which a live layout never has.)
pub fn sort_and_number(monitors: &mut [MonitorInfo]) {
    monitors.sort_by(|a, b| {
        a.bounds
            .l
            .cmp(&b.bounds.l)
            .then(a.bounds.t.cmp(&b.bounds.t))
            .then_with(|| ordinal_ignore_case_cmp(&a.stable_id, &b.stable_id))
            .then_with(|| ordinal_cmp(&a.stable_id, &b.stable_id))
            .then_with(|| ordinal_ignore_case_cmp(&a.device, &b.device))
            .then_with(|| ordinal_cmp(&a.device, &b.device))
            .then(a.bounds.r.cmp(&b.bounds.r))
            .then(a.bounds.b.cmp(&b.bounds.b))
    });
    for (i, m) in monitors.iter_mut().enumerate() {
        m.index = i as u32 + 1;
    }
}

/// C# `MonitorInfo.DisambiguateStableIds`: raw identity = `stable_id + "\n" + device`.
pub fn disambiguate_stable_ids(monitors: &mut [MonitorInfo]) {
    let keyed: Vec<KeyedMonitor> = monitors
        .iter()
        .map(|m| KeyedMonitor {
            key: m.stable_id.clone(),
            left: m.bounds.l,
            top: m.bounds.t,
            raw_identity_key: format!("{}\n{}", m.stable_id, m.device),
        })
        .collect();
    for (m, key) in monitors.iter_mut().zip(disambiguate_keys(&keyed)) {
        m.stable_id = key;
    }
}

/// The pure end of C# `MonitorInfo.All()`: `SortAndNumber`, then `DisambiguateStableIds`. The platform layer
/// enumerates the monitors (raw stable ids, any order) and calls this.
pub fn identify_monitors(mut monitors: Vec<MonitorInfo>) -> Vec<MonitorInfo> {
    sort_and_number(&mut monitors);
    disambiguate_stable_ids(&mut monitors);
    monitors
}

/// The identity half of C# `Settings.MonitorNameFor(names, stableId)`: the nickname stored for a stable id
/// (OrdinalIgnoreCase key, as the normalized map), or `None`. fence-config adds the name normalization.
pub fn monitor_name_lookup<'a>(names: &'a [(String, String)], stable_id: &str) -> Option<&'a str> {
    if stable_id.is_empty() {
        return None;
    }
    names
        .iter()
        .find(|(k, _)| ordinal_ignore_case_eq(k, stable_id))
        .map(|(_, v)| v.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> String {
        v.to_string()
    }

    fn set(items: &[&str]) -> Vec<String> {
        items.iter().map(|x| s(x)).collect()
    }

    fn mon(stable: &str, device: &str, l: i32, t: i32, r: i32, b: i32) -> MonitorInfo {
        MonitorInfo {
            device: s(device),
            stable_id: s(stable),
            bounds: Rect::new(l, t, r, b),
            primary: false,
            index: 0,
        }
    }

    // ---- wall-off safety (Tests.cs:1775-1808)

    #[test]
    fn p136_primary_is_never_excluded_and_one_usable_monitor_remains() {
        let present = set(&["MONITOR\\MAIN\\{guid}", "MONITOR\\DUMMY\\{guid}"]);
        let primary = "MONITOR\\MAIN\\{guid}";
        let requested = set(&["MONITOR\\MAIN\\{guid}", "MONITOR\\DUMMY\\{guid}"]);
        let (excluded, pruned) = sanitize_excluded(&present, Some(primary), &requested);
        assert_eq!(excluded, set(&["MONITOR\\DUMMY\\{guid}"]));
        assert!(!excluded.contains(&s(primary)) && pruned);
    }

    #[test]
    fn p137_never_leaves_zero_usable_monitors() {
        let present = set(&["MONITOR\\ONLY\\{guid}"]);
        let (excluded, pruned) = sanitize_excluded(&present, Some("MONITOR\\PRIMARY-ELSEWHERE"), &present);
        assert!(excluded.is_empty() && pruned, "{excluded:?} {pruned}");
    }

    #[test]
    fn p138_absent_imported_id_is_dormant_not_a_prune() {
        let present = set(&["MONITOR\\MAIN\\{guid}", "MONITOR\\DUMMY\\{guid}"]);
        let requested = set(&["MONITOR\\DUMMY\\{guid}", "MONITOR\\GONE\\{guid}"]);
        let (excluded, pruned) = sanitize_excluded(&present, Some("MONITOR\\MAIN\\{guid}"), &requested);
        assert_eq!(excluded, set(&["MONITOR\\DUMMY\\{guid}"]));
        assert!(!pruned);
    }

    // ---- stable-key identity (Tests.cs:1813-1844)

    #[test]
    fn p139_same_dummy_excluded_under_both_display_orderings() {
        let primary = "MONITOR\\MAIN-EDID\\{guid}";
        let dummy = "MONITOR\\DUMMY-EDID\\{guid}";
        let requested = set(&[dummy]);
        let excluded_monitor = |layout: &[(&str, &str)]| -> Option<(String, String)> {
            let present: Vec<String> = layout.iter().map(|m| s(m.1)).collect();
            let (ex, _) = sanitize_excluded(&present, Some(primary), &requested);
            let hit: Vec<_> = layout.iter().filter(|m| ex.contains(&s(m.1))).collect();
            (hit.len() == 1).then(|| (s(hit[0].0), s(hit[0].1)))
        };
        let e1 = excluded_monitor(&[("\\\\.\\DISPLAY1", primary), ("\\\\.\\DISPLAY2", dummy)]);
        let e2 = excluded_monitor(&[("\\\\.\\DISPLAY3", primary), ("\\\\.\\DISPLAY4", dummy)]);
        assert_eq!(e1, Some((s("\\\\.\\DISPLAY2"), s(dummy))));
        assert_eq!(e2, Some((s("\\\\.\\DISPLAY4"), s(dummy))));
    }

    // ---- ROUND-3 FIX B (Tests.cs:1849-1884)

    #[test]
    fn p140_identical_edid_panels_get_distinct_ids_by_left() {
        let primary_raw = "MONITOR\\PRIMARY-EDID\\{guid}";
        let dup_raw = "MONITOR\\DUP-EDID\\{guid}";
        let present = [
            KeyedMonitor::new(primary_raw, 0, 0),
            KeyedMonitor::new(dup_raw, 2560, 0),
            KeyedMonitor::new(dup_raw, 3360, 0),
        ];
        let keys = disambiguate_keys(&present);
        assert_ne!(keys[1], keys[2]);
        assert_eq!(keys[0], primary_raw, "a unique key is left untouched");
        assert_eq!(keys[1], format!("{dup_raw}#0"));
        assert_eq!(keys[2], format!("{dup_raw}#1"));
    }

    #[test]
    fn p141_excluding_one_identical_panel_excludes_only_that_one() {
        let present = [
            KeyedMonitor::new("MONITOR\\PRIMARY-EDID\\{guid}", 0, 0),
            KeyedMonitor::new("MONITOR\\DUP-EDID\\{guid}", 2560, 0),
            KeyedMonitor::new("MONITOR\\DUP-EDID\\{guid}", 3360, 0),
        ];
        let keys = disambiguate_keys(&present);
        let requested = vec![keys[1].clone()];
        let (excluded, pruned) = sanitize_excluded(&keys, Some(&keys[0]), &requested);
        let count = keys.iter().filter(|k| excluded.contains(k)).count();
        assert_eq!(count, 1);
        assert!(excluded.contains(&keys[1]) && !excluded.contains(&keys[2]) && !pruned);
    }

    // ---- layout rules stable-key (Tests.cs:1902-1925): the C# check resolves links through a
    // StableId -> Device map, which is what Configure does; the port keeps the same lookup.

    #[test]
    fn p143_manual_monitors_and_links_survive_display_renumbering() {
        let bottom = "MONITOR\\BOTTOM\\{guid}";
        let top = "MONITOR\\TOP\\{guid}";
        let requested_manual = set(&[top]);
        let requested_links = [(s(bottom), s(top))];
        let resolve = |layout: &[(&str, &str)]| -> Option<(String, String)> {
            let device_of = |stable: &str| layout.iter().find(|m| m.1 == stable).map(|m| s(m.0));
            let top_set: Vec<&str> = layout
                .iter()
                .filter(|m| requested_manual.contains(&s(m.1)))
                .map(|m| m.1)
                .collect();
            requested_links.iter().find_map(|(from, to)| {
                if top_set.contains(&to.as_str()) {
                    Some((device_of(from)?, device_of(to)?))
                } else {
                    None
                }
            })
        };
        let l1 = resolve(&[("\\\\.\\DISPLAY1", bottom), ("\\\\.\\DISPLAY4", top)]);
        let l2 = resolve(&[("\\\\.\\DISPLAY3", bottom), ("\\\\.\\DISPLAY2", top)]);
        assert_eq!(l1, Some((s("\\\\.\\DISPLAY1"), s("\\\\.\\DISPLAY4"))));
        assert_eq!(l2, Some((s("\\\\.\\DISPLAY3"), s("\\\\.\\DISPLAY2"))));
    }

    #[test]
    fn p144_absent_stable_ids_are_inert() {
        let present = set(&["MONITOR\\MAIN\\{guid}", "MONITOR\\TOP\\{guid}"]);
        let primary = "MONITOR\\MAIN\\{guid}";
        let stale = "MONITOR\\OLD-DUMMY\\{guid}";
        let (excluded, pruned) = sanitize_excluded(&present, Some(primary), &set(&[stale]));
        let manual_matched = present.iter().any(|p| p == stale);
        let links = [(primary, stale), (stale, "MONITOR\\TOP\\{guid}")];
        let any_link = links
            .iter()
            .any(|(f, t)| present.contains(&s(f)) && present.contains(&s(t)));
        assert!(excluded.is_empty() && !pruned && !manual_matched && !any_link);
    }

    #[test]
    fn p145_present_display_keys_convert_and_stale_ones_drop() {
        let map = [
            (s("\\\\.\\DISPLAY1"), s("MONITOR\\MAIN\\{guid}")),
            (s("\\\\.\\DISPLAY4"), s("MONITOR\\TOP\\{guid}")),
        ];
        let manual = set(&["\\\\.\\DISPLAY4", "\\\\.\\DISPLAY9"]);
        let links = [
            (s("\\\\.\\DISPLAY1"), s("\\\\.\\DISPLAY4")),
            (s("\\\\.\\DISPLAY1"), s("\\\\.\\DISPLAY9")),
        ];
        let (m, l) = migrate_layout_keys_to_stable(&manual, &links, &map);
        assert_eq!(m, set(&["MONITOR\\TOP\\{guid}"]));
        assert_eq!(l, vec![(s("MONITOR\\MAIN\\{guid}"), s("MONITOR\\TOP\\{guid}"))]);
    }

    #[test]
    fn p146_reset_returns_to_auto_top_without_manual_picks_or_rules() {
        let (mode, manual, links) = reset_layout_config();
        assert_eq!(mode, "AutoTop");
        assert!(manual.is_empty() && links.is_empty());
    }

    // ---- map layout (Tests.cs:1975-2071)

    /// Port of the C# `ValidMapLayout` checker.
    fn valid_map_layout(source: &[Rect], w: i32, h: i32, pad: i32) -> Result<Vec<Rect>, String> {
        let mapped = map_layout(source, w, h, pad);
        let count = mapped.len() == source.len();
        let inside = mapped
            .iter()
            .all(|r| r.l >= pad && r.t >= pad && r.r <= w - pad && r.b <= h - pad && r.r > r.l && r.b > r.t);
        let mut aspect = count;
        for (i, src) in source.iter().enumerate() {
            if !aspect {
                break;
            }
            let (sw, sh) = ((src.r - src.l) as f64, (src.b - src.t) as f64);
            let (dw, dh) = ((mapped[i].r - mapped[i].l) as f64, (mapped[i].b - mapped[i].t) as f64);
            aspect = (dw / sw - dh / sh).abs() <= 1.1 / sw + 1.1 / sh;
        }
        let mut order = count;
        for i in 0..source.len() {
            for j in 0..source.len() {
                if !order {
                    break;
                }
                if source[i].r <= source[j].l && mapped[i].r > mapped[j].l {
                    order = false;
                }
                if source[i].b <= source[j].t && mapped[i].b > mapped[j].t {
                    order = false;
                }
            }
        }
        let min_l = mapped.iter().map(|r| r.l).min().unwrap_or(0);
        let max_r = mapped.iter().map(|r| r.r).max().unwrap_or(0);
        let min_t = mapped.iter().map(|r| r.t).min().unwrap_or(0);
        let max_b = mapped.iter().map(|r| r.b).max().unwrap_or(0);
        let centered = !mapped.is_empty() && ((min_l + max_r) - w).abs() <= 1 && ((min_t + max_b) - h).abs() <= 1;
        if count && inside && aspect && order && centered {
            Ok(mapped)
        } else {
            Err(format!(
                "count={count} inside={inside} aspect={aspect} order={order} centered={centered} {mapped:?}"
            ))
        }
    }

    fn r(l: i32, t: i32, rr: i32, b: i32) -> Rect {
        Rect::new(l, t, rr, b)
    }

    #[test]
    fn p147_single_monitor_scales_and_centers_exactly() {
        let mapped = valid_map_layout(&[r(0, 0, 1920, 1080)], 400, 300, 10).unwrap();
        assert_eq!(mapped[0], r(10, 43, 390, 257));
    }

    #[test]
    fn p148_side_by_side_keeps_adjacency_and_order() {
        let mapped = valid_map_layout(&[r(0, 0, 1920, 1080), r(1920, 0, 3840, 1080)], 600, 240, 10).unwrap();
        assert!(mapped[0].r == mapped[1].l && mapped[0].t == mapped[1].t, "{mapped:?}");
    }

    #[test]
    fn p149_a_real_desktop_gap_remains_visible() {
        let mapped = valid_map_layout(&[r(0, 0, 1000, 1000), r(1500, 0, 2500, 1000)], 520, 240, 10).unwrap();
        assert_eq!(mapped[1].l - mapped[0].r, 100, "{mapped:?}");
    }

    #[test]
    fn p150_negative_origin_left_monitor_stays_left_of_main() {
        let mapped = valid_map_layout(&[r(-2560, 0, 0, 1440), r(0, 0, 1920, 1080)], 700, 300, 12).unwrap();
        assert_eq!(mapped[0].r, mapped[1].l, "{mapped:?}");
    }

    #[test]
    fn p151_portrait_preserves_its_tall_aspect() {
        let mapped = valid_map_layout(&[r(-1080, -400, 0, 1520), r(0, 0, 1920, 1080)], 640, 420, 10).unwrap();
        assert!(mapped[0].b - mapped[0].t > mapped[0].r - mapped[0].l, "{mapped:?}");
    }

    #[test]
    fn p152_eight_monitor_sprawl_is_bounded_ordered_centered_proportional() {
        let source = [
            r(-5120, -2160, -3200, -1080),
            r(-3000, -2160, -1080, -1080),
            r(0, -2160, 3840, 0),
            r(-6000, 0, -4080, 1080),
            r(-3840, 0, -1920, 1080),
            r(0, 0, 2560, 1440),
            r(2800, 200, 4720, 1280),
            r(5000, 1600, 6080, 3520),
        ];
        valid_map_layout(&source, 960, 540, 14).unwrap();
    }

    #[test]
    fn p153_synthetic_abutting_dummy_keeps_its_seam_and_is_permutation_stable() {
        // Synthetic geometry of commit 22665ee (no real desk layout).
        let source = [
            r(-1200, 45, 0, 765),
            r(-300, -900, 1900, 0),
            r(0, 0, 1600, 900),
            r(1600, 40, 3100, 760),
            r(3100, 560, 3600, 930),
        ];
        let mapped = valid_map_layout(&source, 960, 520, 14).unwrap();
        let again = map_layout(&source, 960, 520, 14);
        let mut reversed_src = source.to_vec();
        reversed_src.reverse();
        let mut reversed = map_layout(&reversed_src, 960, 520, 14);
        reversed.reverse();
        assert_eq!(mapped, again);
        assert_eq!(mapped, reversed);
        assert_eq!(mapped[3].r, mapped[4].l);
        assert!(mapped[3].b.min(mapped[4].b) > mapped[3].t.max(mapped[4].t));
    }

    #[test]
    fn p221_extreme_span_dummy_keeps_a_12x12_footprint() {
        let projected = map_layout(&[r(-32768, 0, -32128, 480), r(30000, 0, 32767, 2160)], 400, 160, 10);
        assert!(
            projected[0].r - projected[0].l >= 12 && projected[0].b - projected[0].t >= 12,
            "{projected:?}"
        );
    }

    #[test]
    fn s_layout_map_layout_degenerate_inputs_match_csharp() {
        assert!(map_layout(&[], 100, 100, 5).is_empty());
        assert_eq!(map_layout(&[r(0, 0, 10, 10)], 0, 100, 5), vec![r(0, 0, 0, 0)]);
        assert_eq!(map_layout(&[r(0, 0, 0, 10)], 100, 100, 5), vec![r(5, 5, 5, 5)]);
        assert_eq!(map_layout(&[r(0, 0, 10, 10)], 100, 50, 999), vec![r(25, 25, 25, 25)]);
    }

    // ---- identity numbering and labels (Tests.cs:2075-2118, synthetic names of 22665ee)

    #[test]
    fn p154_numbering_is_left_top_with_stable_exact_origin_ties() {
        let seed = vec![
            mon("stable-z", "DISPLAY-Z", 0, 0, 1920, 1080),
            mon("stable-right", "DISPLAY-R", 1600, 40, 3100, 760),
            mon("stable-left", "DISPLAY-L", -1200, 45, 0, 765),
            mon("stable-a", "DISPLAY-A", 0, 0, 1600, 900),
            mon("stable-top", "DISPLAY-T", -300, -900, 1900, 0),
        ];
        let run = |mut input: Vec<MonitorInfo>| -> Vec<(String, u32)> {
            sort_and_number(&mut input);
            input.into_iter().map(|m| (m.stable_id, m.index)).collect()
        };
        let forward = run(seed.clone());
        let mut rev = seed;
        rev.reverse();
        let reverse = run(rev);
        let expected = ["stable-left", "stable-top", "stable-a", "stable-z", "stable-right"];
        assert_eq!(forward.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), expected);
        assert_eq!(forward, reverse);
        assert_eq!(forward.iter().map(|x| x.1).collect::<Vec<_>>(), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn p155_label_is_compact_deterministic_and_not_position_based() {
        let a = monitor_label(2, 2560, 1440, true, "[MAIN]", None);
        let b = monitor_label(2, 2560, 1440, true, "[MAIN]", None);
        assert_eq!(a, b);
        assert_eq!(a, "2  •  2560x1440  [MAIN]");
        assert!(!a.contains('@') && !a.contains("DISPLAY") && !a.contains('#'));
    }

    #[test]
    fn p156_list_label_keeps_the_number_and_adds_the_nickname() {
        let named = monitor_label(4, 1920, 1080, false, "[MAIN]", Some("Example Right"));
        assert_eq!(named, "4  •  Example Right  •  1920x1080");
        assert!(named.starts_with("4  •  "));
        assert_eq!(monitor_label(1, 10, 20, true, "", Some("  ")), "1  •  10x20");
        assert_eq!(
            monitor_label(1, 10, 20, false, "(main)", Some(" N ")),
            "1  •  N  •  10x20"
        );
    }

    #[test]
    fn p165_nickname_follows_stable_id_across_renumbering_and_rearrangement() {
        let names = vec![(s("example-right"), s("Example Right"))];
        let mut before = vec![
            mon("other", "\\\\.\\DISPLAY1", 0, 0, 100, 100),
            mon("example-right", "\\\\.\\DISPLAY4", 100, 0, 200, 100),
        ];
        let mut after = vec![
            mon("example-right", "\\\\.\\DISPLAY1", -100, 0, 0, 100),
            mon("other", "\\\\.\\DISPLAY4", 0, 0, 100, 100),
        ];
        sort_and_number(&mut before);
        sort_and_number(&mut after);
        let b = before.iter().find(|m| m.stable_id == "example-right").unwrap();
        let a = after.iter().find(|m| m.stable_id == "example-right").unwrap();
        assert_eq!(monitor_name_lookup(&names, &b.stable_id), Some("Example Right"));
        assert_eq!(monitor_name_lookup(&names, &a.stable_id), Some("Example Right"));
        assert_eq!((b.index, a.index), (2, 1));
        // Case-insensitive like the normalized C# dictionary; an empty id never matches.
        assert_eq!(monitor_name_lookup(&names, "EXAMPLE-RIGHT"), Some("Example Right"));
        assert_eq!(monitor_name_lookup(&names, ""), None);
    }

    #[test]
    fn p166_display_only_names_do_not_affect_identity_or_numbering() {
        let layout = || {
            vec![
                mon("right", "\\\\.\\DISPLAY9", 100, 0, 200, 100),
                mon("left", "\\\\.\\DISPLAY2", -100, 0, 0, 100),
                mon("main", "\\\\.\\DISPLAY5", 0, 0, 100, 100),
            ]
        };
        let names = vec![(s("right"), s("Example Right")), (s("left"), s("Example Left"))];
        let mut unnamed = layout();
        let mut named = layout();
        sort_and_number(&mut unnamed);
        sort_and_number(&mut named);
        let ids = |v: &[MonitorInfo]| v.iter().map(|m| (m.stable_id.clone(), m.index)).collect::<Vec<_>>();
        assert_eq!(ids(&unnamed), ids(&named));
        for m in &named {
            let want = match m.stable_id.as_str() {
                "right" => Some("Example Right"),
                "left" => Some("Example Left"),
                _ => None,
            };
            assert_eq!(monitor_name_lookup(&names, &m.stable_id), want);
        }
    }

    // ---- Intel rotation clash (Tests.cs:2449-2470)

    #[test]
    fn p190_ctrl_alt_arrows_are_rotation_clashes() {
        for vk in [0x26, 0x28, 0x25, 0x27] {
            assert!(is_arrow_rotation_hotkey(vk, true, true, false, false), "{vk:#x}");
        }
    }

    #[test]
    fn p191_ctrl_alt_shift_right_is_not_a_clash() {
        assert!(!is_arrow_rotation_hotkey(0x27, true, true, true, false));
    }

    #[test]
    fn p192_ctrl_alt_win_right_is_not_a_clash() {
        assert!(!is_arrow_rotation_hotkey(0x27, true, true, false, true));
    }

    #[test]
    fn p193_a_non_arrow_key_is_not_a_clash() {
        assert!(!is_arrow_rotation_hotkey(0x47, true, true, false, false));
    }

    #[test]
    fn p194_missing_ctrl_is_not_a_clash() {
        assert!(!is_arrow_rotation_hotkey(0x26, false, true, false, false));
    }

    #[test]
    fn p195_missing_alt_is_not_a_clash() {
        assert!(!is_arrow_rotation_hotkey(0x26, true, false, false, false));
    }

    #[test]
    fn p222_exact_clone_suffix_ties_break_by_ordinal_raw_identity() {
        let input = vec![
            KeyedMonitor {
                key: s("clone"),
                left: 0,
                top: 0,
                raw_identity_key: s("port-z"),
            },
            KeyedMonitor {
                key: s("clone"),
                left: 0,
                top: 0,
                raw_identity_key: s("port-a"),
            },
        ];
        let first = disambiguate_keys(&input);
        let mut rev = input.clone();
        rev.reverse();
        let second = disambiguate_keys(&rev);
        let by_raw = |inp: &[KeyedMonitor], out: &[String]| {
            let mut v: Vec<(String, String)> = inp
                .iter()
                .zip(out)
                .map(|(m, k)| (m.raw_identity_key.clone(), k.clone()))
                .collect();
            v.sort();
            v
        };
        let a = by_raw(&input, &first);
        assert_eq!(a, vec![(s("port-a"), s("clone#0")), (s("port-z"), s("clone#1"))]);
        assert_eq!(a, by_raw(&rev, &second));
    }

    // ---- support

    #[test]
    fn s_layout_string_rules_follow_dotnet() {
        assert_eq!(ordinal_cmp("\u{FF61}", "\u{10000}"), Ordering::Greater, "UTF-16 order");
        assert_eq!(ordinal_ignore_case_cmp("\u{FF61}", "\u{10000}"), Ordering::Less);
        assert_eq!(
            ordinal_ignore_case_cmp("_", "a"),
            Ordering::Greater,
            "compares uppercase"
        );
        assert!(ordinal_ignore_case_eq("Up", "uP"));
        assert!(!ordinal_ignore_case_eq("\u{131}", "i"), "dotless i never maps to ASCII");
        assert!(!ordinal_ignore_case_eq("\u{17F}", "s"));
        assert!(ordinal_ignore_case_eq("\u{3C3}", "\u{3C2}"));
        assert!(ordinal_ignore_case_eq("\u{1FB3}", "\u{1FBC}"));
        assert!(!ordinal_ignore_case_eq("\u{DF}", "\u{1E9E}"));
        assert!(looks_like_gdi_display_name("\\\\.\\display12"));
        assert!(!looks_like_gdi_display_name("\\\\.\\DISPLA"));
        assert!(!looks_like_gdi_display_name("MONITOR\\X"));
    }

    #[test]
    fn s_layout_disambiguation_groups_ignore_case_and_keeps_each_key() {
        let keys = disambiguate_keys(&[
            KeyedMonitor::new("Dup", 5, 0),
            KeyedMonitor::new("dup", 1, 0),
            KeyedMonitor::new("solo", 0, 0),
        ]);
        assert_eq!(keys, vec![s("Dup#1"), s("dup#0"), s("solo")]);
    }

    #[test]
    fn s_layout_identify_monitors_sorts_then_disambiguates() {
        let out = identify_monitors(vec![
            mon("same", "\\\\.\\DISPLAY2", 100, 0, 200, 100),
            mon("same", "\\\\.\\DISPLAY1", 0, 0, 100, 100),
        ]);
        let got: Vec<_> = out.iter().map(|m| (m.stable_id.as_str(), m.index)).collect();
        assert_eq!(got, vec![("same#0", 1), ("same#1", 2)]);
    }

    #[test]
    fn s_layout_migration_edge_cases_follow_csharp() {
        // Whitespace keys drop; already-stable keys stay; duplicates collapse (ordinal); a later map pair wins.
        let map = [(s("\\\\.\\DISPLAY1"), s("A")), (s("\\\\.\\display1"), s("B"))];
        let (m, l) = migrate_layout_keys_to_stable(
            &set(&[" ", "", "x", "x", "X", "\\\\.\\DISPLAY1"]),
            &[(s("x"), s(" ")), (s("x"), s("y")), (s("x"), s("y"))],
            &map,
        );
        assert_eq!(m, set(&["x", "X", "B"]));
        assert_eq!(l, vec![(s("x"), s("y"))]);
    }
}
