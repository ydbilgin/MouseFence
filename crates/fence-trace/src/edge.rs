//! Opt-in edge traces. All storage and replay run off the hook thread.
//!
//! Records contain attempted coordinates, not a complete physical input stream. Replay is a
//! counterfactual comparison of the captured context; it cannot measure physical feel or recover
//! reports lost before capture. The first record seeds the origin of each replay.

use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use fence_config::Settings;
use fence_engine::decide::ClassicEngine;
use fence_types::{CrossingModel, Decider, EdgeRecord, MonitorInfo, MoveEvent, Rect, RuntimeState, Toggles};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MEMORY_US: u64 = 30_000_000;
const PRE_US: u64 = 1_500_000;
const POST_US: u64 = 500_000;
const ACCIDENT_US: u64 = 3_000_000;
const MAX_FILES: usize = 300;
const MAX_BYTES: u64 = 20 * 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);

// Reconstruct from an allowlist, never serialize RawState's unknown members or UI/user text.
const POLICY_KEYS: &[&str] = &[
    "CrossingEase",
    "SettingsVersion",
    "CrossingModel",
    "UpPushDistancePx",
    "SidePushDistancePx",
    "PushAbsorbMs",
    "PushPauseResetMs",
    "SteadyPushMs",
    "DeliberateCross",
    "DescentRouting",
    "UpwardBoundaryDefault",
    "SideBoundaryDefault",
    "DownwardBoundaryDefault",
    "BoundaryRules",
    "UpCrossMin",
    "UpCrossSlack",
    "SideCrossMin",
    "SideCrossSlack",
    "PassageInsetPx",
    "EdgeHoldbackPx",
    "SeamTolerancePx",
    "SafetyEscapes",
    "Mode",
    "ManualMonitors",
    "ExcludedDevices",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraceMonitor {
    pub stable_id: String,
    pub rect: [i32; 4],
    pub primary: bool,
    pub index: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraceHeader {
    pub schema: u32,
    pub layout: Vec<TraceMonitor>,
    /// FNV-1a of canonical, normalized, allowlisted policy JSON; not a security hash.
    pub settings_fingerprint: String,
    pub settings: Value,
    pub estimated_rate: f64,
    pub gate_open: bool,
    pub side_contain: bool,
    pub confine: bool,
    pub paused: bool,
}

impl TraceHeader {
    pub fn from_settings(settings: &Settings, monitors: &[MonitorInfo], estimated_rate: f64) -> Self {
        let mut clean = settings.clone();
        clean.raw.unknown_members.clear();
        fence_config::normalize(&mut clean);
        let mut value: Value =
            serde_json::from_str(&fence_config::to_json_v2(&clean)).expect("the settings serializer produces JSON");
        value
            .as_object_mut()
            .expect("settings object")
            .retain(|key, _| POLICY_KEYS.contains(&key.as_str()));
        let bytes = serde_json::to_vec(&value).expect("JSON value");
        let hash = bytes.iter().fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
        });
        Self {
            schema: 1,
            layout: monitors
                .iter()
                .map(|m| TraceMonitor {
                    stable_id: m.stable_id.clone(),
                    rect: [m.bounds.l, m.bounds.t, m.bounds.r, m.bounds.b],
                    primary: m.primary,
                    index: m.index,
                })
                .collect(),
            settings_fingerprint: format!("fnv1a64:{hash:016x}"),
            settings: value,
            estimated_rate: if estimated_rate.is_finite() {
                estimated_rate.max(0.0)
            } else {
                0.0
            },
            gate_open: !clean.start_gate_closed,
            side_contain: clean.start_side_contain_on,
            confine: clean.start_confine_on,
            paused: clean.start_paused,
        }
    }

    pub fn set_runtime(&mut self, state: RuntimeState) {
        self.gate_open = state.gate_open;
        self.side_contain = state.side_contain;
        self.confine = state.confine;
        self.paused = state.paused;
    }

    fn monitors(&self) -> Vec<MonitorInfo> {
        self.layout
            .iter()
            .map(|m| MonitorInfo {
                stable_id: m.stable_id.clone(),
                device: String::new(),
                bounds: Rect::new(m.rect[0], m.rect[1], m.rect[2], m.rect[3]),
                primary: m.primary,
                index: m.index,
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Record {
    #[serde(default)]
    features: [i32; 12],
    sequence: u64,
    qpc_us: u64,
    time_ms: u32,
    x: i32,
    y: i32,
    action: u8,
    model_state: u8,
}

impl From<EdgeRecord> for Record {
    fn from(r: EdgeRecord) -> Self {
        Self {
            sequence: 0,
            features: r.features,
            qpc_us: r.qpc_us,
            time_ms: r.time_ms,
            x: r.x,
            y: r.y,
            action: r.action,
            model_state: r.model_state,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct FileHeader {
    #[serde(flatten)]
    trace: TraceHeader,
    session: String,
    kind: String,
    captured_us: u64,
    focus_crossing_us: Option<u64>,
    focus_sequence: Option<u64>,
    accident: bool,
}

#[derive(Clone)]
struct Capture {
    header: FileHeader,
    records: Vec<Record>,
    path: Option<PathBuf>,
}

/// Only create this while EdgeTrace is enabled. Construction and header updates do no I/O.
pub struct TraceStore {
    folder: PathBuf,
    header: TraceHeader,
    session: String,
    window: VecDeque<Record>,
    pending: Vec<Capture>,
    latest: Option<Capture>,
    marks: usize,
    last_ingested: Option<u64>,
    next_sequence: u64,
    last_hold: Option<u64>,
}

impl TraceStore {
    pub fn new(folder: PathBuf, header: TraceHeader) -> Self {
        Self {
            folder,
            header,
            session: stamp(),
            window: VecDeque::new(),
            pending: Vec::new(),
            latest: None,
            marks: 0,
            last_ingested: None,
            next_sequence: 0,
            last_hold: None,
        }
    }

    /// Existing pending captures retain their original layout/settings snapshot.
    pub fn update_header(&mut self, header: TraceHeader) {
        self.header = header;
    }

    pub fn ingest(&mut self, records: &[EdgeRecord], now_us: u64) -> io::Result<()> {
        if records.windows(2).any(|r| r[0].qpc_us > r[1].qpc_us)
            || records
                .first()
                .is_some_and(|r| self.last_ingested.is_some_and(|last| r.qpc_us < last))
            || records.last().is_some_and(|r| r.qpc_us > now_us)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "edge timestamps must be monotonic and no later than drain time",
            ));
        }
        for native in records {
            let mut record = Record::from(*native);
            record.sequence = self.next_sequence;
            self.next_sequence = self.next_sequence.saturating_add(1);
            for capture in &mut self.pending {
                if record.qpc_us <= capture.header.captured_us.saturating_add(POST_US) {
                    capture.records.push(record);
                }
            }
            self.window.push_back(record);
            if record.action == 1
                && record.features[7] != 0
                && self
                    .last_hold
                    .is_none_or(|last| record.qpc_us.saturating_sub(last) >= 500_000)
            {
                self.last_hold = Some(record.qpc_us);
                self.pending.push(Capture {
                    header: FileHeader {
                        trace: self.header.clone(),
                        session: self.session.clone(),
                        kind: "held".into(),
                        captured_us: record.qpc_us,
                        focus_crossing_us: None,
                        focus_sequence: None,
                        accident: false,
                    },
                    records: self
                        .window
                        .iter()
                        .filter(|r| r.qpc_us >= record.qpc_us.saturating_sub(PRE_US))
                        .copied()
                        .collect(),
                    path: None,
                });
            }
            if record.action == 2 {
                let start = record.qpc_us.saturating_sub(PRE_US);
                let capture = Capture {
                    header: FileHeader {
                        trace: self.header.clone(),
                        session: self.session.clone(),
                        kind: "cross".into(),
                        captured_us: record.qpc_us,
                        focus_crossing_us: Some(record.qpc_us),
                        focus_sequence: Some(record.sequence),
                        accident: false,
                    },
                    records: self.window.iter().filter(|r| r.qpc_us >= start).copied().collect(),
                    path: None,
                };
                self.latest = Some(capture.clone());
                self.pending.push(capture);
            }
            self.last_ingested = Some(record.qpc_us);
        }
        let cutoff = now_us.saturating_sub(MEMORY_US);
        while self.window.front().is_some_and(|r| r.qpc_us < cutoff) {
            self.window.pop_front();
        }
        let mut index = 0;
        while index < self.pending.len() {
            if now_us >= self.pending[index].header.captured_us.saturating_add(POST_US) {
                let mut capture = self.pending[index].clone();
                let path = self.save(&capture)?;
                capture.path = Some(path);
                if self
                    .latest
                    .as_ref()
                    .is_some_and(|c| c.header.focus_sequence == capture.header.focus_sequence)
                {
                    capture.records.clear();
                    self.latest = Some(capture);
                }
                self.pending.remove(index);
            } else {
                index += 1;
            }
        }
        Ok(())
    }

    pub fn mark_accident(&mut self, now_us: u64) -> io::Result<usize> {
        let start = now_us.saturating_sub(ACCIDENT_US);
        let focus = self
            .latest
            .as_ref()
            .filter(|c| c.header.captured_us >= start && c.header.captured_us <= now_us)
            .map(|c| (c.header.captured_us, c.header.focus_sequence));
        let capture = Capture {
            header: FileHeader {
                trace: self.header.clone(),
                session: self.session.clone(),
                kind: "accident".into(),
                captured_us: now_us,
                focus_crossing_us: focus.map(|v| v.0),
                focus_sequence: focus.and_then(|v| v.1),
                accident: true,
            },
            records: self
                .window
                .iter()
                .filter(|r| r.qpc_us >= start && r.qpc_us <= now_us)
                .copied()
                .collect(),
            path: None,
        };
        self.save(&capture)?;
        if let Some(latest) = &mut self.latest {
            latest.header.accident = true;
            if let Some(path) = &latest.path {
                // Retention may already have removed the latest file. Never recreate it here.
                if path.try_exists()? {
                    checked_file(&self.folder, path)?;
                    let mut written = decode(path)?;
                    written.header.accident = true;
                    fence_config::write_atomic(path, &encode(&written)?)?;
                }
            }
            for pending in &mut self.pending {
                if pending.header.focus_sequence == latest.header.focus_sequence {
                    pending.header.accident = true;
                }
            }
        }
        self.marks = self.marks.saturating_add(1);
        retain_owned(&self.folder, MAX_FILES, MAX_BYTES)?;
        Ok(self.marks)
    }

    fn save(&self, capture: &Capture) -> io::Result<PathBuf> {
        self.save_with(capture, |file, bytes| file.write_all(bytes), fs::File::sync_all)
    }

    fn save_with(
        &self,
        capture: &Capture,
        writer: impl FnOnce(&mut fs::File, &[u8]) -> io::Result<()>,
        sync: impl FnOnce(&fs::File) -> io::Result<()>,
    ) -> io::Result<PathBuf> {
        let folder = checked_folder(&self.folder, true)?;
        let path = folder.join(format!("{}-{}.jsonl", capture.header.kind, stamp()));
        let bytes = encode(capture)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(io::Error::other("one trace exceeds the storage budget"));
        }
        create_capture_atomic(&path, &bytes, writer, sync)?;
        retain_owned(&folder, MAX_FILES, MAX_BYTES)?;
        Ok(path)
    }
}

/// Write a private sibling and atomically add the final name without replacing any existing entry.
/// Hard-link publication is create-if-absent on both Windows and Unix; rename would overwrite a
/// concurrent target. Filesystems without hard links return an error and keep the capture pending.
fn create_capture_atomic(
    path: &Path,
    bytes: &[u8],
    writer: impl FnOnce(&mut fs::File, &[u8]) -> io::Result<()>,
    sync: impl FnOnce(&fs::File) -> io::Result<()>,
) -> io::Result<()> {
    let folder = checked_folder(
        path.parent().ok_or_else(|| io::Error::other("trace has no folder"))?,
        false,
    )?;
    if !path.is_absolute() || !owned_name(path) || path.parent() != Some(folder.as_path()) {
        return Err(io::Error::other(
            "new capture must be an owned absolute child of the trace folder",
        ));
    }
    let temp = folder.join(format!(".pending-{}.tmp", stamp()));
    // Only this successful create_new grants permission to clean up this exact temp sibling.
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temp)?;
    let written = writer(&mut file, bytes).and_then(|()| sync(&file));
    drop(file);
    let result = written.and_then(|()| {
        checked_temp_file(&folder, &temp)?;
        fs::hard_link(&temp, path)
    });
    let cleanup = checked_temp_file(&folder, &temp).and_then(|()| fs::remove_file(&temp));
    // Preserve the original write/sync/publish error. A cleanup error after publication is reported.
    result.and(cleanup)
}

fn encode(capture: &Capture) -> io::Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(&capture.header)?;
    bytes.push(b'\n');
    for record in &capture.records {
        serde_json::to_writer(&mut bytes, record)?;
        bytes.push(b'\n');
    }
    Ok(bytes)
}

fn stamp() -> String {
    let micros = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();
    format!(
        "{micros:020}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

fn owned_name(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let Some(stem) = name.strip_suffix(".jsonl") else {
        return false;
    };
    let Some(stem) = stem
        .strip_prefix("cross-")
        .or_else(|| stem.strip_prefix("accident-"))
        .or_else(|| stem.strip_prefix("held-"))
    else {
        return false;
    };
    let parts: Vec<_> = stem.split('-').collect();
    parts.len() == 3
        && parts[0].len() == 20
        && parts
            .iter()
            .all(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn checked_folder(folder: &Path, create: bool) -> io::Result<PathBuf> {
    if !folder.is_absolute()
        || folder.components().any(|c| {
            matches!(c, std::path::Component::ParentDir) || matches!(c, std::path::Component::Normal(s) if s == "..")
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "trace folder must be an absolute path without parent traversal",
        ));
    }
    let mut part = PathBuf::new();
    for component in folder.components() {
        part.push(component);
        // A Windows drive/UNC prefix alone is not an inspectable directory (notably \\?\C:).
        if matches!(component, std::path::Component::Prefix(_)) {
            continue;
        }
        match fs::symlink_metadata(&part) {
            Ok(m) if is_link(&m) => {
                return Err(io::Error::other("trace folder cannot traverse a link or reparse point"));
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound && create => fs::create_dir(&part)?,
            Err(e) => return Err(e),
        }
    }
    fs::canonicalize(folder)
}

fn checked_file(folder: &Path, path: &Path) -> io::Result<()> {
    checked_child(folder, path, owned_name(path))
}

fn checked_temp_file(folder: &Path, path: &Path) -> io::Result<()> {
    let owned = path
        .file_name()
        .and_then(|s| s.to_str())
        .and_then(|name| name.strip_prefix(".pending-")?.strip_suffix(".tmp"))
        .is_some_and(|stamp| owned_name(Path::new(&format!("cross-{stamp}.jsonl"))));
    checked_child(folder, path, owned)
}

fn checked_child(folder: &Path, path: &Path, owned: bool) -> io::Result<()> {
    let folder = checked_folder(folder, false)?;
    let metadata = fs::symlink_metadata(path)?;
    if is_link(&metadata) || !metadata.is_file() || !owned || fs::canonicalize(path)?.parent() != Some(folder.as_path())
    {
        return Err(io::Error::other(
            "trace file is not an owned regular child of the trace folder",
        ));
    }
    Ok(())
}

fn owned_files(folder: &Path) -> io::Result<Vec<(PathBuf, u64)>> {
    let folder = checked_folder(folder, false)?;
    let mut files = Vec::new();
    for entry in fs::read_dir(&folder)? {
        let path = entry?.path();
        if !owned_name(&path) {
            continue;
        }
        // Links/reparse points are ignored, never followed (including retention and replay).
        let metadata = fs::symlink_metadata(&path)?;
        if is_link(&metadata) || !metadata.is_file() {
            continue;
        }
        checked_file(&folder, &path)?;
        files.push((path, metadata.len()));
    }
    // The UTC epoch-microsecond field is fixed-width, independent of last-write changes on marking.
    files.sort_by(|a, b| {
        let stamp = |p: &Path| {
            let name = p.file_stem().unwrap().to_string_lossy();
            let parts: Vec<_> = name.split('-').collect();
            (
                parts[1].parse::<u128>().unwrap_or_default(),
                parts[2].parse::<u64>().unwrap_or_default(),
                parts[3].parse::<u64>().unwrap_or_default(),
            )
        };
        stamp(&a.0).cmp(&stamp(&b.0)).then_with(|| a.0.cmp(&b.0))
    });
    Ok(files)
}

fn retain_owned(folder: &Path, max_files: usize, max_bytes: u64) -> io::Result<()> {
    let files = owned_files(folder)?;
    let mut count = files.len();
    let mut bytes = files.iter().map(|(_, size)| size).sum::<u64>();
    for (path, size) in files {
        if count <= max_files && bytes <= max_bytes {
            break;
        }
        checked_file(folder, &path)?;
        fs::remove_file(&path)?;
        count -= 1;
        bytes -= size;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplaySetting {
    #[serde(default)]
    pub ease: Option<i32>,
    pub model: CrossingModelName,
    pub min: Option<i32>,
    pub distance_px: Option<i32>,
    pub absorb_ms: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrossingModelName {
    Intent,
    Classic,
    Classic1k,
    StopThenPush,
}

/// Every integer in the specified ranges, with both directions using the same tested candidate.
pub fn replay_grid() -> Vec<ReplaySetting> {
    let mut grid = vec![ReplaySetting {
        ease: None,
        model: CrossingModelName::Classic,
        min: None,
        distance_px: None,
        absorb_ms: None,
    }];
    for min in 2..=15 {
        grid.push(ReplaySetting {
            ease: None,
            model: CrossingModelName::Classic1k,
            min: Some(min),
            distance_px: None,
            absorb_ms: None,
        });
    }
    for distance in 60..=300 {
        for absorb in 0..=150 {
            grid.push(ReplaySetting {
                ease: None,
                model: CrossingModelName::StopThenPush,
                min: None,
                distance_px: Some(distance),
                absorb_ms: Some(absorb),
            });
        }
    }
    for ease in 1..=5 {
        grid.push(ReplaySetting {
            model: CrossingModelName::Intent,
            ease: Some(ease),
            min: None,
            distance_px: None,
            absorb_ms: None,
        });
    }
    grid
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReplayRow {
    pub setting: ReplaySetting,
    pub accidents_still_cross: usize,
    pub intended_crossings_held: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReplayReport {
    pub files: usize,
    pub crossings: usize,
    pub accidents: usize,
    /// Focus events with no earlier captured origin on a known monitor are not classified.
    pub insufficient_context: usize,
    pub limitation: String,
    pub rows: Vec<ReplayRow>,
}

fn decode(path: &Path) -> io::Result<Capture> {
    let text = fs::read_to_string(path)?;
    let mut lines = text.lines();
    let header: FileHeader = serde_json::from_str(lines.next().ok_or_else(|| io::Error::other("empty trace"))?)?;
    if header.trace.schema != 1 {
        return Err(io::Error::other("unsupported edge trace schema"));
    }
    let records: Vec<Record> = lines.map(serde_json::from_str).collect::<Result<_, _>>()?;
    if records
        .windows(2)
        .any(|r| r[0].qpc_us > r[1].qpc_us || r[0].sequence >= r[1].sequence)
        || records.iter().any(|r| r.action > 3)
    {
        return Err(io::Error::other("invalid edge record sequence"));
    }
    Ok(Capture {
        header,
        records,
        path: Some(path.to_path_buf()),
    })
}

pub fn replay_folder(folder: &Path) -> io::Result<ReplayReport> {
    let files = owned_files(folder)?;
    let mut captures: Vec<Capture> = Vec::new();
    // Select one context for statistics, but replay every physical file below.
    let mut cases = BTreeMap::<(String, u64), (usize, bool)>::new();
    for (path, _) in &files {
        let capture = decode(path)?;
        if let Some(focus) = capture.header.focus_sequence {
            if !capture.records.iter().any(|r| r.sequence == focus && r.action == 2) {
                return Err(io::Error::other("trace focus does not name a captured actual crossing"));
            }
            let key = (capture.header.session.clone(), focus);
            if let Some(existing) = cases.get_mut(&key) {
                existing.1 |= capture.header.accident;
                // Prefer a crossing file's exact pre/post context; accident files add the annotation.
                if captures[existing.0].header.kind != "cross" && capture.header.kind == "cross" {
                    existing.0 = captures.len();
                }
            } else {
                cases.insert(key, (captures.len(), capture.header.accident));
            }
        }
        captures.push(capture);
    }
    let accidents = cases.values().filter(|(_, accident)| *accident).count();
    let insufficient_context = cases
        .values()
        .filter(|(index, _)| !has_replay_origin(&captures[*index]))
        .count();
    let mut rows = Vec::new();
    for setting in replay_grid() {
        let mut row = ReplayRow {
            setting,
            accidents_still_cross: 0,
            intended_crossings_held: 0,
        };
        let replayed: Vec<bool> = captures
            .iter()
            .map(|c| replay_crosses(c, setting))
            .collect::<io::Result<_>>()?;
        for (index, accident) in cases.values() {
            let capture = &captures[*index];
            if !has_replay_origin(capture) {
                continue;
            }
            let crosses = replayed[*index];
            if *accident && crosses {
                row.accidents_still_cross += 1;
            }
            if !*accident && !crosses {
                row.intended_crossings_held += 1;
            }
        }
        rows.push(row);
    }
    Ok(ReplayReport { files: files.len(), crossings: cases.len(), accidents, insufficient_context,
        limitation: "Counterfactual replay of sparse captured edge reports; first report seeds origin. Unmarked crossings are assumed intended. Missing movement, live-toggle changes within a capture, ring loss and physical feel cannot be reconstructed.".into(), rows })
}

fn has_replay_origin(capture: &Capture) -> bool {
    capture.records.iter().find(|r| r.action != 3).is_some_and(|first| {
        Some(first.sequence) != capture.header.focus_sequence
            && capture
                .header
                .trace
                .monitors()
                .iter()
                .any(|m| m.bounds.contains(first.x, first.y))
    })
}

fn replay_crosses(capture: &Capture, setting: ReplaySetting) -> io::Result<bool> {
    let header = &capture.header.trace;
    let settings = fence_config::from_json_v2(&header.settings.to_string()).map_err(io::Error::other)?;
    let mut policy = fence_config::to_policy(&settings);
    policy.intent.model = match setting.model {
        CrossingModelName::Intent => CrossingModel::Intent,
        CrossingModelName::Classic => CrossingModel::Classic,
        CrossingModelName::Classic1k => CrossingModel::Classic1k,
        CrossingModelName::StopThenPush => CrossingModel::StopThenPush,
    };
    if let Some(ease) = setting.ease {
        policy.intent.crossing_ease = ease.clamp(1, 5) as u8;
    }
    if let Some(min) = setting.min {
        policy.up_cross_min = min;
        policy.side_cross_min = min;
    }
    if let Some(distance) = setting.distance_px {
        policy.intent.up_push_distance_px = distance;
        policy.intent.side_push_distance_px = distance;
    }
    if let Some(absorb) = setting.absorb_ms {
        policy.intent.push_absorb_ms = absorb;
    }
    let monitors = header.monitors();
    let (mut config, _) = fence_engine::plan::build(&policy, &monitors);
    if let Some(min) = setting.min {
        config.cross_min_up = min;
    }
    let mut engine = ClassicEngine::new(config);
    engine.set_confine(header.confine);
    let toggles = Toggles {
        gate_open: header.gate_open,
        side_contain: header.side_contain,
    };
    let mut current = None;
    let mut focus_crossed = false;
    let focus = capture
        .records
        .iter()
        .find(|r| Some(r.sequence) == capture.header.focus_sequence);
    let target = focus.and_then(|r| {
        let (x, y) = if r.model_state & 128 != 0 {
            (r.features[10], r.features[11])
        } else {
            (r.x, r.y)
        };
        monitors.iter().position(|m| m.bounds.contains(x, y))
    });
    for record in &capture.records {
        if record.action == 3 {
            engine.raw_motion(record.x, record.y, record.features[6] as u16, record.qpc_us);
            continue;
        }
        let Some((x, y)) = current else {
            engine.set_last(record.x, record.y);
            current = Some((record.x, record.y));
            continue;
        };
        let decision = if header.paused {
            fence_types::Decision {
                action: fence_types::Action::Pass,
                x: record.x,
                y: record.y,
            }
        } else {
            engine.decide_event(
                MoveEvent {
                    x: record.x,
                    y: record.y,
                    time_ms: record.time_ms,
                    qpc_us: record.qpc_us,
                },
                toggles,
            )
        };
        let origin = monitors.iter().position(|m| m.bounds.contains(x, y));
        let landing = monitors.iter().position(|m| m.bounds.contains(decision.x, decision.y));
        // Lighter models can cross before the original focus report, pressure models after it.
        // Count the same destination within a bounded +/-100ms episode, not only one report.
        if focus.is_some_and(|r| r.qpc_us.abs_diff(record.qpc_us) <= 100_000) {
            focus_crossed |= origin.is_some() && landing.is_some() && origin != landing && landing == target;
        }
        // Simulate the accepted/clamped position; never force the recorded successful target after a block.
        engine.observe(decision.x, decision.y);
        current = Some((decision.x, decision.y));
    }
    Ok(focus_crossed)
}

/// Recognized even on malformed arguments so a replay request can never fall through to the tray.
pub fn run_replay_cli(args: &[String]) -> Option<i32> {
    if args.first().map(String::as_str) != Some("--replay") {
        return None;
    }
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    let result = replay_cli_to_writer(args, &mut out);
    match result {
        Ok(()) => Some(0),
        Err(error) => {
            eprintln!("Replay failed: {error}");
            Some(2)
        }
    }
}

fn replay_cli_to_writer(args: &[String], out: &mut impl Write) -> io::Result<()> {
    let valid = args.len() == 2 || args.len() == 4 && args[2] == "--json";
    if !valid {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: MouseFence.exe --replay <folder> [--json <out>]",
        ));
    }
    let folder = std::path::absolute(&args[1])?;
    let report = replay_folder(&folder)?;
    if args.len() == 4 {
        let output = std::path::absolute(&args[3])?;
        if fs::symlink_metadata(&output).is_ok_and(|m| is_link(&m)) {
            return Err(io::Error::other("JSON output cannot be a link"));
        }
        fence_config::write_atomic(&output, &serde_json::to_vec_pretty(&report)?)?;
    }
    writeln!(
        out,
        "{} files, {} unique crossings, {} marked accidents, {} with insufficient context (excluded)",
        report.files, report.crossings, report.accidents, report.insufficient_context
    )?;
    writeln!(
        out,
        "model,min,distance_px,absorb_ms,accidents_still_cross,intended_crossings_held,ease"
    )?;
    for row in report.rows {
        writeln!(
            out,
            "{:?},{},{},{},{},{},{}",
            row.setting.model,
            row.setting.min.map(|v| v.to_string()).unwrap_or_default(),
            row.setting.distance_px.map(|v| v.to_string()).unwrap_or_default(),
            row.setting.absorb_ms.map(|v| v.to_string()).unwrap_or_default(),
            row.accidents_still_cross,
            row.intended_crossings_held,
            row.setting.ease.map(|v| v.to_string()).unwrap_or_default()
        )?;
    }
    writeln!(out, "{}", report.limitation)?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fence_config::Stored;
    use fence_types::BoundaryDefault;

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("mousefence-edge-test-{}", stamp()));
            fs::create_dir(&path).unwrap();
            Self(fs::canonicalize(path).unwrap())
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            assert!(self.0.is_absolute());
            assert!(
                self.0
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("mousefence-edge-test-")
            );
            // Only this test's exclusively created directory, never a user trace folder.
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn header() -> TraceHeader {
        let settings = Settings {
            side_boundary_default: Stored::Known(BoundaryDefault::Push),
            ..Settings::default()
        };
        let monitors = [
            MonitorInfo {
                stable_id: "main".into(),
                device: "not-retained".into(),
                bounds: Rect::new(0, 0, 1000, 1000),
                primary: true,
                index: 1,
            },
            MonitorInfo {
                stable_id: "side".into(),
                device: String::new(),
                bounds: Rect::new(1000, 0, 2000, 1000),
                primary: false,
                index: 2,
            },
        ];
        TraceHeader::from_settings(&settings, &monitors, 8.0)
    }
    fn event(us: u64, x: i32, action: u8) -> EdgeRecord {
        EdgeRecord {
            features: [0; 12],
            qpc_us: us,
            time_ms: (us / 1000) as u32,
            x,
            y: 500,
            action,
            model_state: 0,
        }
    }
    fn traces(temp: &Temp) -> Vec<Capture> {
        owned_files(&temp.0)
            .unwrap()
            .iter()
            .map(|(p, _)| decode(p).unwrap())
            .collect()
    }
    fn crossing(store: &mut TraceStore, t: u64) {
        store
            .ingest(
                &[event(t - 2000, 900, 0), event(t - 1000, 995, 0), event(t, 1005, 2)],
                t,
            )
            .unwrap();
    }
    fn candidate(model: CrossingModelName, min: Option<i32>) -> ReplaySetting {
        ReplaySetting {
            ease: None,
            model,
            min,
            distance_px: (model == CrossingModelName::StopThenPush).then_some(120),
            absorb_ms: (model == CrossingModelName::StopThenPush).then_some(75),
        }
    }

    #[test]
    fn j24_store_construction_and_updates_are_io_free() {
        let temp = Temp::new();
        let folder = temp.0.join("disabled-never-created");
        let mut store = TraceStore::new(folder.clone(), header());
        store.update_header(header());
        store.ingest(&[], 50_000_000).unwrap();
        assert!(
            !folder.exists(),
            "no capture means no storage, including while disabled by the caller"
        );
    }

    #[test]
    fn intent_201_raw_features_held_outcome_and_five_ease_candidates_survive_storage_and_replay() {
        let temp = Temp::new();
        let mut store = TraceStore::new(temp.0.clone(), header());
        let mut records = vec![];
        for ms in (0..=300).step_by(5) {
            let us = 1_000_000 + ms * 1000;
            let mut raw = event(us, 4, 3);
            raw.y = 0;
            raw.features[10] = 4;
            records.push(raw);
            let mut screen = event(
                us + 1,
                if ms < 200 { 500 } else { 1001 },
                if ms < 200 {
                    0
                } else if ms < 240 {
                    1
                } else {
                    2
                },
            );
            screen.features[7] = if ms < 200 {
                0
            } else if ms < 240 {
                3
            } else {
                6
            };
            records.push(screen);
            if ms >= 240 {
                break;
            }
        }
        store.ingest(&records, 1_500_000).unwrap();
        store.ingest(&[], 2_000_000).unwrap();
        let captures = traces(&temp);
        assert!(captures.iter().any(|c| c.header.kind == "held"));
        assert!(
            captures
                .iter()
                .any(|c| c.records.iter().any(|r| r.action == 3 && r.features[10] == 4))
        );
        let report = replay_folder(&temp.0).unwrap();
        assert_eq!(report.crossings, 1);
        assert_eq!(report.insufficient_context, 0);
        let rows: Vec<_> = report
            .rows
            .iter()
            .filter(|r| r.setting.model == CrossingModelName::Intent)
            .collect();
        assert_eq!(rows.len(), 5);
        assert!(rows.iter().all(|r| r.intended_crossings_held == 0));
        retain_owned(&temp.0, 1, MAX_BYTES).unwrap();
        assert_eq!(owned_files(&temp.0).unwrap().len(), 1);
    }

    fn failed_capture_attempts_preserve_prior_and_retry(sync_failure: bool) {
        let temp = Temp::new();
        let mut store = TraceStore::new(temp.0.clone(), header());
        crossing(&mut store, 2_000_000);
        store.ingest(&[], 2_500_000).unwrap();
        let prior = owned_files(&temp.0).unwrap().remove(0).0;
        let prior_bytes = fs::read(&prior).unwrap();
        crossing(&mut store, 5_000_000);
        let pending = store.pending[0].clone();
        for _ in 0..=MAX_FILES {
            let error = store
                .save_with(
                    &pending,
                    |file, bytes| {
                        file.write_all(&bytes[..bytes.len() / 2])?;
                        if sync_failure {
                            file.write_all(&bytes[bytes.len() / 2..])
                        } else {
                            Err(io::Error::new(io::ErrorKind::StorageFull, "injected write failure"))
                        }
                    },
                    |_| Err(io::Error::other("injected sync failure")),
                )
                .unwrap_err();
            assert_eq!(
                error.kind(),
                if sync_failure {
                    io::ErrorKind::Other
                } else {
                    io::ErrorKind::StorageFull
                }
            );
        }
        assert_eq!(fs::read(&prior).unwrap(), prior_bytes);
        assert_eq!(
            fs::read_dir(&temp.0).unwrap().count(),
            1,
            "301 returned errors must leave neither owned traces nor temp siblings"
        );
        assert_eq!(store.pending.len(), 1);
        store.ingest(&[], 5_500_000).unwrap();
        assert_eq!(store.pending.len(), 0);
        assert_eq!(owned_files(&temp.0).unwrap().len(), 2);
        assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 2);
        assert_eq!(fs::read(&prior).unwrap(), prior_bytes);
        assert!(traces(&temp).iter().all(|c| !c.records.is_empty()));
    }

    #[test]
    fn j24_failed_writes_leave_no_partial_traces_after_301_attempts_and_retry() {
        failed_capture_attempts_preserve_prior_and_retry(false);
    }

    #[test]
    fn j24_failed_syncs_leave_no_partial_traces_after_301_attempts_and_retry() {
        failed_capture_attempts_preserve_prior_and_retry(true);
    }

    #[test]
    fn j24_capture_is_invisible_until_write_and_sync_complete() {
        let temp = Temp::new();
        let mut store = TraceStore::new(temp.0.clone(), header());
        crossing(&mut store, 2_000_000);
        let capture = store.pending[0].clone();
        let path = store
            .save_with(
                &capture,
                |file, bytes| {
                    file.write_all(&bytes[..bytes.len() / 2])?;
                    assert!(
                        owned_files(&temp.0)?.is_empty(),
                        "partial capture must not be visible to replay"
                    );
                    assert_eq!(
                        fs::read_dir(&temp.0)?.count(),
                        1,
                        "only the private temp sibling exists"
                    );
                    file.write_all(&bytes[bytes.len() / 2..])
                },
                |file| {
                    assert!(
                        owned_files(&temp.0)?.is_empty(),
                        "unsynced capture must not be published"
                    );
                    file.sync_all()
                },
            )
            .unwrap();
        assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 1);
        assert_eq!(decode(&path).unwrap().records.len(), 3);
    }

    #[test]
    fn j24_atomic_capture_collision_never_overwrites_existing_trace_or_leaks_temp() {
        let temp = Temp::new();
        let mut store = TraceStore::new(temp.0.clone(), header());
        crossing(&mut store, 2_000_000);
        let path = temp.0.join("cross-00000000000000000001-1-1.jsonl");
        let prior = encode(&store.pending[0]).unwrap();
        fs::write(&path, &prior).unwrap();
        let error = create_capture_atomic(
            &path,
            b"replacement that must never be published",
            |file, bytes| file.write_all(bytes),
            fs::File::sync_all,
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&path).unwrap(), prior);
        assert_eq!(decode(&path).unwrap().records.len(), 3);
        assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 1);
    }

    #[test]
    fn j24_header_first_line_has_known_policy_layout_and_stable_fingerprint_only() {
        let mut settings = Settings::default();
        settings.raw.unknown_members.push((
            "HarmlessUnknown".into(),
            Value::String("arbitrary private prose".into()),
        ));
        settings.monitor_names.push(("main".into(), "private nickname".into()));
        settings.up_push_distance_px = -999;
        let first = TraceHeader::from_settings(&settings, &header().monitors(), f64::NAN);
        settings.raw.unknown_members.clear();
        settings.monitor_names.clear();
        let second = TraceHeader::from_settings(&settings, &header().monitors(), 8.0);
        assert_eq!(first.settings_fingerprint, second.settings_fingerprint);
        assert_eq!(first.estimated_rate, 0.0);
        assert_eq!(first.layout[1].rect, [1000, 0, 2000, 1000]);
        assert_eq!(first.settings["UpPushDistancePx"], 1);
        let temp = Temp::new();
        let mut store = TraceStore::new(temp.0.clone(), first);
        crossing(&mut store, 2_000_000);
        store.ingest(&[], 2_500_000).unwrap();
        let path = owned_files(&temp.0).unwrap().remove(0).0;
        let text = fs::read_to_string(path).unwrap();
        let line: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!(line["layout"][0]["stable_id"], "main");
        assert_eq!(line["settings_fingerprint"], second.settings_fingerprint);
        for omitted in [
            "HarmlessUnknown",
            "arbitrary private prose",
            "private nickname",
            "HotKey",
            "MonitorNames",
            "AutoStart",
            "not-retained",
        ] {
            assert!(!text.contains(omitted), "unexpected retained data: {omitted}");
        }
        settings.side_cross_min += 1;
        assert_ne!(
            second.settings_fingerprint,
            TraceHeader::from_settings(&settings, &[], 0.0).settings_fingerprint
        );
    }

    #[test]
    fn j24_pre_post_windows_are_inclusive_and_crossing_waits_for_post_context() {
        let temp = Temp::new();
        let mut store = TraceStore::new(temp.0.clone(), header());
        store
            .ingest(
                &[
                    event(499_999, 900, 0),
                    event(500_000, 901, 0),
                    event(1_999_999, 995, 0),
                    event(2_000_000, 1005, 2),
                    event(2_499_999, 1010, 0),
                ],
                2_499_999,
            )
            .unwrap();
        assert!(traces(&temp).is_empty());
        store
            .ingest(&[event(2_500_000, 1011, 0), event(2_500_001, 1012, 0)], 2_500_001)
            .unwrap();
        let captures = traces(&temp);
        let times: Vec<_> = captures[0].records.iter().map(|r| r.qpc_us).collect();
        assert_eq!(times, [500_000, 1_999_999, 2_000_000, 2_499_999, 2_500_000]);
        assert!(store.pending.is_empty());
    }

    #[test]
    fn j24_memory_retains_exactly_thirty_seconds_and_accident_exactly_three() {
        let temp = Temp::new();
        let mut store = TraceStore::new(temp.0.clone(), header());
        store
            .ingest(
                &[
                    event(1, 900, 0),
                    event(2, 901, 0),
                    event(27_000_001, 902, 0),
                    event(27_000_002, 903, 0),
                ],
                30_000_002,
            )
            .unwrap();
        assert_eq!(store.window.front().unwrap().qpc_us, 2);
        assert_eq!(store.mark_accident(30_000_002).unwrap(), 1);
        let captures = traces(&temp);
        assert_eq!(
            captures[0].records.iter().map(|r| r.qpc_us).collect::<Vec<_>>(),
            [27_000_002]
        );
        assert_eq!(captures[0].header.focus_sequence, None);
        store.ingest(&[], 60_000_003).unwrap();
        assert!(store.window.is_empty());
    }

    #[test]
    fn j24_accident_flags_only_newest_pending_crossing_even_with_equal_qpc() {
        let temp = Temp::new();
        let mut store = TraceStore::new(temp.0.clone(), header());
        store
            .ingest(
                &[
                    event(1_000_000, 995, 0),
                    event(2_000_000, 1005, 2),
                    event(2_000_000, 995, 2),
                ],
                2_000_000,
            )
            .unwrap();
        assert_eq!(store.mark_accident(2_000_000).unwrap(), 1);
        assert!(!store.pending[0].header.accident);
        assert!(store.pending[1].header.accident);
        store.ingest(&[], 2_500_000).unwrap();
        let captures = traces(&temp);
        let crosses: Vec<_> = captures.iter().filter(|c| c.header.kind == "cross").collect();
        assert_eq!(crosses.len(), 2);
        assert_eq!(crosses.iter().filter(|c| c.header.accident).count(), 1);
        assert_ne!(crosses[0].header.focus_sequence, crosses[1].header.focus_sequence);
        let report = replay_folder(&temp.0).unwrap();
        assert_eq!((report.files, report.crossings, report.accidents), (3, 2, 1));
    }

    #[test]
    fn j24_accident_rewrites_written_crossing_and_counts_marks_without_balloon() {
        let temp = Temp::new();
        let mut store = TraceStore::new(temp.0.clone(), header());
        crossing(&mut store, 2_000_000);
        store.ingest(&[], 2_500_000).unwrap();
        assert!(
            store.latest.as_ref().unwrap().records.is_empty(),
            "completed data lives on disk, not indefinitely in memory"
        );
        assert_eq!(store.mark_accident(2_600_000).unwrap(), 1);
        assert_eq!(store.mark_accident(2_700_000).unwrap(), 2);
        let captures = traces(&temp);
        assert_eq!(captures.len(), 3);
        assert!(captures.iter().all(|c| c.header.accident));
        assert!(
            captures
                .iter()
                .find(|c| c.header.kind == "cross")
                .unwrap()
                .records
                .len()
                >= 3
        );
    }

    #[test]
    fn j24_pending_captures_keep_original_header_when_live_settings_change() {
        let temp = Temp::new();
        let original = header();
        let mut store = TraceStore::new(temp.0.clone(), original.clone());
        crossing(&mut store, 2_000_000);
        let mut next = original.clone();
        next.set_runtime(RuntimeState {
            gate_open: true,
            side_contain: false,
            confine: true,
            paused: true,
            hook_installed: false,
        });
        store.update_header(next);
        store.ingest(&[], 2_500_000).unwrap();
        assert!(!traces(&temp)[0].header.trace.confine);
        assert!(store.header.confine && store.header.paused && store.header.gate_open && !store.header.side_contain);
    }

    #[test]
    fn j24_retention_enforces_count_bytes_and_oldest_ownership() {
        let temp = Temp::new();
        let unrelated = temp.0.join("cross-user-notes.jsonl");
        fs::write(&unrelated, "keep me").unwrap();
        let directory = temp.0.join("cross-00000000000000000000-1-1.jsonl");
        fs::create_dir(&directory).unwrap();
        for n in 0..=300 {
            fs::write(temp.0.join(format!("cross-{n:020}-1-1.jsonl")), [b'x'; 100]).unwrap_or_else(|e| {
                // n=0 has the directory above; it is never a retention candidate.
                assert_eq!(n, 0, "{e}");
            });
        }
        let oldest = temp.0.join("cross-00000000000000000001-1-1.jsonl");
        retain_owned(&temp.0, MAX_FILES, MAX_BYTES).unwrap();
        assert_eq!(owned_files(&temp.0).unwrap().len(), 300);
        assert!(oldest.exists());
        fs::write(temp.0.join("cross-00000000000000000301-1-1.jsonl"), [b'x'; 100]).unwrap();
        retain_owned(&temp.0, MAX_FILES, MAX_BYTES).unwrap();
        assert!(!oldest.exists());
        // One file exceeds the full production byte budget; remove oldest files until both bounds hold.
        let huge = temp.0.join("cross-00000000000000000302-1-1.jsonl");
        fs::File::create(&huge).unwrap().set_len(MAX_BYTES).unwrap();
        retain_owned(&temp.0, MAX_FILES, MAX_BYTES).unwrap();
        assert_eq!(owned_files(&temp.0).unwrap().len(), 1);
        assert!(huge.exists());
        assert_eq!(fs::read_to_string(unrelated).unwrap(), "keep me");
        assert!(directory.is_dir());
    }

    #[test]
    fn j24_folder_traversal_and_non_owned_files_are_rejected() {
        let temp = Temp::new();
        assert!(checked_folder(Path::new("relative-traces"), true).is_err());
        // PathBuf::join normalizes '..' on Windows verbatim paths, so preserve the raw input here.
        let traversal = PathBuf::from(format!("{}{}..", temp.0.display(), std::path::MAIN_SEPARATOR));
        assert!(checked_folder(&traversal, true).is_err());
        assert!(!owned_name(Path::new("cross-2026-10-30.jsonl")));
        assert!(!owned_name(Path::new("accident-00000000000000000001-1-1.jsonl.backup")));
        assert!(owned_name(Path::new("cross-00000000000000000001-1-1.jsonl")));
        fs::write(temp.0.join("not-owned.jsonl"), "not a trace").unwrap();
        assert!(owned_files(&temp.0).unwrap().is_empty());
    }

    #[test]
    fn j24_bad_time_order_is_rejected_without_partial_ingest() {
        let temp = Temp::new();
        let mut store = TraceStore::new(temp.0.clone(), header());
        assert!(store.ingest(&[event(2, 0, 0), event(1, 0, 0)], 2).is_err());
        assert!(store.ingest(&[event(3, 0, 0)], 2).is_err());
        assert!(store.window.is_empty());
        store.ingest(&[event(3, 0, 0)], 3).unwrap();
        assert!(store.ingest(&[event(2, 0, 0)], 3).is_err());
    }

    #[test]
    fn j24_replay_grid_includes_every_integer_and_both_ends() {
        let grid = replay_grid();
        assert_eq!(grid.len(), 36_411);
        for min in 2..=15 {
            assert!(
                grid.iter()
                    .any(|s| s.model == CrossingModelName::Classic1k && s.min == Some(min))
            );
        }
        for distance in 60..=300 {
            for absorb in 0..=150 {
                assert!(
                    grid[15 + ((distance - 60) * 151 + absorb) as usize]
                        == ReplaySetting {
                            ease: None,
                            model: CrossingModelName::StopThenPush,
                            min: None,
                            distance_px: Some(distance),
                            absorb_ms: Some(absorb as u32)
                        }
                );
            }
        }
    }

    #[test]
    fn j24_replay_classifies_accidents_and_intended_holds_without_duplicate_samples() {
        let temp = Temp::new();
        let mut store = TraceStore::new(temp.0.clone(), header());
        crossing(&mut store, 2_000_000);
        store.ingest(&[], 2_500_000).unwrap();
        store.mark_accident(2_600_000).unwrap();
        crossing(&mut store, 5_000_000);
        store.ingest(&[], 5_500_000).unwrap();
        let report = replay_folder(&temp.0).unwrap();
        assert_eq!((report.files, report.crossings, report.accidents), (3, 2, 1));
        assert_eq!(report.rows.len(), 36_411);
        let classic = &report.rows[0];
        assert_eq!((classic.accidents_still_cross, classic.intended_crossings_held), (1, 0));
        let strict = report.rows.iter().find(|r| r.setting.min == Some(15)).unwrap();
        assert_eq!((strict.accidents_still_cross, strict.intended_crossings_held), (0, 1));
        let stop = report
            .rows
            .iter()
            .find(|r| r.setting.model == CrossingModelName::StopThenPush)
            .unwrap();
        assert_eq!((stop.accidents_still_cross, stop.intended_crossings_held), (0, 1));
    }

    #[test]
    fn j24_replay_obeys_recorded_policy_and_runtime_toggles() {
        let temp = Temp::new();
        let mut store = TraceStore::new(temp.0.clone(), header());
        crossing(&mut store, 2_000_000);
        store.ingest(&[], 2_500_000).unwrap();
        let mut capture = traces(&temp).remove(0);
        assert!(replay_crosses(&capture, candidate(CrossingModelName::Classic, None)).unwrap());
        capture.header.trace.confine = true;
        assert!(!replay_crosses(&capture, candidate(CrossingModelName::Classic, None)).unwrap());
        capture.header.trace.confine = false;
        capture.header.trace.settings["SideBoundaryDefault"] = Value::String("Blocked".into());
        // Otherwise the production anti-trap compiler can demote this two-screen sealed layout.
        capture.header.trace.settings["SafetyEscapes"] = Value::Bool(false);
        assert!(!replay_crosses(&capture, candidate(CrossingModelName::Classic, None)).unwrap());
    }

    #[test]
    fn j24_replay_rejects_malformed_owned_trace_instead_of_reporting_zero() {
        let temp = Temp::new();
        fs::write(temp.0.join("cross-00000000000000000001-1-1.jsonl"), "invalid trace").unwrap();
        assert!(replay_folder(&temp.0).is_err());
    }

    #[test]
    fn j24_replay_missing_origin_is_reported_without_inventing_a_hold() {
        let temp = Temp::new();
        let mut store = TraceStore::new(temp.0.clone(), header());
        store.ingest(&[event(2_000_000, 1005, 2)], 2_000_000).unwrap();
        store.ingest(&[], 2_500_000).unwrap();
        let report = replay_folder(&temp.0).unwrap();
        assert_eq!((report.crossings, report.insufficient_context), (1, 1));
        assert!(
            report
                .rows
                .iter()
                .all(|r| r.accidents_still_cross == 0 && r.intended_crossings_held == 0)
        );
    }

    #[test]
    fn j24_replay_executes_accident_files_without_a_focused_crossing() {
        let temp = Temp::new();
        let mut store = TraceStore::new(temp.0.clone(), header());
        store.mark_accident(1_000_000).unwrap();
        let mut capture = traces(&temp).remove(0);
        assert!(capture.header.focus_sequence.is_none());
        // This file is syntactically valid but cannot configure a model. Skipping unfocused files
        // would incorrectly return a successful zero-sample report instead of this error.
        capture.header.trace.settings["SettingsVersion"] = Value::from(999);
        fence_config::write_atomic(capture.path.as_ref().unwrap(), &encode(&capture).unwrap()).unwrap();
        assert!(replay_folder(&temp.0).is_err());
    }

    #[test]
    fn j24_cli_json_output_is_a_complete_report_and_atomic_replacement() {
        let temp = Temp::new();
        let traces_dir = temp.0.join("traces");
        let mut store = TraceStore::new(traces_dir.clone(), header());
        crossing(&mut store, 2_000_000);
        store.ingest(&[], 2_500_000).unwrap();
        let output = temp.0.join("result.json");
        fs::write(&output, "old report").unwrap();
        let args = vec![
            "--replay".into(),
            traces_dir.to_string_lossy().into_owned(),
            "--json".into(),
            output.to_string_lossy().into_owned(),
        ];
        let mut stdout = Vec::new();
        replay_cli_to_writer(&args, &mut stdout).unwrap();
        let report: ReplayReport = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
        assert_eq!(report.crossings, 1);
        assert_eq!(report.rows.len(), 36_411);
        let printed = String::from_utf8(stdout).unwrap();
        assert!(printed.contains("Classic1k,15,,,0,1"));
        assert!(printed.contains("StopThenPush,,300,150,0,1"));
        assert_eq!(
            fs::read_dir(&temp.0).unwrap().count(),
            2,
            "no leftover atomic temp sibling"
        );
        assert_eq!(run_replay_cli(&["--identify".into()]), None);
        assert_eq!(run_replay_cli(&["--replay".into()]), Some(2));
        assert!(
            replay_cli_to_writer(
                &["--replay".into(), "ignored".into(), "--bogus".into()],
                &mut Vec::new()
            )
            .is_err()
        );
    }

    /// First screen change in a capture under a chosen ease and steady-push setting.
    fn first_crossing_us(capture: &Capture, ease: u8, steady_push_ms: i32) -> Option<u64> {
        let header = &capture.header.trace;
        let settings = fence_config::from_json_v2(&header.settings.to_string()).ok()?;
        let mut policy = fence_config::to_policy(&settings);
        policy.intent.model = CrossingModel::Intent;
        policy.intent.crossing_ease = ease;
        policy.intent.steady_push_ms = steady_push_ms;
        let monitors = header.monitors();
        let (config, _) = fence_engine::plan::build(&policy, &monitors);
        let mut engine = ClassicEngine::new(config);
        engine.set_confine(header.confine);
        let toggles = Toggles {
            gate_open: header.gate_open,
            side_contain: header.side_contain,
        };
        let mut current: Option<(i32, i32)> = None;
        for record in &capture.records {
            if record.action == 3 {
                engine.raw_motion(record.x, record.y, record.features[6] as u16, record.qpc_us);
                continue;
            }
            let Some((x, y)) = current else {
                engine.set_last(record.x, record.y);
                current = Some((record.x, record.y));
                continue;
            };
            let event = MoveEvent {
                x: record.x,
                y: record.y,
                time_ms: record.time_ms,
                qpc_us: record.qpc_us,
            };
            let d = engine.decide_event(event, toggles);
            let origin = monitors.iter().position(|m| m.bounds.contains(x, y));
            let landing = monitors.iter().position(|m| m.bounds.contains(d.x, d.y));
            if origin.is_some() && landing.is_some() && origin != landing {
                return Some(record.qpc_us);
            }
            engine.observe(d.x, d.y);
            current = Some((d.x, d.y));
        }
        None
    }

    /// Tuning aid, not a gate: replays a copy of real traces (`MF_TRACE_DIR`) and prints how long
    /// crossings took from the first judged contact, for 2.0.1 (-1) and the steady-push path.
    #[test]
    #[ignore = "reads MF_TRACE_DIR, a private copy of a real trace folder"]
    fn steady_push_replay_of_local_traces() {
        let Ok(dir) = std::env::var("MF_TRACE_DIR") else {
            return;
        };
        let mut files: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        files.sort();
        let captures: Vec<Capture> = files.iter().filter_map(|p| decode(p).ok()).collect();
        eprintln!("captures={}", captures.len());
        for (ease, steady) in [
            (5u8, -1),
            (5, 0),
            (5, 30),
            (5, 45),
            (4, -1),
            (4, 0),
            (3, -1),
            (3, 0),
            (1, -1),
            (1, 0),
        ] {
            let mut latencies = Vec::new();
            let (mut never, mut held_files, mut held_now_cross) = (0, 0, 0);
            for c in &captures {
                let t0 = c
                    .records
                    .iter()
                    .find(|r| r.action == 1 || r.action == 2)
                    .map(|r| r.qpc_us);
                let crossed_before = c.records.iter().any(|r| r.action == 2);
                let t = first_crossing_us(c, ease, steady);
                if c.header.kind == "cross" {
                    match (t0, t) {
                        (Some(t0), Some(t)) => latencies.push(t.saturating_sub(t0) / 1000),
                        _ => never += 1,
                    }
                } else if !crossed_before {
                    held_files += 1;
                    held_now_cross += usize::from(t.is_some());
                    if t.is_some() && first_crossing_us(c, ease, -1).is_none() {
                        let name = c.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy());
                        eprintln!("  ease={ease} steady={steady} newly crosses held file {name:?}");
                    }
                }
            }
            latencies.sort_unstable();
            let q = |p: f64| {
                latencies
                    .get(((latencies.len() as f64 - 1.0) * p) as usize)
                    .copied()
                    .unwrap_or(0)
            };
            let slow = latencies.iter().filter(|ms| **ms > 150).count();
            eprintln!(
                "ease={ease} steady={steady} crossings={} not_crossed={never} median={}ms p75={}ms p90={}ms over150ms={slow} held_files_without_crossing={held_files} of_which_now_cross={held_now_cross}",
                latencies.len(),
                q(0.5),
                q(0.75),
                q(0.9)
            );
        }
    }
}
