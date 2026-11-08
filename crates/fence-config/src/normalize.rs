//! C# `Settings.Normalize` (with the v0-v3 meaning migration and the clamps), `MigrateLayoutKeysToStable`,
//! `AddShiftToArrowHotkeys`, `ResetLayoutConfig`, `DeepClone`, `SanitizedExcluded` and `ValidateNoSeal`,
//! plus the mapping to the engine's `Policy`.

use fence_engine::fixes::Fixes;
use fence_engine::layout::{self, looks_like_gdi_display_name, ordinal_cmp, ordinal_ignore_case_eq};
use fence_types::{
    BoundaryDefault, BoundaryRule, Control, Direction, MonitorInfo, Policy, RuleBehavior, Screen, SealIssue, TopMode,
};

use crate::text::normalize_monitor_name;
use crate::{CURRENT_SETTINGS_VERSION, Settings, Stored, StoredRule, UpLink, json};

/// C# `Settings.Normalize()`.
pub fn normalize(s: &mut Settings) {
    s.up_push_distance_px = s.up_push_distance_px.clamp(1, 2000);
    s.side_push_distance_px = s.side_push_distance_px.clamp(1, 2000);
    s.crossing_ease = s.crossing_ease.clamp(1, 5);
    s.push_absorb_ms = s.push_absorb_ms.clamp(0, 500);
    s.push_pause_reset_ms = s.push_pause_reset_ms.clamp(20, 2000);
    s.steady_push_ms = crate::steady_push_ms(s.steady_push_ms);
    // MonitorNames: ordered by key (ordinal), keys trimmed, names normalized, blanks dropped, the first of
    // keys equal ignoring case kept (the C# map becomes OrdinalIgnoreCase).
    let mut pairs = std::mem::take(&mut s.monitor_names);
    pairs.sort_by(|a, b| ordinal_cmp(&a.0, &b.0));
    let mut names: Vec<(String, String)> = Vec::with_capacity(pairs.len());
    for (key, value) in pairs {
        let id = key.trim();
        let name = normalize_monitor_name(&value);
        if !id.is_empty() && !name.is_empty() && !names.iter().any(|(k, _)| ordinal_ignore_case_eq(k, id)) {
            names.push((id.to_string(), name));
        }
    }
    s.monitor_names = names;

    let loaded_version = if s.raw.version_specified { s.settings_version } else { 0 };
    if loaded_version < CURRENT_SETTINGS_VERSION {
        let valid = matches!(s.legacy_crossing_path_mode.as_deref(), Some("Default" | "Custom"));
        if !valid {
            let any_links = !s.legacy_up_links.is_empty() || s.raw.legacy_up_link_nulls > 0;
            s.legacy_crossing_path_mode = Some(if any_links { "Custom" } else { "Default" }.to_string());
        }
        if s.legacy_crossing_path_mode.as_deref() == Some("Custom") {
            s.upward_boundary_default = Stored::Known(BoundaryDefault::Blocked);
            s.boundary_rules = s
                .legacy_up_links
                .iter()
                .map(|l| StoredRule {
                    from_stable_id: l.from_device.clone(),
                    to_stable_id: l.to_device.clone(),
                    direction: Stored::Known(Direction::Up),
                    behavior: Stored::Known(RuleBehavior::OverallFeel),
                    control: Stored::Known(Control::Automatic),
                })
                .collect();
        } else {
            s.upward_boundary_default = Stored::Known(BoundaryDefault::Automatic);
            s.boundary_rules.clear();
        }
        s.side_boundary_default = Stored::Known(BoundaryDefault::Automatic);
        s.downward_boundary_default = Stored::Known(BoundaryDefault::Free);
        s.edge_holdback_px = 0;
        s.start_confine_on = false;
        s.start_paused = false;
        s.boundary_tip_seen = false;
        if s.mode == "Manual" && s.manual_monitors.is_empty() {
            s.mode = "AutoTop".to_string();
        }
        s.set_settings_version(CURRENT_SETTINGS_VERSION);
    }

    s.up_cross_min = s.up_cross_min.clamp(1, 200);
    s.up_cross_slack = s.up_cross_slack.clamp(0, 200);
    s.side_cross_min = s.side_cross_min.clamp(1, 200);
    s.side_cross_slack = s.side_cross_slack.clamp(0, 200);
    s.passage_inset_px = s.passage_inset_px.clamp(0, 500);
    s.edge_holdback_px = s.edge_holdback_px.clamp(0, 200);
    s.seam_tolerance_px = s.seam_tolerance_px.clamp(0, 200);

    let mut repairable = s.upward_boundary_default == Stored::Unknown
        || s.side_boundary_default == Stored::Unknown
        || s.downward_boundary_default == Stored::Unknown;
    // Valid rules keyed by the C# identity text "from\nto\nDirection" (OrdinalIgnoreCase); a later valid rule
    // replaces an earlier one in its slot (import contract: last valid wins). Invalid rules follow in order.
    let mut dedup: Vec<(String, StoredRule)> = Vec::new();
    let mut invalid: Vec<StoredRule> = Vec::new();
    for mut rule in std::mem::take(&mut s.boundary_rules) {
        rule.from_stable_id = rule.from_stable_id.trim().to_string();
        rule.to_stable_id = rule.to_stable_id.trim().to_string();
        let enum_invalid =
            rule.direction == Stored::Unknown || rule.behavior == Stored::Unknown || rule.control == Stored::Unknown;
        if enum_invalid
            || rule.from_stable_id.is_empty()
            || rule.to_stable_id.is_empty()
            || ordinal_ignore_case_eq(&rule.from_stable_id, &rule.to_stable_id)
        {
            repairable = true;
            invalid.push(rule);
            continue;
        }
        let identity = format!(
            "{}\n{}\n{}",
            rule.from_stable_id,
            rule.to_stable_id,
            json::direction_name(&rule.direction)
        );
        match dedup.iter_mut().find(|(k, _)| ordinal_ignore_case_eq(k, &identity)) {
            Some(slot) => slot.1 = rule,
            None => dedup.push((identity, rule)),
        }
    }
    let mut valid: Vec<StoredRule> = dedup.into_iter().map(|(_, r)| r).collect();
    valid.sort_by(|a, b| {
        ordinal_cmp(&a.from_stable_id, &b.from_stable_id)
            .then_with(|| ordinal_cmp(&a.to_stable_id, &b.to_stable_id))
            .then_with(|| direction_ordinal(&a.direction).cmp(&direction_ordinal(&b.direction)))
    });
    valid.extend(invalid);
    s.boundary_rules = valid;
    s.has_repairable_import_errors = repairable;

    if s.mode != "AutoTop" && s.mode != "Manual" {
        s.mode = "AutoTop".to_string();
    }
}

fn direction_ordinal(d: &Stored<Direction>) -> i32 {
    match d {
        Stored::Known(x) => x.csharp_ordinal(),
        Stored::Unknown => 0,
    }
}

/// Why the C# layout-key migration throws (Load then keeps the settings unmigrated and does not save).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MigrateError {
    /// Two live monitors whose device names differ only by case (C# `ToDictionary` ArgumentException).
    DuplicateDevice(String),
    /// A null element in the legacy UpLinks list (C# NullReferenceException).
    NullLegacyLink,
}

impl MigrateError {
    /// The exception type 1.1.1 throws, as the reference reports it.
    pub fn csharp_exception(&self) -> &'static str {
        match self {
            MigrateError::DuplicateDevice(_) => "ArgumentException",
            MigrateError::NullLegacyLink => "NullReferenceException",
        }
    }
}

impl std::fmt::Display for MigrateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrateError::DuplicateDevice(d) => write!(f, "two monitors share the device name {d} (ignoring case)"),
            MigrateError::NullLegacyLink => write!(f, "the legacy UpLinks list holds a null entry"),
        }
    }
}

/// C# `Settings.MigrateLayoutKeysToStable(monitors)`: `\\.\DISPLAYn` keys in ManualMonitors, the legacy
/// UpLinks and the rules become stable ids through the live monitors; stale display names drop (rules get an
/// empty id and become repairable). Ends with Normalize. On error nothing has changed.
pub fn migrate_layout_keys_to_stable(s: &mut Settings, monitors: &[MonitorInfo]) -> Result<(), MigrateError> {
    // monitors.GroupBy(Device).ToDictionary(Key, First().StableId, OrdinalIgnoreCase)
    let mut map: Vec<(String, String)> = Vec::new();
    for m in monitors {
        if map.iter().any(|(d, _)| d == &m.device) {
            continue;
        }
        if map.iter().any(|(d, _)| ordinal_ignore_case_eq(d, &m.device)) {
            return Err(MigrateError::DuplicateDevice(m.device.clone()));
        }
        map.push((m.device.clone(), m.stable_id.clone()));
    }
    if s.raw.legacy_up_link_nulls > 0 {
        return Err(MigrateError::NullLegacyLink);
    }
    let links: Vec<(String, String)> = s
        .legacy_up_links
        .iter()
        .map(|l| (l.from_device.clone(), l.to_device.clone()))
        .collect();
    let (manual, migrated) = layout::migrate_layout_keys_to_stable(&s.manual_monitors, &links, &map);
    s.manual_monitors = manual;
    s.raw.manual_monitor_nulls.clear();
    s.legacy_up_links = migrated
        .into_iter()
        .map(|(from_device, to_device)| UpLink { from_device, to_device })
        .collect();
    let lookup = |key: &str| {
        map.iter()
            .find(|(d, _)| ordinal_ignore_case_eq(d, key))
            .map(|(_, v)| v.clone())
    };
    for rule in &mut s.boundary_rules {
        if let Some(stable) = lookup(&rule.from_stable_id) {
            rule.from_stable_id = stable;
        } else if looks_like_gdi_display_name(&rule.from_stable_id) {
            rule.from_stable_id.clear();
        }
        if let Some(stable) = lookup(&rule.to_stable_id) {
            rule.to_stable_id = stable;
        } else if looks_like_gdi_display_name(&rule.to_stable_id) {
            rule.to_stable_id.clear();
        }
    }
    normalize(s);
    Ok(())
}

/// C# `Settings.AddShiftToArrowHotkeys()`: on an Intel GPU, add Shift to the up and side hotkeys when they are
/// exactly Ctrl+Alt+Arrow; `intel_defaults_applied` tells whether anything changed.
pub fn add_shift_to_arrow_hotkeys(s: &mut Settings) {
    let mut changed = false;
    if layout::is_arrow_rotation_hotkey(s.hot_key, s.mod_ctrl, s.mod_alt, s.mod_shift, s.mod_win) {
        s.mod_shift = true;
        changed = true;
    }
    if layout::is_arrow_rotation_hotkey(
        s.side_hot_key,
        s.side_mod_ctrl,
        s.side_mod_alt,
        s.side_mod_shift,
        s.side_mod_win,
    ) {
        s.side_mod_shift = true;
        changed = true;
    }
    s.intel_defaults_applied = changed;
}

/// C# `Settings.ResetLayoutConfig()`: AutoTop, no manual picks, no rules, default boundary feel. Exclusions and
/// nicknames stay.
pub fn reset_layout_config(s: &mut Settings) {
    let (mode, manual, _links) = layout::reset_layout_config();
    s.mode = mode;
    s.manual_monitors = manual;
    s.raw.manual_monitor_nulls.clear();
    s.upward_boundary_default = Stored::Known(BoundaryDefault::Automatic);
    s.side_boundary_default = Stored::Known(BoundaryDefault::Automatic);
    s.downward_boundary_default = Stored::Known(BoundaryDefault::Free);
    s.boundary_rules.clear();
    s.up_cross_min = 3;
    s.up_cross_slack = 5;
    s.side_cross_min = 3;
    s.side_cross_slack = 5;
    s.passage_inset_px = 24;
    s.edge_holdback_px = 0;
    s.seam_tolerance_px = 2;
    s.safety_escapes = true;
}

/// C# `Settings.DeepClone()`: serialize, deserialize, Normalize (transient flags are not carried over).
pub fn deep_clone(s: &Settings) -> Settings {
    let text = json::write_settings(s, false);
    let mut clone = json::read_settings(&text)
        .ok()
        .flatten()
        .unwrap_or_else(Settings::unspecified);
    normalize(&mut clone);
    clone
}

/// The entries of a nullable list that are real strings (C# nulls never match a monitor).
fn real_entries(items: &[String], nulls: &[usize]) -> Vec<String> {
    items
        .iter()
        .enumerate()
        .filter(|(i, v)| !(v.is_empty() && nulls.contains(i)))
        .map(|(_, v)| v.clone())
        .collect()
}

/// C# `Settings.SanitizedExcluded(monitors)`: the requested exclusions that are present, never the primary,
/// never every screen.
pub fn sanitized_excluded(s: &Settings, monitors: &[MonitorInfo]) -> Vec<String> {
    let present: Vec<String> = monitors.iter().map(|m| m.stable_id.clone()).collect();
    let primary = monitors
        .iter()
        .find(|m| m.primary)
        .or(monitors.first())
        .map(|m| m.stable_id.as_str());
    let requested = real_entries(&s.excluded_devices, &s.raw.excluded_device_nulls);
    layout::sanitize_excluded(&present, primary, &requested).0
}

/// The body of C# `Settings.ValidateNoSeal(monitors)` after its Normalize: sanitize the exclusions, build the
/// screens (C# `FromMonitors` with the OrdinalIgnoreCase set `SanitizedExcluded` returns), then the compiler's
/// seal check without geometry safety (fence-engine `compile::validate_no_seal_with`, Track A) with the chosen
/// judged fixes, plus the `HIT_*` bits of the fixes that changed the answer.
pub(crate) fn seal_issues_of_normalized(s: &Settings, monitors: &[MonitorInfo], fixes: Fixes) -> (Vec<SealIssue>, u16) {
    let excluded = sanitized_excluded(s, monitors);
    let screens: Vec<Screen> = monitors
        .iter()
        .map(|m| Screen {
            stable_id: m.stable_id.clone(),
            bounds: m.bounds,
            primary: m.primary,
            excluded: excluded.iter().any(|e| ordinal_ignore_case_eq(e, &m.stable_id)),
        })
        .collect();
    fence_engine::compile::validate_no_seal_with(&to_policy(s), &screens, fixes)
}

/// The engine-facing view of the settings. Unknown boundary defaults become `Blocked`, which is what the C#
/// compiler resolves them to (`BoundaryGraph.Resolve`, `_ => Blocked`); invalid rules are dropped (C#
/// `NormalizeRuleMap`); valid rules keep their order and trimmed ids; JSON nulls in the id lists are dropped.
pub fn to_policy(s: &Settings) -> Policy {
    let known = |v: &Stored<BoundaryDefault>| match v {
        Stored::Known(x) => *x,
        Stored::Unknown => BoundaryDefault::Blocked,
    };
    let rules = s
        .boundary_rules
        .iter()
        .filter_map(|r| {
            let (Stored::Known(direction), Stored::Known(behavior), Stored::Known(control)) =
                (&r.direction, &r.behavior, &r.control)
            else {
                return None;
            };
            let (from, to) = (r.from_stable_id.trim(), r.to_stable_id.trim());
            if from.is_empty() || to.is_empty() || ordinal_ignore_case_eq(from, to) {
                return None;
            }
            Some(BoundaryRule {
                from_stable_id: from.to_string(),
                to_stable_id: to.to_string(),
                direction: *direction,
                behavior: *behavior,
                control: *control,
            })
        })
        .collect();
    Policy {
        intent: s.intent_config(),
        upward_default: known(&s.upward_boundary_default),
        side_default: known(&s.side_boundary_default),
        downward_default: known(&s.downward_boundary_default),
        rules,
        up_cross_min: s.up_cross_min,
        up_cross_slack: s.up_cross_slack,
        side_cross_min: s.side_cross_min,
        side_cross_slack: s.side_cross_slack,
        passage_inset_px: s.passage_inset_px,
        edge_holdback_px: s.edge_holdback_px,
        seam_tolerance_px: s.seam_tolerance_px,
        safety_escapes: s.safety_escapes,
        deliberate_cross: s.deliberate_cross,
        descent_routing: s.descent_routing,
        mode: if s.mode == "Manual" {
            TopMode::Manual
        } else {
            TopMode::AutoTop
        },
        manual_monitors: real_entries(&s.manual_monitors, &s.raw.manual_monitor_nulls),
        excluded_devices: real_entries(&s.excluded_devices, &s.raw.excluded_device_nulls),
    }
}
