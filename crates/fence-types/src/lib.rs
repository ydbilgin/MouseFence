//! Shared types for MouseFence 2.0.
//!
//! Owned by the skeleton. Tracks may ADD items here (new types, new methods, new enum variants behind
//! `#[non_exhaustive]` where marked) but must not rename or change existing ones, because every other
//! track compiles against them. Coordinates are physical pixels in the Windows virtual desktop
//! (PerMonitorV2), stored as i32; products and distances widen to i64/i128 (PLAN T7).
#![forbid(unsafe_code)]

use std::cmp::Ordering;

/// Half-open rectangle `[l, r) x [t, b)`, the same convention as the C# `(int L, int T, int R, int B)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    pub l: i32,
    pub t: i32,
    pub r: i32,
    pub b: i32,
}

impl Rect {
    pub const fn new(l: i32, t: i32, r: i32, b: i32) -> Self {
        Self { l, t, r, b }
    }

    /// C# `InRect`: `x >= L && x < R && y >= T && y < B`.
    pub const fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.l && x < self.r && y >= self.t && y < self.b
    }

    /// C# `BoundaryGraph.Valid`: positive width and height.
    pub const fn is_valid(&self) -> bool {
        self.r > self.l && self.b > self.t
    }

    pub const fn width(&self) -> i64 {
        self.r as i64 - self.l as i64
    }

    pub const fn height(&self) -> i64 {
        self.b as i64 - self.t as i64
    }

    /// C# `GuardCore.CompareRect`: lexicographic L, T, R, B. Used for deterministic tie-breaks.
    pub fn cmp_ltrb(&self, other: &Rect) -> Ordering {
        (self.l, self.t, self.r, self.b).cmp(&(other.l, other.t, other.r, other.b))
    }
}

/// A seam direction, from the source screen's point of view (C# `BoundaryDirection` without `Unknown`;
/// `Unknown` exists only in imported settings drafts, see fence-config).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub const fn is_vertical(self) -> bool {
        matches!(self, Direction::Up | Direction::Down)
    }
    /// The C# enum ordinal (Unknown = 0, Up = 1, Down = 2, Left = 3, Right = 4). Sorting and rule keys use it.
    pub const fn csharp_ordinal(self) -> i32 {
        match self {
            Direction::Up => 1,
            Direction::Down => 2,
            Direction::Left => 3,
            Direction::Right => 4,
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Direction::Up => "Up",
            Direction::Down => "Down",
            Direction::Left => "Left",
            Direction::Right => "Right",
        }
    }
}

/// Effective behaviour of one compiled transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Behavior {
    Free,
    Push,
    Blocked,
}

/// Behaviour stored in a saved rule. `OverallFeel` is the v3 "allowed with the overall push feel" value and
/// compiles to `Push` (C# `BoundaryGraph.Compile`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RuleBehavior {
    Free,
    Push,
    Blocked,
    OverallFeel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CrossingModel {
    Intent,
    Classic,
    #[default]
    Classic1k,
    StopThenPush,
}

impl CrossingModel {
    pub fn parse(value: &str) -> Self {
        if value.trim().eq_ignore_ascii_case("intent") {
            Self::Intent
        } else if value.trim().eq_ignore_ascii_case("classic") {
            Self::Classic
        } else if value.trim().eq_ignore_ascii_case("stopthenpush") {
            Self::StopThenPush
        } else {
            Self::Classic1k
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::Intent => "Intent",
            Self::Classic => "Classic",
            Self::Classic1k => "Classic1k",
            Self::StopThenPush => "StopThenPush",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntentConfig {
    pub crossing_ease: u8,
    pub model: CrossingModel,
    pub up_push_distance_px: i32,
    pub side_push_distance_px: i32,
    pub push_absorb_ms: u32,
    pub push_pause_reset_ms: u32,
    /// Intent steady-push hold: -1 off, 0 follow the ease slider, else milliseconds.
    pub steady_push_ms: i32,
}

impl Default for IntentConfig {
    fn default() -> Self {
        Self {
            crossing_ease: 3,
            model: CrossingModel::Classic1k,
            up_push_distance_px: 120,
            side_push_distance_px: 120,
            push_absorb_ms: 75,
            push_pause_reset_ms: 150,
            steady_push_ms: 0,
        }
    }
}

/// Whether a rule follows the live quick toggles (`Automatic`) or ignores them (`Always`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Control {
    Automatic,
    Always,
}

/// Per-direction default for seams without an explicit rule (C# `BoundaryDefault` without `Unknown`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BoundaryDefault {
    Automatic,
    Free,
    Push,
    Blocked,
}

/// How "top" screens are chosen (C# `Settings.Mode`: "AutoTop" | "Manual").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum TopMode {
    #[default]
    AutoTop,
    Manual,
}

/// One saved directional rule. Identity = case-insensitive, trimmed (from, to, direction).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundaryRule {
    pub from_stable_id: String,
    pub to_stable_id: String,
    pub direction: Direction,
    pub behavior: RuleBehavior,
    pub control: Control,
}

/// The engine-facing subset of the settings (fence-config maps `Settings` to this; Track B owns that
/// mapping, Track A consumes it). Rules hold only valid entries, in saved order (last valid wins).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Policy {
    pub intent: IntentConfig,
    pub upward_default: BoundaryDefault,
    pub side_default: BoundaryDefault,
    pub downward_default: BoundaryDefault,
    pub rules: Vec<BoundaryRule>,
    pub up_cross_min: i32,
    pub up_cross_slack: i32,
    pub side_cross_min: i32,
    pub side_cross_slack: i32,
    pub passage_inset_px: i32,
    pub edge_holdback_px: i32,
    pub seam_tolerance_px: i32,
    pub safety_escapes: bool,
    pub deliberate_cross: bool,
    pub descent_routing: bool,
    pub mode: TopMode,
    pub manual_monitors: Vec<String>,
    pub excluded_devices: Vec<String>,
}

impl Default for Policy {
    /// The C# `new Settings()` defaults, schema v4.
    fn default() -> Self {
        Self {
            intent: IntentConfig::default(),
            upward_default: BoundaryDefault::Automatic,
            side_default: BoundaryDefault::Automatic,
            downward_default: BoundaryDefault::Free,
            rules: Vec::new(),
            up_cross_min: 3,
            up_cross_slack: 5,
            side_cross_min: 3,
            side_cross_slack: 5,
            passage_inset_px: 24,
            edge_holdback_px: 0,
            seam_tolerance_px: 2,
            safety_escapes: true,
            deliberate_cross: true,
            descent_routing: false,
            mode: TopMode::AutoTop,
            manual_monitors: Vec::new(),
            excluded_devices: Vec::new(),
        }
    }
}

/// A live monitor as the platform layer reports it (C# `MonitorInfo`). `index` is the 1-based
/// MouseFence number after Left, Top ordering (C# `SortAndNumber`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonitorInfo {
    /// GDI display name (`\\.\DISPLAYn`); renumber-prone.
    pub device: String,
    /// Per-port device interface path, disambiguated with `#n` suffixes when two monitors share it.
    pub stable_id: String,
    pub bounds: Rect,
    pub primary: bool,
    pub index: u32,
}

/// Compiler input (C# `BoundaryScreen`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screen {
    pub stable_id: String,
    pub bounds: Rect,
    pub primary: bool,
    pub excluded: bool,
}

/// One precompiled directed transition (C# `RuntimeTransition`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeTransition {
    pub from_stable_id: String,
    pub to_stable_id: String,
    pub direction: Direction,
    pub from: Rect,
    pub to: Rect,
    pub corridor_start: i32,
    pub corridor_end: i32,
    pub gap_px: i32,
    pub seam_tolerance_px: i32,
    pub behavior: Behavior,
    pub control: Control,
    pub push_min: i32,
    pub push_slack: i32,
    pub passage_inset_px: i32,
    pub edge_holdback_px: i32,
    pub explicit_rule: bool,
    pub sole_up_safety_escape: bool,
    pub safety_demoted: bool,
    pub sort_order: i32,
    /// Authorized landings per live toggle state, indexed by [`landing_index`].
    pub allowed_landings: [Vec<Rect>; 4],
}

impl RuntimeTransition {
    pub fn corridor_width(&self) -> i32 {
        self.corridor_end - self.corridor_start
    }
    pub fn allowed_landings_for(&self, gates_enabled: bool, side_contain: bool) -> &[Rect] {
        &self.allowed_landings[landing_index(gates_enabled, side_contain)]
    }
}

/// Index into `RuntimeTransition::allowed_landings`: 0 = up open + side on, 1 = up open + side off,
/// 2 = up closed + side on, 3 = up closed + side off.
pub const fn landing_index(gates_enabled: bool, side_contain: bool) -> usize {
    match (gates_enabled, side_contain) {
        (true, true) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (false, false) => 3,
    }
}

/// A screen whose every exit is blocked (C# `BoundaryValidationIssue`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealIssue {
    pub stable_id: String,
    pub focus_to_stable_id: String,
    pub focus_direction: Direction,
    pub focus_corridor_width: i32,
}

/// C# `BoundaryCompileResult`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompileResult {
    pub transitions: Vec<RuntimeTransition>,
    pub sealed_screens: Vec<SealIssue>,
}

impl CompileResult {
    pub fn has_safety_demotions(&self) -> bool {
        self.transitions.iter().any(|t| t.safety_demoted)
    }
}

/// Everything `MouseGuard.Configure` hands to `GuardCore` (C#), compiled off the hook thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineConfig {
    pub intent: IntentConfig,
    pub has_top: bool,
    pub barrier_y: i32,
    pub gates: Vec<(i32, i32)>,
    pub monitors: Vec<Rect>,
    pub transitions: Vec<RuntimeTransition>,
    pub descent_routes: Vec<(Rect, Rect)>,
    pub forbidden: Vec<Rect>,
    pub has_sides: bool,
    pub main: Rect,
    pub deliberate_cross: bool,
    pub descent_routing: bool,
    /// GuardCore.CrossMinUp: 3 in 1.1.1 and never set from settings (UpCrossMin only reaches transitions).
    pub cross_min_up: i32,
    /// GuardCore.CrossSlack: 5 in 1.1.1, never set from settings.
    pub cross_slack: i32,
    pub side_cross_min: i32,
    pub side_cross_slack: i32,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            intent: IntentConfig::default(),
            has_top: false,
            barrier_y: 0,
            gates: Vec::new(),
            monitors: Vec::new(),
            transitions: Vec::new(),
            descent_routes: Vec::new(),
            forbidden: Vec::new(),
            has_sides: false,
            main: Rect::default(),
            deliberate_cross: true,
            descent_routing: false,
            cross_min_up: 3,
            cross_slack: 5,
            side_cross_min: 3,
            side_cross_slack: 5,
        }
    }
}

/// What the tray needs to know after a (re)configure (C# balloons in `TrayApplicationContext.Configure`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanReport {
    pub has_safety_demotions: bool,
    /// A present, requested exclusion was refused (primary, or the keep-one rule).
    pub pruned: bool,
    /// Isolated usable screens (C# `AntiTrap` warn list), as stable ids, in monitor order.
    pub isolated: Vec<String>,
    /// Same screens as human labels ("2" or "2 — Name"), built by the caller with the names map.
    pub isolated_indices: Vec<u32>,
    pub sealed: Vec<SealIssue>,
    pub excluded: Vec<String>,
}

/// Live quick toggles read by every decision (C# `Decide(gatesEnabled, sideContain)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Toggles {
    /// Upward passage open (C# `GateOpen` / `gatesEnabled`).
    pub gate_open: bool,
    /// Side protection on (C# `SideContain`).
    pub side_contain: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Pass,
    Block,
}

/// One decision. For `Block`, `(x, y)` is where the cursor is placed; for `Pass` it is the target itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    pub action: Action,
    pub x: i32,
    pub y: i32,
}

/// A physical report with the Windows millisecond tick and monotonic callback-entry time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MoveEvent {
    pub x: i32,
    pub y: i32,
    pub time_ms: u32,
    pub qpc_us: u64,
}

impl MoveEvent {
    pub const fn untimed(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            time_ms: 0,
            qpc_us: 0,
        }
    }
}

/// The preallocated hook-to-tray edge record. 0=pass, 1=held, 2=actual guarded crossing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EdgeRecord {
    /// angle millidegrees, speed counts/s, trend permille, approach counts,
    /// pressure counts, contact ms, buttons, reason, direction, raw age ms, motion x, motion y.
    /// For action 3 motion is raw dx/dy. With model_state bit 128 it is the accepted screen position.
    pub features: [i32; 12],
    pub qpc_us: u64,
    pub time_ms: u32,
    pub x: i32,
    pub y: i32,
    pub action: u8,
    pub model_state: u8,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EdgeState {
    pub features: [i32; 12],
    pub near: bool,
    pub crossed: bool,
    pub attempt_crossed: bool,
    pub model_state: u8,
    pub estimated_rate: f64,
}

/// Cumulative fixed-storage hook counters. The percentile is a histogram upper bound.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HookMetrics {
    pub callbacks: u64,
    pub sampled_steps: u64,
    pub max_step_us: u64,
    pub p99_upper_us: u64,
    pub reinstall_attempts: u64,
    pub panics: u64,
    pub degraded: u64,
    pub trace_dropped: u64,
}

/// The decision engine as the hook thread sees it. Track A implements it (fence-engine `ClassicEngine`);
/// Track C drives it from the hook and tests with fakes. Must not allocate, lock, log or do I/O.
pub trait Decider: Send {
    /// Relative device counts, never cursor pixels. Default for legacy/test deciders.
    fn raw_motion(&mut self, _dx: i32, _dy: i32, _buttons: u16, _qpc_us: u64) {}
    /// C# `GuardCore.NeedsProcessing`; when false the hook passes the event without calling `decide`.
    fn needs_processing(&self) -> bool;
    /// C# `GuardCore.Decide` for a non-injected WM_MOUSEMOVE to `(x, y)`. Mutates the last-position state.
    fn decide(&mut self, event: MoveEvent, toggles: Toggles) -> Decision;
    /// Replace the origin with a physical cursor observation, never a requested clamp.
    fn observe(&mut self, x: i32, y: i32) {
        self.set_last(x, y);
    }
    /// Test deciders may ignore origin state; the production engine overrides this.
    fn set_last(&mut self, _x: i32, _y: i32) {}
    fn edge_state(&self) -> EdgeState {
        EdgeState::default()
    }
    /// C# `GuardCore.Reset` (HaveLast = false, OnTop = false).
    fn reset(&mut self);
    /// Game mode (C# `GuardCore.Confine`).
    fn set_confine(&mut self, on: bool);
    fn confine(&self) -> bool;
}

/// C# `HookHealthVerdict`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookHealthVerdict {
    Alive,
    Inconclusive,
    Dead,
}

/// A global hotkey chord (Windows virtual-key code plus modifiers). `vk == 0` means "none".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Hotkey {
    pub vk: u32,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

impl Hotkey {
    /// Win32 `MOD_*` bits (MOD_ALT 1, MOD_CONTROL 2, MOD_SHIFT 4, MOD_WIN 8), C# `Settings.Modifiers()`.
    pub const fn modifiers(&self) -> u32 {
        (if self.alt { 0x1 } else { 0 })
            | (if self.ctrl { 0x2 } else { 0 })
            | (if self.shift { 0x4 } else { 0 })
            | (if self.win { 0x8 } else { 0 })
    }
    /// C# "a key was set": key not None and at least one modifier.
    pub const fn is_set(&self) -> bool {
        self.vk != 0 && self.modifiers() != 0
    }
}

/// The four live switches the tray owns (C# `MouseGuard.GateOpen/SideContain/Confine` plus `Enabled`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeState {
    pub gate_open: bool,
    pub side_contain: bool,
    pub confine: bool,
    pub paused: bool,
    /// False while running means the hook is down (C# honesty gate: `Enabled && !HookInstalled`).
    pub hook_installed: bool,
}

/// The tray <-> settings-process contract (PLAN T16). The tray starts `mousefence.exe --settings --state ...`
/// (one settings process at a time), the settings process saves the v2 settings file itself and exits with
/// `EXIT_SAVED`; the tray then reloads the file and re-applies hotkeys, language, autostart and the plan, as
/// C# `OpenSettings` did after `ShowDialog() == OK`. While open, the settings process re-reads the live
/// switches from the state file the tray rewrites on every change (`%LOCALAPPDATA%\MouseFence\v2\state.txt`).
pub mod launch {
    use super::RuntimeState;

    pub const SETTINGS_FLAG: &str = "--settings";
    pub const STATE_FLAG: &str = "--state";
    pub const STATE_FILE_NAME: &str = "state.txt";
    /// Settings saved; the tray reloads them.
    pub const EXIT_SAVED: i32 = 0;
    /// Closed without saving (C# DialogResult.Cancel).
    pub const EXIT_CANCELLED: i32 = 1;
    /// The settings process failed; the tray keeps its settings and shows a balloon.
    pub const EXIT_FAILED: i32 = 2;

    fn b(v: bool) -> char {
        if v { '1' } else { '0' }
    }

    /// `gate=1,side=1,confine=0,paused=0,hook=1`
    pub fn state_to_text(s: &RuntimeState) -> String {
        format!(
            "gate={},side={},confine={},paused={},hook={}",
            b(s.gate_open),
            b(s.side_contain),
            b(s.confine),
            b(s.paused),
            b(s.hook_installed)
        )
    }

    /// Parses `state_to_text` output (whitespace, newlines and key order are tolerated; every key required).
    pub fn state_from_text(text: &str) -> Option<RuntimeState> {
        let mut vals = [None::<bool>; 5];
        for part in text.split([',', '\n', '\r']).map(str::trim).filter(|p| !p.is_empty()) {
            let (k, v) = part.split_once('=')?;
            let v = match v.trim() {
                "1" => true,
                "0" => false,
                _ => return None,
            };
            let slot = match k.trim() {
                "gate" => 0,
                "side" => 1,
                "confine" => 2,
                "paused" => 3,
                "hook" => 4,
                _ => return None,
            };
            vals[slot] = Some(v);
        }
        Some(RuntimeState {
            gate_open: vals[0]?,
            side_contain: vals[1]?,
            confine: vals[2]?,
            paused: vals[3]?,
            hook_installed: vals[4]?,
        })
    }

    /// Arguments after the exe path for the settings process.
    pub fn settings_args(s: &RuntimeState) -> Vec<String> {
        vec![SETTINGS_FLAG.to_string(), STATE_FLAG.to_string(), state_to_text(s)]
    }

    /// `Some(state)` when `args` (without the exe path) ask for the settings window.
    pub fn parse_settings_args(args: &[String]) -> Option<RuntimeState> {
        if args.first().map(String::as_str) != Some(SETTINGS_FLAG) {
            return None;
        }
        let pos = args.iter().position(|a| a == STATE_FLAG)?;
        state_from_text(args.get(pos + 1)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s_types_launch_state_round_trips() {
        for bits in 0u8..32 {
            let s = RuntimeState {
                gate_open: bits & 1 != 0,
                side_contain: bits & 2 != 0,
                confine: bits & 4 != 0,
                paused: bits & 8 != 0,
                hook_installed: bits & 16 != 0,
            };
            assert_eq!(launch::state_from_text(&launch::state_to_text(&s)), Some(s));
            assert_eq!(launch::parse_settings_args(&launch::settings_args(&s)), Some(s));
        }
        assert_eq!(launch::state_from_text("gate=1,side=1"), None, "every key is required");
        assert_eq!(launch::state_from_text("gate=2,side=1,confine=0,paused=0,hook=1"), None);
        assert_eq!(launch::parse_settings_args(&["--identify".to_string()]), None);
    }

    #[test]
    fn s_types_rect_is_half_open() {
        let r = Rect::new(0, 0, 10, 5);
        assert!(r.contains(0, 0));
        assert!(r.contains(9, 4));
        assert!(!r.contains(10, 0));
        assert!(!r.contains(0, 5));
        assert!(!r.contains(-1, 0));
        assert!(r.is_valid());
        assert!(!Rect::new(3, 3, 3, 9).is_valid());
    }

    #[test]
    fn s_types_rect_order_is_ltrb() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(0, 1, 5, 5);
        assert_eq!(a.cmp_ltrb(&b), Ordering::Less);
        assert_eq!(b.cmp_ltrb(&a), Ordering::Greater);
        assert_eq!(a.cmp_ltrb(&a), Ordering::Equal);
    }

    #[test]
    fn s_types_landing_index_covers_four_states() {
        let mut seen = [false; 4];
        for g in [true, false] {
            for s in [true, false] {
                seen[landing_index(g, s)] = true;
            }
        }
        assert!(seen.iter().all(|&v| v));
    }

    #[test]
    fn s_types_hotkey_modifier_bits_match_win32() {
        let h = Hotkey {
            vk: 0x26,
            ctrl: true,
            alt: true,
            shift: false,
            win: false,
        };
        assert_eq!(h.modifiers(), 0x3);
        assert!(h.is_set());
        assert!(
            !Hotkey {
                vk: 0x26,
                ..Default::default()
            }
            .is_set()
        );
        assert!(
            !Hotkey {
                vk: 0,
                ctrl: true,
                ..Default::default()
            }
            .is_set()
        );
    }

    #[test]
    fn s_types_policy_default_matches_csharp_settings() {
        let p = Policy::default();
        assert_eq!(
            (p.up_cross_min, p.up_cross_slack, p.side_cross_min, p.side_cross_slack),
            (3, 5, 3, 5)
        );
        assert_eq!(
            (p.passage_inset_px, p.edge_holdback_px, p.seam_tolerance_px),
            (24, 0, 2)
        );
        assert_eq!(p.downward_default, BoundaryDefault::Free);
        assert!(p.safety_escapes && p.deliberate_cross && !p.descent_routing);
    }
}
