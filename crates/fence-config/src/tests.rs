//! fence-config unit tests: the 1.1.1 suite rows of Track B (pNNN_), the file behaviour rows (b030_, b031_,
//! b033_, j04_) in temp folders, and the pinned reference answers for b032_. Every layout is synthetic.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use fence_types::{BoundaryDefault, Control, Direction, MonitorInfo, Rect, RuleBehavior, SealIssue};

use super::*;

fn parsed(text: &str) -> Settings {
    from_json(text).expect("test JSON parses")
}

fn normalized(text: &str) -> Settings {
    let mut s = parsed(text);
    normalize(&mut s);
    s
}

fn mon(device: &str, id: &str, l: i32, t: i32, r: i32, b: i32, primary: bool) -> MonitorInfo {
    MonitorInfo {
        device: device.to_string(),
        stable_id: id.to_string(),
        bounds: Rect::new(l, t, r, b),
        primary,
        index: 0,
    }
}

/// A fresh folder under the system temp folder; removed by `Drop` (only this exact folder).
struct TempHome {
    root: PathBuf,
}

impl TempHome {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("mf-config-test-{tag}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&root).expect("temp folder");
        Self { root }
    }
    fn paths(&self) -> Paths {
        Paths::under_appdata(&self.root)
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        let temp = std::env::temp_dir();
        let name_ok = self
            .root
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("mf-config-test-"));
        if self.root.parent() == Some(temp.as_path()) && name_ok {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn no_seal(_: &Settings) -> Vec<SealIssue> {
    Vec::new()
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|it| {
            it.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

// ------------------------------------------------------------------------------------------------
// Skeleton checks
// ------------------------------------------------------------------------------------------------

#[test]
fn s_config_defaults_match_csharp_new_settings() {
    let s = Settings::default();
    assert_eq!(s.settings_version, 4);
    assert_eq!(
        (s.hot_key, s.confine_hot_key, s.pause_hot_key, s.side_hot_key),
        (38, 71, 80, 39)
    );
    assert!(s.mod_ctrl && s.mod_alt && !s.mod_shift && !s.mod_win);
    assert!(s.start_gate_closed && s.start_side_contain_on && !s.start_confine_on && !s.start_paused);
    assert!(!s.auto_start && s.deliberate_cross && !s.descent_routing && s.safety_escapes);
    assert_eq!(s.mode, "AutoTop");
    assert_eq!((s.language.as_str(), s.theme.as_str()), ("auto", "System"));
    assert_eq!(s.hotkey().modifiers(), 0x3);
    assert_eq!(s.hot_key_text(), "Ctrl+Alt+Up");
    assert_eq!(s.side_hot_key_text(), "Ctrl+Alt+Right");
    assert!(s.raw.version_specified);
    assert!(!Settings::unspecified().raw.version_specified);
}

#[test]
fn s_config_paths_never_point_v2_at_the_v1_file() {
    let p = Paths::under_appdata(&std::env::temp_dir().join("MouseFence-test-appdata"));
    assert_ne!(p.v1_file, p.v2_file);
    assert!(p.v2_file.ends_with("MouseFence/v2/settings.json"));
    assert!(p.v1_file.ends_with("MouseFence/settings.json"));
    assert!(p.last_good.ends_with("MouseFence/v2/settings.last-good.json"));
}

// ------------------------------------------------------------------------------------------------
// 1.1.1 suite rows (tests/Tests.cs)
// ------------------------------------------------------------------------------------------------

#[test]
fn p158_v3_default_becomes_automatic_with_no_explicit_rules() {
    let m = normalized("{\"SettingsVersion\":3,\"CrossingPathMode\":\"Default\",\"UpLinks\":[]}");
    assert_eq!(m.settings_version, 4);
    assert_eq!(m.upward_boundary_default, Stored::Known(BoundaryDefault::Automatic));
    assert!(m.boundary_rules.is_empty());
    assert_eq!(m.edge_holdback_px, 0);
}

#[test]
fn p159_v3_custom_becomes_blocked_default_plus_overall_feel_allowance() {
    let m = normalized(
        "{\"SettingsVersion\":3,\"CrossingPathMode\":\"Custom\",\"UpLinks\":[{\"FromDevice\":\"bottom\",\"ToDevice\":\"top\"}]}",
    );
    assert_eq!(m.upward_boundary_default, Stored::Known(BoundaryDefault::Blocked));
    assert_eq!(m.boundary_rules.len(), 1);
    let r = &m.boundary_rules[0];
    assert_eq!((r.from_stable_id.as_str(), r.to_stable_id.as_str()), ("bottom", "top"));
    assert_eq!(r.direction, Stored::Known(Direction::Up));
    assert_eq!(r.behavior, Stored::Known(RuleBehavior::OverallFeel));
    assert_eq!(r.control, Stored::Known(Control::Automatic));
}

#[test]
fn p160_empty_v3_manual_falls_back_to_autotop_while_empty_custom_stays_none() {
    let m = normalized(
        "{\"SettingsVersion\":3,\"Mode\":\"Manual\",\"ManualMonitors\":[],\"CrossingPathMode\":\"Custom\",\"UpLinks\":[]}",
    );
    assert_eq!(m.mode, "AutoTop");
    assert_eq!(m.upward_boundary_default, Stored::Known(BoundaryDefault::Blocked));
    assert!(m.boundary_rules.is_empty());
}

#[test]
fn p161_rules_round_trip_and_legacy_members_stop_emitting() {
    let mut exported = Settings {
        upward_boundary_default: Stored::Known(BoundaryDefault::Blocked),
        ..Settings::default()
    };
    exported.boundary_rules.push(StoredRule {
        from_stable_id: "bottom".into(),
        to_stable_id: "top".into(),
        direction: Stored::Known(Direction::Up),
        behavior: Stored::Known(RuleBehavior::OverallFeel),
        control: Stored::Known(Control::Automatic),
    });
    let json = to_json_compact(&exported);
    let imported = normalized(&json);
    assert_eq!(imported.boundary_rules.len(), 1);
    assert_eq!(
        imported.boundary_rules[0].behavior,
        Stored::Known(RuleBehavior::OverallFeel)
    );
    assert!(
        !json.contains("CrossingPathMode") && !json.contains("UpLinks"),
        "{json}"
    );
}

#[test]
fn p162_monitor_names_persist_through_export_and_import_including_an_absent_id() {
    let mut exported = Settings::default();
    exported
        .monitor_names
        .push(("edid-example".into(), "Example Left".into()));
    exported
        .monitor_names
        .push(("edid-absent-dummy".into(), "Dummy".into()));
    let export_json = to_json_compact(&exported);
    let imported = normalized(&export_json);
    let re_export = to_json_compact(&imported);
    let round_trip = normalized(&re_export);
    assert_eq!(round_trip.monitor_names.len(), 2, "{re_export}");
    assert_eq!(round_trip.monitor_name("edid-example"), "Example Left");
    assert_eq!(round_trip.monitor_name("edid-absent-dummy"), "Dummy");
}

#[test]
fn p163_absent_and_null_monitor_names_are_safe_empty_maps() {
    assert!(normalized("{}").monitor_names.is_empty());
    assert!(normalized("{\"MonitorNames\":null}").monitor_names.is_empty());
}

#[test]
fn p164_absent_stable_id_stays_inert_and_never_retargets_a_live_monitor() {
    let mut s = Settings::default();
    s.monitor_names.push(("stable-unplugged".into(), "Old Dummy".into()));
    normalize(&mut s);
    assert_eq!(monitor_name_for(&s.monitor_names, "stable-live"), "");
    assert_eq!(monitor_name_for(&s.monitor_names, "stable-unplugged"), "Old Dummy");
    assert_eq!(s.monitor_names.len(), 1);
}

#[test]
fn p218_field_absent_legacy_json_migrates_as_v0_and_keeps_custom_up_links() {
    let m =
        normalized("{\"CrossingPathMode\":\"Custom\",\"UpLinks\":[{\"FromDevice\":\"bottom\",\"ToDevice\":\"top\"}]}");
    assert_eq!(m.settings_version, CURRENT_SETTINGS_VERSION);
    assert_eq!(m.upward_boundary_default, Stored::Known(BoundaryDefault::Blocked));
    assert_eq!(m.boundary_rules.len(), 1);
    let r = &m.boundary_rules[0];
    assert_eq!((r.from_stable_id.as_str(), r.to_stable_id.as_str()), ("bottom", "top"));
    assert_eq!(r.direction, Stored::Known(Direction::Up));
    assert_eq!(r.behavior, Stored::Known(RuleBehavior::OverallFeel));
}

#[test]
fn p220_unknown_enum_values_stay_a_repairable_draft() {
    let draft = normalized(
        "{\"SettingsVersion\":4,\"UpwardBoundaryDefault\":\"Surprise\",\"BoundaryRules\":[{\"FromStableId\":\"a\",\"ToStableId\":\"b\",\"Direction\":\"Up\",\"Behavior\":\"Mystery\",\"Control\":\"Automatic\"}]}",
    );
    assert!(draft.has_repairable_import_errors);
    assert_eq!(draft.upward_boundary_default, Stored::Unknown);
    assert_eq!(draft.boundary_rules.len(), 1);
    assert_eq!(draft.boundary_rules[0].behavior, Stored::Unknown);
    // The draft round-trips its unknown values as "Unknown" and is never saved.
    let json = to_json(&draft);
    assert!(json.contains("\"UpwardBoundaryDefault\": \"Unknown\""), "{json}");
}

// ------------------------------------------------------------------------------------------------
// Support checks (normalize details the reference diff also covers)
// ------------------------------------------------------------------------------------------------

#[test]
fn s_config_normalize_dedupes_sorts_and_clamps_like_csharp() {
    let s = normalized(concat!(
        "{\"SettingsVersion\":4,\"UpCrossMin\":0,\"UpCrossSlack\":999,\"PassageInsetPx\":-4,\"SeamTolerancePx\":201,",
        "\"Mode\":\"manual\",\"MonitorNames\":{\" b \":\"  Two\\tLines\\n\",\"B\":\"dup\",\"a\":\"\",\"A\":\"One\"},",
        "\"BoundaryRules\":[",
        "{\"FromStableId\":\" z \",\"ToStableId\":\"y\",\"Direction\":\"Down\",\"Behavior\":\"Push\",\"Control\":\"Always\"},",
        "{\"FromStableId\":\"a\",\"ToStableId\":\"A\",\"Direction\":\"Up\",\"Behavior\":\"Free\",\"Control\":\"Always\"},",
        "{\"FromStableId\":\"Z\",\"ToStableId\":\"Y\",\"Direction\":\"down\",\"Behavior\":\"Blocked\",\"Control\":\"Automatic\"},",
        "{\"FromStableId\":\"m\",\"ToStableId\":\"n\",\"Direction\":\"Up\",\"Behavior\":\"Free\",\"Control\":\"Always\"}]}"
    ));
    assert_eq!(
        (
            s.up_cross_min,
            s.up_cross_slack,
            s.passage_inset_px,
            s.seam_tolerance_px
        ),
        (1, 200, 0, 200)
    );
    assert_eq!(s.mode, "AutoTop", "Mode is case-sensitive");
    // " b " sorts first (space), so its trimmed key "b" wins over "B"; "a" is blank and dropped, "A" stays.
    assert_eq!(
        s.monitor_names,
        vec![
            ("b".to_string(), "Two Lines".to_string()),
            ("A".to_string(), "One".to_string())
        ]
    );
    let ids: Vec<(&str, &str)> = s
        .boundary_rules
        .iter()
        .map(|r| (r.from_stable_id.as_str(), r.to_stable_id.as_str()))
        .collect();
    // The later "Z"->"Y" Down replaces " z "->"y" in place; sorted ordinal; the self rule is invalid and last.
    assert_eq!(ids, vec![("Z", "Y"), ("m", "n"), ("a", "A")]);
    assert_eq!(s.boundary_rules[0].behavior, Stored::Known(RuleBehavior::Blocked));
    assert!(s.has_repairable_import_errors);
    let p = to_policy(&s);
    assert_eq!(p.rules.len(), 2, "the invalid rule never reaches the engine");
}

#[test]
fn s_config_to_policy_resolves_unknown_defaults_as_the_compiler_does() {
    let s = normalized("{\"SettingsVersion\":4,\"SideBoundaryDefault\":\"Mystery\",\"ExcludedDevices\":[null,\"x\"]}");
    let p = to_policy(&s);
    assert_eq!(p.side_default, BoundaryDefault::Blocked);
    assert_eq!(p.excluded_devices, vec!["x".to_string()]);
    assert_eq!(
        to_json(&s).matches("null").count(),
        1,
        "the null element is written back"
    );
}

#[test]
fn s_config_layout_key_migration_converts_drops_and_refuses_like_csharp() {
    let live = fence_engine::layout::identify_monitors(vec![
        mon("\\\\.\\DISPLAY1", "edid-main", 0, 0, 100, 100, true),
        mon("\\\\.\\DISPLAY2", "edid-top", 0, -100, 100, 0, false),
    ]);
    let text = concat!(
        "{\"SettingsVersion\":3,\"Mode\":\"Manual\",\"ManualMonitors\":[\"\\\\\\\\.\\\\DISPLAY2\",\"\\\\\\\\.\\\\DISPLAY7\",\"kept\"],",
        "\"CrossingPathMode\":\"Custom\",\"UpLinks\":[{\"FromDevice\":\"\\\\\\\\.\\\\display1\",\"ToDevice\":\"\\\\\\\\.\\\\DISPLAY2\"},",
        "{\"FromDevice\":\"\\\\\\\\.\\\\DISPLAY9\",\"ToDevice\":\"\\\\\\\\.\\\\DISPLAY2\"}]}"
    );
    let parsed = parse_and_migrate(text, &live).unwrap();
    assert_eq!(parsed.migration, Migration::Migrated);
    let s = &parsed.settings;
    assert_eq!(s.manual_monitors, vec!["edid-top".to_string(), "kept".to_string()]);
    let rules: Vec<(&str, &str)> = s
        .boundary_rules
        .iter()
        .map(|r| (r.from_stable_id.as_str(), r.to_stable_id.as_str()))
        .collect();
    // Present display names became stable ids; the stale DISPLAY9 left an empty, repairable id.
    assert_eq!(rules, vec![("edid-main", "edid-top"), ("", "edid-top")]);
    assert!(s.has_repairable_import_errors);

    let clash = vec![
        mon("\\\\.\\DISPLAY1", "x", 0, 0, 10, 10, true),
        mon("\\\\.\\display1", "y", 10, 0, 20, 10, false),
    ];
    let failed = parse_and_migrate("{\"SettingsVersion\":2}", &clash).unwrap();
    assert!(matches!(
        failed.migration,
        Migration::Failed(MigrateError::DuplicateDevice(_))
    ));
    let null_link = parse_and_migrate("{\"UpLinks\":[null]}", &live).unwrap();
    assert_eq!(null_link.migration, Migration::Failed(MigrateError::NullLegacyLink));
}

#[test]
fn s_config_intel_shift_only_touches_exact_ctrl_alt_arrow() {
    let mut s = Settings::default();
    add_shift_to_arrow_hotkeys(&mut s);
    assert!(s.mod_shift && s.side_mod_shift && s.intel_defaults_applied);
    assert!(!s.confine_mod_shift && !s.pause_mod_shift);
    let mut again = s.clone();
    add_shift_to_arrow_hotkeys(&mut again);
    assert!(!again.intel_defaults_applied, "nothing left to change");
}

#[test]
fn s_config_reset_and_clone_keep_names_and_exclusions() {
    let mut s = normalized(
        "{\"SettingsVersion\":4,\"Mode\":\"Manual\",\"ManualMonitors\":[\"a\"],\"ExcludedDevices\":[\"b\"],\"MonitorNames\":{\"a\":\"Left\"},\"UpCrossMin\":9}",
    );
    let clone = deep_clone(&s);
    assert_eq!(to_json(&clone), to_json(&s));
    reset_layout_config(&mut s);
    assert_eq!(
        (s.mode.as_str(), s.manual_monitors.len(), s.up_cross_min),
        ("AutoTop", 0, 3)
    );
    assert_eq!(s.excluded_devices, vec!["b".to_string()]);
    assert_eq!(s.monitor_name("A"), "Left");
}

#[test]
fn s_config_decode_text_follows_read_all_text() {
    assert_eq!(decode_text(b"\xEF\xBB\xBF{}"), "{}");
    assert_eq!(decode_text(b"\xFF\xFE{\x00}\x00"), "{}");
    assert_eq!(decode_text(b"\xFE\xFF\x00{\x00}"), "{}");
    assert_eq!(decode_text(b"\xFF\xFE\x00\x00{\x00\x00\x00"), "{");
    assert_eq!(decode_text(b"{\xFF}"), "{\u{FFFD}}");
}

// ------------------------------------------------------------------------------------------------
// Files: B030, B031, B033, J04
// ------------------------------------------------------------------------------------------------

const V1_LEGACY: &str = concat!(
    "{\"SettingsVersion\":3,\"HotKey\":38,\"Mode\":\"Manual\",\"ManualMonitors\":[\"\\\\\\\\.\\\\DISPLAY2\"],",
    "\"CrossingPathMode\":\"Custom\",\"UpLinks\":[{\"FromDevice\":\"\\\\\\\\.\\\\DISPLAY1\",\"ToDevice\":\"\\\\\\\\.\\\\DISPLAY2\"}],",
    "\"MonitorNames\":{\"edid-top\":\"Example Top\"}}"
);

#[test]
fn steady_push_ms_defaults_to_the_slider_roundtrips_and_clamps() {
    assert_eq!(Settings::default().steady_push_ms, 0);
    assert_eq!(
        from_json_v2(r#"{"SettingsVersion":4}"#)
            .unwrap()
            .intent_config()
            .steady_push_ms,
        0
    );
    for (value, expected) in [(-7, -1), (-1, -1), (0, 0), (1, 30), (45, 45), (80, 80), (9999, 500)] {
        let mut s = from_json_v2(&format!(r#"{{"SettingsVersion":4,"SteadyPushMs":{value}}}"#)).unwrap();
        assert_eq!(s.intent_config().steady_push_ms, expected, "{value}");
        normalize(&mut s);
        assert_eq!(s.steady_push_ms, expected);
        assert_eq!(from_json_v2(&to_json_v2(&s)).unwrap().steady_push_ms, expected);
        assert!(!to_json(&s).contains("SteadyPushMs"), "legacy 1.x JSON stays unchanged");
    }
}

#[test]
fn intent_201_slider_roundtrips_clamps_and_production_defaults_preserve_existing_choices() {
    for (value, expected) in [(-10, 1), (1, 1), (3, 3), (5, 5), (99, 5)] {
        let mut s = from_json_v2(&format!(
            r#"{{"SettingsVersion":4,"CrossingModel":"Intent","CrossingEase":{value}}}"#
        ))
        .unwrap();
        normalize(&mut s);
        assert_eq!(s.crossing_ease, expected);
        assert_eq!(s.intent_config().model, fence_types::CrossingModel::Intent);
        assert_eq!(from_json_v2(&to_json_v2(&s)).unwrap().crossing_ease, expected);
        assert!(!to_json(&s).contains("CrossingEase"));
    }
    let home = TempHome::new("intent-production");
    let paths = home.paths();
    assert_eq!(
        load_production(&paths, &live_two(), false).settings.crossing_model,
        fence_types::CrossingModel::Intent
    );
    fs::create_dir_all(paths.v1_file.parent().unwrap()).unwrap();
    fs::write(&paths.v1_file, V1_LEGACY).unwrap();
    let bytes = fs::read(&paths.v1_file).unwrap();
    assert_eq!(
        load_production(&paths, &live_two(), false).settings.crossing_model,
        fence_types::CrossingModel::Intent
    );
    assert_eq!(fs::read(&paths.v1_file).unwrap(), bytes);
    assert_eq!(
        load_production(&paths, &live_two(), false).settings.crossing_model,
        fence_types::CrossingModel::Intent
    );
    let mut s = load_production(&paths, &live_two(), false).settings;
    s.crossing_model = fence_types::CrossingModel::Classic;
    assert_eq!(save(&mut s, &paths, &|_| vec![]), SaveOutcome::Saved);
    assert_eq!(
        load_production(&paths, &live_two(), false).settings.crossing_model,
        fence_types::CrossingModel::Classic
    );
}

fn live_two() -> Vec<MonitorInfo> {
    fence_engine::layout::identify_monitors(vec![
        mon("\\\\.\\DISPLAY1", "edid-main", 0, 0, 100, 100, true),
        mon("\\\\.\\DISPLAY2", "edid-top", 0, -100, 100, 0, false),
    ])
}

#[test]
fn b030_load_order_v2_then_v1_import_then_fresh() {
    // Fresh: nothing exists; Intel adds Shift; nothing is written.
    let home = TempHome::new("b030-fresh");
    let p = home.paths();
    let d = load_detailed(&p, &live_two(), true);
    assert_eq!(d.loaded.source, LoadSource::Fresh);
    assert!(d.loaded.settings.mod_shift && d.loaded.settings.intel_defaults_applied);
    assert!(d.save.is_none());
    assert!(!p.v2_file.exists() && !p.v1_file.exists());
    let plain = load(&p, &live_two(), false);
    assert!(!plain.settings.mod_shift && !plain.settings.intel_defaults_applied);

    // Import: the legacy 1.x file migrates its display names to stable ids and is saved to the v2 file.
    let home = TempHome::new("b030-import");
    let p = home.paths();
    write(&p.v1_file, V1_LEGACY.as_bytes());
    let d = load_detailed(&p, &live_two(), true);
    assert_eq!(d.loaded.source, LoadSource::ImportedV1);
    assert_eq!(d.migration, Migration::Migrated);
    assert_eq!(d.save, Some(SaveOutcome::Saved));
    let s = &d.loaded.settings;
    assert_eq!(s.settings_version, 4);
    assert_eq!(s.manual_monitors, vec!["edid-top".to_string()]);
    assert_eq!(s.boundary_rules.len(), 1);
    assert_eq!(s.boundary_rules[0].from_stable_id, "edid-main");
    assert!(!s.mod_shift, "an imported file is not a fresh install");
    let v2_text = fs::read_to_string(&p.v2_file).unwrap();
    assert_eq!(v2_text, to_json_v2(s));
    assert_eq!(fs::read(&p.last_good).unwrap(), v2_text.as_bytes());

    // V2: once the v2 file exists it wins, even over a changed 1.x file.
    write(&p.v1_file, b"{\"SettingsVersion\":4,\"HotKey\":71}");
    let again = load_detailed(&p, &live_two(), false);
    assert_eq!(again.loaded.source, LoadSource::V2);
    assert_eq!(again.loaded.settings.hot_key, 38);
    assert!(again.save.is_none(), "a clean v4 v2 file is not rewritten");

    // An older v2 file migrates and saves, as C# Load did.
    write(&p.v2_file, V1_LEGACY.as_bytes());
    let migrated = load_detailed(&p, &live_two(), false);
    assert_eq!(migrated.loaded.source, LoadSource::V2);
    assert_eq!(migrated.save, Some(SaveOutcome::Saved));
    assert!(
        fs::read_to_string(&p.v2_file)
            .unwrap()
            .contains("\"SettingsVersion\": 4")
    );

    // A migration that throws keeps the settings and saves nothing (retried next load).
    let home = TempHome::new("b030-throw");
    let p = home.paths();
    write(&p.v1_file, V1_LEGACY.as_bytes());
    let clash = vec![
        mon("\\\\.\\DISPLAY1", "x", 0, 0, 10, 10, true),
        mon("\\\\.\\display1", "y", 10, 0, 20, 10, false),
    ];
    let d = load_detailed(&p, &clash, false);
    assert_eq!(d.loaded.source, LoadSource::ImportedV1);
    assert!(matches!(d.migration, Migration::Failed(_)));
    assert!(d.save.is_none() && !p.v2_file.exists());

    // A corrupt v2 file: last-good is used and the bad bytes are kept.
    let home = TempHome::new("b030-lastgood");
    let p = home.paths();
    write(&p.last_good, b"{\"SettingsVersion\":4,\"PauseHotKey\":72}");
    write(&p.v2_file, b"{\"SettingsVersion\":4,");
    let d = load_detailed(&p, &live_two(), false);
    assert!(
        matches!(d.loaded.source, LoadSource::LastGood { .. }),
        "{:?}",
        d.loaded.source
    );
    assert_eq!(d.loaded.settings.pause_hot_key, 72);
    assert_eq!(fs::read(&p.v2_file).unwrap(), b"{\"SettingsVersion\":4,");
    assert_eq!(fs::read(d.preserved.unwrap()).unwrap(), b"{\"SettingsVersion\":4,");
}

#[test]
fn b031_save_normalizes_refuses_and_writes_atomically_with_last_good() {
    let home = TempHome::new("b031");
    let p = home.paths();

    // Normalize + stamp version, then write v2 and last-good with the same bytes; no temp file left.
    let mut s = Settings::unspecified();
    s.settings_version = 2;
    s.up_cross_min = 0;
    s.mode = "Other".into();
    let outcome = save(&mut s, &p, &no_seal);
    assert_eq!(outcome, SaveOutcome::Saved);
    assert_eq!((s.settings_version, s.up_cross_min, s.mode.as_str()), (4, 1, "AutoTop"));
    let written = fs::read_to_string(&p.v2_file).unwrap();
    assert_eq!(written, to_json_v2(&s));
    assert!(written.starts_with("{\r\n  \"SettingsVersion\": 4,"), "{written}");
    assert!(!written.starts_with('\u{FEFF}'));
    assert_eq!(fs::read_to_string(&p.last_good).unwrap(), written);
    assert_eq!(
        files_in(&p.v2_dir()),
        vec!["settings.json".to_string(), "settings.last-good.json".to_string()]
    );

    // Repairable import errors: refused, file untouched.
    let mut draft = normalized("{\"SettingsVersion\":4,\"UpwardBoundaryDefault\":\"Surprise\"}");
    assert_eq!(save(&mut draft, &p, &no_seal), SaveOutcome::RefusedRepairable);
    assert_eq!(fs::read_to_string(&p.v2_file).unwrap(), written);

    // A sealed layout: refused with the issues, file untouched.
    let issue = SealIssue {
        stable_id: "edid-top".into(),
        focus_to_stable_id: "edid-main".into(),
        focus_direction: Direction::Down,
        focus_corridor_width: 100,
    };
    let sealed = |_: &Settings| vec![issue.clone()];
    let mut ok = Settings {
        hot_key: 71,
        ..Settings::default()
    };
    assert_eq!(
        save(&mut ok, &p, &sealed),
        SaveOutcome::RefusedSealed(vec![issue.clone()])
    );
    assert_eq!(fs::read_to_string(&p.v2_file).unwrap(), written);

    // The seal check sees the normalized settings.
    let seen = std::cell::Cell::new(0);
    let probe = |x: &Settings| {
        seen.set(x.up_cross_min);
        Vec::new()
    };
    let mut low = Settings {
        up_cross_min: -7,
        ..Settings::default()
    };
    assert_eq!(save(&mut low, &p, &probe), SaveOutcome::Saved);
    assert_eq!(seen.get(), 1);

    // A target that is the 1.x file: refused, 1.x file untouched.
    write(&p.v1_file, b"{\"SettingsVersion\":4}");
    let bad = Paths {
        v1_file: p.v1_file.clone(),
        v2_file: p.v1_file.parent().unwrap().join(".").join("settings.json"),
        last_good: p.last_good.clone(),
    };
    let outcome = save(&mut Settings::default(), &bad, &no_seal);
    assert!(matches!(outcome, SaveOutcome::Failed(_)), "{outcome:?}");
    assert_eq!(fs::read(&p.v1_file).unwrap(), b"{\"SettingsVersion\":4}");
}

#[test]
fn b033_the_1x_file_is_never_written() {
    for (tag, content) in [
        ("legacy", V1_LEGACY.as_bytes().to_vec()),
        ("v4", b"{\"SettingsVersion\":4,\"HotKey\":71}".to_vec()),
        (
            "repairable",
            b"{\"SettingsVersion\":4,\"SideBoundaryDefault\":\"Mystery\"}".to_vec(),
        ),
        ("corrupt", b"{\"SettingsVersion\":".to_vec()),
        ("utf16", b"\xFF\xFE{\x00}\x00".to_vec()),
    ] {
        let home = TempHome::new(&format!("b033-{tag}"));
        let p = home.paths();
        write(&p.v1_file, &content);
        let stamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1_600_000_000);
        fs::File::options()
            .write(true)
            .open(&p.v1_file)
            .unwrap()
            .set_modified(stamp)
            .unwrap();
        let before = fs::metadata(&p.v1_file).unwrap().modified().unwrap();
        assert_eq!(before, stamp);

        let live = live_two();
        let first = load_detailed(&p, &live, true);
        let mut s = first.loaded.settings.clone();
        s.hot_key = 80;
        let _ = save(&mut s, &p, &|x: &Settings| seal_issues(x, &live));
        let _ = load_detailed(&p, &live, false);
        let mut again = Settings::default();
        let _ = save(&mut again, &p, &no_seal);
        let _ = load_detailed(&p, &live, false);

        assert_eq!(fs::read(&p.v1_file).unwrap(), content, "{tag}: bytes");
        assert_eq!(
            fs::metadata(&p.v1_file).unwrap().modified().unwrap(),
            stamp,
            "{tag}: modified time"
        );
        assert_eq!(
            files_in(p.v1_file.parent().unwrap()),
            vec!["settings.json".to_string(), "v2".to_string()]
        );
    }
}

/// One entry of [`snapshot`]: a path, and for a file its bytes and modified time (`None` for a folder).
type Entry = (PathBuf, Option<(Vec<u8>, SystemTime)>);

/// The files a read-only case starts with: (tag, 1.x bytes, v2 bytes, last-good bytes).
type Case = (&'static str, Option<Vec<u8>>, Option<Vec<u8>>, Option<Vec<u8>>);

/// Every file and folder under `root`, with the bytes and modified time of each file.
fn snapshot(root: &Path) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()) {
            let path = entry.path();
            let meta = entry.metadata().unwrap();
            if meta.is_dir() {
                stack.push(path.clone());
                out.push((path, None));
            } else {
                out.push((path.clone(), Some((fs::read(&path).unwrap(), meta.modified().unwrap()))));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

// JUDGE-FINAL M1: `load_read_only` (the --screenshot dev mode) picks what `load` picks and writes nothing: no v2
// file appears, and the 1.x file keeps its bytes and its modified time.
#[test]
fn s_config_load_read_only_picks_what_load_picks_and_writes_nothing() {
    let stamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1_600_000_000);
    let v4 = b"{\"SettingsVersion\":4,\"HotKey\":71}".to_vec();
    let cases: Vec<Case> = vec![
        ("nothing", None, None, None),
        ("v1-legacy", Some(V1_LEGACY.as_bytes().to_vec()), None, None),
        ("v1-v4", Some(v4.clone()), None, None),
        (
            "v1-repairable",
            Some(b"{\"SettingsVersion\":4,\"SideBoundaryDefault\":\"Mystery\"}".to_vec()),
            None,
            None,
        ),
        ("v1-corrupt", Some(b"{\"SettingsVersion\":".to_vec()), None, None),
        ("v1-utf16", Some(b"\xFF\xFE{\x00}\x00".to_vec()), None, None),
        ("v2-legacy", Some(v4.clone()), Some(V1_LEGACY.as_bytes().to_vec()), None),
        (
            "v2-clean",
            Some(v4.clone()),
            Some(b"{\"SettingsVersion\":4,\"HotKey\":80}".to_vec()),
            None,
        ),
        (
            "v2-corrupt-lastgood",
            Some(v4.clone()),
            Some(b"{\"SettingsVersion\":4,".to_vec()),
            Some(b"{\"SettingsVersion\":4,\"PauseHotKey\":72}".to_vec()),
        ),
        (
            "v2-corrupt-alone",
            Some(v4.clone()),
            Some(b"{\"SettingsVersion\":4,".to_vec()),
            None,
        ),
    ];
    let live = live_two();
    for (tag, v1, v2, last_good) in cases {
        for intel in [false, true] {
            let place = |home: &TempHome| {
                let p = home.paths();
                for (path, bytes) in [(&p.v1_file, &v1), (&p.v2_file, &v2), (&p.last_good, &last_good)] {
                    if let Some(bytes) = bytes {
                        write(path, bytes);
                        fs::File::options()
                            .write(true)
                            .open(path)
                            .unwrap()
                            .set_modified(stamp)
                            .unwrap();
                    }
                }
                p
            };
            let ro_home = TempHome::new(&format!("ro-{tag}"));
            let p = place(&ro_home);
            let before = snapshot(&ro_home.root);
            let got = load_read_only(&p, &live, intel);
            assert_eq!(snapshot(&ro_home.root), before, "{tag}/{intel}: something was written");
            if v2.is_none() {
                assert!(!p.v2_dir().exists(), "{tag}/{intel}: a v2 folder appeared");
            }
            if let Some(bytes) = &v1 {
                assert_eq!(&fs::read(&p.v1_file).unwrap(), bytes, "{tag}/{intel}: 1.x bytes");
                assert_eq!(
                    fs::metadata(&p.v1_file).unwrap().modified().unwrap(),
                    stamp,
                    "{tag}/{intel}: 1.x modified time"
                );
            }

            // The writing load on an identical copy picks the same settings from the same source.
            let rw_home = TempHome::new(&format!("rw-{tag}"));
            let q = place(&rw_home);
            let want = load(&q, &live, intel);
            assert_eq!(got.settings, want.settings, "{tag}/{intel}: settings");
            assert_eq!(
                std::mem::discriminant(&got.source),
                std::mem::discriminant(&want.source),
                "{tag}/{intel}: {:?} vs {:?}",
                got.source,
                want.source
            );
        }
    }
}

#[test]
fn j04_an_unreadable_file_never_becomes_saved_defaults() {
    // Corrupt v2, no last-good: defaults in memory, marked not saved; the bad bytes stay and are copied aside.
    let home = TempHome::new("j04");
    let p = home.paths();
    let bad: &[u8] = b"{\"SettingsVersion\":4,\"HotKey\":";
    write(&p.v2_file, bad);
    let d = load_detailed(&p, &live_two(), false);
    assert!(
        matches!(d.loaded.source, LoadSource::DefaultsNotSaved { .. }),
        "{:?}",
        d.loaded.source
    );
    assert!(d.save.is_none());
    assert_eq!(
        fs::read(&p.v2_file).unwrap(),
        bad,
        "load never writes defaults over the file"
    );
    let copy = d.preserved.clone().unwrap();
    assert_eq!(copy, p.unreadable_copy(1));
    assert_eq!(fs::read(&copy).unwrap(), bad);
    assert!(!p.last_good.exists());

    // A second load reuses the identical copy instead of piling up new ones.
    let d2 = load_detailed(&p, &live_two(), false);
    assert_eq!(d2.preserved, Some(copy.clone()));
    assert_eq!(
        files_in(&p.v2_dir()),
        vec!["settings.json".to_string(), "settings.unreadable.json".to_string()]
    );

    // A different corrupt file gets its own copy.
    write(&p.v2_file, b"not json");
    let d3 = load_detailed(&p, &live_two(), false);
    assert_eq!(d3.preserved, Some(p.unreadable_copy(2)));

    // Saving later (an explicit user action) keeps the current bad bytes first, then writes.
    write(&p.v2_file, b"still not json");
    let mut s = Settings::default();
    let detail = save_detailed(&mut s, &p, &no_seal);
    assert_eq!(detail.outcome, SaveOutcome::Saved);
    assert_eq!(fs::read(detail.preserved.unwrap()).unwrap(), b"still not json");
    assert_eq!(fs::read(&copy).unwrap(), bad, "earlier copies are never overwritten");

    // A corrupt 1.x file (no v2): defaults not saved, nothing written anywhere.
    let home = TempHome::new("j04-v1");
    let p = home.paths();
    write(&p.v1_file, b"\x00\x01garbage");
    let d = load_detailed(&p, &live_two(), false);
    assert!(matches!(d.loaded.source, LoadSource::DefaultsNotSaved { .. }));
    assert!(!p.v2_dir().exists());
}

#[cfg(windows)]
#[test]
fn j04_a_v2_file_that_cannot_be_read_is_never_replaced() {
    use std::os::windows::fs::OpenOptionsExt;
    let home = TempHome::new("j04-locked");
    let p = home.paths();
    write(&p.v2_file, b"{\"SettingsVersion\":4,\"HotKey\":72}");
    write(&p.last_good, b"{\"SettingsVersion\":4,\"HotKey\":71}");
    // Exclusive handle: every other open fails with a sharing violation while it lives.
    let lock = fs::File::options().read(true).share_mode(0).open(&p.v2_file).unwrap();
    let d = load_detailed(&p, &live_two(), false);
    assert!(
        matches!(d.loaded.source, LoadSource::LastGood { .. }),
        "{:?}",
        d.loaded.source
    );
    assert_eq!(d.loaded.settings.hot_key, 71);
    let mut s = d.loaded.settings.clone();
    let outcome = save(&mut s, &p, &no_seal);
    assert!(matches!(outcome, SaveOutcome::Failed(_)), "{outcome:?}");
    drop(lock);
    assert_eq!(fs::read(&p.v2_file).unwrap(), b"{\"SettingsVersion\":4,\"HotKey\":72}");
}

// ------------------------------------------------------------------------------------------------
// B032: pinned reference answers (tools/reference-cs `normalize` verb, 1.1.1 code, 2026-10-13)
// ------------------------------------------------------------------------------------------------

#[test]
fn b032_settings_written_by_1x_normalize_and_write_like_1x() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("../tests/fixtures/b032_normalize.json")).expect("fixture JSON");
    let cases = cases.as_array().expect("an array of cases");
    assert!(cases.len() >= 20, "zero or too few cases: {}", cases.len());
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let input = case["json"].as_str().unwrap();
        let want = &case["expect"];
        assert_eq!(
            read_version(input) as i64,
            want["loaded_version"].as_i64().unwrap(),
            "{name}: version"
        );
        let parsed = from_json(input);
        if want.get("parse_error").is_some() {
            assert!(parsed.is_err(), "{name}: 1.1.1 rejects it ({})", want["parse_error"]);
            continue;
        }
        let mut s = parsed.unwrap_or_else(|e| panic!("{name}: {e}"));
        normalize(&mut s);
        normalize(&mut s);
        s.set_settings_version(CURRENT_SETTINGS_VERSION);
        assert_eq!(to_json(&s), want["json"].as_str().unwrap(), "{name}: json");
        assert_eq!(
            s.has_repairable_import_errors,
            want["repairable"].as_bool().unwrap(),
            "{name}: repairable"
        );
        let texts = &want["texts"];
        assert_eq!(s.hot_key_text(), texts["hot"].as_str().unwrap(), "{name}: hot");
        assert_eq!(
            s.confine_hot_key_text(),
            texts["confine"].as_str().unwrap(),
            "{name}: confine"
        );
        assert_eq!(
            s.pause_hot_key_text(),
            texts["pause"].as_str().unwrap(),
            "{name}: pause"
        );
        assert_eq!(s.side_hot_key_text(), texts["side"].as_str().unwrap(), "{name}: side");
        let mods: Vec<u64> = [s.hotkey(), s.confine_hotkey(), s.pause_hotkey(), s.side_hotkey()]
            .iter()
            .map(|h| h.modifiers() as u64)
            .collect();
        let want_mods: Vec<u64> = want["modifiers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        assert_eq!(mods, want_mods, "{name}: modifiers");
    }
}

#[test]
fn f07_v2_unknown_members_survive_save_after_known_fields_but_legacy_import_ignores_them() {
    let home = TempHome::new("f07");
    let p = home.paths();
    let text = r#"{"SettingsVersion":4,"UpCrossSpeed":3000,"CrossingMode":"Instant","UpPressureWindowMs":123,"FutureObject":{"x":true},"FutureArray":[1,"ç",null]}"#;
    write(&p.v2_file, text.as_bytes());
    let mut loaded = load(&p, &[], false);
    assert_eq!(
        loaded
            .settings
            .raw
            .unknown_members
            .iter()
            .map(|(k, _)| k.as_str())
            .collect::<Vec<_>>(),
        [
            "UpCrossSpeed",
            "CrossingMode",
            "UpPressureWindowMs",
            "FutureObject",
            "FutureArray"
        ]
    );
    assert_eq!(save(&mut loaded.settings, &p, &no_seal), SaveOutcome::Saved);
    let written = fs::read_to_string(&p.v2_file).unwrap();
    let before: serde_json::Value = serde_json::from_str(text).unwrap();
    let after: serde_json::Value = serde_json::from_str(&written).unwrap();
    for (key, value) in &loaded.settings.raw.unknown_members {
        assert_eq!(&after[key], value);
        assert_eq!(after[key], before[key]);
    }
    assert!(written.find("\"Theme\"").unwrap() < written.find("\"UpCrossSpeed\"").unwrap());
    let legacy = from_json(text).unwrap();
    assert!(legacy.raw.unknown_members.is_empty());
    assert!(!to_json(&legacy).contains("UpCrossSpeed"));
    let import_home = TempHome::new("f07-import");
    let import_paths = import_home.paths();
    write(&import_paths.v1_file, text.as_bytes());
    let imported = load(&import_paths, &[], false);
    assert_eq!(imported.source, LoadSource::ImportedV1);
    assert!(imported.settings.raw.unknown_members.is_empty());
    assert!(
        !fs::read_to_string(&import_paths.v2_file)
            .unwrap()
            .contains("UpCrossSpeed")
    );
    let duplicate = from_json_v2(r#"{"SettingsVersion":4,"Future":1,"Future":2}"#).unwrap();
    assert_eq!(
        duplicate.raw.unknown_members,
        [
            ("Future".into(), serde_json::json!(1)),
            ("Future".into(), serde_json::json!(2))
        ]
    );
    let written = to_json_v2(&duplicate);
    assert_eq!(
        from_json_v2(&written).unwrap().raw.unknown_members,
        duplicate.raw.unknown_members
    );
    let observed: serde_json::Value = serde_json::from_str(&written).unwrap();
    assert_eq!(
        observed["Future"], 2,
        "duplicate unknown names must preserve last-wins semantics"
    );
}

#[test]
fn f06_future_and_null_settings_are_not_loaded_or_destructively_downgraded() {
    for text in ["null", "{broken", r#"{"SettingsVersion":999,"Theme":"Dark"}"#] {
        let home = TempHome::new("f06-invalid");
        let p = home.paths();
        write(&p.v2_file, text.as_bytes());
        let mut loaded = load(&p, &[], false);
        assert!(matches!(loaded.source, LoadSource::DefaultsNotSaved { .. }));
        assert!(matches!(
            save(&mut loaded.settings, &p, &no_seal),
            SaveOutcome::Failed(_)
        ));
        assert_eq!(fs::read(&p.v2_file).unwrap(), text.as_bytes());
    }
    let home = TempHome::new("f06-save-future");
    let p = home.paths();
    write(&p.v2_file, b"previous bytes");
    let mut s = Settings::default();
    s.set_settings_version(999);
    assert!(matches!(save(&mut s, &p, &no_seal), SaveOutcome::Failed(_)));
    assert_eq!(s.settings_version, 999);
    assert_eq!(fs::read(&p.v2_file).unwrap(), b"previous bytes");
}

#[test]
fn f11_v2_intent_settings_round_trip_normalize_and_map_without_changing_legacy_json() {
    use fence_types::CrossingModel;
    let text = r#"{"SettingsVersion":4,"CrossingModel":"sToPtHeNpUsH","UpPushDistancePx":-1,"SidePushDistancePx":9999,"PushAbsorbMs":-7,"PushPauseResetMs":9999,"Future":1,"Future":2}"#;
    let mut s = from_json_v2(text).unwrap();
    normalize(&mut s);
    assert_eq!(s.crossing_model, CrossingModel::StopThenPush);
    assert_eq!(
        (
            s.up_push_distance_px,
            s.side_push_distance_px,
            s.push_absorb_ms,
            s.push_pause_reset_ms
        ),
        (1, 2000, 0, 2000)
    );
    assert_eq!(to_policy(&s).intent, s.intent_config());
    s.raw
        .unknown_members
        .push(("CrossingModel".into(), serde_json::json!("Classic")));
    let written = to_json_v2(&s);
    let restored = from_json_v2(&written).unwrap();
    assert_eq!(restored.intent_config(), s.intent_config());
    assert_eq!(restored.raw.unknown_members, s.raw.unknown_members[..2]);
    assert_eq!(
        written.matches("\"CrossingModel\"").count(),
        1,
        "a raw extension cannot shadow the recognized model"
    );
    assert!(
        !restored
            .raw
            .unknown_members
            .iter()
            .any(|(name, _)| name == "CrossingModel")
    );
    let legacy = from_json(text).unwrap();
    assert_eq!(legacy.crossing_model, CrossingModel::Classic1k);
    assert_eq!(legacy.up_push_distance_px, 120);
    assert!(!to_json(&s).contains("CrossingModel"));
    assert!(!to_json_compact(&s).contains("UpPushDistancePx"));
    assert_eq!(deep_clone(&s).crossing_model, CrossingModel::Classic1k);
    for (name, model) in [
        ("Classic", CrossingModel::Classic),
        ("cLaSsIc1K", CrossingModel::Classic1k),
        ("unknown", CrossingModel::Classic1k),
    ] {
        assert_eq!(
            from_json_v2(&format!(r#"{{"SettingsVersion":4,"CrossingModel":"{name}"}}"#))
                .unwrap()
                .crossing_model,
            model
        );
    }
    let mut defaults = Settings {
        up_push_distance_px: 5000,
        side_push_distance_px: -3,
        push_absorb_ms: 900,
        push_pause_reset_ms: -1,
        ..Settings::default()
    };
    normalize(&mut defaults);
    assert_eq!(
        (
            defaults.up_push_distance_px,
            defaults.side_push_distance_px,
            defaults.push_absorb_ms,
            defaults.push_pause_reset_ms
        ),
        (2000, 1, 500, 20)
    );
}

#[test]
fn f13_trace_defaults_off_and_mark_accident_round_trips_with_all_chord_conflicts_checked() {
    let defaults = Settings::default();
    assert!(!defaults.edge_trace);
    assert_eq!(hotkey_text(&defaults.mark_accident_hotkey()), "Ctrl+Alt+M");
    let mut s = from_json_v2(r#"{"SettingsVersion":4,"EdgeTrace":true,"MarkAccidentHotKey":135,"MarkAccidentModCtrl":true,"MarkAccidentModAlt":false,"MarkAccidentModShift":true,"MarkAccidentModWin":true}"#).unwrap();
    s.raw
        .unknown_members
        .push(("EdgeTrace".into(), serde_json::json!(false)));
    let written = to_json_v2(&s);
    let restored = from_json_v2(&written).unwrap();
    assert!(restored.edge_trace);
    assert_eq!(written.matches("\"EdgeTrace\"").count(), 1);
    assert_eq!(hotkey_text(&restored.mark_accident_hotkey()), "Ctrl+Shift+Win+F24");
    assert!(!to_json(&s).contains("EdgeTrace"));
    assert!(!from_json(&written).unwrap().edge_trace);
    for (first, h) in [s.hotkey(), s.confine_hotkey(), s.pause_hotkey(), s.side_hotkey()]
        .into_iter()
        .enumerate()
    {
        s.mark_accident_hot_key = h.vk;
        s.mark_accident_mod_ctrl = h.ctrl;
        s.mark_accident_mod_alt = h.alt;
        s.mark_accident_mod_shift = h.shift;
        s.mark_accident_mod_win = h.win;
        assert_eq!(shortcut_issue(&s), Some(ShortcutIssue::Duplicate { first, later: 4 }));
    }
    s.mark_accident_hot_key = 0x87;
    s.mark_accident_mod_ctrl = false;
    s.mark_accident_mod_alt = false;
    s.mark_accident_mod_shift = false;
    s.mark_accident_mod_win = false;
    assert_eq!(shortcut_issue(&s), Some(ShortcutIssue::Incomplete { index: 4 }));
    s.mark_accident_hot_key = 0;
    assert_eq!(shortcut_issue(&s), None);
}

#[test]
fn f03_ephemeral_display_names_exclusions_manual_routes_and_rules_are_never_saved() {
    for kind in 0..5 {
        let home = TempHome::new("f03-ownership");
        let p = home.paths();
        let previous = b"{\"SettingsVersion\":4,\"Theme\":\"Dark\"}";
        write(&p.v2_file, previous);
        let mut s = Settings::default();
        let id = "display#ambiguous:session".to_string();
        match kind {
            0 => s.monitor_names.push((id, "name".into())),
            1 => s.excluded_devices.push(id),
            2 => s.manual_monitors.push(id),
            3 => s.boundary_rules.push(StoredRule {
                from_stable_id: id,
                to_stable_id: "other".into(),
                direction: Stored::Known(Direction::Up),
                behavior: Stored::Known(RuleBehavior::Free),
                control: Stored::Known(Control::Always),
            }),
            _ => s.legacy_up_links.push(UpLink {
                from_device: "other".into(),
                to_device: id,
            }),
        }
        assert!(has_ambiguous_display_choices(&s));
        assert!(matches!(save(&mut s, &p, &no_seal), SaveOutcome::Failed(_)));
        assert_eq!(fs::read(&p.v2_file).unwrap(), previous);
    }
}

#[test]
fn f06_interrupted_atomic_write_preserves_previous_bytes_and_cleans_temp_siblings() {
    use std::io::Write;
    let home = TempHome::new("f06-interrupted");
    let target = home.root.join("backup.json");
    fs::write(&target, b"previous bytes").unwrap();
    let result = crate::write_atomic_with(&target, b"{\"Theme\":\"Light\"}", |file, _| {
        file.write_all(b"{")?;
        Err(std::io::Error::other("injected interrupted write"))
    });
    assert!(result.is_err());
    assert_eq!(fs::read(&target).unwrap(), b"previous bytes");
    assert_eq!(files_in(&home.root), ["backup.json"]);
    crate::write_atomic(&target, b"{\"Theme\":\"Light\"}").unwrap();
    let restored: serde_json::Value = serde_json::from_slice(&fs::read(&target).unwrap()).unwrap();
    assert_eq!(restored["Theme"], "Light");
    assert_eq!(files_in(&home.root), ["backup.json"]);
}

// JUDGE-B S2 (join 2026-10-13): a temp name that already exists (a stale temp of an earlier process with the same
// pid) is skipped and kept; the save still succeeds.
#[test]
fn s_config_write_atomic_skips_a_stale_temp_name_and_never_deletes_it() {
    let home = TempHome::new("stale-temp");
    let target = home.root.join("settings.json");
    let next = crate::store::TEMP_COUNTER.load(std::sync::atomic::Ordering::Relaxed);
    let stale: Vec<PathBuf> = (next..next + 4)
        .map(|n| home.root.join(format!("settings.json.{}.{n}.tmp", std::process::id())))
        .collect();
    for p in &stale {
        fs::write(p, b"stale").expect("stale temp");
    }
    crate::store::write_atomic(&target, b"{\"ok\":1}").expect("the save picks a free temp name");
    assert_eq!(fs::read(&target).unwrap(), b"{\"ok\":1}");
    for p in &stale {
        assert_eq!(fs::read(p).unwrap(), b"stale", "stale temp {} kept", p.display());
    }
}
