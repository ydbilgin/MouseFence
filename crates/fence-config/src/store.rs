//! The settings files (PLAN T15; parity rows B030, B031, B033, J04).
//!
//! Load order: the v2 file; else the 1.x file, imported read-only (normalized, migrated when older than v4,
//! then saved to the v2 file); else a fresh install (Intel: Shift added to the arrow hotkeys, as C# `Load`).
//! A v2 file that cannot be parsed is copied to `settings.unreadable.json` (or `-2`, `-3`, ...; an identical
//! copy is reused) and `settings.last-good.json` is used; when that fails too, defaults are returned marked
//! not saved. Loading never writes defaults anywhere.
//! `load_read_only` (the `--screenshot` dev mode) picks the same settings and writes nothing at all.
//!
//! Save: Normalize, stamp version 4, refuse repairable import errors, refuse a sealed layout (the check is
//! injected), keep an unparseable v2 file's bytes first, then write the v2 file and the last-good copy
//! atomically (temp file in the same folder, flush to disk, rename over). UTF-8 without a byte order mark,
//! as C# `File.WriteAllText`. Nothing in this module opens the 1.x file for writing; a save whose target
//! resolves to the 1.x file fails instead.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use fence_types::{MonitorInfo, SealIssue};

use crate::normalize::{self, MigrateError};
use crate::{CURRENT_SETTINGS_VERSION, Settings, json};

/// Where 2.0 keeps its files. Built from `%APPDATA%` by the caller (fence-win reads the folder).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paths {
    /// `%APPDATA%\MouseFence\settings.json`: the 1.x file. Read once for import; never written.
    pub v1_file: PathBuf,
    /// `%APPDATA%\MouseFence\v2\settings.json`.
    pub v2_file: PathBuf,
    /// `%APPDATA%\MouseFence\v2\settings.last-good.json`.
    pub last_good: PathBuf,
}

impl Paths {
    pub fn under_appdata(appdata: &Path) -> Self {
        let root = appdata.join("MouseFence");
        let v2 = root.join("v2");
        Self {
            v1_file: root.join("settings.json"),
            v2_file: v2.join("settings.json"),
            last_good: v2.join("settings.last-good.json"),
        }
    }

    /// The folder unreadable copies go to (the v2 file's folder).
    pub fn v2_dir(&self) -> PathBuf {
        self.v2_file.parent().map(Path::to_path_buf).unwrap_or_default()
    }

    /// `settings.unreadable.json` for `n == 1`, else `settings.unreadable-<n>.json`, beside the v2 file.
    pub fn unreadable_copy(&self, n: u32) -> PathBuf {
        let name = if n <= 1 {
            "settings.unreadable.json".to_string()
        } else {
            format!("settings.unreadable-{n}.json")
        };
        self.v2_dir().join(name)
    }
}

/// Where the loaded settings came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadSource {
    /// The v2 file loaded cleanly.
    V2,
    /// No v2 file yet; the 1.x file was imported read-only (and migrated if older than v4).
    ImportedV1,
    /// Neither file exists: a fresh install (Intel arrow-hotkey adjustment applied when `intel_gpu`).
    Fresh,
    /// The v2 file was unreadable or corrupt; `settings.last-good.json` was used. Bad bytes untouched.
    LastGood { problem: String },
    /// Nothing usable: in-memory defaults, marked not saved; the tray says so. Bad bytes untouched.
    DefaultsNotSaved { problem: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loaded {
    pub settings: Settings,
    pub source: LoadSource,
}

/// The layout-key migration C# `Load` runs for a file older than schema v4.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Migration {
    /// The file was schema v4 or newer.
    NotNeeded,
    /// Display names became stable ids and the version is 4 (C# then saves).
    Migrated,
    /// C# `MigrateLayoutKeysToStable` threw: the settings stay as normalized and nothing is saved, so the
    /// migration is retried on the next load.
    Failed(MigrateError),
}

/// A settings text after C# `Load`'s in-memory steps: ReadVersion, Deserialize, Normalize, migration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parsed {
    pub settings: Settings,
    /// C# `ReadVersion(json)` (0 for a missing or non-Int32 member).
    pub loaded_version: i32,
    pub migration: Migration,
}

/// The in-memory half of C# `Settings.Load()` for one file's text. `Err` is the reason the text is
/// unusable (the C# load would have fallen back to defaults).
pub fn parse_and_migrate(text: &str, live_monitors: &[MonitorInfo]) -> Result<Parsed, String> {
    parse_and_migrate_inner(text, live_monitors, false)
}

fn parse_and_migrate_v2(text: &str, live_monitors: &[MonitorInfo]) -> Result<Parsed, String> {
    parse_and_migrate_inner(text, live_monitors, true)
}

fn parse_and_migrate_inner(text: &str, live_monitors: &[MonitorInfo], production: bool) -> Result<Parsed, String> {
    let loaded_version = json::read_version(text);
    let mut settings = if production {
        crate::from_json_v2(text)?
    } else {
        json::read_settings(text)?.unwrap_or_else(Settings::unspecified)
    };
    normalize::normalize(&mut settings);
    let migration = if loaded_version < CURRENT_SETTINGS_VERSION {
        match normalize::migrate_layout_keys_to_stable(&mut settings, live_monitors) {
            Ok(()) => {
                settings.set_settings_version(CURRENT_SETTINGS_VERSION);
                Migration::Migrated
            }
            Err(e) => Migration::Failed(e),
        }
    } else {
        Migration::NotNeeded
    };
    Ok(Parsed {
        settings,
        loaded_version,
        migration,
    })
}

/// C# `File.ReadAllText`: a byte order mark selects UTF-8, UTF-16 LE/BE or UTF-32 LE/BE; anything else is
/// UTF-8. Invalid sequences become U+FFFD.
pub fn decode_text(bytes: &[u8]) -> String {
    fn utf16(rest: &[u8], le: bool) -> String {
        let units: Vec<u16> = rest
            .chunks(2)
            .map(|c| match (c, le) {
                ([a, b], true) => u16::from_le_bytes([*a, *b]),
                ([a, b], false) => u16::from_be_bytes([*a, *b]),
                _ => 0xFFFD,
            })
            .collect();
        String::from_utf16_lossy(&units)
    }
    fn utf32(rest: &[u8], le: bool) -> String {
        rest.chunks(4)
            .map(|c| match c {
                [a, b, c2, d] => {
                    let v = if le {
                        u32::from_le_bytes([*a, *b, *c2, *d])
                    } else {
                        u32::from_be_bytes([*a, *b, *c2, *d])
                    };
                    char::from_u32(v).unwrap_or('\u{FFFD}')
                }
                _ => '\u{FFFD}',
            })
            .collect()
    }
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(rest).into_owned()
    } else if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE, 0x00, 0x00]) {
        utf32(rest, true)
    } else if let Some(rest) = bytes.strip_prefix(&[0x00, 0x00, 0xFE, 0xFF]) {
        utf32(rest, false)
    } else if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        utf16(rest, true)
    } else if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        utf16(rest, false)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// Everything `load` did, for the tray and the tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadDetail {
    pub loaded: Loaded,
    /// The migration of the file that was used (`NotNeeded` for fresh defaults).
    pub migration: Migration,
    /// The save after an import or a migration; `None` when load wrote nothing.
    pub save: Option<SaveOutcome>,
    /// Where the bytes of an unparseable v2 file were kept.
    pub preserved: Option<PathBuf>,
}

/// Load at startup. `live_monitors` feed the v0-v3 key migration (C# `MigrateLayoutKeysToStable(MonitorInfo.All())`).
pub fn load(paths: &Paths, live_monitors: &[MonitorInfo], intel_gpu: bool) -> Loaded {
    load_detailed(paths, live_monitors, intel_gpu).loaded
}

/// Application defaults for 2.0.1. Exact/reference constructors retain their legacy default.
/// Existing v2 choices are preserved; fresh/first imported settings start with Intent.
pub fn load_production(paths: &Paths, monitors: &[MonitorInfo], intel_gpu: bool) -> Loaded {
    let mut detail = load_detailed(paths, monitors, intel_gpu);
    if matches!(detail.loaded.source, LoadSource::Fresh | LoadSource::ImportedV1)
        && !matches!(detail.migration, Migration::Failed(_))
        && !detail.loaded.settings.raw.write_protected
    {
        detail.loaded.settings.crossing_model = fence_types::CrossingModel::Intent;
        if detail.save == Some(SaveOutcome::Saved) {
            let seal = |s: &Settings| crate::seal_issues(s, monitors);
            let _ = save(&mut detail.loaded.settings, paths, &seal);
        }
    }
    detail.loaded
}

enum Probe {
    Missing,
    Present,
    Problem(String),
}

fn probe(path: &Path) -> Probe {
    match fs::metadata(path) {
        Ok(m) if m.is_file() => Probe::Present,
        Ok(_) => Probe::Problem(format!("{} is not a file", path.display())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Probe::Missing,
        Err(e) => Probe::Problem(format!("cannot inspect {}: {e}", path.display())),
    }
}

/// C# fresh settings: `new Settings { SettingsVersion = 4 }` plus `AddShiftToArrowHotkeys` on an Intel GPU.
fn fresh(intel_gpu: bool) -> Settings {
    let mut s = Settings::default();
    if intel_gpu {
        normalize::add_shift_to_arrow_hotkeys(&mut s);
    }
    s
}

fn needs_save(p: &Parsed) -> bool {
    !matches!(p.migration, Migration::Failed(_))
}

/// [`load`] with the details (migration, the save it caused, the kept copy of unreadable bytes).
pub fn load_detailed(paths: &Paths, live_monitors: &[MonitorInfo], intel_gpu: bool) -> LoadDetail {
    load_inner(paths, live_monitors, intel_gpu, true)
}

/// The same settings [`load`] would pick, but nothing is ever written: no import save, no migration save, no
/// copy of unreadable bytes, no folder created. The 1.x file (or the v2 file) is parsed and migrated in memory
/// only. For the `--screenshot` dev mode, so an agent QA run never makes 2.0's one-time import of the 1.x file
/// early (JUDGE-FINAL M1; PLAN T15). The tray and the settings window keep using [`load`].
pub fn load_read_only(paths: &Paths, live_monitors: &[MonitorInfo], intel_gpu: bool) -> Loaded {
    load_inner(paths, live_monitors, intel_gpu, false).loaded
}

fn load_inner(paths: &Paths, live_monitors: &[MonitorInfo], intel_gpu: bool, may_write: bool) -> LoadDetail {
    let seal = |s: &Settings| crate::seal_issues(s, live_monitors);
    let mut detail = LoadDetail {
        loaded: Loaded {
            settings: Settings::default(),
            source: LoadSource::Fresh,
        },
        migration: Migration::NotNeeded,
        save: None,
        preserved: None,
    };

    let problem = match probe(&paths.v2_file) {
        Probe::Present => match fs::read(&paths.v2_file) {
            Err(e) => format!("cannot read {}: {e}", paths.v2_file.display()),
            Ok(bytes) => match parse_and_migrate_v2(&decode_text(&bytes), live_monitors) {
                Ok(mut parsed) => {
                    if may_write && parsed.migration == Migration::Migrated {
                        detail.save = Some(save(&mut parsed.settings, paths, &seal));
                    }
                    detail.migration = parsed.migration;
                    detail.loaded = Loaded {
                        settings: parsed.settings,
                        source: LoadSource::V2,
                    };
                    return detail;
                }
                Err(e) if !may_write => {
                    format!("{} is not valid settings JSON: {e}", paths.v2_file.display())
                }
                Err(e) => {
                    match preserve_unreadable(paths, &bytes) {
                        Ok(copy) => detail.preserved = Some(copy),
                        Err(pe) => {
                            return defaults_not_saved(
                                detail,
                                intel_gpu,
                                format!(
                                    "{} is not valid settings JSON ({e}) and its bytes could not be copied aside ({pe})",
                                    paths.v2_file.display()
                                ),
                            );
                        }
                    }
                    format!("{} is not valid settings JSON: {e}", paths.v2_file.display())
                }
            },
        },
        Probe::Problem(p) => p,
        Probe::Missing => return import_or_fresh(paths, live_monitors, intel_gpu, may_write, detail),
    };

    // The v2 file exists but is unusable: last-good, else defaults that are never written by load.
    if let Probe::Present = probe(&paths.last_good)
        && let Ok(bytes) = fs::read(&paths.last_good)
        && let Ok(parsed) = parse_and_migrate_v2(&decode_text(&bytes), live_monitors)
    {
        detail.migration = parsed.migration;
        detail.loaded = Loaded {
            settings: parsed.settings,
            source: LoadSource::LastGood { problem },
        };
        return detail;
    }
    defaults_not_saved(detail, intel_gpu, problem)
}

fn defaults_not_saved(mut detail: LoadDetail, intel_gpu: bool, problem: String) -> LoadDetail {
    detail.loaded = Loaded {
        settings: fresh(intel_gpu),
        source: LoadSource::DefaultsNotSaved { problem },
    };
    detail.migration = Migration::NotNeeded;
    detail.loaded.settings.raw.write_protected = true;
    detail
}

fn import_or_fresh(
    paths: &Paths,
    live_monitors: &[MonitorInfo],
    intel_gpu: bool,
    may_write: bool,
    mut detail: LoadDetail,
) -> LoadDetail {
    let seal = |s: &Settings| crate::seal_issues(s, live_monitors);
    match probe(&paths.v1_file) {
        Probe::Missing => {
            detail.loaded = Loaded {
                settings: fresh(intel_gpu),
                source: LoadSource::Fresh,
            };
            detail
        }
        Probe::Problem(p) => defaults_not_saved(detail, intel_gpu, p),
        // Read-only: fs::read opens the 1.x file for reading and nothing else.
        Probe::Present => match fs::read(&paths.v1_file) {
            Err(e) => defaults_not_saved(
                detail,
                intel_gpu,
                format!("cannot read the 1.x settings {}: {e}", paths.v1_file.display()),
            ),
            Ok(bytes) => match parse_and_migrate_v2(&decode_text(&bytes), live_monitors) {
                Err(e) => defaults_not_saved(
                    detail,
                    intel_gpu,
                    format!(
                        "the 1.x settings {} are not valid settings JSON: {e}",
                        paths.v1_file.display()
                    ),
                ),
                Ok(mut parsed) => {
                    // 1.x extension settings are intentionally ignored on the one-time legacy import.
                    parsed.settings.raw.unknown_members.clear();
                    if may_write && needs_save(&parsed) {
                        detail.save = Some(save(&mut parsed.settings, paths, &seal));
                    }
                    detail.migration = parsed.migration;
                    detail.loaded = Loaded {
                        settings: parsed.settings,
                        source: LoadSource::ImportedV1,
                    };
                    detail
                }
            },
        },
    }
}

/// Copies unusable v2 bytes beside the v2 file (an existing identical copy is reused). Never overwrites.
fn preserve_unreadable(paths: &Paths, bytes: &[u8]) -> io::Result<PathBuf> {
    fs::create_dir_all(paths.v2_dir())?;
    for n in 1..=999 {
        let copy = paths.unreadable_copy(n);
        match fs::read(&copy) {
            Ok(existing) if existing == bytes => return Ok(copy),
            Ok(_) => continue,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(_) => continue,
        }
        match OpenOptions::new().write(true).create_new(true).open(&copy) {
            Ok(mut f) => {
                f.write_all(bytes)?;
                f.sync_all()?;
                return Ok(copy);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::other("999 unreadable copies already exist"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveOutcome {
    Saved,
    /// C# returned silently when `HasRepairableImportErrors`.
    RefusedRepairable,
    /// C# returned silently when `ValidateNoSeal` found a sealed screen.
    RefusedSealed(Vec<SealIssue>),
    Failed(String),
}

/// Everything `save` did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveDetail {
    pub outcome: SaveOutcome,
    /// The last-good copy failed after the v2 file was saved (the save itself still counts).
    pub last_good_error: Option<String>,
    /// Where the bytes of an unparseable v2 file were kept before it was replaced.
    pub preserved: Option<PathBuf>,
}

/// Atomic save to the v2 file. `validate_no_seal` is injected by the caller (usually
/// `&|s| fence_config::seal_issues(s, &live_monitors)`).
pub fn save(
    settings: &mut Settings,
    paths: &Paths,
    validate_no_seal: &dyn Fn(&Settings) -> Vec<SealIssue>,
) -> SaveOutcome {
    save_detailed(settings, paths, validate_no_seal).outcome
}

/// [`save`] with the details.
pub fn save_detailed(
    settings: &mut Settings,
    paths: &Paths,
    validate_no_seal: &dyn Fn(&Settings) -> Vec<SealIssue>,
) -> SaveDetail {
    save_detailed_with_fixes(settings, paths, validate_no_seal, fence_engine::fixes::Fixes::ALL)
}

pub fn save_detailed_with_fixes(
    settings: &mut Settings,
    paths: &Paths,
    validate_no_seal: &dyn Fn(&Settings) -> Vec<SealIssue>,
    fixes: fence_engine::fixes::Fixes,
) -> SaveDetail {
    let mut detail = SaveDetail {
        outcome: SaveOutcome::Saved,
        last_good_error: None,
        preserved: None,
    };
    if fixes.j18_settings_durability && settings.raw.write_protected {
        detail.outcome = SaveOutcome::Failed("settings could not be loaded; restore a valid file before saving".into());
        return detail;
    }
    if fixes.j18_settings_durability && settings.settings_version > CURRENT_SETTINGS_VERSION {
        detail.outcome = SaveOutcome::Failed("the settings require a newer MouseFence version".into());
        return detail;
    }
    if fixes.j11_segment_and_identity && crate::has_ambiguous_display_choices(settings) {
        detail.outcome = SaveOutcome::Failed("ambiguous display identities cannot own saved choices".into());
        return detail;
    }
    normalize::normalize(settings);
    settings.set_settings_version(CURRENT_SETTINGS_VERSION);
    if settings.has_repairable_import_errors {
        detail.outcome = SaveOutcome::RefusedRepairable;
        return detail;
    }
    let issues = validate_no_seal(settings);
    if !issues.is_empty() {
        detail.outcome = SaveOutcome::RefusedSealed(issues);
        return detail;
    }
    if let Err(e) = write_files(settings, paths, &mut detail, fixes) {
        detail.outcome = SaveOutcome::Failed(e);
    }
    detail
}

fn write_files(
    settings: &Settings,
    paths: &Paths,
    detail: &mut SaveDetail,
    fixes: fence_engine::fixes::Fixes,
) -> Result<(), String> {
    for target in [&paths.v2_file, &paths.last_good] {
        if same_file(target, &paths.v1_file) {
            return Err(format!(
                "refusing to write {}: it is the 1.x settings file",
                target.display()
            ));
        }
    }
    let dir = paths.v2_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;

    // An existing v2 file that does not parse is kept before it is replaced (J04); one that cannot even be
    // read is not replaced at all.
    match probe(&paths.v2_file) {
        Probe::Missing => {}
        Probe::Problem(p) => return Err(p),
        Probe::Present => {
            let bytes = fs::read(&paths.v2_file)
                .map_err(|e| format!("cannot read the current {} to keep it: {e}", paths.v2_file.display()))?;
            if fixes.j18_settings_durability && json::read_version(&decode_text(&bytes)) > CURRENT_SETTINGS_VERSION {
                return Err("refusing to replace settings from a newer MouseFence version".into());
            }
            if json::read_settings(&decode_text(&bytes)).is_err() {
                let copy = preserve_unreadable(paths, &bytes)
                    .map_err(|e| format!("cannot keep the unreadable {}: {e}", paths.v2_file.display()))?;
                detail.preserved = Some(copy);
            }
        }
    }

    let text = crate::to_json_v2_with_fixes(settings, fixes);
    write_atomic(&paths.v2_file, text.as_bytes())
        .map_err(|e| format!("cannot write {}: {e}", paths.v2_file.display()))?;
    if let Err(e) = write_atomic(&paths.last_good, text.as_bytes()) {
        detail.last_good_error = Some(format!("cannot write {}: {e}", paths.last_good.display()));
    }
    Ok(())
}

pub(crate) static TEMP_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Temp sibling, write, flush to disk, rename over the target (std's rename replaces an existing file).
pub fn write_atomic(target: &Path, bytes: &[u8]) -> io::Result<()> {
    write_atomic_with(target, bytes, |file, bytes| file.write_all(bytes))
}

/// Atomic replacement with an injected file writer, for deterministic interrupted-write verification.
/// The writer receives only the newly created sibling, never the destination file.
pub fn write_atomic_with(
    target: &Path,
    bytes: &[u8],
    writer: impl FnOnce(&mut fs::File, &[u8]) -> io::Result<()>,
) -> io::Result<()> {
    let dir = target
        .parent()
        .ok_or_else(|| io::Error::other("the target has no folder"))?;
    let name = target
        .file_name()
        .ok_or_else(|| io::Error::other("the target has no file name"))?
        .to_string_lossy()
        .into_owned();
    // A name already taken (a stale temp of an earlier process with the same pid) is never touched: the next
    // counter is tried instead (JUDGE-B S2: the old code deleted it and then failed the save).
    let mut attempts = 0;
    let (temp, mut f) = loop {
        let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp = dir.join(format!("{name}.{}.{n}.tmp", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(f) => break (temp, f),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && attempts < 16 => attempts += 1,
            Err(e) => return Err(e),
        }
    };
    let result = (|| {
        writer(&mut f, bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&temp, target)
    })();
    if result.is_err() {
        // Only the temp file this call created (create_new succeeded above) is removed.
        let _ = fs::remove_file(&temp);
    }
    result
}

/// The full path a target resolves to (symbolic links followed for the part that exists), compared
/// case-insensitively as Windows paths are.
fn resolved(p: &Path) -> String {
    let full = fs::canonicalize(p).ok().or_else(|| {
        let parent = p.parent()?;
        Some(fs::canonicalize(parent).ok()?.join(p.file_name()?))
    });
    full.unwrap_or_else(|| p.to_path_buf())
        .to_string_lossy()
        .replace('/', "\\")
        .to_lowercase()
}

fn same_file(a: &Path, b: &Path) -> bool {
    a == b || resolved(a) == resolved(b)
}
