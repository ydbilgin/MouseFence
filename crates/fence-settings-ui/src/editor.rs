//! The settings window's behaviour without its drawing: the draft transaction, the boundary drawer, screen names
//! and keep-pointer-off, the Advanced values, shortcut capture and validation, the General page with backup and
//! restore, Save and Cancel. Each method ports the C# `SettingsForm` method of the same name (Ui/SettingsForm*.cs);
//! the egui layer (`crate::ui`) only draws this state and calls these methods.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use fence_config::{CURRENT_SETTINGS_VERSION, Settings, Stored, StoredRule};
use fence_engine::fixes::Fixes;
use fence_types::{
    Behavior, BoundaryDefault, Control, CrossingModel, Direction, MonitorInfo, RuleBehavior, RuntimeState,
    RuntimeTransition, SealIssue,
};

use crate::backend::{Backend, read_version};
use crate::boundary::{self, BoundaryView, contains_id, same};
use crate::text::Tr;

/// C# `BoundaryEndpoint(From, To, Direction, Runtime)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Endpoint {
    pub from: MonitorInfo,
    pub to: MonitorInfo,
    pub direction: Direction,
    pub runtime: Option<RuntimeTransition>,
}

/// C# `ThemedButtonRole`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonRole {
    Primary,
    Secondary,
    Quiet,
    DangerText,
}

/// The three choices a tile offers (C# `BoundaryBehavior.Free/Push/Blocked`).
pub const TILE_BEHAVIORS: [Behavior; 3] = [Behavior::Free, Behavior::Push, Behavior::Blocked];

/// One "Set each direction separately" row (C# `AddDirectionRow`).
#[derive(Clone, Debug, PartialEq)]
pub struct SeparateRow {
    pub label: String,
    pub endpoint: Endpoint,
    pub selected: [bool; 3],
}

/// The drawer's controls as C# `UpdateDrawer` leaves them.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawerControls {
    pub from_chip: String,
    pub to_chip: String,
    pub separator: &'static str,
    pub meta: String,
    pub direction_caption: String,
    pub return_line: String,
    pub return_enabled: bool,
    pub separate_link_text: String,
    pub separate_link_enabled: bool,
    pub separate_panel_enabled: bool,
    pub tiles_enabled: bool,
    pub tiles_selected: [bool; 3],
    pub overall_text: String,
    pub overall_enabled: bool,
    pub when_enabled: bool,
    pub automatic_checked: bool,
    pub always_checked: bool,
    pub radios_enabled: bool,
    pub allow_enabled: bool,
    pub allow_role: ButtonRole,
    pub allow_description: String,
    pub status: String,
    pub separate_rows: Vec<SeparateRow>,
}

impl Default for DrawerControls {
    fn default() -> Self {
        Self {
            from_chip: String::new(),
            to_chip: String::new(),
            separator: "↔",
            meta: String::new(),
            direction_caption: String::new(),
            return_line: String::new(),
            return_enabled: true,
            separate_link_text: String::new(),
            separate_link_enabled: true,
            separate_panel_enabled: true,
            tiles_enabled: true,
            tiles_selected: [false; 3],
            overall_text: String::new(),
            overall_enabled: true,
            when_enabled: true,
            automatic_checked: true,
            always_checked: false,
            radios_enabled: true,
            allow_enabled: false,
            allow_role: ButtonRole::Quiet,
            allow_description: String::new(),
            status: String::new(),
            separate_rows: Vec::new(),
        }
    }
}

/// The footer's status label and its "Open that boundary" action (C# `_inlineStatus`, `_inlineAction`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InlineStatus {
    pub text: String,
    pub error: bool,
    pub action: bool,
}

/// The General page controls that C# only copies into the draft on Save/Export (`SyncSimpleControls`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneralControls {
    pub start_gate: bool,
    pub start_side: bool,
    pub auto_start: bool,
    pub start_confine: bool,
    pub start_paused: bool,
    /// 0 Automatic, 1 English, 2 Turkish.
    pub language: usize,
    /// 0 System, 1 Light, 2 Dark.
    pub theme: usize,
}

impl GeneralControls {
    fn from_draft(d: &Settings) -> Self {
        Self {
            start_gate: d.start_gate_closed,
            start_side: d.start_side_contain_on,
            auto_start: d.auto_start,
            start_confine: d.start_confine_on,
            start_paused: d.start_paused,
            language: match d.language.as_str() {
                "en" => 1,
                "tr" => 2,
                _ => 0,
            },
            theme: theme_index(&d.theme),
        }
    }
}

pub fn theme_index(theme: &str) -> usize {
    match theme {
        "Light" => 1,
        "Dark" => 2,
        _ => 0,
    }
}

pub fn theme_name(index: usize) -> &'static str {
    match index {
        1 => "Light",
        2 => "Dark",
        _ => "System",
    }
}

/// How the window ended (C# `DialogResult` plus `Result`).
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Open,
    /// Save, or a confirmed restore: the settings to persist (C# `Result`, `DialogResult.OK`).
    Saved(Box<Settings>),
    Cancelled,
}

/// The file dialogs and the restore confirmation (C# `SaveFileDialog`, `OpenFileDialog`, `MessageBox`), injected
/// so tests run with temp paths and no window (C# `RestorePathPickerForTests` / `RestoreConfirmForTests`).
pub trait Dialogs {
    fn pick_backup_path(&mut self, title: &str, filter: &str) -> Option<PathBuf>;
    fn pick_restore_path(&mut self, title: &str, filter: &str) -> Option<PathBuf>;
    fn confirm_restore(&mut self, text: &str, caption: &str) -> bool;
}

/// Four legacy shortcut rows plus the local accidental-crossing marker.
pub const SHORTCUT_HEADS: [&str; 5] = [
    "HotkeyHead",
    "GameHotkeyHead",
    "PauseHotkeyHead",
    "SideHotkeyHead",
    "MarkAccidentHotkeyHead",
];
pub const SHORTCUT_HINTS: [&str; 5] = [
    "HotkeyHint",
    "GameHotkeyHint",
    "PauseHotkeyHint",
    "SideHotkeyHint",
    "MarkAccidentHotkeyHint",
];

/// Windows virtual-key codes of the modifier keys (C# `Keys.ControlKey`, `Menu`, `ShiftKey`, `LWin`, `RWin`).
pub const VK_SHIFT: u32 = 0x10;
pub const VK_CONTROL: u32 = 0x11;
pub const VK_MENU: u32 = 0x12;
pub const VK_LWIN: u32 = 0x5B;
pub const VK_RWIN: u32 = 0x5C;

pub struct Editor {
    pub fixes: Fixes,
    pub draft: Settings,
    pub monitors: Vec<MonitorInfo>,
    backend: Rc<dyn Backend>,
    pub tr: Tr,
    /// 0 Displays, 1 Advanced, 2 Shortcuts, 3 General.
    pub tab: usize,
    pub boundaries: Vec<BoundaryView>,
    /// The map's selection (C# `SelectedBoundary` / `SelectedStableId`).
    pub map_selected_boundary: Option<usize>,
    pub map_selected_monitor: Option<String>,
    /// The boundary the drawer edits (C# `_boundary`) and its endpoints.
    pub boundary: Option<usize>,
    pub primary: Option<Endpoint>,
    pub reverse: Option<Endpoint>,
    /// C# `_screenSplit.Visible`.
    pub drawer_visible: bool,
    pub drawer: DrawerControls,
    pub separate_expanded: bool,
    pub when_expanded: bool,
    pub advanced_expanded: bool,
    pub more_startup_expanded: bool,
    /// Preview mode of the docs capture (C# `hideScreenList`).
    pub screen_list_hidden: bool,
    /// The Advanced overview capture (C# `OpenAdvancedOverviewForPreview`) hides everything after the top choice.
    pub advanced_overview_only: bool,
    pub inline: InlineStatus,
    pending_issue: Option<SealIssue>,
    pub hotkey_errors: [bool; 5],
    pub hotkey_error_text: [String; 5],
    pub general: GeneralControls,
    /// The live switches the tray reports (C# `_runtimeState`), `None` when nothing reports them.
    pub live: Option<RuntimeState>,
    pub outcome: Outcome,
    pub map_hint: String,
    /// Bumped when C# would rebuild the screen rows (their text boxes reload from the draft).
    pub rows_version: u64,
}

impl Editor {
    /// C# `new SettingsForm(settings, monitors, runtimeState)`: the draft is a normalized deep copy; the caller's
    /// settings are never touched.
    pub fn new(
        live_settings: &Settings,
        monitors: Vec<MonitorInfo>,
        backend: Rc<dyn Backend>,
        tr: Tr,
        live: Option<RuntimeState>,
    ) -> Self {
        Self::new_with_fixes(live_settings, monitors, backend, tr, live, Fixes::ALL)
    }

    pub fn new_with_fixes(
        live_settings: &Settings,
        monitors: Vec<MonitorInfo>,
        backend: Rc<dyn Backend>,
        tr: Tr,
        live: Option<RuntimeState>,
        fixes: Fixes,
    ) -> Self {
        let mut draft = live_settings.clone();
        backend.normalize(&mut draft);
        let tip = !draft.boundary_tip_seen && draft.boundary_rules.is_empty();
        let general = GeneralControls::from_draft(&draft);
        let mut e = Self {
            fixes,
            draft,
            monitors,
            backend,
            tr,
            tab: 0,
            boundaries: Vec::new(),
            map_selected_boundary: None,
            map_selected_monitor: None,
            boundary: None,
            primary: None,
            reverse: None,
            drawer_visible: false,
            drawer: DrawerControls::default(),
            separate_expanded: false,
            when_expanded: false,
            advanced_expanded: false,
            more_startup_expanded: false,
            screen_list_hidden: false,
            advanced_overview_only: false,
            inline: InlineStatus::default(),
            pending_issue: None,
            hotkey_errors: [false; 5],
            hotkey_error_text: Default::default(),
            general,
            live,
            outcome: Outcome::Open,
            map_hint: String::new(),
            rows_version: 0,
        };
        e.inline.text = e.tr.t(if tip { "BoundaryTip" } else { "UnsavedFooter" });
        e.map_hint = e.tr.t(if e.usable_monitor_count() <= 1 {
            "MapOnlyOne"
        } else {
            "MapHintV2"
        });
        e.refresh_map();
        e.update_shortcut_errors();
        if e.fixes.j11_segment_and_identity && e.monitors.iter().any(|m| m.stable_id.contains("#ambiguous:")) {
            e.show_inline(&e.tr.t("AmbiguousDisplayIdentity"), false);
        }
        e
    }

    pub fn backend(&self) -> &dyn Backend {
        &*self.backend
    }

    pub fn usable_monitor_count(&self) -> usize {
        self.monitors
            .iter()
            .filter(|m| !contains_id(&self.draft.excluded_devices, &m.stable_id))
            .count()
    }

    pub fn is_excluded(&self, id: &str) -> bool {
        contains_id(&self.draft.excluded_devices, id)
    }

    // ---------------------------------------------------------------- names

    /// C# `Settings.MonitorNameFor(names, id)`: case-insensitive lookup, normalized, or "".
    pub fn monitor_name_for(&self, id: &str) -> String {
        self.draft
            .monitor_names
            .iter()
            .find(|(k, _)| same(k, id))
            .map(|(_, v)| self.backend.normalize_monitor_name(v))
            .unwrap_or_default()
    }

    /// C# `ScreenName(m)`: the nickname, else "Main" for the primary, else "Screen n".
    pub fn screen_name(&self, m: &MonitorInfo) -> String {
        let name = self.monitor_name_for(&m.stable_id);
        if !name.is_empty() {
            name
        } else if m.primary {
            self.tr.t("MainWord")
        } else {
            self.tr.f("DefaultScreenName", &[&m.index.to_string()])
        }
    }

    pub fn screen_name_by_id(&self, id: &str) -> String {
        self.monitors
            .iter()
            .find(|m| same(&m.stable_id, id))
            .map(|m| self.screen_name(m))
            .unwrap_or_default()
    }

    /// C# `ChipText(m)`.
    pub fn chip_text(&self, m: &MonitorInfo) -> String {
        format!("{} · {}", m.index, self.screen_name(m))
    }

    /// The name box of one screen row changed (C# `name.TextChanged`).
    pub fn set_monitor_name(&mut self, id: &str, raw: &str) {
        if self.fixes.j11_segment_and_identity && id.contains("#ambiguous:") {
            return;
        }
        let clean = self.backend.normalize_monitor_name(raw);
        self.draft.monitor_names.retain(|(k, _)| !same(k, id));
        if !clean.is_empty() {
            self.draft.monitor_names.push((id.to_string(), clean));
        }
        self.refresh_map();
    }

    /// One screen row's "Keep pointer off" box changed (C# `check.CheckedChanged`).
    pub fn set_keep_out(&mut self, id: &str, keep_out: bool) {
        if self.fixes.j11_segment_and_identity && id.contains("#ambiguous:") {
            return;
        }
        if keep_out {
            self.draft.excluded_devices.push(id.to_string());
        } else {
            self.draft.excluded_devices.retain(|x| !same(x, id));
        }
        self.sanitize_excluded();
        self.refresh_map();
        self.rebuild_screen_rows();
        self.update_drawer();
    }

    /// C# `SanitizeExcluded()`.
    pub fn sanitize_excluded(&mut self) {
        let ids: Vec<String> = self.monitors.iter().map(|m| m.stable_id.clone()).collect();
        let primary = self
            .monitors
            .iter()
            .find(|m| m.primary)
            .or(self.monitors.first())
            .map(|m| m.stable_id.clone());
        let mut requested: Vec<String> = Vec::new();
        for id in &self.draft.excluded_devices {
            if !contains_id(&requested, id) {
                requested.push(id.clone());
            }
        }
        self.draft.excluded_devices = self.backend.sanitize_excluded(&ids, primary.as_deref(), &requested);
    }

    fn rebuild_screen_rows(&mut self) {
        self.rows_version += 1;
    }

    // ---------------------------------------------------------------- map and drawer

    /// C# `RefreshMap()`: recompile the map's boundaries and keep the drawer on the same boundary.
    pub fn refresh_map(&mut self) {
        let compiled = self.backend.compile(&self.draft, &self.monitors);
        let previous = self.map_selected_boundary.and_then(|i| self.boundaries.get(i).cloned());
        self.boundaries = boundary::build_boundaries(&self.monitors, &self.draft, &compiled);
        self.map_selected_boundary = previous.and_then(|p| self.boundaries.iter().position(|b| b.same_pair(&p)));
        if self.usable_monitor_count() > 1 {
            self.map_hint = self.tr.t("MapHintV2");
            self.update_boundary_tip_visibility();
        }
        if self.boundary.is_some() {
            let p = self.primary.clone().expect("a drawer boundary has endpoints");
            self.boundary = boundary::find_boundary(&self.boundaries, &p.from.stable_id, &p.to.stable_id, p.direction);
            if self.boundary.is_some() {
                self.configure_endpoints();
                self.map_selected_boundary = self.boundary;
            }
        }
    }

    pub fn current_boundary(&self) -> Option<&BoundaryView> {
        self.boundary.and_then(|i| self.boundaries.get(i))
    }

    /// C# `OpenBoundary(boundary)`.
    pub fn open_boundary(&mut self, index: usize) {
        if index >= self.boundaries.len() {
            return;
        }
        self.boundary = Some(index);
        self.map_selected_boundary = Some(index);
        self.map_selected_monitor = None;
        self.configure_endpoints();
        self.draft.boundary_tip_seen = true;
        if self.inline.text == self.tr.t("BoundaryTip") {
            self.set_inline_status(&self.tr.t("UnsavedFooter"), false, false);
        }
        self.drawer_visible = true;
        self.update_drawer();
    }

    /// C# `OpenFirstBoundaryForPreview(hideScreenList)`.
    pub fn open_first_boundary_for_preview(&mut self, hide_screen_list: bool) {
        if self.boundaries.is_empty() {
            return;
        }
        self.select_tab(0);
        self.open_boundary(0);
        if hide_screen_list {
            self.screen_list_hidden = true;
        }
    }

    /// C# `ConfigureEndpoints()`: vertical boundaries edit the upward direction first.
    fn configure_endpoints(&mut self) {
        let Some(b) = self.current_boundary().cloned() else {
            return;
        };
        let ep = |from: &MonitorInfo, to: &MonitorInfo, direction: Direction, runtime: &Option<RuntimeTransition>| {
            Endpoint {
                from: from.clone(),
                to: to.clone(),
                direction,
                runtime: runtime.clone(),
            }
        };
        if b.vertical_connection() {
            let first_up = b.first_to_second_direction == Direction::Up;
            if first_up {
                self.primary = Some(ep(&b.first, &b.second, Direction::Up, &b.first_to_second));
                self.reverse = Some(ep(&b.second, &b.first, Direction::Down, &b.second_to_first));
            } else {
                self.primary = Some(ep(&b.second, &b.first, Direction::Up, &b.second_to_first));
                self.reverse = Some(ep(&b.first, &b.second, Direction::Down, &b.first_to_second));
            }
        } else {
            self.primary = Some(ep(&b.first, &b.second, b.first_to_second_direction, &b.first_to_second));
            self.reverse = Some(ep(
                &b.second,
                &b.first,
                b.second_to_first_direction(),
                &b.second_to_first,
            ));
        }
    }

    fn rule_matches(r: &StoredRule, e: &Endpoint) -> bool {
        same(&r.from_stable_id, &e.from.stable_id)
            && same(&r.to_stable_id, &e.to.stable_id)
            && r.direction == Stored::Known(e.direction)
    }

    /// C# `FindRule(e)`: the last saved rule for this ordered pair and direction.
    pub fn find_rule(&self, e: &Endpoint) -> Option<&StoredRule> {
        self.draft
            .boundary_rules
            .iter()
            .rev()
            .find(|r| Self::rule_matches(r, e))
    }

    fn remove_rule(&mut self, e: &Endpoint) {
        self.draft.boundary_rules.retain(|r| !Self::rule_matches(r, e));
    }

    /// C# `ResolvedBehavior(e)`: the saved choice, else what the compiler resolved, else Free.
    pub fn resolved_behavior(&self, e: &Endpoint) -> Behavior {
        match self.find_rule(e).map(|r| &r.behavior) {
            Some(Stored::Known(RuleBehavior::OverallFeel)) | Some(Stored::Known(RuleBehavior::Push)) => Behavior::Push,
            Some(Stored::Known(RuleBehavior::Free)) => Behavior::Free,
            Some(Stored::Known(RuleBehavior::Blocked)) => Behavior::Blocked,
            _ => e.runtime.as_ref().map_or(Behavior::Free, |t| t.behavior),
        }
    }

    fn rule_control(&self, e: &Endpoint) -> Option<Stored<Control>> {
        self.find_rule(e).map(|r| r.control.clone())
    }

    /// C# `DirectionsDiffer()`.
    fn directions_differ(&self) -> bool {
        let (Some(p), Some(r)) = (&self.primary, &self.reverse) else {
            return false;
        };
        self.resolved_behavior(p) != self.resolved_behavior(r) || self.rule_control(p) != self.rule_control(r)
    }

    /// C# `ExcludedBoundaryTarget()`.
    pub fn excluded_boundary_target(&self) -> Option<MonitorInfo> {
        let p = self.primary.as_ref()?;
        self.current_boundary()?;
        if self.is_excluded(&p.to.stable_id) {
            return Some(p.to.clone());
        }
        if self.is_excluded(&p.from.stable_id) {
            return Some(p.from.clone());
        }
        None
    }

    fn behavior_title(&self, b: Behavior) -> String {
        self.tr.t(match b {
            Behavior::Blocked => "NeverCrosses",
            Behavior::Push => "NeedsFirmPush",
            Behavior::Free => "CrossesFreely",
        })
    }

    /// C# `UpdateDrawer()`.
    pub fn update_drawer(&mut self) {
        let Some(b) = self.current_boundary().cloned() else {
            return;
        };
        let (Some(primary), Some(reverse)) = (self.primary.clone(), self.reverse.clone()) else {
            return;
        };
        let mut d = std::mem::take(&mut self.drawer);
        d.from_chip = self.chip_text(&primary.from);
        d.to_chip = self.chip_text(&primary.to);
        d.separator = if b.vertical_connection() { "↕" } else { "↔" };
        d.meta = if b.gap_px > self.draft.seam_tolerance_px {
            self.tr.f("EditorMeetGap", &[&b.gap_px.to_string()])
        } else {
            self.tr.t("EditorMeetTouching")
        };
        let horizontal = !b.vertical_connection();
        let split_truth = horizontal && self.directions_differ();
        if split_truth {
            self.separate_expanded = true;
        }
        d.direction_caption = if split_truth {
            self.tr.t("DifferentByDirection")
        } else if horizontal {
            self.tr.t("BothDirections")
        } else {
            self.tr.f("GoingUpInto", &[&self.screen_name(&primary.to)])
        };
        d.return_line = if horizontal {
            String::new()
        } else {
            self.tr.t(match self.resolved_behavior(&reverse) {
                Behavior::Blocked => "ComingDownBlocked",
                Behavior::Push => "ComingDownPush",
                Behavior::Free => "ComingDownFree",
            })
        };
        if !horizontal {
            self.separate_expanded = false;
        }
        d.separate_link_text = format!(
            "{}{}",
            if self.separate_expanded { "⌄  " } else { "›  " },
            self.tr.t(if split_truth {
                "DifferentByDirection"
            } else {
                "SetDirectionsSeparately"
            })
        );

        let excluded_target = self.excluded_boundary_target();
        let excluded = excluded_target.is_some();
        let ambiguous = self.fixes.j11_segment_and_identity
            && (primary.from.stable_id.contains("#ambiguous:") || primary.to.stable_id.contains("#ambiguous:"));
        let editable = !excluded && !ambiguous;
        d.allow_enabled = excluded && !ambiguous;
        d.allow_role = if excluded {
            ButtonRole::Primary
        } else {
            ButtonRole::Quiet
        };
        d.allow_description = match &excluded_target {
            Some(m) => self.tr.f("ExcludedTargetExplanation", &[&self.screen_name(m)]),
            None => String::new(),
        };
        d.tiles_enabled = editable;
        d.separate_link_enabled = horizontal && editable;
        d.separate_panel_enabled = editable;
        d.when_enabled = editable;
        d.radios_enabled = editable;
        d.return_enabled = editable;
        if excluded {
            d.direction_caption = self.tr.t("BlockedByKeepOut");
        }

        let selected = self.resolved_behavior(&primary);
        let show = editable && !split_truth;
        d.tiles_selected = [
            show && selected == Behavior::Free,
            show && selected == Behavior::Push,
            show && selected == Behavior::Blocked,
        ];
        let primary_rule = self.find_rule(&primary).cloned();
        d.overall_text = self.tr.t(
            if self.fixes.j15_side_to_top_blocked
                && primary_rule.is_none()
                && selected == Behavior::Blocked
                && primary.direction == Direction::Up
                && !primary.from.primary
                && self.draft.upward_boundary_default == Stored::Known(BoundaryDefault::Automatic)
            {
                "AutomaticBlocked"
            } else if primary_rule.is_none() {
                "UsingOverallBehavior"
            } else {
                "UseOverallBehavior"
            },
        );
        d.overall_enabled = primary_rule.is_some() && editable;
        let control = match primary_rule.as_ref().map(|r| &r.control) {
            Some(Stored::Known(c)) => Some(*c),
            Some(Stored::Unknown) => None,
            None => Some(Control::Automatic),
        };
        d.automatic_checked = control == Some(Control::Automatic);
        d.always_checked = control == Some(Control::Always);
        self.drawer = d;
        self.build_separate_rows(!editable);
        self.update_live_status();
    }

    /// C# `BuildSeparateRows(suppressSelection)`.
    fn build_separate_rows(&mut self, suppress_selection: bool) {
        self.drawer.separate_rows.clear();
        if !self.separate_expanded {
            return;
        }
        let (Some(p), Some(r)) = (self.primary.clone(), self.reverse.clone()) else {
            return;
        };
        for e in [p, r] {
            let label = self.tr.f(
                "DirectionFromTo",
                &[&self.screen_name(&e.from), &self.screen_name(&e.to)],
            );
            let resolved = self.resolved_behavior(&e);
            let selected = TILE_BEHAVIORS.map(|b| !suppress_selection && resolved == b);
            self.drawer.separate_rows.push(SeparateRow {
                label,
                endpoint: e,
                selected,
            });
        }
    }

    /// The main tiles' titles and help texts (C# `NewModeTile` in the constructor).
    pub fn tile_texts(&self) -> [(String, String); 3] {
        [
            (self.tr.t("CrossesFreely"), self.tr.t("CrossesFreelyHelp")),
            (self.tr.t("NeedsFirmPush"), self.tr.t("NeedsFirmPushHelp")),
            (self.tr.t("NeverCrosses"), self.tr.t("NeverCrossesHelp")),
        ]
    }

    pub fn separate_tile_title(&self, i: usize) -> String {
        self.behavior_title(TILE_BEHAVIORS[i])
    }

    /// A main tile was clicked (C# `ChoosePrimaryBehavior`). Disabled tiles ignore clicks, as WinForms does.
    pub fn choose_primary_behavior(&mut self, behavior: Behavior) {
        if self.boundary.is_none() || !self.drawer.tiles_enabled {
            return;
        }
        let Some(b) = self.current_boundary().cloned() else {
            return;
        };
        let (Some(p), Some(r)) = (self.primary.clone(), self.reverse.clone()) else {
            return;
        };
        self.set_endpoint_behavior(&p, behavior);
        if !b.vertical_connection() {
            self.set_endpoint_behavior(&r, behavior);
            self.separate_expanded = false;
        } else if self.find_rule(&r).is_none() {
            self.set_endpoint_behavior(&r, Behavior::Free);
        }
        self.refresh_map();
        self.update_drawer();
    }

    /// A tile of one separate-direction row was clicked (C# `AddDirectionRow` tile click).
    pub fn choose_separate_behavior(&mut self, row: usize, behavior: Behavior) {
        if !self.drawer.separate_panel_enabled {
            return;
        }
        let Some(e) = self.drawer.separate_rows.get(row).map(|r| r.endpoint.clone()) else {
            return;
        };
        self.set_endpoint_behavior(&e, behavior);
        self.refresh_map();
        self.update_drawer();
    }

    /// C# `SetEndpointBehavior(endpoint, behavior)`.
    fn set_endpoint_behavior(&mut self, e: &Endpoint, behavior: Behavior) {
        if self.fixes.j11_segment_and_identity
            && (e.from.stable_id.contains("#ambiguous:") || e.to.stable_id.contains("#ambiguous:"))
        {
            return;
        }
        let value = match behavior {
            Behavior::Free => RuleBehavior::Free,
            Behavior::Push => RuleBehavior::Push,
            Behavior::Blocked => RuleBehavior::Blocked,
        };
        if let Some(rule) = self
            .draft
            .boundary_rules
            .iter_mut()
            .rev()
            .find(|r| Self::rule_matches(r, e))
        {
            rule.behavior = Stored::Known(value);
            return;
        }
        self.draft.boundary_rules.push(StoredRule {
            from_stable_id: e.from.stable_id.clone(),
            to_stable_id: e.to.stable_id.clone(),
            direction: Stored::Known(e.direction),
            behavior: Stored::Known(value),
            control: Stored::Known(Control::Automatic),
        });
    }

    /// The "When this applies" radios (C# `SetControl`).
    pub fn set_control(&mut self, control: Control) {
        if !self.drawer.radios_enabled {
            return;
        }
        let Some(b) = self.current_boundary().cloned() else {
            return;
        };
        let (Some(p), Some(r)) = (self.primary.clone(), self.reverse.clone()) else {
            return;
        };
        let targets = if !b.vertical_connection() && !self.separate_expanded {
            vec![p, r]
        } else {
            vec![p]
        };
        for e in targets {
            if let Some(rule) = self
                .draft
                .boundary_rules
                .iter_mut()
                .rev()
                .find(|x| Self::rule_matches(x, &e))
            {
                rule.control = Stored::Known(control);
            }
        }
        self.drawer.automatic_checked = control == Control::Automatic;
        self.drawer.always_checked = control == Control::Always;
        self.refresh_map();
        self.update_live_status();
    }

    /// The "Use overall behavior" link (C# `UseOverall`).
    pub fn use_overall(&mut self) {
        if !self.drawer.overall_enabled {
            return;
        }
        let Some(b) = self.current_boundary().cloned() else {
            return;
        };
        let (Some(p), Some(r)) = (self.primary.clone(), self.reverse.clone()) else {
            return;
        };
        self.remove_rule(&p);
        if !b.vertical_connection() && !self.separate_expanded {
            self.remove_rule(&r);
        }
        self.refresh_map();
        self.update_drawer();
    }

    /// C# `ToggleSeparate()`.
    pub fn toggle_separate(&mut self) {
        if !self.drawer.separate_link_enabled {
            return;
        }
        match self.current_boundary() {
            Some(b) if !b.vertical_connection() => {}
            _ => return,
        }
        self.separate_expanded = !self.separate_expanded;
        let suppress = !self.drawer.separate_panel_enabled;
        self.build_separate_rows(suppress);
    }

    /// C# `ToggleWhen()`.
    pub fn toggle_when(&mut self) {
        if !self.drawer.when_enabled {
            return;
        }
        self.when_expanded = !self.when_expanded;
    }

    /// The "Allow this screen" button (C# `AllowBoundaryScreen`).
    pub fn allow_boundary_screen(&mut self) {
        if !self.drawer.allow_enabled {
            return;
        }
        let Some(p) = self.primary.clone() else { return };
        self.draft
            .excluded_devices
            .retain(|id| !same(id, &p.to.stable_id) && !same(id, &p.from.stable_id));
        self.rebuild_screen_rows();
        self.refresh_map();
        self.update_drawer();
    }

    /// C# `UpdateLiveStatus()`: the drawer's status line, shown only when something overrides the chosen tile.
    pub fn update_live_status(&mut self) {
        if self.current_boundary().is_none() {
            return;
        }
        let Some(p) = self.primary.clone() else { return };
        let mut status = String::new();
        if self.fixes.j11_segment_and_identity
            && (p.from.stable_id.contains("#ambiguous:") || p.to.stable_id.contains("#ambiguous:"))
        {
            status = self.tr.t("AmbiguousDisplayIdentity");
        } else if let Some(m) = self.excluded_boundary_target() {
            status = self.tr.f("ExcludedTargetExplanation", &[&self.screen_name(&m)]);
        } else if p.runtime.as_ref().is_some_and(|t| t.safety_demoted) {
            status = self.tr.f("StatusSafety", &[&self.screen_name(&p.from)]);
        } else if let Some(state) = self.live {
            let always = matches!(
                self.find_rule(&p).map(|r| &r.control),
                Some(Stored::Known(Control::Always))
            );
            let chosen = self.resolved_behavior(&p);
            if !always && p.direction == Direction::Up && !state.gate_open && chosen != Behavior::Blocked {
                status = self.tr.t("StatusUpClosed");
            } else if !always
                && matches!(p.direction, Direction::Left | Direction::Right)
                && !state.side_contain
                && chosen != Behavior::Free
            {
                status = self.tr.t("StatusSideOff");
            }
        }
        self.drawer.status = status;
    }

    /// The tray reported new live switches (C# `_runtimeTimer` tick, every 400 ms).
    pub fn set_live(&mut self, state: Option<RuntimeState>) {
        self.live = state;
        self.update_live_status();
    }

    /// C# `CloseDrawer()`.
    pub fn close_drawer(&mut self) {
        if !self.drawer_visible {
            return;
        }
        self.drawer_visible = false;
        self.boundary = None;
        self.map_selected_boundary = None;
    }

    /// C# `MonitorChosen(monitor)`: a click on a screen only selects it.
    pub fn monitor_chosen(&mut self, id: &str) {
        self.map_selected_monitor = Some(id.to_string());
        self.map_selected_boundary = None;
    }

    /// C# `DrawerStatusCandidates()`: every text the status line can show, for its reserved height.
    pub fn drawer_status_candidates(&self) -> Vec<String> {
        let longest = self
            .monitors
            .iter()
            .map(|m| self.screen_name(m))
            .fold(String::new(), |best, n| {
                if n.chars().count() > best.chars().count() {
                    n
                } else {
                    best
                }
            });
        vec![
            self.tr.t("StatusUpClosed"),
            self.tr.t("StatusSideOff"),
            self.tr.f("StatusSafety", &[&longest]),
            self.tr.t("BlockedByKeepOut"),
            self.tr.f("ExcludedTargetExplanation", &[&longest]),
            self.tr.t("SavedInactive"),
            " ".to_string(),
        ]
    }

    // ---------------------------------------------------------------- tabs and status

    /// C# `SelectTab(i)` (and the sidebar buttons).
    pub fn select_tab(&mut self, i: usize) {
        if i < 4 && i != self.tab {
            self.tab = i;
            self.update_boundary_tip_visibility();
        }
    }

    fn show_inline(&mut self, text: &str, action: bool) {
        self.set_inline_status(text, true, action);
    }

    /// C# `SetInlineStatus(text, error, action)`.
    pub fn set_inline_status(&mut self, text: &str, error: bool, action: bool) {
        self.inline = InlineStatus {
            text: text.to_string(),
            error,
            action,
        };
    }

    /// C# `UpdateBoundaryTipVisibility()`.
    fn update_boundary_tip_visibility(&mut self) {
        let tip = self.tr.t("BoundaryTip");
        let showing_tip = self.inline.text == tip;
        if self.tab != 0 {
            if showing_tip {
                self.set_inline_status("", false, false);
            }
            return;
        }
        if self.inline.text.is_empty()
            && !self.draft.boundary_tip_seen
            && self.draft.boundary_rules.is_empty()
            && self.usable_monitor_count() > 1
        {
            self.set_inline_status(&tip, false, false);
        }
    }

    /// The footer's "Open that boundary" action (C# `OpenValidationBoundary`).
    pub fn open_validation_boundary(&mut self) {
        let Some(issue) = self.pending_issue.clone() else {
            return;
        };
        if let Some(i) = boundary::find_boundary(
            &self.boundaries,
            &issue.stable_id,
            &issue.focus_to_stable_id,
            issue.focus_direction,
        ) {
            self.select_tab(0);
            self.open_boundary(i);
        }
    }

    // ---------------------------------------------------------------- Advanced

    /// C# `ToggleAdvanced()`: the "Fine-tune in physical pixels" fold.
    pub fn toggle_advanced(&mut self) {
        self.advanced_expanded = !self.advanced_expanded;
    }

    pub fn advanced_toggle_text(&self) -> String {
        format!(
            "{}{}",
            if self.advanced_expanded { "⌄  " } else { "›  " },
            self.tr.t("FineTunePixels")
        )
    }

    /// C# `OpenAdvancedForPreview()`.
    pub fn open_advanced_for_preview(&mut self) {
        self.select_tab(1);
        self.advanced_expanded = true;
    }

    /// C# `OpenAdvancedOverviewForPreview()`.
    pub fn open_advanced_overview_for_preview(&mut self) {
        self.select_tab(1);
        self.advanced_overview_only = true;
    }

    /// One "Overall behavior" combo (C# `DefaultRow`): `which` 0 up, 1 side, 2 down.
    pub fn set_default(&mut self, which: usize, value: BoundaryDefault) {
        let slot = match which {
            0 => &mut self.draft.upward_boundary_default,
            1 => &mut self.draft.side_boundary_default,
            _ => &mut self.draft.downward_boundary_default,
        };
        *slot = Stored::Known(value);
        self.refresh_map();
    }

    pub fn default_value(&self, which: usize) -> Stored<BoundaryDefault> {
        match which {
            0 => self.draft.upward_boundary_default.clone(),
            1 => self.draft.side_boundary_default.clone(),
            _ => self.draft.downward_boundary_default.clone(),
        }
    }

    /// The numeric rows of the Advanced grid (C# `NumericRow` list): label, help, min, max, default.
    pub fn numeric_specs(&self) -> [NumericSpec; 11] {
        let t = |k: &str| self.tr.t(k);
        [
            NumericSpec::new(t("UpStrength"), t("UpStrengthHelp"), 1, 200, 3),
            NumericSpec::new(t("UpSlack"), t("UpSlackHelp"), 0, 200, 5),
            NumericSpec::new(t("SideStrength"), t("SideStrengthHelp"), 1, 200, 3),
            NumericSpec::new(t("SideSlackM2"), t("SideSlackHelpM2"), 0, 200, 5),
            NumericSpec::new(t("PassageInset"), t("PassageInsetHelp"), 0, 500, 24),
            NumericSpec::new(t("EdgeHoldback"), t("EdgeHoldbackHelpV2"), 0, 200, 0),
            NumericSpec::new(t("SeamTolerance"), t("SeamToleranceHelp"), 0, 200, 2),
            NumericSpec::new(t("UpPushDistance"), t("UpPushDistanceHelp"), 1, 2000, 120),
            NumericSpec::new(t("SidePushDistance"), t("SidePushDistanceHelp"), 1, 2000, 120),
            NumericSpec::new(t("PushAbsorb"), t("PushAbsorbHelp"), 0, 500, 75),
            NumericSpec::new(t("PushPauseReset"), t("PushPauseResetHelp"), 20, 2000, 150),
        ]
    }

    pub fn numeric_value(&self, i: usize) -> i32 {
        let d = &self.draft;
        match i {
            0 => d.up_cross_min,
            1 => d.up_cross_slack,
            2 => d.side_cross_min,
            3 => d.side_cross_slack,
            4 => d.passage_inset_px,
            5 => d.edge_holdback_px,
            6 => d.seam_tolerance_px,
            7 => d.up_push_distance_px,
            8 => d.side_push_distance_px,
            9 => d.push_absorb_ms,
            _ => d.push_pause_reset_ms,
        }
    }

    /// A numeric box changed (C# `number.ValueChanged`): the control range clamps the value.
    pub fn set_numeric(&mut self, i: usize, value: i32) {
        let spec = &self.numeric_specs()[i.min(10)];
        let v = value.clamp(spec.min, spec.max);
        let d = &mut self.draft;
        match i {
            0 => d.up_cross_min = v,
            1 => d.up_cross_slack = v,
            2 => d.side_cross_min = v,
            3 => d.side_cross_slack = v,
            4 => d.passage_inset_px = v,
            5 => d.edge_holdback_px = v,
            6 => d.seam_tolerance_px = v,
            7 => d.up_push_distance_px = v,
            8 => d.side_push_distance_px = v,
            9 => d.push_absorb_ms = v,
            _ => d.push_pause_reset_ms = v,
        }
        self.refresh_map();
    }

    pub fn set_deliberate_cross(&mut self, on: bool) {
        self.draft.deliberate_cross = on;
    }

    pub fn set_crossing_model(&mut self, model: CrossingModel) {
        self.draft.crossing_model = model;
        self.refresh_map();
    }

    pub fn set_edge_trace(&mut self, on: bool) {
        self.draft.edge_trace = on;
    }

    pub fn shortcut_count(&self) -> usize {
        if self.fixes.j16_shortcut_validation { 5 } else { 4 }
    }

    pub fn set_safety_escapes(&mut self, on: bool) {
        self.draft.safety_escapes = on;
        self.refresh_map();
    }

    pub fn set_descent_routing(&mut self, on: bool) {
        self.draft.descent_routing = on;
    }

    /// The upper-group radios (C# `auto` / `manual` CheckedChanged).
    pub fn set_manual_mode(&mut self, manual: bool) {
        self.draft.mode = if manual { "Manual" } else { "AutoTop" }.to_string();
        self.refresh_map();
    }

    pub fn is_manual_mode(&self) -> bool {
        self.draft.mode == "Manual"
    }

    /// One manual upper-group box (C# `check.CheckedChanged` in the manual list).
    pub fn set_manual_monitor(&mut self, id: &str, on: bool) {
        if self.fixes.j11_segment_and_identity && id.contains("#ambiguous:") {
            return;
        }
        if on {
            if !contains_id(&self.draft.manual_monitors, id) {
                self.draft.manual_monitors.push(id.to_string());
            }
        } else {
            self.draft.manual_monitors.retain(|x| !same(x, id));
        }
        self.refresh_map();
    }

    /// The manual list: (monitor, checked, enabled, text) for every non-primary screen.
    pub fn manual_rows(&self) -> Vec<(MonitorInfo, bool, bool, String)> {
        self.monitors
            .iter()
            .filter(|m| !m.primary)
            .map(|m| {
                let unusual = !self.backend.is_top(m.bounds);
                let text = if unusual {
                    format!("{} · {}", self.screen_name(m), self.tr.t("UnusualUpper"))
                } else {
                    self.screen_name(m)
                };
                (
                    m.clone(),
                    contains_id(&self.draft.manual_monitors, &m.stable_id),
                    !self.is_excluded(&m.stable_id)
                        && !(self.fixes.j11_segment_and_identity && m.stable_id.contains("#ambiguous:")),
                    text,
                )
            })
            .collect()
    }

    /// C# `chooseCount` of "Choose the upper group myself · {0} screens can be chosen".
    pub fn manual_choose_count(&self) -> usize {
        self.monitors
            .iter()
            .filter(|m| {
                !m.primary
                    && !self.is_excluded(&m.stable_id)
                    && !(self.fixes.j11_segment_and_identity && m.stable_id.contains("#ambiguous:"))
            })
            .count()
    }

    /// The "Restore automatic boundary behavior" button (C# `ResetLayoutConfig` + `ResetDone`).
    pub fn reset_behavior(&mut self) {
        let d = &mut self.draft;
        d.mode = "AutoTop".to_string();
        d.manual_monitors.clear();
        d.upward_boundary_default = Stored::Known(BoundaryDefault::Automatic);
        d.side_boundary_default = Stored::Known(BoundaryDefault::Automatic);
        d.downward_boundary_default = Stored::Known(BoundaryDefault::Free);
        d.boundary_rules.clear();
        d.up_cross_min = 3;
        d.up_cross_slack = 5;
        d.side_cross_min = 3;
        d.side_cross_slack = 5;
        d.passage_inset_px = 24;
        d.edge_holdback_px = 0;
        d.seam_tolerance_px = 2;
        d.safety_escapes = true;
        self.set_inline_status(&self.tr.t("ResetDone"), false, false);
        self.refresh_map();
    }

    // ---------------------------------------------------------------- Shortcuts

    fn chord(&self, i: usize) -> (u32, bool, bool, bool, bool) {
        let d = &self.draft;
        match i {
            0 => (d.hot_key, d.mod_ctrl, d.mod_alt, d.mod_shift, d.mod_win),
            1 => (
                d.confine_hot_key,
                d.confine_mod_ctrl,
                d.confine_mod_alt,
                d.confine_mod_shift,
                d.confine_mod_win,
            ),
            2 => (
                d.pause_hot_key,
                d.pause_mod_ctrl,
                d.pause_mod_alt,
                d.pause_mod_shift,
                d.pause_mod_win,
            ),
            3 => (
                d.side_hot_key,
                d.side_mod_ctrl,
                d.side_mod_alt,
                d.side_mod_shift,
                d.side_mod_win,
            ),
            _ => (
                d.mark_accident_hot_key,
                d.mark_accident_mod_ctrl,
                d.mark_accident_mod_alt,
                d.mark_accident_mod_shift,
                d.mark_accident_mod_win,
            ),
        }
    }

    /// C# `SetHotkey(i, key, ctrl, alt, shift, win)`.
    fn set_hotkey(&mut self, i: usize, key: u32, ctrl: bool, alt: bool, shift: bool, win: bool) {
        let d = &mut self.draft;
        let slots = match i {
            0 => (
                &mut d.hot_key,
                &mut d.mod_ctrl,
                &mut d.mod_alt,
                &mut d.mod_shift,
                &mut d.mod_win,
            ),
            1 => (
                &mut d.confine_hot_key,
                &mut d.confine_mod_ctrl,
                &mut d.confine_mod_alt,
                &mut d.confine_mod_shift,
                &mut d.confine_mod_win,
            ),
            2 => (
                &mut d.pause_hot_key,
                &mut d.pause_mod_ctrl,
                &mut d.pause_mod_alt,
                &mut d.pause_mod_shift,
                &mut d.pause_mod_win,
            ),
            3 => (
                &mut d.side_hot_key,
                &mut d.side_mod_ctrl,
                &mut d.side_mod_alt,
                &mut d.side_mod_shift,
                &mut d.side_mod_win,
            ),
            _ => (
                &mut d.mark_accident_hot_key,
                &mut d.mark_accident_mod_ctrl,
                &mut d.mark_accident_mod_alt,
                &mut d.mark_accident_mod_shift,
                &mut d.mark_accident_mod_win,
            ),
        };
        *slots.0 = key;
        *slots.1 = ctrl;
        *slots.2 = alt;
        *slots.3 = shift;
        *slots.4 = win;
    }

    /// Compatibility entry point for callers without a held Windows-key state.
    pub fn capture_hotkey(&mut self, i: usize, vk: u32, ctrl: bool, alt: bool, shift: bool) {
        self.capture_hotkey_with_win(i, vk, ctrl, alt, shift, false);
    }

    /// Capture a chord including an already-held left or right Windows key.
    pub fn capture_hotkey_with_win(&mut self, i: usize, vk: u32, ctrl: bool, alt: bool, shift: bool, held_win: bool) {
        let modifier = matches!(vk, VK_SHIFT | VK_CONTROL | VK_MENU | VK_LWIN | VK_RWIN | 0xA0..=0xA5);
        let key = if modifier { 0 } else { vk };
        let win = (self.fixes.j17_win_capture && held_win) || matches!(vk, VK_LWIN | VK_RWIN);
        self.set_hotkey(i, key, ctrl, alt, shift, win);
        self.update_shortcut_errors();
    }

    /// The Clear button of a shortcut row (C# `ClearHotkey` + `UpdateShortcutErrors`); row 0 is required.
    pub fn clear_hotkey(&mut self, i: usize) {
        if i == 0 {
            return;
        }
        self.set_hotkey(i, 0, false, false, false, false);
        self.update_shortcut_errors();
    }

    /// C# `HotkeyText(i)`: "Ctrl + Alt + Up", or "(press keys)".
    pub fn hotkey_text(&self, i: usize) -> String {
        let (key, c, a, s, w) = self.chord(i);
        let mut parts: Vec<String> = Vec::new();
        if c {
            parts.push("Ctrl".into());
        }
        if a {
            parts.push("Alt".into());
        }
        if s {
            parts.push("Shift".into());
        }
        if w {
            parts.push("Win".into());
        }
        if key != 0 {
            parts.push(self.backend.key_name(key));
        }
        if parts.is_empty() {
            self.tr.t("HotkeyPressKeys")
        } else {
            parts.join(" + ")
        }
    }

    /// C# `UpdateShortcutErrors()`: the later row of the first duplicate pair is flagged and the footer names both.
    pub fn update_shortcut_errors(&mut self) {
        self.hotkey_errors = [false; 5];
        self.hotkey_error_text = Default::default();
        let issue = fence_config::shortcut_issue_with_fixes(&self.draft, self.fixes);
        if let Some(issue) = issue {
            let (later, message) = match issue {
                fence_config::ShortcutIssue::Incomplete { index } => (index, self.tr.t("ValidationPickModifier")),
                fence_config::ShortcutIssue::Duplicate { first, later } => (
                    later,
                    self.tr.f(
                        "DuplicateShortcutsNamed",
                        &[&self.tr.t(SHORTCUT_HEADS[first]), &self.tr.t(SHORTCUT_HEADS[later])],
                    ),
                ),
            };
            self.hotkey_errors[later] = true;
            self.hotkey_error_text[later] = message.clone();
            self.set_inline_status(&message, true, false);
        }
    }

    // ---------------------------------------------------------------- General

    /// The theme combo applies at once (C# `theme.SelectedIndexChanged`).
    pub fn set_theme_index(&mut self, index: usize) {
        self.general.theme = index.min(2);
        self.draft.theme = theme_name(self.general.theme).to_string();
    }

    /// C# `SyncSimpleControls()`.
    fn sync_simple_controls(&mut self) {
        let g = &self.general;
        let d = &mut self.draft;
        d.start_gate_closed = g.start_gate;
        d.start_side_contain_on = g.start_side;
        d.auto_start = g.auto_start;
        d.start_confine_on = g.start_confine;
        d.start_paused = g.start_paused;
        d.language = match g.language {
            1 => "en",
            2 => "tr",
            _ => "auto",
        }
        .to_string();
        d.theme = theme_name(g.theme).to_string();
    }

    /// C# `LoadDraftIntoControls()`.
    fn load_draft_into_controls(&mut self) {
        self.general = GeneralControls::from_draft(&self.draft);
        self.rebuild_screen_rows();
        self.refresh_map();
        self.close_drawer();
    }

    /// The Startup warning (C# `UpdateWarning`): shown while Game mode at start and Start with Windows are both on.
    pub fn startup_warning(&self) -> String {
        if self.general.start_confine && self.general.auto_start {
            self.tr.t("StartupConfineWarning")
        } else {
            String::new()
        }
    }

    fn seal_message(&self, issue: &SealIssue) -> String {
        self.tr
            .f("InlineSealError", &[&self.screen_name_by_id(&issue.stable_id)])
    }

    /// C# `ValidateDraftForBackup()`.
    fn validate_draft_for_backup(&mut self) -> bool {
        if self.fixes.j11_segment_and_identity && fence_config::has_ambiguous_display_choices(&self.draft) {
            self.show_inline(&self.tr.t("AmbiguousDisplayIdentity"), false);
            return false;
        }
        let issues = self.backend.validate_no_seal(&mut self.draft, &self.monitors);
        let Some(first) = issues.first().cloned() else {
            return true;
        };
        let message = self.seal_message(&first);
        self.pending_issue = Some(first);
        self.show_inline(&message, true);
        false
    }

    /// C# `WriteBackupFile(path)`.
    fn write_backup_file(&mut self, path: &Path) -> bool {
        let json = self.backend.to_json(&self.draft);
        let result = if self.fixes.j18_settings_durability {
            fence_config::write_atomic(path, json.as_bytes())
        } else {
            std::fs::write(path, json)
        };
        self.backup_write_result(result)
    }

    fn backup_write_result(&mut self, result: std::io::Result<()>) -> bool {
        match result {
            Ok(()) => {
                self.set_inline_status(&self.tr.t("BackupDone"), false, false);
                true
            }
            Err(_) => {
                self.show_inline(&self.tr.t("ExportFailed"), false);
                false
            }
        }
    }

    /// C# `BackupToFileForTests(path)`: validate, then write, without a dialog.
    pub fn backup_to_file(&mut self, path: &Path) -> bool {
        self.validate_draft_for_backup() && self.write_backup_file(path)
    }

    #[cfg(test)]
    pub(crate) fn backup_with_writer(
        &mut self,
        path: &Path,
        writer: impl FnOnce(&mut std::fs::File, &[u8]) -> std::io::Result<()>,
    ) -> bool {
        if !self.validate_draft_for_backup() {
            return false;
        }
        let json = self.backend.to_json(&self.draft);
        let result = if self.fixes.j18_settings_durability {
            fence_config::write_atomic_with(path, json.as_bytes(), writer)
        } else {
            std::fs::File::create(path).and_then(|mut file| writer(&mut file, json.as_bytes()))
        };
        self.backup_write_result(result)
    }

    /// The "Back up to a file…" button (C# `Export_Click`).
    pub fn export(&mut self, dialogs: &mut dyn Dialogs) {
        self.sync_simple_controls();
        if !self.validate_draft_for_backup() {
            return;
        }
        let (title, filter) = (self.tr.t("ExportTitle"), self.tr.t("SettingsFileFilter"));
        if let Some(path) = dialogs.pick_backup_path(&title, &filter) {
            self.write_backup_file(&path);
        }
    }

    /// The "Restore from a file…" button (C# `Import_Click`): the file is validated like a save; a repairable or
    /// sealing file becomes the draft without applying; a valid file applies only after confirmation.
    pub fn import(&mut self, dialogs: &mut dyn Dialogs) {
        let (title, filter) = (self.tr.t("ImportTitle"), self.tr.t("SettingsFileFilter"));
        let Some(path) = dialogs.pick_restore_path(&title, &filter) else {
            return;
        };
        let invalid = self.tr.t("ImportInvalid");
        let Ok(json) = std::fs::read_to_string(&path) else {
            self.show_inline(&invalid, false);
            return;
        };
        let Ok(mut imported) = self.backend.parse_json(&json) else {
            self.show_inline(&invalid, false);
            return;
        };
        let version = read_version(&json);
        self.backend.normalize(&mut imported);
        if version < CURRENT_SETTINGS_VERSION
            && self
                .backend
                .migrate_layout_keys_to_stable(&mut imported, &self.monitors)
                .is_err()
        {
            // C# throws here and its catch shows ImportInvalid; nothing is applied (SettingsForm.General.cs:119).
            self.show_inline(&invalid, false);
            return;
        }
        let issues = self.backend.validate_no_seal(&mut imported, &self.monitors);
        if imported.has_repairable_import_errors
            || !issues.is_empty()
            || fence_config::shortcut_issue_with_fixes(&imported, self.fixes).is_some()
            || (self.fixes.j11_segment_and_identity && fence_config::has_ambiguous_display_choices(&imported))
        {
            self.copy_into_draft(imported);
            self.load_draft_into_controls();
            if self.fixes.j11_segment_and_identity && fence_config::has_ambiguous_display_choices(&self.draft) {
                self.show_inline(&self.tr.t("AmbiguousDisplayIdentity"), false);
                return;
            }
            if fence_config::shortcut_issue_with_fixes(&self.draft, self.fixes).is_some() {
                self.update_shortcut_errors();
                return;
            }
            if let Some(first) = issues.first().cloned() {
                let message = self.seal_message(&first);
                self.pending_issue = Some(first);
                self.show_inline(&message, true);
            } else {
                self.show_inline(&invalid, false);
            }
            return;
        }
        if !dialogs.confirm_restore(&self.tr.t("ImportConfirm"), &self.tr.t("SettingsTitle")) {
            return;
        }
        self.copy_into_draft(imported);
        let mut result = self.draft.clone();
        self.backend.normalize(&mut result);
        self.outcome = Outcome::Saved(Box::new(result));
    }

    /// C# `CopySettings(source, target)`: a deep copy, then Normalize.
    fn copy_into_draft(&mut self, source: Settings) {
        self.draft = source;
        self.backend.normalize(&mut self.draft);
    }

    // ---------------------------------------------------------------- Save / Cancel

    /// The Save button (C# `Save_Click`).
    pub fn save(&mut self) {
        self.sync_simple_controls();
        if self.fixes.j11_segment_and_identity && fence_config::has_ambiguous_display_choices(&self.draft) {
            self.show_inline(&self.tr.t("AmbiguousDisplayIdentity"), false);
            return;
        }
        self.backend.normalize(&mut self.draft);
        if fence_config::shortcut_issue_with_fixes(&self.draft, self.fixes).is_some() {
            self.update_shortcut_errors();
            return;
        }
        let issues = self.backend.validate_no_seal(&mut self.draft, &self.monitors);
        if self.draft.has_repairable_import_errors || !issues.is_empty() {
            if let Some(first) = issues.first().cloned() {
                let message = self.seal_message(&first);
                self.pending_issue = Some(first);
                self.show_inline(&message, true);
            } else {
                self.show_inline(&self.tr.t("ImportInvalid"), false);
            }
            return;
        }
        let mut result = self.draft.clone();
        self.backend.normalize(&mut result);
        self.outcome = Outcome::Saved(Box::new(result));
    }

    /// Cancel, Escape with no drawer open, or the window's close box (C# `DialogResult.Cancel`).
    pub fn cancel(&mut self) {
        self.outcome = Outcome::Cancelled;
    }

    /// The settings process could not persist a saved result: keep the window open with the draft and say so.
    pub fn save_failed(&mut self, message: &str) {
        self.outcome = Outcome::Open;
        self.show_inline(message, false);
    }

    /// The settings file refused a sealed layout: keep the draft open, name the screen, offer its boundary.
    pub fn show_seal_issue(&mut self, issue: SealIssue) {
        self.outcome = Outcome::Open;
        let message = self.seal_message(&issue);
        self.pending_issue = Some(issue);
        self.show_inline(&message, true);
    }

    /// C# `DisplayRefreshTimer_Tick`: the live layout changed while the window was open.
    pub fn apply_monitors(&mut self, monitors: Vec<MonitorInfo>) {
        self.monitors = monitors;
        self.sanitize_excluded();
        self.rebuild_screen_rows();
        self.refresh_map();
        self.close_drawer();
        self.set_inline_status(&self.tr.t("LayoutChangedBanner"), false, false);
    }

    /// Screens in the order of the list (C# `_monitors.OrderBy(m => m.Index)`).
    pub fn rows(&self) -> Vec<MonitorInfo> {
        let mut rows = self.monitors.clone();
        rows.sort_by_key(|m| m.index);
        rows
    }
}

/// One numeric row of the Advanced grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumericSpec {
    pub label: String,
    pub help: String,
    pub min: i32,
    pub max: i32,
    pub default: i32,
}

impl NumericSpec {
    fn new(label: String, help: String, min: i32, max: i32, default: i32) -> Self {
        Self {
            label,
            help,
            min,
            max,
            default,
        }
    }
}
