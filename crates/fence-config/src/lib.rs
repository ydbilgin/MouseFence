//! MouseFence settings (port of C# `Settings.cs`). OWNED BY TRACK B.
//!
//! Decisions (PLAN T15):
//! - The JSON content stays schema v4, byte-compatible with what 1.1.1 writes (indented, same property
//!   names and order, WinForms `Keys` as numbers, tolerant enums as names), so 1.x backups restore into 2.0.
//! - 2.0 NEVER writes the 1.x file `%APPDATA%\MouseFence\settings.json`. It keeps its own file
//!   `%APPDATA%\MouseFence\v2\settings.json` plus `settings.last-good.json`, and imports the 1.x file once,
//!   read-only, when the v2 file is missing.
//! - A load failure never silently becomes defaults that a later save writes over the user's file (fix of
//!   1.x L12): the bad bytes are kept, last-good is used when valid, else defaults marked not-saved.
//! - Saves are atomic (temp sibling, flush, replace) and refused while the draft has repairable import
//!   errors or seals a screen (C# `Save` silently returned; 2.0 returns the reason).
//!
//! Modules: `json` (System.Text.Json reading and writing), `normalize` (Normalize, the v0-v3 migration,
//! layout-key migration, policy mapping), `keys` (WinForms key names, hotkey text), `text` (nicknames and
//! text elements), `store` (files: load order, atomic save, last-good, unreadable copies).

use fence_engine::fixes::Fixes;
use fence_types::{
    BoundaryDefault, Control, CrossingModel, Direction, Hotkey, IntentConfig, MonitorInfo, Policy, RuleBehavior,
    SealIssue,
};

mod json;
pub mod keys;
mod normalize;
mod store;
mod text;

pub use keys::{ShortcutIssue, hotkey_text, keys_to_string, shortcut_issue, shortcut_issue_with_fixes};
pub use normalize::{MigrateError, add_shift_to_arrow_hotkeys, deep_clone, reset_layout_config, sanitized_excluded};
pub use store::{
    LoadDetail, LoadSource, Loaded, Migration, Parsed, Paths, SaveDetail, SaveOutcome, decode_text, load,
    load_detailed, load_production, load_read_only, parse_and_migrate, save, save_detailed, save_detailed_with_fixes,
    write_atomic, write_atomic_with,
};
pub use text::{MAX_NAME_ELEMENTS, monitor_name_for, normalize_monitor_name, text_element_starts};

pub const CURRENT_SETTINGS_VERSION: i32 = 4;

/// A stored enum value that may be unknown after a hand-edited or newer import (C# `TolerantEnumConverter`
/// maps unknown text to the enum's `Unknown` member so the settings window can show a repairable draft).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stored<T> {
    Known(T),
    Unknown,
}

/// One saved rule as stored (C# `BoundaryRule`, all three enums tolerant).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredRule {
    pub from_stable_id: String,
    pub to_stable_id: String,
    pub direction: Stored<Direction>,
    pub behavior: Stored<RuleBehavior>,
    pub control: Stored<Control>,
}

/// Legacy v0-v3 allowed upward crossing (C# `UpLink`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpLink {
    pub from_device: String,
    pub to_device: String,
}

/// What the C# object knows beyond the plain values: which members were present or JSON `null`. Needed to
/// reproduce 1.1.1 exactly (a missing `SettingsVersion` is legacy v0; a `null` Language is written back as
/// `null`; `null` list elements keep their place). Empty for settings built in code.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RawState {
    /// A failed production load produced unsaved defaults; editing them must never overwrite that file.
    pub write_protected: bool,
    /// Unknown v2 members in input order; exact 1.1.1 parsing deliberately leaves this empty.
    pub unknown_members: Vec<(String, serde_json::Value)>,
    /// C# `_settingsVersionWasSpecified`: the member was present, or code set the version.
    pub version_specified: bool,
    /// `"Language": null` was read (the value is then "" and is written back as `null`).
    pub language_null: bool,
    /// `"Theme": null` was read.
    pub theme_null: bool,
    /// `"CrossingPathMode": null` was read (legacy member).
    pub crossing_path_mode_null: bool,
    /// Positions in `manual_monitors` that hold a JSON `null` (stored there as "").
    pub manual_monitor_nulls: Vec<usize>,
    /// Positions in `excluded_devices` that hold a JSON `null` (stored there as "").
    pub excluded_device_nulls: Vec<usize>,
    /// Number of `null` entries in the legacy `UpLinks` list (they make the layout-key migration throw).
    pub legacy_up_link_nulls: usize,
}

/// Legacy persisted settings plus the production v2 crossing and tuning options. The exact legacy JSON
/// serializer omits the v2-only fields. WinForms `Keys` values
/// are kept as their integer value (`hot_key` etc.), which equals the Windows virtual-key code; the full
/// 32 bits are kept (`-1` is stored as `u32::MAX`) so any 1.x value round-trips.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub crossing_ease: i32,
    pub crossing_model: CrossingModel,
    pub up_push_distance_px: i32,
    pub side_push_distance_px: i32,
    pub push_absorb_ms: i32,
    pub push_pause_reset_ms: i32,
    /// Intent steady-push hold (v2 only): -1 off, 0 follows the ease slider, 30..=500 milliseconds.
    pub steady_push_ms: i32,
    pub edge_trace: bool,
    pub mark_accident_hot_key: u32,
    pub mark_accident_mod_ctrl: bool,
    pub mark_accident_mod_alt: bool,
    pub mark_accident_mod_shift: bool,
    pub mark_accident_mod_win: bool,
    pub settings_version: i32,
    pub hot_key: u32,
    pub mod_ctrl: bool,
    pub mod_alt: bool,
    pub mod_shift: bool,
    pub mod_win: bool,
    pub confine_hot_key: u32,
    pub confine_mod_ctrl: bool,
    pub confine_mod_alt: bool,
    pub confine_mod_shift: bool,
    pub confine_mod_win: bool,
    pub pause_hot_key: u32,
    pub pause_mod_ctrl: bool,
    pub pause_mod_alt: bool,
    pub pause_mod_shift: bool,
    pub pause_mod_win: bool,
    pub side_hot_key: u32,
    pub side_mod_ctrl: bool,
    pub side_mod_alt: bool,
    pub side_mod_shift: bool,
    pub side_mod_win: bool,
    pub start_gate_closed: bool,
    pub start_confine_on: bool,
    pub start_paused: bool,
    pub start_side_contain_on: bool,
    pub auto_start: bool,
    pub deliberate_cross: bool,
    pub descent_routing: bool,
    pub upward_boundary_default: Stored<BoundaryDefault>,
    pub side_boundary_default: Stored<BoundaryDefault>,
    pub downward_boundary_default: Stored<BoundaryDefault>,
    pub boundary_rules: Vec<StoredRule>,
    pub up_cross_min: i32,
    pub up_cross_slack: i32,
    pub passage_inset_px: i32,
    pub edge_holdback_px: i32,
    pub seam_tolerance_px: i32,
    pub safety_escapes: bool,
    pub boundary_tip_seen: bool,
    pub side_cross_min: i32,
    pub side_cross_slack: i32,
    /// "AutoTop" | "Manual" (raw text kept; Normalize repairs anything else to AutoTop).
    pub mode: String,
    pub manual_monitors: Vec<String>,
    pub excluded_devices: Vec<String>,
    /// Stable id -> nickname, case-insensitive keys, in the order Normalize leaves them.
    pub monitor_names: Vec<(String, String)>,
    /// "auto" | "en" | "tr".
    pub language: String,
    /// "System" | "Light" | "Dark".
    pub theme: String,
    /// Legacy v0-v3 fields, read for migration and written only while `settings_version < 4`.
    pub legacy_crossing_path_mode: Option<String>,
    pub legacy_up_links: Vec<UpLink>,
    /// Transient (never persisted): set by Normalize when the draft has repairable import errors.
    pub has_repairable_import_errors: bool,
    /// Transient: a fresh install on an Intel GPU had Shift added to its arrow hotkeys.
    pub intel_defaults_applied: bool,
    /// Presence and `null` bookkeeping of the JSON that was read (see [`RawState`]).
    pub raw: RawState,
}

impl Default for Settings {
    /// C# `new Settings { SettingsVersion = CurrentSettingsVersion }` (a fresh install before the Intel check).
    fn default() -> Self {
        Self {
            crossing_ease: 3,
            crossing_model: CrossingModel::Classic1k,
            up_push_distance_px: 120,
            side_push_distance_px: 120,
            push_absorb_ms: 75,
            push_pause_reset_ms: 150,
            steady_push_ms: 0,
            edge_trace: false,
            mark_accident_hot_key: 0x4D,
            mark_accident_mod_ctrl: true,
            mark_accident_mod_alt: true,
            mark_accident_mod_shift: false,
            mark_accident_mod_win: false,
            settings_version: CURRENT_SETTINGS_VERSION,
            hot_key: keys::UP,
            mod_ctrl: true,
            mod_alt: true,
            mod_shift: false,
            mod_win: false,
            confine_hot_key: keys::G,
            confine_mod_ctrl: true,
            confine_mod_alt: true,
            confine_mod_shift: false,
            confine_mod_win: false,
            pause_hot_key: keys::P,
            pause_mod_ctrl: true,
            pause_mod_alt: true,
            pause_mod_shift: false,
            pause_mod_win: false,
            side_hot_key: keys::RIGHT,
            side_mod_ctrl: true,
            side_mod_alt: true,
            side_mod_shift: false,
            side_mod_win: false,
            start_gate_closed: true,
            start_confine_on: false,
            start_paused: false,
            start_side_contain_on: true,
            auto_start: false,
            deliberate_cross: true,
            descent_routing: false,
            upward_boundary_default: Stored::Known(BoundaryDefault::Automatic),
            side_boundary_default: Stored::Known(BoundaryDefault::Automatic),
            downward_boundary_default: Stored::Known(BoundaryDefault::Free),
            boundary_rules: Vec::new(),
            up_cross_min: 3,
            up_cross_slack: 5,
            passage_inset_px: 24,
            edge_holdback_px: 0,
            seam_tolerance_px: 2,
            safety_escapes: true,
            boundary_tip_seen: false,
            side_cross_min: 3,
            side_cross_slack: 5,
            mode: "AutoTop".to_string(),
            manual_monitors: Vec::new(),
            excluded_devices: Vec::new(),
            monitor_names: Vec::new(),
            language: "auto".to_string(),
            theme: "System".to_string(),
            legacy_crossing_path_mode: None,
            legacy_up_links: Vec::new(),
            has_repairable_import_errors: false,
            intel_defaults_applied: false,
            raw: RawState {
                version_specified: true,
                ..RawState::default()
            },
        }
    }
}

fn chord(vk: u32, ctrl: bool, alt: bool, shift: bool, win: bool) -> Hotkey {
    Hotkey {
        vk,
        ctrl,
        alt,
        shift,
        win,
    }
}

/// `SteadyPushMs`: any negative value means off (-1), 0 follows the ease slider, other values are
/// clamped to 30..=500 milliseconds.
pub(crate) fn steady_push_ms(value: i32) -> i32 {
    match value {
        v if v < 0 => -1,
        0 => 0,
        v => v.clamp(30, 500),
    }
}

impl Settings {
    pub fn intent_config(&self) -> IntentConfig {
        IntentConfig {
            crossing_ease: self.crossing_ease.clamp(1, 5) as u8,
            model: self.crossing_model,
            up_push_distance_px: self.up_push_distance_px.clamp(1, 2000),
            side_push_distance_px: self.side_push_distance_px.clamp(1, 2000),
            push_absorb_ms: self.push_absorb_ms.clamp(0, 500) as u32,
            push_pause_reset_ms: self.push_pause_reset_ms.clamp(20, 2000) as u32,
            steady_push_ms: steady_push_ms(self.steady_push_ms),
        }
    }

    pub fn mark_accident_hotkey(&self) -> Hotkey {
        chord(
            self.mark_accident_hot_key,
            self.mark_accident_mod_ctrl,
            self.mark_accident_mod_alt,
            self.mark_accident_mod_shift,
            self.mark_accident_mod_win,
        )
    }
    /// C# `new Settings()` without a version: what the deserializer starts from (a missing
    /// `SettingsVersion` member then means legacy v0).
    pub fn unspecified() -> Self {
        let mut s = Self::default();
        s.raw.version_specified = false;
        s
    }

    /// The C# `SettingsVersion` setter (marks the version as specified).
    pub fn set_settings_version(&mut self, version: i32) {
        self.settings_version = version;
        self.raw.version_specified = true;
    }

    pub fn hotkey(&self) -> Hotkey {
        chord(self.hot_key, self.mod_ctrl, self.mod_alt, self.mod_shift, self.mod_win)
    }
    pub fn confine_hotkey(&self) -> Hotkey {
        chord(
            self.confine_hot_key,
            self.confine_mod_ctrl,
            self.confine_mod_alt,
            self.confine_mod_shift,
            self.confine_mod_win,
        )
    }
    pub fn pause_hotkey(&self) -> Hotkey {
        chord(
            self.pause_hot_key,
            self.pause_mod_ctrl,
            self.pause_mod_alt,
            self.pause_mod_shift,
            self.pause_mod_win,
        )
    }
    pub fn side_hotkey(&self) -> Hotkey {
        chord(
            self.side_hot_key,
            self.side_mod_ctrl,
            self.side_mod_alt,
            self.side_mod_shift,
            self.side_mod_win,
        )
    }

    /// C# `HotKeyText()`.
    pub fn hot_key_text(&self) -> String {
        hotkey_text(&self.hotkey())
    }
    /// C# `ConfineHotKeyText()`.
    pub fn confine_hot_key_text(&self) -> String {
        hotkey_text(&self.confine_hotkey())
    }
    /// C# `PauseHotKeyText()`.
    pub fn pause_hot_key_text(&self) -> String {
        hotkey_text(&self.pause_hotkey())
    }
    /// C# `SideHotKeyText()`.
    pub fn side_hot_key_text(&self) -> String {
        hotkey_text(&self.side_hotkey())
    }

    /// C# `Settings.MonitorNameFor(names, stableId)` (case-insensitive lookup, normalized name or "").
    pub fn monitor_name(&self, stable_id: &str) -> String {
        monitor_name_for(&self.monitor_names, stable_id)
    }

    /// Sets or clears (blank name) a nickname; the key matches case-insensitively and the name is normalized.
    pub fn set_monitor_name(&mut self, stable_id: &str, name: &str) {
        let id = stable_id.trim();
        let name = normalize_monitor_name(name);
        let slot = self
            .monitor_names
            .iter()
            .position(|(k, _)| fence_engine::layout::ordinal_ignore_case_eq(k, id));
        match (slot, name.is_empty() || id.is_empty()) {
            (Some(i), true) => {
                self.monitor_names.remove(i);
            }
            (Some(i), false) => self.monitor_names[i].1 = name,
            (None, false) => self.monitor_names.push((id.to_string(), name)),
            (None, true) => {}
        }
    }

    /// C# computed `CrossingPathMode` (the M1 text editor's view): "Custom" when the upward default is
    /// Blocked, else "Default".
    pub fn crossing_path_mode(&self) -> &'static str {
        if self.upward_boundary_default == Stored::Known(BoundaryDefault::Blocked) {
            "Custom"
        } else {
            "Default"
        }
    }

    /// C# computed `UpLinks`: the Up + OverallFeel rules as (from, to) pairs.
    pub fn up_links(&self) -> Vec<UpLink> {
        self.boundary_rules
            .iter()
            .filter(|r| {
                r.direction == Stored::Known(Direction::Up) && r.behavior == Stored::Known(RuleBehavior::OverallFeel)
            })
            .map(|r| UpLink {
                from_device: r.from_stable_id.clone(),
                to_device: r.to_stable_id.clone(),
            })
            .collect()
    }

    /// C# `Settings.ValidateNoSeal(monitors)`: Normalize in place, then the sealed screens of the layout with
    /// the sanitized exclusions (compiled without geometry safety by fence-engine).
    pub fn validate_no_seal(&mut self, monitors: &[MonitorInfo]) -> Vec<SealIssue> {
        self.validate_no_seal_with(monitors, Fixes::ALL).0
    }

    /// [`Settings::validate_no_seal`] with the chosen judged fixes (`Fixes::EXACT_1_1_1` is 1.1.1 exactly, the
    /// form a validator diff against the C# reference needs), plus the `fence_engine::fixes::HIT_*` bits of the
    /// fixes that changed the answer. Same pattern as `fence_engine::compile::validate_no_seal_with`.
    pub fn validate_no_seal_with(&mut self, monitors: &[MonitorInfo], fixes: Fixes) -> (Vec<SealIssue>, u16) {
        normalize(self);
        normalize::seal_issues_of_normalized(self, monitors, fixes)
    }

    /// C# `Settings.MigrateLayoutKeysToStable(monitors)`; on error nothing changed.
    pub fn migrate_layout_keys_to_stable(&mut self, monitors: &[MonitorInfo]) -> Result<(), MigrateError> {
        normalize::migrate_layout_keys_to_stable(self, monitors)
    }
}

/// The seal check for [`save`]: `ValidateNoSeal` on a normalized copy (the settings passed in stay as they
/// are). The usual closure is `&|s| fence_config::seal_issues(s, &live_monitors)`.
pub fn seal_issues(settings: &Settings, monitors: &[MonitorInfo]) -> Vec<SealIssue> {
    seal_issues_with(settings, monitors, Fixes::ALL).0
}

/// [`seal_issues`] with the chosen judged fixes, plus the `HIT_*` bits (see [`Settings::validate_no_seal_with`]).
pub fn seal_issues_with(settings: &Settings, monitors: &[MonitorInfo], fixes: Fixes) -> (Vec<SealIssue>, u16) {
    let mut copy = settings.clone();
    copy.validate_no_seal_with(monitors, fixes)
}

/// C# `Settings.Normalize()` including the v0-v3 migration of the crossing-path meaning and the clamps.
pub fn normalize(settings: &mut Settings) {
    normalize::normalize(settings);
}

/// The engine-facing view of the settings (valid rules only, saved order kept; an Unknown default compiles
/// as Blocked, as the C# compiler resolves it).
pub fn to_policy(settings: &Settings) -> Policy {
    normalize::to_policy(settings)
}

/// Serialize exactly as C# `JsonSerializer.Serialize(settings, WriteIndented = true)` (what `Save` writes).
/// The settings are written as they are; `save` normalizes first, as C# `Save` does.
pub fn to_json(settings: &Settings) -> String {
    json::write_settings(settings, true)
}

/// Serialize as C# `JsonSerializer.Serialize(settings)` without indentation (what `DeepClone` uses).
pub fn to_json_compact(settings: &Settings) -> String {
    json::write_settings(settings, false)
}

/// C# `JsonSerializer.Deserialize<Settings>(text) ?? new Settings()` with the "member present" tracking for
/// SettingsVersion and the legacy members. Does not normalize. The error text says what was rejected.
pub fn from_json(text: &str) -> Result<Settings, String> {
    Ok(json::read_settings(text)?.unwrap_or_else(Settings::unspecified))
}

/// Production v2 parsing, preserving unknown top-level fields and refusing future schemas or null documents.
pub fn from_json_v2(text: &str) -> Result<Settings, String> {
    from_json_v2_with_fixes(text, Fixes::ALL)
}

pub fn from_json_v2_with_fixes(text: &str, fixes: Fixes) -> Result<Settings, String> {
    if fixes.j18_settings_durability && read_version(text) > CURRENT_SETTINGS_VERSION {
        return Err("the settings require a newer MouseFence version".into());
    }
    let settings = if fixes.j19_unknown_settings {
        json::read_settings_v2(text)?
    } else {
        json::read_settings(text)?
    };
    match settings {
        Some(s) => Ok(s),
        None if !fixes.j18_settings_durability => Ok(Settings::unspecified()),
        None => Err("the settings JSON must contain an object".into()),
    }
}

/// Production v2 writing; the exact legacy writer remains available as [`to_json`].
pub fn to_json_v2(settings: &Settings) -> String {
    to_json_v2_with_fixes(settings, Fixes::ALL)
}

pub fn to_json_v2_with_fixes(settings: &Settings, fixes: Fixes) -> String {
    if fixes.j19_unknown_settings {
        json::write_settings_v2(settings)
    } else {
        to_json(settings)
    }
}

/// Ephemeral display identities must never acquire saved per-monitor ownership.
pub fn has_ambiguous_display_choices(settings: &Settings) -> bool {
    let ambiguous = |id: &str| id.contains("#ambiguous:");
    settings.monitor_names.iter().any(|(id, _)| ambiguous(id))
        || settings.excluded_devices.iter().any(|id| ambiguous(id))
        || settings.manual_monitors.iter().any(|id| ambiguous(id))
        || settings
            .boundary_rules
            .iter()
            .any(|r| ambiguous(&r.from_stable_id) || ambiguous(&r.to_stable_id))
        || settings
            .legacy_up_links
            .iter()
            .any(|r| ambiguous(&r.from_device) || ambiguous(&r.to_device))
}

/// C# `Settings.ReadVersion(json)`: the `SettingsVersion` member when it is an Int32, else 0.
pub fn read_version(text: &str) -> i32 {
    json::read_version(text)
}

#[cfg(test)]
mod tests;
