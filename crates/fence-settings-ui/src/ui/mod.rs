//! The window drawn with egui: sidebar, the four pages and the footer (C# `SettingsForm` constructor and
//! `ApplyTheme`). Everything here is plain egui on a `Ui`, so the same code runs in the eframe window and in the
//! headless tests that feed `egui::Context` raw input.

pub mod displays;
pub mod pages;
pub mod widgets;

use std::sync::Arc;

use eframe::egui::{
    self, CornerRadius, Event, FontData, FontDefinitions, FontFamily, Id, Key, Rect, Stroke, Ui, pos2, vec2,
};
use fence_types::{Behavior, BoundaryDefault, Control, MonitorInfo};

use crate::boundary::same;
use crate::editor::{ButtonRole, Dialogs, Editor};
use crate::theme::{self, SystemColors, Theme};
use displays::{DisplaysGeometry, DisplaysView, FOOTER_H, MapState, NameBoxes, SIDEBAR_W};
use widgets::{self as w, HAlign, Recorder, regular, semibold};

/// Everything a frame can ask the window to do; applied after drawing, in order (C# event handlers).
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Identify,
    OpenBoundary(usize),
    MonitorChosen(String),
    SelectMonitorKeyboard(String),
    SelectBoundaryKeyboard(usize),
    CloseDrawer,
    UseOverall,
    ChoosePrimary(Behavior),
    ToggleSeparate,
    ToggleWhen,
    ChooseSeparate(usize, Behavior),
    SetControl(Control),
    AllowScreen,
    SetKeepOut(String, bool),
    SetName(String, String),
    SelectTab(usize),
    Save,
    Cancel,
    OpenValidationBoundary,
    ToggleAdvanced,
    SetDefault(usize, BoundaryDefault),
    SetNumeric(usize, i32),
    SetDeliberate(bool),
    SetCrossingModel(fence_types::CrossingModel),
    SetCrossingEase(i32),
    SetEdgeTrace(bool),
    SetSafety(bool),
    SetDescent(bool),
    SetManualMode(bool),
    SetManualMonitor(String, bool),
    ResetBehavior,
    CaptureHotkey(usize, u32, bool, bool, bool),
    CaptureHotkeyWithWin(usize, u32, bool, bool, bool, bool),
    ClearHotkey(usize),
    SetStartGate(bool),
    SetStartSide(bool),
    SetAutoStart(bool),
    SetStartConfine(bool),
    SetStartPaused(bool),
    ToggleMoreStartup,
    SetLanguage(usize),
    SetTheme(usize),
    Export,
    Import,
}

/// The OS facts the theme reads (C# `SystemInformation.HighContrast`, AppsUseLightTheme, `SystemColors`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SystemTheme {
    pub high_contrast: bool,
    pub system_dark: bool,
    pub colors: SystemColors,
}

/// What the window needs from the outside world; the tests pass fakes.
pub trait Services {
    fn windows_key_down(&self) -> bool {
        false
    }
    fn dialogs(&mut self) -> &mut dyn Dialogs;
    /// "Identify all screens" (C# `IdentifyOverlay.Flash`): the usable screens, their names, the window's theme.
    fn identify(&mut self, monitors: &[MonitorInfo], names: &[(String, String)], theme: &Theme);
    fn system_theme(&self) -> SystemTheme;
}

/// C# `Screenshot.CaptureMetrics` without the client size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutMetrics {
    pub mode: &'static str,
    pub body: i32,
    pub map: i32,
    pub band: i32,
}

impl LayoutMetrics {
    /// C# `BandPercent`.
    pub fn band_percent(&self) -> f64 {
        if self.body <= 0 {
            0.0
        } else {
            self.band as f64 * 100.0 / self.body as f64
        }
    }

    /// The heights in physical pixels at `ppp` (C# measured them on a form already scaled to that DPI).
    pub fn in_pixels(self, ppp: f32) -> Self {
        let px = |v: i32| (v as f32 * ppp).round() as i32;
        Self {
            mode: self.mode,
            body: px(self.body),
            map: px(self.map),
            band: px(self.band),
        }
    }
}

/// Per-page widget state that is not part of the settings.
#[derive(Clone, Debug, Default)]
pub struct PageState {
    pub held_win: bool,
    /// Seven legacy numeric boxes plus four StopThenPush parameters.
    pub numbers: [String; 11],
    /// Modifiers seen last frame by each focused shortcut box (C# KeyDown of a modifier alone).
    pub shortcut_mods: [Option<egui::Modifiers>; 5],
    /// A sidebar button to focus next frame (C# `SelectRelativeTab` focuses the new item).
    pub focus_nav: Option<usize>,
    /// Content height of each page measured on the previous frame (the scroll extent).
    pub heights: [f32; 4],
}

pub struct SettingsUi {
    pub editor: Editor,
    pub theme: Theme,
    pub system: SystemTheme,
    pub rec: Recorder,
    pub map: MapState,
    pub names: NameBoxes,
    pub pages: PageState,
    pub scroll_to_drawer: bool,
    pub last_displays: Option<DisplaysGeometry>,
    applied_theme: Option<Theme>,
}

/// Ids of the keyboard-capturing widgets on which Enter still means Save (C# AcceptButton).
fn enter_saves(id: Id, editor: &Editor) -> bool {
    (0..editor.shortcut_count()).any(|i| id == pages::shortcut_id(i))
        || (0..11).any(|i| id == pages::numeric_id(i))
        || editor
            .monitors
            .iter()
            .any(|m| id == Id::new(("name-box", &m.stable_id)))
}

impl SettingsUi {
    pub fn new(editor: Editor, system: SystemTheme) -> Self {
        let theme = Theme::for_kind(
            theme::resolve(&editor.draft.theme, system.high_contrast, system.system_dark),
            &system.colors,
        );
        Self {
            editor,
            theme,
            system,
            rec: Recorder::default(),
            map: MapState::default(),
            names: NameBoxes::default(),
            pages: PageState::default(),
            scroll_to_drawer: false,
            last_displays: None,
            applied_theme: None,
        }
    }

    /// C# `Theming.Resolve(_draft.Theme)` after the theme combo changed or Windows switched its colours.
    pub fn resolve_theme(&mut self, system: SystemTheme) {
        self.system = system;
        self.theme = Theme::for_kind(
            theme::resolve(&self.editor.draft.theme, system.high_contrast, system.system_dark),
            &system.colors,
        );
    }

    /// Draw one frame and apply what it asked for.
    pub fn show(&mut self, ui: &mut Ui, services: &mut dyn Services) {
        self.pages.held_win = services.windows_key_down();
        if self.applied_theme != Some(self.theme) {
            ui.ctx().set_visuals(self.theme.visuals());
            self.applied_theme = Some(self.theme);
        }
        self.rec.clear();
        let mut actions: Vec<Action> = Vec::new();
        let client = ui.max_rect();
        let (cw, ch) = (client.width().round() as i32, client.height().round() as i32);
        let t = self.theme;
        let p = ui.painter().clone();
        p.rect_filled(client, CornerRadius::ZERO, t.back);

        // Sidebar (C# _tabStrip).
        let side = Rect::from_min_size(client.min, vec2(SIDEBAR_W, client.height()));
        p.rect_filled(side, CornerRadius::ZERO, t.sidebar());
        p.line_segment(
            [
                pos2(side.right() - 0.5, side.top()),
                pos2(side.right() - 0.5, side.bottom()),
            ],
            Stroke::new(1.0, t.border),
        );
        let product = Rect::from_min_size(client.min + vec2(12.0, 21.0), vec2(144.0, 25.0));
        w::label(
            ui,
            product,
            "▣  MouseFence",
            semibold(10.5),
            t.text,
            HAlign::Left,
            true,
            false,
        );
        self.rec.add("brand", product, "▣  MouseFence", "");
        let nav_label = Rect::from_min_size(client.min + vec2(12.0, 66.0), vec2(144.0, 18.0));
        w::label(
            ui,
            nav_label,
            &self.editor.tr.t("SettingsNavLabel"),
            semibold(7.5),
            t.faint,
            HAlign::Left,
            true,
            false,
        );
        // Taken before the loop: a move asked for this frame lands next frame, so one key press moves one page.
        let focus_now = self.pages.focus_nav.take();
        for (i, key) in ["NavDisplays", "NavAdvanced", "NavShortcuts", "NavGeneral"]
            .iter()
            .enumerate()
        {
            let r = Rect::from_min_size(client.min + vec2(12.0, 92.0 + 46.0 * i as f32), vec2(144.0, 42.0));
            let resp = w::sidebar_button(
                ui,
                &mut self.rec,
                &format!("nav.{i}"),
                r,
                &self.editor.tr.t(key),
                self.editor.tab == i,
                &t,
            );
            if focus_now == Some(i) {
                resp.request_focus();
            }
            if resp.clicked() {
                actions.push(Action::SelectTab(i));
                // egui does not focus a clicked button; WinForms does, and the arrows then move between pages.
                resp.request_focus();
            }
            if resp.has_focus() {
                let (up, down) = ui.input(|inp| (inp.key_pressed(Key::ArrowUp), inp.key_pressed(Key::ArrowDown)));
                if up || down {
                    let next = (i as i32 + if up { -1 } else { 1 } + 4) as usize % 4;
                    actions.push(Action::SelectTab(next));
                    self.pages.focus_nav = Some(next);
                }
                ui.memory_mut(|m| {
                    m.set_focus_lock_filter(
                        resp.id,
                        egui::EventFilter {
                            vertical_arrows: true,
                            horizontal_arrows: false,
                            tab: false,
                            escape: false,
                        },
                    )
                });
            }
        }

        // The page (C# TabPage / _bodyFlow).
        let page = Rect::from_min_max(
            pos2(side.right(), client.top()),
            pos2(client.right(), client.bottom() - FOOTER_H),
        );
        let mut page_ui = ui.new_child(egui::UiBuilder::new().max_rect(page).id_salt(("page", self.editor.tab)));
        page_ui.set_clip_rect(page);
        match self.editor.tab {
            0 => {
                let mut view = DisplaysView {
                    editor: &self.editor,
                    theme: &t,
                    rec: &mut self.rec,
                    actions: &mut actions,
                    map: &mut self.map,
                    names: &mut self.names,
                    client: (cw, ch),
                    scroll_to_drawer: self.scroll_to_drawer,
                };
                self.last_displays = Some(view.show(&mut page_ui, page));
                self.scroll_to_drawer = false;
            }
            tab => {
                self.last_displays = Some(displays::geometry(&page_ui, &self.editor, (cw, ch)));
                let mut view = pages::PageView {
                    editor: &self.editor,
                    theme: &t,
                    rec: &mut self.rec,
                    actions: &mut actions,
                    state: &mut self.pages,
                    client: (cw, ch),
                };
                view.show(&mut page_ui, page, tab);
            }
        }
        drop(page_ui);

        // Footer (C# _footer).
        let footer = Rect::from_min_max(pos2(side.right(), client.bottom() - FOOTER_H), client.max);
        p.rect_filled(footer, CornerRadius::ZERO, t.surface);
        p.line_segment([footer.left_top(), footer.right_top()], Stroke::new(1.0, t.border));
        self.rec.add("footer", footer, "", "");
        let top = footer.top() + 13.0;
        let save = Rect::from_min_size(pos2(footer.right() - 22.0 - 94.0, top), vec2(94.0, 36.0));
        let cancel = Rect::from_min_size(pos2(save.left() - 10.0 - 94.0, top), vec2(94.0, 36.0));
        let inline = &self.editor.inline;
        let mut status_right = cancel.left() - 10.0;
        if inline.action {
            let action = Rect::from_min_size(pos2(cancel.left() - 13.0 - 145.0, top), vec2(145.0, 36.0));
            if w::button(
                ui,
                &mut self.rec,
                "footer.action",
                action,
                &self.editor.tr.t("OpenBoundary"),
                ButtonRole::Quiet,
                true,
                &t,
                false,
            )
            .clicked()
            {
                actions.push(Action::OpenValidationBoundary);
            }
            status_right = action.left() - 3.0;
        }
        let status = Rect::from_min_max(
            pos2(footer.left() + 22.0, top),
            pos2(status_right.max(footer.left() + 23.0), top + 36.0),
        );
        w::label(
            ui,
            status,
            &inline.text,
            regular(9.0),
            if inline.error { t.danger } else { t.faint },
            HAlign::Left,
            true,
            false,
        );
        self.rec.add(
            "footer.status",
            status,
            inline.text.clone(),
            if inline.error { "error" } else { "" },
        );
        if w::button(
            ui,
            &mut self.rec,
            "footer.cancel",
            cancel,
            &self.editor.tr.t("Cancel"),
            ButtonRole::Secondary,
            true,
            &t,
            false,
        )
        .clicked()
        {
            actions.push(Action::Cancel);
        }
        if w::button(
            ui,
            &mut self.rec,
            "footer.save",
            save,
            &self.editor.tr.t("Save"),
            ButtonRole::Primary,
            true,
            &t,
            false,
        )
        .clicked()
        {
            actions.push(Action::Save);
        }

        // Enter and Escape (C# AcceptButton, CancelButton and ProcessCmdKey).
        let (enter, escape) = ui.input(|i| {
            let pressed = |k: Key| {
                i.events
                    .iter()
                    .any(|e| matches!(e, Event::Key { key, pressed: true, .. } if *key == k))
            };
            (pressed(Key::Enter), pressed(Key::Escape))
        });
        let focused = ui.memory(|m| m.focused());
        if enter && focused.is_none_or(|id| enter_saves(id, &self.editor)) && !actions.contains(&Action::Save) {
            actions.push(Action::Save);
        }
        if escape && !ui.ctx().any_popup_open() {
            actions.push(if self.editor.drawer_visible {
                Action::CloseDrawer
            } else {
                Action::Cancel
            });
        }

        for a in actions {
            self.apply(a, services);
        }
    }

    pub fn apply(&mut self, action: Action, services: &mut dyn Services) {
        let e = &mut self.editor;
        match action {
            Action::Identify => {
                let usable: Vec<MonitorInfo> = e
                    .monitors
                    .iter()
                    .filter(|m| !e.is_excluded(&m.stable_id))
                    .cloned()
                    .collect();
                services.identify(&usable, &e.draft.monitor_names, &self.theme);
            }
            Action::OpenBoundary(i) => {
                e.open_boundary(i);
                self.scroll_to_drawer = e.drawer_visible;
            }
            Action::MonitorChosen(id) => e.monitor_chosen(&id),
            Action::SelectMonitorKeyboard(id) => {
                e.map_selected_monitor = Some(id);
                e.map_selected_boundary = None;
            }
            Action::SelectBoundaryKeyboard(i) => {
                e.map_selected_boundary = Some(i);
                e.map_selected_monitor = None;
            }
            Action::CloseDrawer => e.close_drawer(),
            Action::UseOverall => e.use_overall(),
            Action::ChoosePrimary(b) => e.choose_primary_behavior(b),
            Action::ToggleSeparate => e.toggle_separate(),
            Action::ToggleWhen => e.toggle_when(),
            Action::ChooseSeparate(row, b) => e.choose_separate_behavior(row, b),
            Action::SetControl(c) => e.set_control(c),
            Action::AllowScreen => e.allow_boundary_screen(),
            Action::SetKeepOut(id, on) => e.set_keep_out(&id, on),
            Action::SetName(id, text) => e.set_monitor_name(&id, &text),
            Action::SelectTab(i) => e.select_tab(i),
            Action::Save => e.save(),
            Action::Cancel => e.cancel(),
            Action::OpenValidationBoundary => {
                e.open_validation_boundary();
                self.scroll_to_drawer = e.drawer_visible;
            }
            Action::ToggleAdvanced => e.toggle_advanced(),
            Action::SetDefault(which, v) => e.set_default(which, v),
            Action::SetNumeric(i, v) => e.set_numeric(i, v),
            Action::SetDeliberate(on) => e.set_deliberate_cross(on),
            Action::SetCrossingModel(model) => e.set_crossing_model(model),
            Action::SetCrossingEase(value) => e.draft.crossing_ease = value.clamp(1, 5),
            Action::SetEdgeTrace(on) => e.set_edge_trace(on),
            Action::SetSafety(on) => e.set_safety_escapes(on),
            Action::SetDescent(on) => e.set_descent_routing(on),
            Action::SetManualMode(m) => e.set_manual_mode(m),
            Action::SetManualMonitor(id, on) => e.set_manual_monitor(&id, on),
            Action::ResetBehavior => e.reset_behavior(),
            Action::CaptureHotkey(i, vk, c, a, s) => e.capture_hotkey(i, vk, c, a, s),
            Action::CaptureHotkeyWithWin(i, vk, c, a, s, w) => e.capture_hotkey_with_win(i, vk, c, a, s, w),
            Action::ClearHotkey(i) => e.clear_hotkey(i),
            Action::SetStartGate(on) => e.general.start_gate = on,
            Action::SetStartSide(on) => e.general.start_side = on,
            Action::SetAutoStart(on) => e.general.auto_start = on,
            Action::SetStartConfine(on) => e.general.start_confine = on,
            Action::SetStartPaused(on) => e.general.start_paused = on,
            Action::ToggleMoreStartup => e.more_startup_expanded = !e.more_startup_expanded,
            Action::SetLanguage(i) => e.general.language = i.min(2),
            Action::SetTheme(i) => {
                e.set_theme_index(i);
                let system = services.system_theme();
                self.resolve_theme(system);
            }
            Action::Export => e.export(services.dialogs()),
            Action::Import => {
                e.import(services.dialogs());
                let system = services.system_theme();
                self.resolve_theme(system);
            }
        }
    }

    /// C# `OpenFirstBoundaryForPreview(hideScreenList)`; with the list hidden the page stays at the top.
    pub fn open_first_boundary_for_preview(&mut self, hide_screen_list: bool) {
        self.editor.open_first_boundary_for_preview(hide_screen_list);
        self.scroll_to_drawer = self.editor.drawer_visible && !hide_screen_list;
    }

    /// C# `DisplayLayoutMetrics()`: the Displays mode ("drawer" when the editor shows without the list), the body
    /// height, the map height and the empty band under the last visible block (scroll position 0 unless the
    /// Displays page is showing).
    pub fn layout_metrics(&self) -> Option<LayoutMetrics> {
        if self.editor.fixes.j21_ui_keepers && self.editor.tab != 0 {
            return Some(LayoutMetrics {
                mode: match self.editor.tab {
                    1 => "advanced",
                    2 => "shortcuts",
                    _ => "general",
                },
                body: self.rec.get("body")?.rect.height().round() as i32,
                map: 0,
                band: 0,
            });
        }
        let g = self.last_displays.as_ref()?;
        let e = &self.editor;
        let mode = if e.drawer_visible && e.screen_list_hidden {
            "drawer"
        } else {
            "list"
        };
        let mut last_bottom = (g.content_h - displays::PAD_BOTTOM) as f32;
        if e.tab == 0
            && let (Some(body), Some(last)) = (self.rec.get("body"), self.rec.get("last-block"))
        {
            last_bottom = last.rect.bottom() - body.rect.top();
        }
        let band = (g.body_h as f32 - last_bottom).round().max(0.0) as i32;
        Some(LayoutMetrics {
            mode,
            body: g.body_h,
            map: g.map_h,
            band,
        })
    }

    /// The monitor the map shows as hovered from the list (C# `HoverRow`), for tests.
    pub fn hovered_from_list(&self) -> Option<&str> {
        self.map.external_hover.as_deref()
    }

    /// True when `id` is one of the current screens (helper for the process's display polling).
    pub fn has_monitor(&self, id: &str) -> bool {
        self.editor.monitors.iter().any(|m| same(&m.stable_id, id))
    }
}

/// The window's fonts: Segoe UI, Segoe UI Semibold and Segoe UI Symbol from the Windows font folder when
/// `windows_fonts` is set (the 1.x faces), with egui's built-in faces behind them for anything missing.
/// The headless tests use the built-in faces only, so they never read the machine's fonts.
pub fn font_definitions(windows_fonts: bool) -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let proportional = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut regular_list = proportional.clone();
    let mut semibold_list = proportional;
    if windows_fonts {
        let dir = std::env::var_os("WINDIR").map(std::path::PathBuf::from);
        let load = |name: &str| {
            dir.as_ref()
                .and_then(|dir| std::fs::read(dir.join("Fonts").join(name)).ok())
        };
        let mut symbol = None;
        if let Some(bytes) = load("seguisym.ttf") {
            fonts
                .font_data
                .insert("segoe-symbol".into(), Arc::new(FontData::from_owned(bytes)));
            symbol = Some("segoe-symbol".to_string());
        }
        if let Some(bytes) = load("segoeui.ttf") {
            fonts
                .font_data
                .insert("segoe-ui".into(), Arc::new(FontData::from_owned(bytes)));
            regular_list.insert(0, "segoe-ui".into());
        }
        if let Some(bytes) = load("seguisb.ttf") {
            fonts
                .font_data
                .insert("segoe-semibold".into(), Arc::new(FontData::from_owned(bytes)));
            semibold_list.insert(0, "segoe-semibold".into());
        }
        if let Some(s) = symbol {
            regular_list.insert(regular_list.len().min(1), s.clone());
            semibold_list.insert(semibold_list.len().min(1), s);
        }
    }
    fonts.families.insert(FontFamily::Proportional, regular_list);
    fonts
        .families
        .insert(FontFamily::Name(widgets::SEMIBOLD.into()), semibold_list);
    fonts
}
