//! Owner-drawn controls (ports of the C# themed controls in Ui/Theming.cs and SettingsForm.cs: ThemedButton,
//! ThemedCheckBox, ThemedControlFrame, ModeTile, NumberChip, FlatLinkLabel, the sidebar button and the close button)
//! drawn with an egui painter at explicit rectangles. Every interactive control is a real egui widget (focusable,
//! keyboard-activated, with AccessKit info) and is recorded for the headless tests.

use std::sync::Arc;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontFamily, FontId, Galley, Id, Pos2, Rect, Response, Sense, Shape, Stroke,
    StrokeKind, Ui, Vec2, WidgetInfo, WidgetType, pos2, text::LayoutJob, vec2,
};
use fence_types::Behavior;

use crate::editor::ButtonRole;
use crate::theme::Theme;

/// One recorded control: its tag, rectangle (points), visible text and a state word for the tests.
#[derive(Clone, Debug, PartialEq)]
pub struct Rec {
    pub tag: String,
    pub rect: Rect,
    pub text: String,
    pub state: String,
}

/// What the last frame drew, for the tests and the screenshot metrics.
#[derive(Clone, Debug, Default)]
pub struct Recorder {
    pub items: Vec<Rec>,
}

impl Recorder {
    pub fn clear(&mut self) {
        self.items.clear();
    }
    pub fn add(&mut self, tag: impl Into<String>, rect: Rect, text: impl Into<String>, state: impl Into<String>) {
        self.items.push(Rec {
            tag: tag.into(),
            rect,
            text: text.into(),
            state: state.into(),
        });
    }
    pub fn get(&self, tag: &str) -> Option<&Rec> {
        self.items.iter().rev().find(|r| r.tag == tag)
    }
    pub fn all(&self, prefix: &str) -> Vec<&Rec> {
        self.items.iter().filter(|r| r.tag.starts_with(prefix)).collect()
    }
}

/// WinForms points to egui points at 96 DPI (`pt * 96 / 72`).
pub const fn pt(points: f32) -> f32 {
    points * 4.0 / 3.0
}

pub fn regular(points: f32) -> FontId {
    FontId::new(pt(points), FontFamily::Proportional)
}

pub fn semibold(points: f32) -> FontId {
    FontId::new(pt(points), FontFamily::Name(SEMIBOLD.into()))
}

/// The family name of the semibold face (Segoe UI Semibold in the window; the default face in tests).
pub const SEMIBOLD: &str = "semibold";

/// Lay out text: `wrap` = Some(width) wraps, None keeps one line.
pub fn galley(ui: &Ui, text: &str, font: FontId, color: Color32, wrap: Option<f32>, italics: bool) -> Arc<Galley> {
    let mut job = LayoutJob::simple(text.to_string(), font, color, wrap.unwrap_or(f32::INFINITY));
    if italics {
        for s in &mut job.sections {
            s.format.italics = true;
        }
    }
    if wrap.is_none() {
        job.wrap.max_rows = 1;
    }
    ui.painter().layout_job(job)
}

/// The single-line width of a text (C# `TextRenderer.MeasureText(..., SingleLine | NoPadding)`).
pub fn text_width(ui: &Ui, text: &str, font: FontId) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_string(), font, Color32::WHITE)
        .size()
        .x
}

/// The wrapped height of a text at a width (C# `TextRenderer.MeasureText(..., new Size(width, 0), WordBreak)`).
pub fn text_height(ui: &Ui, text: &str, font: FontId, width: f32, italics: bool) -> f32 {
    galley(ui, text, font, Color32::WHITE, Some(width), italics).size().y
}

/// Horizontal placement of a label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HAlign {
    Left,
    Center,
    Right,
}

/// Paint text in a rectangle: one line with an end ellipsis, or wrapped (clipped to the rectangle).
#[allow(clippy::too_many_arguments)]
pub fn label(ui: &Ui, rect: Rect, text: &str, font: FontId, color: Color32, align: HAlign, v_center: bool, wrap: bool) {
    if text.is_empty() || rect.width() <= 0.0 {
        return;
    }
    let mut job = LayoutJob::simple(text.to_string(), font, color, rect.width().max(1.0));
    if !wrap {
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        job.wrap.overflow_character = Some('…');
    }
    job.halign = match align {
        HAlign::Left => egui::Align::LEFT,
        HAlign::Center => egui::Align::Center,
        HAlign::Right => egui::Align::RIGHT,
    };
    let g = ui.painter().layout_job(job);
    let x = match align {
        HAlign::Left => rect.left(),
        HAlign::Center => rect.center().x,
        HAlign::Right => rect.right(),
    };
    let y = if v_center {
        rect.center().y - g.size().y / 2.0
    } else {
        rect.top()
    };
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .galley(pos2(x, y), g, color);
}

/// Italic text (the drawer status line, C# `FontStyle.Italic`).
pub fn italic_label(ui: &Ui, rect: Rect, text: &str, font: FontId, color: Color32) {
    if text.is_empty() {
        return;
    }
    let g = galley(ui, text, font, color, Some(rect.width().max(1.0)), true);
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .galley(rect.min, g, color);
}

fn interact(ui: &Ui, rect: Rect, id: Id, enabled: bool) -> Response {
    ui.interact(rect, id, if enabled { Sense::click() } else { Sense::hover() })
}

fn focus_ring(ui: &Ui, rect: Rect, theme: &Theme, radius: u8) {
    ui.painter().rect_stroke(
        rect.shrink(2.0),
        CornerRadius::same(radius),
        Stroke::new(2.0, theme.focus_ring),
        StrokeKind::Inside,
    );
}

/// C# `ThemedButton` with its four roles.
#[allow(clippy::too_many_arguments)]
pub fn button(
    ui: &Ui,
    rec: &mut Recorder,
    tag: &str,
    rect: Rect,
    text: &str,
    role: ButtonRole,
    enabled: bool,
    theme: &Theme,
    align_left: bool,
) -> Response {
    let r = interact(ui, rect, Id::new(("btn", tag)), enabled);
    let mut fill = if role == ButtonRole::Primary {
        theme.accent
    } else {
        theme.surface2
    };
    let mut ink = match role {
        ButtonRole::Primary => theme.accent_text(),
        ButtonRole::DangerText => theme.danger,
        _ => theme.text,
    };
    let mut border = if matches!(role, ButtonRole::Quiet | ButtonRole::DangerText) {
        Color32::TRANSPARENT
    } else {
        theme.border_strong
    };
    if !enabled {
        fill = theme.disabled_surface;
        ink = theme.faint;
        border = if role == ButtonRole::Quiet {
            Color32::TRANSPARENT
        } else {
            theme.border
        };
    } else if r.is_pointer_button_down_on() {
        fill = if role == ButtonRole::Primary {
            theme.accent_pressed()
        } else {
            theme.accent_weak
        };
    } else if r.hovered() {
        fill = if role == ButtonRole::Primary {
            theme.accent_hover()
        } else {
            theme.accent_weak
        };
    }
    let p = ui.painter();
    p.rect(
        rect,
        CornerRadius::same(5),
        fill,
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    if align_left {
        label(
            ui,
            rect.shrink2(vec2(10.0, 0.0)),
            text,
            regular(9.0),
            ink,
            HAlign::Left,
            true,
            false,
        );
    } else {
        label(
            ui,
            rect.shrink2(vec2(6.0, 0.0)),
            text,
            regular(9.0),
            ink,
            HAlign::Center,
            true,
            false,
        );
    }
    if r.has_focus() {
        focus_ring(ui, rect, theme, 4);
    }
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, text));
    let role_word = format!("{role:?}");
    rec.add(
        tag,
        rect,
        text,
        format!("{}{}", if enabled { "enabled " } else { "disabled " }, role_word),
    );
    r
}

/// C# `ThemedCheckBox`: an 18-point box, the check mark in the accent text color, the label after it.
#[allow(clippy::too_many_arguments)]
pub fn checkbox(
    ui: &Ui,
    rec: &mut Recorder,
    tag: &str,
    rect: Rect,
    text: &str,
    checked: bool,
    enabled: bool,
    theme: &Theme,
) -> Response {
    let r = interact(ui, rect, Id::new(("check", tag)), enabled);
    let b = 18.0;
    let boxr = Rect::from_min_size(pos2(rect.left() + 2.0, rect.center().y - b / 2.0), vec2(b, b));
    let fill = if !enabled {
        theme.disabled_surface
    } else if checked {
        theme.accent
    } else {
        theme.surface2
    };
    let border = if r.has_focus() {
        theme.accent
    } else if r.hovered() {
        theme.subtle
    } else {
        theme.border_strong
    };
    let p = ui.painter();
    p.rect(
        boxr,
        CornerRadius::ZERO,
        fill,
        Stroke::new(if r.has_focus() { 2.0 } else { 1.0 }, border),
        StrokeKind::Inside,
    );
    if checked {
        let ink = if enabled { theme.accent_text() } else { theme.faint };
        let pts = vec![
            pos2(boxr.left() + b / 4.0, boxr.top() + b / 2.0),
            pos2(boxr.left() + b * 7.0 / 16.0, boxr.top() + b * 11.0 / 16.0),
            pos2(boxr.left() + b * 3.0 / 4.0, boxr.top() + b / 3.0),
        ];
        p.add(Shape::line(pts, Stroke::new((b / 8.0).max(2.0), ink)));
    }
    let text_rect = Rect::from_min_max(pos2(boxr.right() + 9.0, rect.top()), rect.max);
    label(
        ui,
        text_rect,
        text,
        regular(9.0),
        if enabled { theme.text } else { theme.faint },
        HAlign::Left,
        true,
        false,
    );
    r.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, checked, text));
    rec.add(
        tag,
        rect,
        text,
        format!(
            "{}{}",
            if checked { "checked" } else { "unchecked" },
            if enabled { "" } else { " disabled" }
        ),
    );
    r
}

/// A radio choice (WinForms `RadioButton`, themed like the check box).
#[allow(clippy::too_many_arguments)]
pub fn radio(
    ui: &Ui,
    rec: &mut Recorder,
    tag: &str,
    rect: Rect,
    text: &str,
    checked: bool,
    enabled: bool,
    theme: &Theme,
) -> Response {
    let r = interact(ui, rect, Id::new(("radio", tag)), enabled);
    let c = pos2(rect.left() + 10.0, rect.center().y);
    let p = ui.painter();
    let ring = if !enabled {
        theme.border
    } else if r.has_focus() || r.hovered() {
        theme.accent
    } else {
        theme.border_strong
    };
    p.circle(
        c,
        7.5,
        if enabled {
            theme.surface2
        } else {
            theme.disabled_surface
        },
        Stroke::new(1.0, ring),
    );
    if checked {
        p.circle_filled(c, 4.0, if enabled { theme.accent } else { theme.faint });
    }
    let text_rect = Rect::from_min_max(pos2(rect.left() + 24.0, rect.top()), rect.max);
    label(
        ui,
        text_rect,
        text,
        regular(9.0),
        if enabled { theme.text } else { theme.faint },
        HAlign::Left,
        true,
        false,
    );
    r.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, enabled, checked, text));
    rec.add(
        tag,
        rect,
        text,
        format!(
            "{}{}",
            if checked { "checked" } else { "unchecked" },
            if enabled { "" } else { " disabled" }
        ),
    );
    r
}

/// C# `FlatLinkLabel`: accent text that acts as a link; disabled text is flat subtle ink.
#[allow(clippy::too_many_arguments)]
pub fn link(
    ui: &Ui,
    rec: &mut Recorder,
    tag: &str,
    rect: Rect,
    text: &str,
    enabled: bool,
    theme: &Theme,
    align: HAlign,
) -> Response {
    let r = interact(ui, rect, Id::new(("link", tag)), enabled);
    let ink = if enabled { theme.accent } else { theme.subtle };
    label(ui, rect, text, regular(9.0), ink, align, true, false);
    if enabled && r.hovered() {
        let w = text_width(ui, text, regular(9.0)).min(rect.width());
        let x0 = match align {
            HAlign::Left => rect.left(),
            HAlign::Center => rect.center().x - w / 2.0,
            HAlign::Right => rect.right() - w,
        };
        let y = rect.center().y + pt(9.0) * 0.6;
        ui.painter()
            .line_segment([pos2(x0, y), pos2(x0 + w, y)], Stroke::new(1.0, ink));
    }
    if r.has_focus() {
        focus_ring(ui, rect, theme, 3);
    }
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Link, enabled, text));
    rec.add(tag, rect, text, if enabled { "enabled" } else { "disabled" });
    r
}

/// The glyph of a behaviour (C# `ModeTile.DrawGlyph` and `MonitorMapControl.DrawModeGlyph`).
pub fn mode_glyph(ui: &Ui, center: Pos2, behavior: Behavior, color: Color32, scale: f32, blocked_color: Color32) {
    let p = ui.painter();
    let n = (6.0 * scale).max(5.0).floor();
    match behavior {
        Behavior::Blocked => {
            p.line_segment(
                [center - vec2(n + 2.0, 0.0), center + vec2(n + 2.0, 0.0)],
                Stroke::new((5.0 * scale).max(3.0), blocked_color),
            );
        }
        Behavior::Push => {
            let pts = vec![
                center - vec2(0.0, n + 2.0),
                center + vec2(n + 2.0, 0.0),
                center + vec2(0.0, n + 2.0),
                center - vec2(n + 2.0, 0.0),
            ];
            p.add(Shape::convex_polygon(pts, color, Stroke::NONE));
        }
        Behavior::Free => {
            p.circle_stroke(center, n + 1.0, Stroke::new(2.0 * scale.max(1.0), color));
        }
    }
}

/// C# `ModeTile`: a choice tile with a glyph, a bold title and an optional help line.
#[allow(clippy::too_many_arguments)]
pub fn tile(
    ui: &Ui,
    rec: &mut Recorder,
    tag: &str,
    rect: Rect,
    behavior: Behavior,
    title: &str,
    help: &str,
    enabled: bool,
    selected: bool,
    theme: &Theme,
) -> Response {
    let r = interact(ui, rect, Id::new(("tile", tag)), enabled);
    let fill = if !enabled {
        theme.disabled_surface
    } else if selected {
        theme.accent_weak
    } else {
        theme.surface
    };
    let border = if enabled && selected {
        theme.accent
    } else {
        theme.border
    };
    let title_ink = if !enabled {
        theme.faint
    } else if selected {
        theme.accent
    } else {
        theme.text
    };
    let body = rect.shrink(1.0);
    ui.painter().rect(
        body,
        CornerRadius::same(8),
        fill,
        Stroke::new(if enabled && selected { 1.8 } else { 1.0 }, border),
        StrokeKind::Inside,
    );
    let glyph_color = if enabled { theme.accent } else { theme.faint };
    let blocked_color = if enabled { theme.text } else { theme.faint };
    mode_glyph(
        ui,
        pos2(rect.left() + 22.0, rect.center().y),
        behavior,
        glyph_color,
        1.0,
        blocked_color,
    );
    let short = rect.height() < 60.0 && !help.is_empty();
    let left = rect.left() + 44.0;
    let w = (rect.width() - 52.0).max(1.0);
    let title_rect = if help.is_empty() {
        Rect::from_min_size(pos2(left, rect.top() + 4.0), vec2(w, rect.height() - 8.0))
    } else if short {
        Rect::from_min_size(pos2(left, rect.top() + 2.0), vec2(w, 19.0))
    } else {
        Rect::from_min_size(pos2(left, rect.top() + 10.0), vec2(w, 22.0))
    };
    label(
        ui,
        title_rect,
        title,
        semibold(9.0),
        title_ink,
        HAlign::Left,
        true,
        false,
    );
    if !help.is_empty() {
        let help_rect = if short {
            Rect::from_min_size(pos2(left, rect.top() + 21.0), vec2(w, rect.height() - 24.0))
        } else {
            Rect::from_min_size(pos2(left, rect.top() + 31.0), vec2(w, rect.height() - 35.0))
        };
        label(
            ui,
            help_rect,
            help,
            regular(9.0),
            if enabled { theme.subtle } else { theme.faint },
            HAlign::Left,
            false,
            !short,
        );
    }
    if enabled && r.has_focus() {
        focus_ring(ui, rect.shrink(2.0), theme, 6);
    }
    let name = if help.is_empty() {
        title.to_string()
    } else {
        format!("{title}. {help}")
    };
    r.widget_info(|| WidgetInfo::selected(WidgetType::Button, enabled, selected, &name));
    rec.add(
        tag,
        rect,
        title,
        format!(
            "{}{}",
            if selected { "selected" } else { "unselected" },
            if enabled { "" } else { " disabled" }
        ),
    );
    r
}

/// C# `ThemedControlFrame` around a text box or shortcut box: border, focus ring, error ring.
pub fn frame(ui: &Ui, rect: Rect, focused: bool, hovered: bool, error: bool, enabled: bool, theme: &Theme) {
    let p = ui.painter();
    p.rect_filled(
        rect,
        CornerRadius::same(5),
        if enabled {
            theme.surface2
        } else {
            theme.disabled_surface
        },
    );
    let border = if error {
        theme.danger
    } else if focused {
        theme.accent
    } else if hovered {
        theme.subtle
    } else {
        theme.border_strong
    };
    p.rect_stroke(
        rect,
        CornerRadius::same(5),
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    if error || focused {
        let ring = if error {
            theme.danger.gamma_multiply(110.0 / 255.0)
        } else {
            theme.focus_ring
        };
        p.rect_stroke(
            rect.shrink(2.0),
            CornerRadius::same(4),
            Stroke::new(2.0, ring),
            StrokeKind::Inside,
        );
    }
}

/// C# `NumberChip`: the screen number on a rounded chip; the primary's chip is accent-tinted.
pub fn number_chip(ui: &Ui, rect: Rect, number: u32, accent: bool, theme: &Theme) {
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(6),
        if accent { theme.accent_weak } else { theme.chip },
    );
    label(
        ui,
        rect,
        &number.to_string(),
        semibold(9.0),
        if accent { theme.accent } else { theme.chip_text },
        HAlign::Center,
        true,
        false,
    );
}

/// C# `DrawerCloseButton`: a round "×" button.
pub fn close_button(ui: &Ui, rec: &mut Recorder, tag: &str, rect: Rect, name: &str, theme: &Theme) -> Response {
    let r = interact(ui, rect, Id::new(("close", tag)), true);
    if r.hovered() {
        ui.painter().circle_filled(
            rect.center(),
            rect.width().min(rect.height()) / 2.0 - 1.0,
            theme.accent_weak,
        );
    }
    label(
        ui,
        rect,
        "×",
        regular(12.0),
        if r.hovered() { theme.accent } else { theme.subtle },
        HAlign::Center,
        true,
        false,
    );
    if r.has_focus() {
        focus_ring(ui, rect, theme, 8);
    }
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, name));
    rec.add(tag, rect, name, "enabled");
    r
}

/// C# `SidebarButton`.
#[allow(clippy::too_many_arguments)]
pub fn sidebar_button(
    ui: &Ui,
    rec: &mut Recorder,
    tag: &str,
    rect: Rect,
    text: &str,
    selected: bool,
    theme: &Theme,
) -> Response {
    let r = interact(ui, rect, Id::new(("nav", tag)), true);
    let fill = if selected {
        theme.accent_weak
    } else if r.hovered() {
        theme.surface2
    } else {
        theme.sidebar()
    };
    ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
    let ink = if selected {
        theme.accent
    } else if r.hovered() {
        theme.text
    } else {
        theme.subtle
    };
    let text_rect = Rect::from_min_max(
        pos2(rect.left() + 10.0, rect.top()),
        pos2(rect.right() - 8.0, rect.bottom()),
    );
    label(ui, text_rect, text, semibold(9.0), ink, HAlign::Left, true, false);
    if r.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(4.0),
            CornerRadius::same(3),
            Stroke::new(1.0, theme.text),
            StrokeKind::Inside,
        );
    }
    r.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, selected, text));
    rec.add(tag, rect, text, if selected { "selected" } else { "unselected" });
    r
}

/// Rounded rectangle helper (C# `Rounded`).
pub fn rounded(ui: &Ui, rect: Rect, radius: u8, fill: Color32, stroke: Stroke) {
    ui.painter()
        .rect(rect, CornerRadius::same(radius), fill, stroke, StrokeKind::Inside);
}

/// Diagonal hatch inside a rectangle (C# `HatchBrush(ForwardDiagonal)`).
pub fn hatch(ui: &Ui, rect: Rect, color: Color32) {
    let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let step = 8.0;
    let mut x = rect.left() - rect.height();
    while x < rect.right() {
        p.line_segment(
            [pos2(x, rect.bottom()), pos2(x + rect.height(), rect.top())],
            Stroke::new(1.0, color),
        );
        x += step;
    }
}

/// A filled triangle.
pub fn triangle(ui: &Ui, a: Pos2, b: Pos2, c: Pos2, color: Color32) {
    ui.painter()
        .add(Shape::convex_polygon(vec![a, b, c], color, Stroke::NONE));
}

/// Text anchored at a point (for small map badges).
pub fn text_at(ui: &Ui, pos: Pos2, anchor: Align2, text: &str, font: FontId, color: Color32) -> Rect {
    ui.painter().text(pos, anchor, text, font, color)
}

pub fn size(w: f32, h: f32) -> Vec2 {
    vec2(w, h)
}
