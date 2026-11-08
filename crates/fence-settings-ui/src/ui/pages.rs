//! The Advanced, Shortcuts and General pages (C# SettingsForm.Advanced.cs, .Shortcuts.cs, .General.cs). Each page
//! is a top-down flow of fixed-height rows, as the WinForms FlowLayoutPanel was; the sizes are the 1.x ones
//! (control heights plus the default 3-point margins).

use eframe::egui::{self, Event, Id, Key, Rect, Sense, TextEdit, Ui, UiBuilder, pos2, vec2};
use fence_types::{BoundaryDefault, CrossingModel};

use super::displays::{SIDEBAR_W, measure};
use super::widgets::{self as w, HAlign, Recorder, regular, semibold};
use super::{Action, PageState};
use crate::editor::{ButtonRole, Editor, SHORTCUT_HEADS, SHORTCUT_HINTS};
use crate::theme::Theme;

pub const PAGE_TOP: f32 = 23.0;
pub const PAGE_BOTTOM: f32 = 32.0;
/// WinForms default `Control.Margin` (3 on every side) of flow children that keep it.
const M: f32 = 3.0;

pub fn shortcut_id(i: usize) -> Id {
    Id::new(("shortcut-box", i))
}

pub fn numeric_id(i: usize) -> Id {
    Id::new(("numeric-box", i))
}

/// The values of the three "Overall behavior" combos (C# `DefaultRow`): up and side offer Automatic.
pub fn default_choices(which: usize) -> &'static [BoundaryDefault] {
    if which < 2 {
        &[
            BoundaryDefault::Automatic,
            BoundaryDefault::Free,
            BoundaryDefault::Push,
            BoundaryDefault::Blocked,
        ]
    } else {
        &[BoundaryDefault::Free, BoundaryDefault::Push, BoundaryDefault::Blocked]
    }
}

fn default_text(e: &Editor, v: BoundaryDefault) -> String {
    e.tr.t(match v {
        BoundaryDefault::Automatic => "DefaultAutomatic",
        BoundaryDefault::Free => "DefaultFree",
        BoundaryDefault::Push => "DefaultPush",
        BoundaryDefault::Blocked => "DefaultBlocked",
    })
}

/// Map an egui key to the Windows virtual-key code the hotkey settings store (C# `Keys`). Enter, Escape and Tab
/// are not captured: in 1.x the form's Accept/Cancel buttons and focus navigation took them first.
pub fn vk_for(key: Key) -> Option<u32> {
    use Key::*;
    let letters = [
        A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    ];
    if let Some(i) = letters.iter().position(|k| *k == key) {
        return Some(0x41 + i as u32);
    }
    let digits = [Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9];
    if let Some(i) = digits.iter().position(|k| *k == key) {
        return Some(0x30 + i as u32);
    }
    let fkeys = [
        F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23, F24,
    ];
    if let Some(i) = fkeys.iter().position(|k| *k == key) {
        return Some(0x70 + i as u32);
    }
    Some(match key {
        ArrowLeft => 0x25,
        ArrowUp => 0x26,
        ArrowRight => 0x27,
        ArrowDown => 0x28,
        Space => 0x20,
        PageUp => 0x21,
        PageDown => 0x22,
        End => 0x23,
        Home => 0x24,
        Insert => 0x2D,
        Delete => 0x2E,
        Backspace => 0x08,
        Semicolon => 0xBA,
        Equals | Plus => 0xBB,
        Comma => 0xBC,
        Minus => 0xBD,
        Period => 0xBE,
        Slash => 0xBF,
        Backtick => 0xC0,
        OpenBracket => 0xDB,
        Backslash => 0xDC,
        CloseBracket => 0xDD,
        Quote => 0xDE,
        _ => return None,
    })
}

pub struct PageView<'a> {
    pub editor: &'a Editor,
    pub theme: &'a Theme,
    pub rec: &'a mut Recorder,
    pub actions: &'a mut Vec<Action>,
    pub state: &'a mut PageState,
    pub client: (i32, i32),
}

/// A top-down flow cursor.
struct Flow {
    x: f32,
    y: f32,
    w: f32,
}

impl Flow {
    fn take(&mut self, h: f32, margin: f32) -> Rect {
        let r = Rect::from_min_size(pos2(self.x, self.y + margin), vec2(self.w, h));
        self.y += h + margin * 2.0;
        r
    }
}

impl PageView<'_> {
    /// The flow's height is known only after it is drawn, so the scroll area uses the height measured on the
    /// previous frame and asks for one more frame when it changed (no window ever shows the first frame: the
    /// process starts hidden and the tests run several frames).
    pub fn show(&mut self, ui: &mut Ui, page: Rect, tab: usize) {
        let m = measure(self.client.0, self.client.0 - SIDEBAR_W as i32);
        let height = self.state.heights[tab].max(1.0);
        let mut measured = height;
        ui.scope_builder(UiBuilder::new().max_rect(page).id_salt(("page-scope", tab)), |ui| {
            egui::ScrollArea::vertical()
                .id_salt(("page-scroll", tab))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let (content, _) = ui.allocate_exact_size(vec2(page.width(), height), Sense::hover());
                    let mut f = Flow {
                        x: content.left() + m.left as f32,
                        y: content.top() + PAGE_TOP,
                        w: m.width as f32,
                    };
                    match tab {
                        1 => self.advanced(ui, &mut f, m.narrow),
                        2 => self.shortcuts(ui, &mut f),
                        _ => self.general(ui, &mut f, m.narrow),
                    }
                    measured = f.y + PAGE_BOTTOM - content.top();
                    self.rec.add(
                        format!("page.{tab}"),
                        Rect::from_min_max(content.min, pos2(content.right(), f.y)),
                        "",
                        "",
                    );
                });
        });
        if (measured - self.state.heights[tab]).abs() > 0.5 {
            self.state.heights[tab] = measured;
            ui.ctx().request_repaint();
        }
        self.rec
            .add("body", page, "", format!("padding {} {}", m.left, m.right));
    }

    fn header(&mut self, ui: &Ui, f: &mut Flow, title: &str, subtitle: &str) {
        let r = f.take(70.0, 0.0);
        w::label(
            ui,
            Rect::from_min_size(r.min, vec2(r.width(), 29.0)),
            title,
            semibold(18.0),
            self.theme.text,
            HAlign::Left,
            true,
            false,
        );
        w::label(
            ui,
            Rect::from_min_size(r.min + vec2(0.0, 34.0), vec2(r.width(), 28.0)),
            subtitle,
            regular(9.0),
            self.theme.subtle,
            HAlign::Left,
            false,
            true,
        );
        self.rec.add("page.header", r, title, "");
    }

    fn section(&mut self, ui: &Ui, f: &mut Flow, text: &str) {
        let r = f.take(46.0, M);
        w::label(
            ui,
            Rect::from_min_max(r.min + vec2(0.0, 16.0), r.max),
            text,
            semibold(10.5),
            self.theme.text,
            HAlign::Left,
            true,
            false,
        );
        self.rec.add(format!("section.{text}"), r, text, "");
    }

    fn help(&mut self, ui: &Ui, f: &mut Flow, text: &str, h: f32, tag: &str) {
        let r = f.take(h, M);
        w::label(ui, r, text, regular(9.0), self.theme.subtle, HAlign::Left, false, true);
        self.rec.add(tag, r, text, "");
    }

    #[allow(clippy::too_many_arguments)]
    fn check(
        &mut self,
        ui: &Ui,
        f: &mut Flow,
        tag: &str,
        text: &str,
        checked: bool,
        tip: Option<String>,
        make: fn(bool) -> Action,
    ) {
        let r = f.take(32.0, M);
        let resp = w::checkbox(ui, self.rec, tag, r, text, checked, true, self.theme);
        let resp = match tip {
            Some(t) => resp.on_hover_text(t),
            None => resp,
        };
        if resp.clicked() {
            self.actions.push(make(!checked));
        }
    }

    fn combo(&mut self, ui: &mut Ui, rect: Rect, salt: &str, selected: usize, items: &[String]) -> Option<usize> {
        let mut chosen = None;
        let mut current = selected;
        ui.scope_builder(UiBuilder::new().max_rect(rect).id_salt(("combo-scope", salt)), |ui| {
            ui.spacing_mut().interact_size.y = rect.height();
            ui.spacing_mut().combo_width = rect.width();
            let text = items.get(selected).cloned().unwrap_or_default();
            egui::ComboBox::from_id_salt(("combo", salt))
                .width(rect.width())
                .height(400.0)
                .selected_text(egui::RichText::new(text).font(regular(9.0)))
                .show_ui(ui, |ui| {
                    for (i, item) in items.iter().enumerate() {
                        if ui
                            .selectable_value(&mut current, i, egui::RichText::new(item).font(regular(9.0)))
                            .changed()
                        {
                            chosen = Some(i);
                        }
                    }
                });
        });
        self.rec.add(
            format!("combo.{salt}"),
            rect,
            items.get(selected).cloned().unwrap_or_default(),
            selected.to_string(),
        );
        chosen
    }

    // ------------------------------------------------------------------ Advanced

    fn advanced(&mut self, ui: &mut Ui, f: &mut Flow, narrow: bool) {
        let e = self.editor;
        let t = *self.theme;
        self.header(ui, f, &e.tr.t("NavAdvanced"), &e.tr.t("AdvancedSubtitle"));
        self.section(ui, f, &e.tr.t("DefaultBoundaryHead"));
        for (which, key) in ["UpwardDefault", "SideDefault", "DownDefault"].iter().enumerate() {
            let row = f.take(44.0, 0.0);
            let label = Rect::from_min_max(row.min, pos2(row.right() - 250.0, row.bottom()));
            w::label(ui, label, &e.tr.t(key), regular(9.0), t.text, HAlign::Left, true, false);
            let choices = default_choices(which);
            let items: Vec<String> = choices.iter().map(|v| default_text(e, *v)).collect();
            let selected = match e.default_value(which) {
                fence_config::Stored::Known(v) => choices.iter().position(|c| *c == v).unwrap_or(0),
                fence_config::Stored::Unknown => 0,
            };
            let combo = Rect::from_min_size(pos2(row.right() - 250.0, row.top() + 4.0), vec2(250.0, 36.0));
            if let Some(i) = self.combo(ui, combo, &format!("default.{which}"), selected, &items) {
                self.actions.push(Action::SetDefault(which, choices[i]));
            }
            self.rec
                .add(format!("adv.default.{which}"), row, items[selected].clone(), "");
        }
        self.section(ui, f, &e.tr.t("CrossingFeelHead"));
        if e.fixes.j13_classic_1k || e.fixes.j14_stop_then_push {
            let row = f.take(44.0, 0.0);
            w::label(
                ui,
                Rect::from_min_max(row.min, pos2(row.right() - 250.0, row.bottom())),
                &e.tr.t("CrossingModel"),
                regular(9.0),
                t.text,
                HAlign::Left,
                true,
                false,
            );
            let models = [
                CrossingModel::Classic,
                CrossingModel::Classic1k,
                CrossingModel::StopThenPush,
                CrossingModel::Intent,
            ];
            let items = [
                "CrossingModelClassic",
                "CrossingModelClassic1k",
                "CrossingModelStopThenPush",
                "CrossingModelIntent",
            ]
            .map(|key| e.tr.t(key));
            let selected = models
                .iter()
                .position(|model| *model == e.draft.crossing_model)
                .unwrap_or(1);
            let rect = Rect::from_min_size(pos2(row.right() - 250.0, row.top() + 4.0), vec2(250.0, 36.0));
            if let Some(i) = self.combo(ui, rect, "crossing-model", selected, &items) {
                self.actions.push(Action::SetCrossingModel(models[i]));
            }
            if e.draft.crossing_model == CrossingModel::StopThenPush {
                self.numeric_grid(ui, f, narrow, true);
            }
            if e.draft.crossing_model == CrossingModel::Intent {
                let row = f.take(48.0, 0.0);
                let mut value = e.draft.crossing_ease;
                let label = e.tr.t("CrossingEase");
                let response = ui.put(row, egui::Slider::new(&mut value, 1..=5).integer().text(&label));
                self.rec.add("adv.crossing-ease", row, label, value.to_string());
                if response.changed() {
                    self.actions.push(Action::SetCrossingEase(value));
                }
                self.help(ui, f, &e.tr.t("CrossingEaseHelp"), 38.0, "adv.intent.help");
            }
        }
        self.check(
            ui,
            f,
            "adv.deliberate",
            &e.tr.t("FirmPushRecommended"),
            e.draft.deliberate_cross,
            None,
            Action::SetDeliberate,
        );
        self.help(ui, f, &e.tr.t("CrossingFeelSectionHelp"), 34.0, "adv.feel.help");
        let toggle = f.take(36.0, M);
        let text = e.advanced_toggle_text();
        let resp = w::button(
            ui,
            self.rec,
            "adv.toggle",
            toggle,
            &text,
            ButtonRole::Quiet,
            true,
            &t,
            true,
        );
        if resp.clicked() {
            self.actions.push(Action::ToggleAdvanced);
        }
        if e.advanced_expanded {
            self.numeric_grid(ui, f, narrow, false);
        }
        if e.fixes.j24_edge_trace {
            self.check(
                ui,
                f,
                "adv.edge-trace",
                &e.tr.t("EdgeTrace"),
                e.draft.edge_trace,
                None,
                Action::SetEdgeTrace,
            );
        }
        self.section(ui, f, &e.tr.t("SafetyHead"));
        self.check(
            ui,
            f,
            "adv.safety",
            &e.tr.t("SafetyEscapes"),
            e.draft.safety_escapes,
            None,
            Action::SetSafety,
        );
        self.help(ui, f, &e.tr.t("SafetyEscapesHelp"), 30.0, "adv.safety.help");
        self.check(
            ui,
            f,
            "adv.descent",
            &e.tr.t("GuideDownward"),
            e.draft.descent_routing,
            None,
            Action::SetDescent,
        );
        self.help(ui, f, &e.tr.t("GuideDownwardHelp"), 30.0, "adv.descent.help");
        self.section(ui, f, &e.tr.t("UpperGroupHead"));
        let manual = e.is_manual_mode();
        let auto_r = f.take(28.0, M);
        if w::radio(
            ui,
            self.rec,
            "adv.auto",
            auto_r,
            &e.tr.t("UpperAutomatic"),
            !manual,
            true,
            &t,
        )
        .clicked()
            && manual
        {
            self.actions.push(Action::SetManualMode(false));
        }
        let manual_r = f.take(28.0, M);
        let manual_text = e.tr.f("UpperManualV2", &[&e.manual_choose_count().to_string()]);
        if w::radio(ui, self.rec, "adv.manual", manual_r, &manual_text, manual, true, &t).clicked() && !manual {
            self.actions.push(Action::SetManualMode(true));
        }
        if e.advanced_overview_only {
            return;
        }
        // The manual list (padding 24,0,0,8; each box 32 with margins).
        f.y += M;
        let rows = e.manual_rows();
        for (i, (m, checked, enabled, text)) in rows.iter().enumerate() {
            let r = Rect::from_min_size(pos2(f.x + 24.0, f.y + M), vec2((f.w - 24.0).max(200.0), 32.0));
            let resp = w::checkbox(
                ui,
                self.rec,
                &format!("adv.manual.{i}"),
                r,
                text,
                *checked,
                manual && *enabled,
                &t,
            );
            if resp.clicked() {
                self.actions
                    .push(Action::SetManualMonitor(m.stable_id.clone(), !checked));
            }
            f.y += 32.0 + M * 2.0;
        }
        f.y += 8.0 + M;
        self.section(ui, f, &e.tr.t("ResetBehavior"));
        self.help(ui, f, &e.tr.t("ResetBehaviorHelp"), 52.0, "adv.reset.help");
        let reset = Rect::from_min_size(pos2(f.x, f.y + M), vec2(286.0, 36.0));
        f.y += 36.0 + M * 2.0;
        if w::button(
            ui,
            self.rec,
            "adv.reset",
            reset,
            &e.tr.t("ResetBehavior"),
            ButtonRole::DangerText,
            true,
            &t,
            false,
        )
        .clicked()
        {
            self.actions.push(Action::ResetBehavior);
        }
    }

    /// C# `NumericRow` sizes: label 190 wide at (0,5); number host (194,2,82,36); "Use default" (282,2,98,32);
    /// help 375 wide at max(42, 5 + label + 4), at least 36 high. Two columns, one when narrow.
    fn numeric_grid(&mut self, ui: &mut Ui, f: &mut Flow, narrow: bool, intent_only: bool) {
        let e = self.editor;
        let t = *self.theme;
        let specs = e.numeric_specs();
        let cols = if narrow { 1 } else { 2 };
        let cell_w = (f.w / cols as f32).floor();
        let grid_top = f.y;
        let mut row_top = f.y;
        let mut row_h: f32 = 0.0;
        for (cell, (i, spec)) in specs
            .iter()
            .enumerate()
            .filter(|(i, _)| if intent_only { *i >= 7 } else { *i < 7 })
            .enumerate()
        {
            let col = cell % cols;
            if col == 0 && cell > 0 {
                row_top += row_h;
                row_h = 0.0;
            }
            let help_h = (w::text_height(ui, &spec.help, regular(9.0), 335.0, false).ceil() + 4.0).max(36.0);
            let label_h = (w::text_height(ui, &spec.label, regular(9.0), 190.0, false).ceil() + 4.0).max(28.0);
            let help_top = 42f32.max(5.0 + label_h + 4.0);
            let panel = Rect::from_min_size(
                pos2(f.x + col as f32 * cell_w, row_top),
                vec2(385.0f32.min(cell_w - 10.0).max(300.0), help_top + help_h),
            );
            row_h = row_h.max(panel.height() + 8.0);
            w::label(
                ui,
                Rect::from_min_size(panel.min + vec2(0.0, 5.0), vec2(190.0, label_h)),
                &spec.label,
                regular(9.0),
                t.text,
                HAlign::Left,
                false,
                true,
            );
            let host = Rect::from_min_size(panel.min + vec2(194.0, 2.0), vec2(82.0, 36.0));
            self.number_box(ui, i, host, spec.min, spec.max);
            let reset = Rect::from_min_size(panel.min + vec2(282.0, 2.0), vec2(98.0, 32.0));
            if w::button(
                ui,
                self.rec,
                &format!("adv.numeric.{i}.reset"),
                reset,
                &e.tr.t("UseDefault"),
                ButtonRole::Quiet,
                true,
                &t,
                false,
            )
            .clicked()
            {
                self.actions.push(Action::SetNumeric(i, spec.default));
                self.state.numbers[i] = spec.default.to_string();
            }
            let help = Rect::from_min_size(panel.min + vec2(0.0, help_top), vec2(375.0, help_h));
            w::label(ui, help, &spec.help, regular(9.0), t.subtle, HAlign::Left, false, true);
            self.rec
                .add(format!("adv.numeric.{i}.help"), help, spec.help.clone(), "");
            self.rec.add(
                format!("adv.numeric.{i}"),
                panel,
                spec.label.clone(),
                e.numeric_value(i).to_string(),
            );
        }
        f.y = row_top + row_h;
        self.rec.add(
            "adv.grid",
            Rect::from_min_max(pos2(f.x, grid_top), pos2(f.x + f.w, f.y)),
            "",
            "",
        );
    }

    /// C# `ThemedNumericUpDown` in its host with the spinner rail on the right.
    fn number_box(&mut self, ui: &mut Ui, i: usize, host: Rect, min: i32, max: i32) {
        let e = self.editor;
        let t = *self.theme;
        let id = numeric_id(i);
        let focused = ui.memory(|m| m.has_focus(id));
        let value = e.numeric_value(i);
        if !focused {
            self.state.numbers[i] = value.to_string();
        }
        w::frame(ui, host, focused, ui.rect_contains_pointer(host), false, true, &t);
        let text_rect = Rect::from_min_max(
            host.min + vec2(6.0, 4.0),
            pos2(host.right() - 21.0, host.bottom() - 4.0),
        );
        let buf = &mut self.state.numbers[i];
        let r = ui.put(
            text_rect,
            TextEdit::singleline(buf)
                .id(id)
                .frame(egui::Frame::NONE)
                .font(regular(9.0))
                .text_color(t.text)
                .desired_width(text_rect.width())
                .vertical_align(egui::Align::Center),
        );
        if r.changed() {
            buf.retain(|c| c.is_ascii_digit());
            if let Ok(v) = buf.parse::<i32>() {
                self.actions.push(Action::SetNumeric(i, v.clamp(min, max)));
            }
        }
        let mut step = 0;
        if r.has_focus() {
            ui.input(|inp| {
                for ev in &inp.events {
                    if let Event::Key { key, pressed: true, .. } = ev {
                        match key {
                            Key::ArrowUp => step += 1,
                            Key::ArrowDown => step -= 1,
                            _ => {}
                        }
                    }
                }
            });
        }
        // Spinner rail (C# ThemedSpinnerRail): upper half +1, lower half -1.
        let rail = Rect::from_min_max(
            pos2(host.right() - 20.0, host.top() + 1.0),
            pos2(host.right() - 1.0, host.bottom() - 1.0),
        );
        let up = Rect::from_min_max(rail.min, pos2(rail.right(), rail.center().y));
        let down = Rect::from_min_max(pos2(rail.left(), rail.center().y), rail.max);
        for (half, delta, tag) in [(up, 1, "up"), (down, -1, "down")] {
            let resp = ui.interact(half, Id::new(("spin", i, tag)), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(half, egui::CornerRadius::ZERO, t.accent_weak);
            }
            let c = half.center();
            let s = if delta > 0 { -1.0 } else { 1.0 };
            w::triangle(
                ui,
                c + vec2(-4.0, -2.0 * s),
                c + vec2(4.0, -2.0 * s),
                c + vec2(0.0, 2.0 * s),
                t.subtle,
            );
            if resp.clicked() {
                step += delta;
            }
            self.rec.add(format!("adv.numeric.{i}.{tag}"), half, "", "");
        }
        if step != 0 {
            let v = (value + step).clamp(min, max);
            self.actions.push(Action::SetNumeric(i, v));
            self.state.numbers[i] = v.to_string();
        }
        self.rec
            .add(format!("adv.numeric.{i}.box"), host, self.state.numbers[i].clone(), "");
    }

    // ------------------------------------------------------------------ Shortcuts

    fn shortcuts(&mut self, ui: &mut Ui, f: &mut Flow) {
        let e = self.editor;
        let t = *self.theme;
        self.header(ui, f, &e.tr.t("NavShortcuts"), &e.tr.t("ShortcutSubtitle"));
        let header = f.take(32.0, 0.0);
        let c1 = header.right() - 238.0 - 72.0;
        let c2 = header.right() - 72.0;
        w::label(
            ui,
            Rect::from_min_max(header.min, pos2(c1, header.bottom())),
            &e.tr.t("ShortcutActionColumn"),
            regular(9.0),
            t.faint,
            HAlign::Left,
            true,
            false,
        );
        w::label(
            ui,
            Rect::from_min_max(pos2(c1, header.top()), pos2(c2, header.bottom())),
            &e.tr.t("ShortcutColumn"),
            regular(9.0),
            t.faint,
            HAlign::Left,
            true,
            false,
        );
        w::label(
            ui,
            Rect::from_min_max(pos2(c2, header.top()), header.max),
            &e.tr.t("Clear"),
            regular(9.0),
            t.faint,
            HAlign::Center,
            true,
            false,
        );
        for i in 0..e.shortcut_count() {
            let row = f.take(83.0, 0.0);
            ui.painter()
                .line_segment([row.left_top(), row.right_top()], egui::Stroke::new(1.0, t.border));
            let inner = row.shrink2(vec2(0.0, 8.0));
            let required = e.tr.t(if i == 0 { "RequiredStatus" } else { "OptionalStatus" });
            let head = format!("{}  ·  {}", e.tr.t(SHORTCUT_HEADS[i]), required);
            let copy_w = (c1 - inner.left()).max(1.0);
            w::label(
                ui,
                Rect::from_min_size(inner.min + vec2(0.0, 4.0), vec2(copy_w.min(440.0), 22.0)),
                &head,
                semibold(9.5),
                t.text,
                HAlign::Left,
                true,
                false,
            );
            let mut help = e.tr.t(SHORTCUT_HINTS[i]);
            if i == 1 {
                help = format!("{} {}", help, e.tr.t("GameModeSuffix"));
            }
            w::label(
                ui,
                Rect::from_min_size(inner.min + vec2(0.0, 27.0), vec2(copy_w.min(445.0), 36.0)),
                &help,
                regular(9.0),
                t.subtle,
                HAlign::Left,
                false,
                true,
            );
            self.rec.add(format!("short.{i}.head"), row, head, "");
            let height = if e.fixes.j21_ui_keepers {
                36.0
            } else {
                inner.height() - 28.0
            };
            let top = inner.center().y - height / 2.0;
            let frame = Rect::from_min_max(pos2(c1, top), pos2(c2 - 8.0, top + height));
            self.shortcut_box(ui, i, frame);
            let clear = Rect::from_min_max(pos2(c2 + 4.0, top), pos2(inner.right(), top + height));
            let resp = w::button(
                ui,
                self.rec,
                &format!("short.{i}.clear"),
                clear,
                &e.tr.t("Clear"),
                ButtonRole::Quiet,
                i != 0,
                &t,
                false,
            );
            let resp = if i == 0 {
                resp.on_disabled_hover_text(e.tr.t("RequiredClearReason"))
            } else {
                resp
            };
            if resp.clicked() {
                self.actions.push(Action::ClearHotkey(i));
            }
        }
        self.help(ui, f, &e.tr.t("ShortcutsHealthy"), 32.0, "short.healthy");
    }

    /// One read-only capture box (C# `TextBox.KeyDown += CaptureHotkey` in a `ThemedControlFrame`).
    fn shortcut_box(&mut self, ui: &mut Ui, i: usize, frame: Rect) {
        let e = self.editor;
        let t = *self.theme;
        let id = shortcut_id(i);
        let resp = ui.interact(frame, id, Sense::click());
        if resp.clicked() {
            resp.request_focus();
        }
        let focused = resp.has_focus();
        let error = e.hotkey_errors[i];
        w::frame(ui, frame, focused, resp.hovered(), error, true, &t);
        let text = e.hotkey_text(i);
        w::label(
            ui,
            frame.shrink2(vec2(6.0, 0.0)),
            &text,
            semibold(9.0),
            t.text,
            HAlign::Center,
            true,
            false,
        );
        let name = e.tr.t(SHORTCUT_HEADS[i]);
        let description = e.hotkey_error_text[i].clone();
        resp.widget_info(|| {
            let mut info = egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, &name);
            info.current_text_value = Some(text.clone());
            info
        });
        if !description.is_empty() {
            let _ = resp.clone().on_hover_text(description.clone());
        }
        self.rec
            .add(format!("short.{i}.box"), frame, text, if error { "error" } else { "" });
        if !focused {
            self.state.shortcut_mods[i] = None;
            return;
        }
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                id,
                egui::EventFilter {
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    tab: false,
                    escape: false,
                },
            )
        });
        let (events, mods) = ui.input(|inp| (inp.events.clone(), inp.modifiers));
        let mut captured = false;
        for ev in events {
            if let Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = ev
            {
                if matches!(key, Key::Enter | Key::Escape | Key::Tab) {
                    continue;
                }
                if let Some(vk) = vk_for(key) {
                    self.actions.push(Action::CaptureHotkeyWithWin(
                        i,
                        vk,
                        modifiers.ctrl,
                        modifiers.alt,
                        modifiers.shift,
                        self.state.held_win || modifiers.mac_cmd,
                    ));
                    captured = true;
                }
            }
        }
        // A modifier pressed alone records the modifiers with no key, as C# did for ControlKey/Menu/ShiftKey.
        let previous = self.state.shortcut_mods[i];
        if !captured
            && previous.is_some_and(|p| p != mods)
            && (mods.ctrl || mods.alt || mods.shift || mods.mac_cmd)
            && (mods.ctrl && !previous.is_some_and(|p| p.ctrl)
                || mods.alt && !previous.is_some_and(|p| p.alt)
                || mods.shift && !previous.is_some_and(|p| p.shift)
                || mods.mac_cmd && !previous.is_some_and(|p| p.mac_cmd))
        {
            let vk = if mods.mac_cmd && !previous.is_some_and(|p| p.mac_cmd) {
                crate::editor::VK_LWIN
            } else if mods.shift && !previous.is_some_and(|p| p.shift) {
                crate::editor::VK_SHIFT
            } else if mods.alt && !previous.is_some_and(|p| p.alt) {
                crate::editor::VK_MENU
            } else {
                crate::editor::VK_CONTROL
            };
            self.actions.push(Action::CaptureHotkeyWithWin(
                i,
                vk,
                mods.ctrl,
                mods.alt,
                mods.shift,
                mods.mac_cmd,
            ));
        }
        self.state.shortcut_mods[i] = Some(mods);
    }

    // ------------------------------------------------------------------ General

    fn general(&mut self, ui: &mut Ui, f: &mut Flow, narrow: bool) {
        let e = self.editor;
        let t = *self.theme;
        let g = &e.general;
        self.header(ui, f, &e.tr.t("NavGeneral"), &e.tr.t("GeneralSubtitle"));
        self.section(ui, f, &e.tr.t("StartupHead"));
        self.check(
            ui,
            f,
            "gen.gate",
            &e.tr.t("StartGateClosed"),
            g.start_gate,
            Some(e.tr.t("StartGateClosedTip")),
            Action::SetStartGate,
        );
        self.check(
            ui,
            f,
            "gen.side",
            &e.tr.t("StartSideContainOn"),
            g.start_side,
            Some(e.tr.t("StartSideContainTip")),
            Action::SetStartSide,
        );
        self.check(
            ui,
            f,
            "gen.auto",
            &e.tr.t("StartWithWindows"),
            g.auto_start,
            Some(e.tr.t("StartWithWindowsTip")),
            Action::SetAutoStart,
        );
        let more = f.take(36.0, M);
        let more_text = format!(
            "{}{}",
            if e.more_startup_expanded { "⌄  " } else { "›  " },
            e.tr.t("MoreStartupChoices")
        );
        if w::button(
            ui,
            self.rec,
            "gen.more",
            more,
            &more_text,
            ButtonRole::Quiet,
            true,
            &t,
            true,
        )
        .clicked()
        {
            self.actions.push(Action::ToggleMoreStartup);
        }
        if e.more_startup_expanded {
            let panel = f.take(206.0, M);
            ui.painter().rect_filled(panel, egui::CornerRadius::ZERO, t.surface);
            let at = |x: f32, y: f32, w: f32, h: f32| {
                Rect::from_min_size(panel.min + vec2(x, y), vec2(w.min(panel.width() - x), h))
            };
            if w::checkbox(
                ui,
                self.rec,
                "gen.confine",
                at(0.0, 0.0, 790.0, 32.0),
                &e.tr.t("StartConfineOn"),
                g.start_confine,
                true,
                &t,
            )
            .clicked()
            {
                self.actions.push(Action::SetStartConfine(!g.start_confine));
            }
            w::label(
                ui,
                at(28.0, 31.0, 760.0, 30.0),
                &e.tr.t("StartConfineHelp"),
                regular(9.0),
                t.subtle,
                HAlign::Left,
                false,
                true,
            );
            if w::checkbox(
                ui,
                self.rec,
                "gen.paused",
                at(0.0, 64.0, 790.0, 32.0),
                &e.tr.t("StartPaused"),
                g.start_paused,
                true,
                &t,
            )
            .clicked()
            {
                self.actions.push(Action::SetStartPaused(!g.start_paused));
            }
            w::label(
                ui,
                at(28.0, 95.0, 760.0, 38.0),
                &e.tr.t("StartPausedHelp"),
                regular(9.0),
                t.subtle,
                HAlign::Left,
                false,
                true,
            );
            let warning = e.startup_warning();
            let wr = at(0.0, 138.0, 790.0, 64.0);
            w::label(ui, wr, &warning, regular(9.0), t.warning, HAlign::Left, false, true);
            self.rec.add("gen.warning", wr, warning, "");
        }
        self.section(ui, f, &e.tr.t("AppearanceHead"));
        let grid = f.take(if narrow { 156.0 } else { 78.0 }, M);
        let cells: [Rect; 2] = if narrow {
            [
                Rect::from_min_size(grid.min, vec2(grid.width(), 78.0)),
                Rect::from_min_size(grid.min + vec2(0.0, 78.0), vec2(grid.width(), 78.0)),
            ]
        } else {
            let half = (grid.width() / 2.0).floor();
            [
                Rect::from_min_size(grid.min, vec2(half, 78.0)),
                Rect::from_min_size(grid.min + vec2(half, 0.0), vec2(half, 78.0)),
            ]
        };
        let lang_items = vec![e.tr.t("LangAuto"), e.tr.t("LangEnglish"), e.tr.t("LangTurkish")];
        let theme_items = vec![e.tr.t("ThemeSystem"), e.tr.t("ThemeLight"), e.tr.t("ThemeDark")];
        for (k, (label, items, selected)) in [
            (e.tr.t("LanguageLabel"), lang_items, g.language),
            (e.tr.t("ThemeLabel"), theme_items, g.theme),
        ]
        .into_iter()
        .enumerate()
        {
            let cell = cells[k];
            w::label(
                ui,
                Rect::from_min_size(cell.min + vec2(3.0, 3.0), vec2(310.0f32.min(cell.width() - 6.0), 24.0)),
                &label,
                regular(9.0),
                t.subtle,
                HAlign::Left,
                true,
                false,
            );
            let combo = Rect::from_min_size(cell.min + vec2(3.0, 31.0), vec2(370.0f32.min(cell.width() - 6.0), 36.0));
            let salt = if k == 0 { "language" } else { "theme" };
            if let Some(i) = self.combo(ui, combo, salt, selected, &items) {
                self.actions.push(if k == 0 {
                    Action::SetLanguage(i)
                } else {
                    Action::SetTheme(i)
                });
            }
        }
        self.section(ui, f, &e.tr.t("BackupHead"));
        self.help(ui, f, &e.tr.t("BackupHelpV2"), 44.0, "gen.backup.help");
        let row = f.take(48.0, M);
        let export = Rect::from_min_size(row.min + vec2(3.0, 3.0), vec2(170.0, 36.0));
        let import = Rect::from_min_size(row.min + vec2(179.0, 3.0), vec2(170.0, 36.0));
        if w::button(
            ui,
            self.rec,
            "gen.export",
            export,
            &e.tr.t("ExportLabel"),
            ButtonRole::Secondary,
            true,
            &t,
            false,
        )
        .clicked()
        {
            self.actions.push(Action::Export);
        }
        if w::button(
            ui,
            self.rec,
            "gen.import",
            import,
            &e.tr.t("ImportLabel"),
            ButtonRole::Secondary,
            true,
            &t,
            false,
        )
        .clicked()
        {
            self.actions.push(Action::Import);
        }
    }
}
