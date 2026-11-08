//! The settings window's port to the engine (Track A) and the settings model (Track B).
//!
//! Every call the window makes into fence-engine and fence-config goes through [`Backend`], so the window's own
//! behaviour can be tested headless against a test double while the other tracks are still stubs; [`RealBackend`]
//! is what the settings process and the screenshot modes use. At the join the same scenarios run against
//! `RealBackend` (the `s_join_*` tests, run since the join of 2026-10-13). Since the join `RealBackend` only
//! delegates: every rule it needs (seal check, layout-key migration, version read, case rules) is the one
//! fence-config or fence-engine owns, so the window, the save path and the reference-proven code are one code.

use fence_config::Settings;
use fence_types::{CompileResult, Hotkey, MonitorInfo, Rect, Screen, SealIssue};

/// What the settings window needs from the engine and the settings model (C# `BoundaryGraph`, `Settings`,
/// `GuardCore.Layout`, `MonitorInfo` helpers).
pub trait Backend {
    /// C# `BoundaryGraph.Compile(settings, FromMonitors(monitors, excluded))` as the map calls it: the excluded set
    /// is the raw `ExcludedDevices` list (case-insensitive), geometry safety on.
    fn compile(&self, settings: &Settings, monitors: &[MonitorInfo]) -> CompileResult;
    /// C# `Settings.ValidateNoSeal(monitors)`: normalizes `settings`, sanitizes the exclusions against the live
    /// monitors, compiles without geometry safety and returns the sealed screens.
    fn validate_no_seal(&self, settings: &mut Settings, monitors: &[MonitorInfo]) -> Vec<SealIssue>;
    /// C# `Settings.Normalize()`.
    fn normalize(&self, settings: &mut Settings);
    /// C# `JsonSerializer.Serialize(settings, WriteIndented = true)`.
    fn to_json(&self, settings: &Settings) -> String;
    /// C# `JsonSerializer.Deserialize<Settings>` (not normalized).
    fn parse_json(&self, text: &str) -> Result<Settings, String>;
    /// C# `Settings.MigrateLayoutKeysToStable(monitors)` (ends with Normalize). `Err` carries the name of the
    /// exception 1.1.1 throws (two live devices equal but for case, a null legacy up-link); the restore path then
    /// shows ImportInvalid and applies nothing, as the C# `catch` does (Ui/SettingsForm.General.cs:103, :119).
    fn migrate_layout_keys_to_stable(&self, settings: &mut Settings, monitors: &[MonitorInfo]) -> Result<(), String>;
    /// C# `GuardCore.SanitizeExcluded(present, primary, requested, out _)`.
    fn sanitize_excluded(&self, present: &[String], primary: Option<&str>, requested: &[String]) -> Vec<String>;
    /// C# `GuardCore.MapLayout(rects, w, h, pad)`.
    fn map_layout(&self, rects: &[Rect], width: i32, height: i32, pad: i32) -> Vec<Rect>;
    /// C# `Settings.NormalizeMonitorName`.
    fn normalize_monitor_name(&self, value: &str) -> String;
    /// WinForms `Keys.ToString()` of a virtual-key code ("Up", "G", "F24", "OemMinus").
    fn key_name(&self, vk: u32) -> String;
    /// C# `GuardCore.IsTop` (used by `MonitorInfo.IsAbovePrimary`).
    fn is_top(&self, bounds: Rect) -> bool;
    /// C# `MonitorInfo.SortAndNumber`.
    fn sort_and_number(&self, monitors: &mut [MonitorInfo]);
}

/// Screens for the compiler with a case-insensitive exclusion set (C# `HashSet(..., OrdinalIgnoreCase)`).
/// One rule for every crate (join 2026-10-13): fence-engine's `screens_from_monitors_ignore_case`, whose
/// OrdinalIgnoreCase is the .NET-proven one (it was `eq_ignore_ascii_case` here).
pub fn screens_for(monitors: &[MonitorInfo], excluded: &[String]) -> Vec<Screen> {
    fence_engine::compile::screens_from_monitors_ignore_case(monitors, excluded)
}

/// C# `Settings.ReadVersion(json)`: the `SettingsVersion` member when it is an Int32, else 0. Track B's
/// (fence-config), reference-checked; this crate kept its own copy until the join.
pub use fence_config::read_version;

/// C# `Settings.SanitizedExcluded(monitors)`: present ids, primary (or first) never excluded, keep one usable.
pub fn sanitized_excluded(backend: &dyn Backend, settings: &Settings, monitors: &[MonitorInfo]) -> Vec<String> {
    let present: Vec<String> = monitors.iter().map(|m| m.stable_id.clone()).collect();
    let primary = monitors
        .iter()
        .find(|m| m.primary)
        .or(monitors.first())
        .map(|m| m.stable_id.as_str());
    backend.sanitize_excluded(&present, primary, &settings.excluded_devices)
}

/// The production backend: fence-engine (Track A) and fence-config / layout helpers (Track B).
#[derive(Clone, Copy, Debug, Default)]
pub struct RealBackend;

impl Backend for RealBackend {
    fn compile(&self, settings: &Settings, monitors: &[MonitorInfo]) -> CompileResult {
        let policy = fence_config::to_policy(settings);
        fence_engine::compile::compile(&policy, &screens_for(monitors, &settings.excluded_devices), true)
    }

    fn validate_no_seal(&self, settings: &mut Settings, monitors: &[MonitorInfo]) -> Vec<SealIssue> {
        // fence-config's ValidateNoSeal (normalize, sanitized exclusions with JSON nulls dropped, compile without
        // geometry safety, 2.0 fixes on): the same call the save path makes, so window and save agree.
        settings.validate_no_seal(monitors)
    }

    fn normalize(&self, settings: &mut Settings) {
        fence_config::normalize(settings);
    }

    fn to_json(&self, settings: &Settings) -> String {
        fence_config::to_json_v2(settings)
    }

    fn parse_json(&self, text: &str) -> Result<Settings, String> {
        fence_config::from_json_v2(text)
    }

    fn migrate_layout_keys_to_stable(&self, settings: &mut Settings, monitors: &[MonitorInfo]) -> Result<(), String> {
        // Track B's port (JUDGE-D J1): OrdinalIgnoreCase device lookup, byte-wise \\.\DISPLAY check, the two C#
        // exceptions as errors, Normalize at the end; on error nothing changed.
        settings
            .migrate_layout_keys_to_stable(monitors)
            .map_err(|e| e.csharp_exception().to_string())
    }

    fn sanitize_excluded(&self, present: &[String], primary: Option<&str>, requested: &[String]) -> Vec<String> {
        fence_engine::layout::sanitize_excluded(present, primary, requested).0
    }

    fn map_layout(&self, rects: &[Rect], width: i32, height: i32, pad: i32) -> Vec<Rect> {
        fence_engine::layout::map_layout(rects, width, height, pad)
    }

    fn normalize_monitor_name(&self, value: &str) -> String {
        fence_config::normalize_monitor_name(value)
    }

    fn key_name(&self, vk: u32) -> String {
        // C# HotKeyText() with no modifiers is exactly Keys.ToString().
        fence_config::hotkey_text(&Hotkey {
            vk,
            ..Hotkey::default()
        })
    }

    fn is_top(&self, bounds: Rect) -> bool {
        fence_engine::topology::is_top(bounds)
    }

    fn sort_and_number(&self, monitors: &mut [MonitorInfo]) {
        fence_engine::layout::sort_and_number(monitors);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fence_config::{Stored, StoredRule};

    #[test]
    fn s_ui_read_version_matches_csharp_read_version() {
        assert_eq!(read_version(r#"{"SettingsVersion":3,"x":1}"#), 3);
        assert_eq!(read_version(r#"{"x":1}"#), 0, "absent field is legacy v0");
        assert_eq!(
            read_version(r#"{"SettingsVersion":"4"}"#),
            0,
            "a string is not an Int32"
        );
        assert_eq!(read_version(r#"{"SettingsVersion":4.5}"#), 0);
        assert_eq!(read_version("not json"), 0);
        assert_eq!(
            read_version(r#"{"SettingsVersion":99999999999}"#),
            0,
            "out of Int32 range"
        );
    }

    #[test]
    fn s_ui_screens_for_excludes_case_insensitively() {
        let m = |id: &str| MonitorInfo {
            device: String::new(),
            stable_id: id.into(),
            bounds: Rect::new(0, 0, 10, 10),
            primary: false,
            index: 1,
        };
        let s = screens_for(&[m("Abc"), m("def")], &["ABC".to_string()]);
        assert!(s[0].excluded && !s[1].excluded);
    }

    #[test]
    fn s_ui_gdi_display_prefix_never_panics_on_non_ascii_ids() {
        // The window now uses Track B's check (one implementation); this keeps Track D's regression cases on it.
        use fence_engine::layout::looks_like_gdi_display_name;
        // Byte 11 falls inside a multi-byte character in each of these; `id[..11]` panicked on them.
        for id in ["ééééééx", "Ekran-çalışma", "ğğğğğğğğğğğğ", "é", ""] {
            assert!(!looks_like_gdi_display_name(id), "{id}");
        }
        assert!(looks_like_gdi_display_name(r"\\.\DISPLAY1"));
        assert!(looks_like_gdi_display_name(r"\\.\display2"));
        assert!(looks_like_gdi_display_name(r"\\.\DISPLAY"));
        assert!(looks_like_gdi_display_name(r"\\.\DISPLAYé"));
        assert!(!looks_like_gdi_display_name(r"\\.\DISPLA"));
        assert!(
            !looks_like_gdi_display_name(r"\\.\DİSPLAY1"),
            "dotted capital I is not I under an ordinal comparison"
        );
    }

    #[test]
    fn s_ui_restore_of_an_old_backup_with_non_ascii_rule_ids_does_not_panic() {
        // editor.rs restore path: a backup older than CURRENT_SETTINGS_VERSION runs the migration over its rules.
        let rule = |from: &str, to: &str| StoredRule {
            from_stable_id: from.into(),
            to_stable_id: to.into(),
            direction: Stored::Unknown,
            behavior: Stored::Unknown,
            control: Stored::Unknown,
        };
        let mut settings = Settings {
            boundary_rules: vec![rule("ééééééx", "Ekran-çalışma"), rule(r"\\.\DISPLAY1", "Ekran-çalışma")],
            ..Settings::default()
        };
        RealBackend
            .migrate_layout_keys_to_stable(&mut settings, &[])
            .expect("no C# exception for these ids");
        // Two live devices equal but for case: C# ToDictionary throws ArgumentException, the import shows
        // ImportInvalid and nothing changes.
        let monitor = |device: &str, id: &str| MonitorInfo {
            device: device.into(),
            stable_id: id.into(),
            bounds: Rect::new(0, 0, 10, 10),
            primary: false,
            index: 1,
        };
        let mut old = Settings {
            boundary_rules: vec![rule(r"\\.\DISPLAY1", r"\\.\DISPLAY2")],
            ..Settings::default()
        };
        let before = old.clone();
        let clash = [monitor(r"\\.\DISPLAY1", "A"), monitor(r"\\.\display1", "B")];
        assert_eq!(
            RealBackend.migrate_layout_keys_to_stable(&mut old, &clash),
            Err("ArgumentException".to_string())
        );
        assert_eq!(old, before, "on error nothing changed");
        assert_eq!(settings.boundary_rules[0].from_stable_id, "ééééééx");
        assert_eq!(settings.boundary_rules[0].to_stable_id, "Ekran-çalışma");
        assert_eq!(
            settings.boundary_rules[1].from_stable_id, "",
            "an unmapped GDI name is cleared as in C#"
        );
        assert_eq!(settings.boundary_rules[1].to_stable_id, "Ekran-çalışma");
    }
}
