//! The Displays page: heading, the screen map with its clickable gates, the boundary drawer and the screen list
//! (C# `SettingsForm` constructor, SettingsForm.Displays.cs, SettingsForm.BoundaryEditor.cs, MonitorMapControl.cs).

use std::collections::HashMap;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, Event, Id, Key, Pos2, Rect, Sense, Stroke, StrokeKind, TextEdit, Ui,
    UiBuilder, accesskit, pos2, vec2,
};
use fence_types::{Behavior, Control, MonitorInfo, Rect as IRect};

use super::Action;
use super::widgets::{self as w, HAlign, Recorder, regular, semibold};
use crate::boundary::{self, BoundaryView, MapTexts, contains_id, same};
use crate::editor::{ButtonRole, Editor, TILE_BEHAVIORS};
use crate::theme::Theme;

/// C# `SystemInformation.VerticalScrollBarWidth` at 96 DPI.
pub const SCROLLBAR: i32 = 17;
pub const SIDEBAR_W: f32 = 168.0;
pub const FOOTER_H: f32 = 64.0;
/// C# `WideContentMeasure`.
pub const WIDE_CONTENT: i32 = 1240;

/// Keep a readable name above an overlapping gate pill when sixteen logical pixels remain.
pub fn name_above_pill(label: Rect, pill: Rect) -> Option<Rect> {
    name_above_pill_with_fixes(label, pill, fence_engine::fixes::Fixes::ALL)
}

pub fn name_above_pill_with_fixes(label: Rect, pill: Rect, fixes: fence_engine::fixes::Fixes) -> Option<Rect> {
    if !fixes.j21_ui_keepers {
        return None;
    }
    let bottom = pill.top() - 4.0;
    (bottom - label.top() >= 16.0).then(|| Rect::from_min_max(label.min, pos2(label.right(), bottom)))
}

/// C# `ResizeV2Layout` horizontal math for one page: the content width and its centred padding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    pub narrow: bool,
    pub width: i32,
    pub left: i32,
    pub right: i32,
}

pub fn measure(client_w: i32, page_client_w: i32) -> Measure {
    let narrow = client_w < 900;
    let horizontal = if narrow { 18 } else { 28 };
    let viewport = 420.max(page_client_w - SCROLLBAR - 4);
    let width = 420.max(WIDE_CONTENT.min(viewport - horizontal * 2));
    let spare = 0.max(viewport - width);
    let left = horizontal.max(spare / 2);
    let right = horizontal.max(spare - left);
    Measure {
        narrow,
        width,
        left,
        right,
    }
}

/// The vertical composition of the Displays page (C# `ResizeV2Layout` and `RestingBoundaryDrawerHeight`).
#[derive(Clone, Debug, PartialEq)]
pub struct DisplaysGeometry {
    pub m: Measure,
    pub map_h: i32,
    pub status_h: i32,
    pub tiles_h: i32,
    pub drawer_resting_h: i32,
    pub drawer_h: i32,
    pub rows_h: i32,
    pub row_pitch: i32,
    pub content_h: i32,
    pub body_h: i32,
}

pub const PAD_TOP: i32 = 23;
pub const PAD_BOTTOM: i32 = 32;
pub const HEADER_H: i32 = 72;
pub const HEADER_MARGIN: i32 = 10;
pub const HINT_H: i32 = 38;
pub const SECTION_H: i32 = 48;
pub const SECTION_MARGIN: i32 = 5;
pub const ROW_HEADER: i32 = 31;
pub const DRAWER_PAD_X: i32 = 14;
pub const DRAWER_PAD_Y: i32 = 12;
pub const D_HEADER: i32 = 51;
pub const D_QUESTION: i32 = 29;
pub const D_RETURN: i32 = 20;
pub const D_FOLDS: i32 = 31;
pub const D_ALLOW: i32 = 32 + 6;
pub const D_SEPARATE_ROW: i32 = 78;
pub const D_WHEN: i32 = 94;

/// Inputs to the Displays composition.
pub struct DisplaysInput {
    pub client_w: i32,
    pub client_h: i32,
    pub monitors: usize,
    pub drawer_visible: bool,
    pub list_hidden: bool,
    pub separate_rows: usize,
    pub when_expanded: bool,
    /// The reserved status height (C# `ReserveDrawerStatusHeight`), measured by the caller.
    pub status_h: i32,
}

pub fn displays_geometry(i: &DisplaysInput) -> DisplaysGeometry {
    let page_w = i.client_w - SIDEBAR_W as i32;
    let m = measure(i.client_w, page_w);
    let body_h = i.client_h - FOOTER_H as i32;
    let tiles_h = if m.narrow { 250 } else { 84 };
    let fixed = D_HEADER + D_QUESTION + tiles_h + D_RETURN + i.status_h + D_FOLDS + D_ALLOW;
    let drawer_resting_h = 245.max(DRAWER_PAD_Y * 2 + 2 + fixed);
    let extra = (i.separate_rows as i32) * D_SEPARATE_ROW + if i.when_expanded { D_WHEN } else { 0 };
    let drawer_h = 245.max(DRAWER_PAD_Y * 2 + 2 + fixed + extra);
    let row_pitch = if m.narrow { 72 } else { 53 };
    let rows_h = 1.max(ROW_HEADER + (i.monitors as i32) * row_pitch);
    let list_h = SECTION_H + SECTION_MARGIN + rows_h;
    let non_map = PAD_TOP + PAD_BOTTOM + HEADER_H + HEADER_MARGIN + HINT_H + drawer_resting_h.max(list_h);
    let leftover = body_h - non_map;
    let floor = 220.max((body_h as f64 * 0.39).round() as i32);
    let ceiling = floor.max((body_h as f64 * 0.59).round() as i32);
    let map_h = leftover.clamp(floor, ceiling);
    let mut content_h = PAD_TOP + HEADER_H + HEADER_MARGIN + map_h + HINT_H;
    if i.drawer_visible {
        content_h += drawer_h;
    }
    if !i.list_hidden {
        content_h += SECTION_MARGIN + SECTION_H + rows_h;
    }
    content_h += PAD_BOTTOM;
    DisplaysGeometry {
        m,
        map_h,
        status_h: i.status_h,
        tiles_h,
        drawer_resting_h,
        drawer_h,
        rows_h,
        row_pitch,
        content_h,
        body_h,
    }
}

/// Map interaction state that is not part of the settings (C# `MonitorMapControl` private fields).
#[derive(Clone, Debug, Default)]
pub struct MapState {
    pub hovered_boundary: Option<usize>,
    pub hovered_monitor: Option<String>,
    pub external_hover: Option<String>,
    pub last_click: Option<(Pos2, f64)>,
    pub cluster_index: usize,
}

/// Name boxes of the screen rows (reloaded from the draft when C# would rebuild the rows).
#[derive(Clone, Debug, Default)]
pub struct NameBoxes {
    pub version: Option<u64>,
    pub text: HashMap<String, String>,
}

pub struct DisplaysView<'a> {
    pub editor: &'a Editor,
    pub theme: &'a Theme,
    pub rec: &'a mut Recorder,
    pub actions: &'a mut Vec<Action>,
    pub map: &'a mut MapState,
    pub names: &'a mut NameBoxes,
    pub client: (i32, i32),
    pub scroll_to_drawer: bool,
}

/// The reserved height of the drawer's status line (C# `ReserveDrawerStatusHeight`).
pub fn status_reserve(ui: &Ui, editor: &Editor, width: f32) -> i32 {
    let font = regular(8.5);
    let w = width.max(220.0);
    let tallest = editor
        .drawer_status_candidates()
        .iter()
        .map(|t| w::text_height(ui, t, font.clone(), w, true))
        .fold(0.0f32, f32::max);
    tallest.ceil() as i32 + 4
}

/// The Displays composition for the current editor state at a client size (also computed while another page is
/// shown, as C# laid out the hidden tab page too; the screenshot metrics read it).
pub fn geometry(ui: &Ui, e: &Editor, client: (i32, i32)) -> DisplaysGeometry {
    let probe = measure(client.0, client.0 - SIDEBAR_W as i32);
    let drawer_child_w = 430.max(probe.width - 28);
    let status_h = status_reserve(ui, e, drawer_child_w as f32);
    displays_geometry(&DisplaysInput {
        client_w: client.0,
        client_h: client.1,
        monitors: e.monitors.len(),
        drawer_visible: e.drawer_visible,
        list_hidden: e.screen_list_hidden,
        separate_rows: e.drawer.separate_rows.len(),
        when_expanded: e.when_expanded,
        status_h,
    })
}

impl DisplaysView<'_> {
    pub fn show(&mut self, ui: &mut Ui, page: Rect) -> DisplaysGeometry {
        let g = geometry(ui, self.editor, self.client);
        let scroll_to = self.scroll_to_drawer;
        ui.scope_builder(UiBuilder::new().max_rect(page).id_salt("displays-page"), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("displays-scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let (content, _) = ui.allocate_exact_size(vec2(page.width(), g.content_h as f32), Sense::hover());
                    self.draw(ui, content, &g, page, scroll_to);
                });
        });
        g
    }

    fn draw(&mut self, ui: &mut Ui, content: Rect, g: &DisplaysGeometry, page: Rect, scroll_to: bool) {
        let e = self.editor;
        let t = self.theme;
        let x = content.left() + g.m.left as f32;
        let wdt = g.m.width as f32;
        let mut y = content.top() + PAD_TOP as f32;
        self.rec
            .add("body", page, "", format!("padding {} {}", g.m.left, g.m.right));

        // Heading row (C# mapHeader).
        let header = Rect::from_min_size(pos2(x, y), vec2(wdt, HEADER_H as f32));
        w::label(
            ui,
            Rect::from_min_size(header.min, vec2(wdt - 170.0, 30.0)),
            &e.tr.t("DisplaysTitle"),
            semibold(18.0),
            t.text,
            HAlign::Left,
            false,
            false,
        );
        w::label(
            ui,
            Rect::from_min_size(header.min + vec2(0.0, 33.0), vec2(wdt - 170.0, 23.0)),
            &e.tr.t("DisplaysSubtitle"),
            regular(9.0),
            t.subtle,
            HAlign::Left,
            false,
            false,
        );
        let identify = Rect::from_min_size(
            pos2(header.right() - 154.0, header.center().y - 18.0),
            vec2(154.0, 36.0),
        );
        if w::button(
            ui,
            self.rec,
            "identify",
            identify,
            &e.tr.t("IdentifyAllScreensM2"),
            ButtonRole::Secondary,
            true,
            t,
            false,
        )
        .clicked()
        {
            self.actions.push(Action::Identify);
        }
        self.rec.add("header", header, "", "");
        y += (HEADER_H + HEADER_MARGIN) as f32;

        // The map.
        let map_rect = Rect::from_min_size(pos2(x, y), vec2(wdt, g.map_h as f32));
        self.draw_map(ui, map_rect);
        y += g.map_h as f32;

        // Hint line.
        let hint = Rect::from_min_size(pos2(x, y), vec2(wdt, HINT_H as f32));
        w::label(ui, hint, &e.map_hint, regular(9.0), t.subtle, HAlign::Left, true, false);
        self.rec.add("hint", hint, e.map_hint.clone(), "");
        y += HINT_H as f32;

        if e.drawer_visible {
            let drawer = Rect::from_min_size(pos2(x, y), vec2(wdt, g.drawer_h as f32));
            self.draw_drawer(ui, drawer, g);
            if scroll_to {
                // C# OpenBoundary: scroll the drawer into view, but never past the map hint above it.
                let bottom = drawer.bottom().min(hint.top() + page.height());
                ui.scroll_to_rect(Rect::from_min_max(pos2(x, hint.top()), pos2(x + wdt, bottom)), None);
            }
            y += g.drawer_h as f32;
        }
        if !e.screen_list_hidden {
            y += SECTION_MARGIN as f32;
            let section = Rect::from_min_size(pos2(x, y), vec2(wdt, SECTION_H as f32));
            self.draw_section(ui, section);
            y += SECTION_H as f32;
            let rows = Rect::from_min_size(pos2(x, y), vec2(wdt, g.rows_h as f32));
            self.draw_rows(ui, rows, g);
        }
        // The page's last visible block (C# DisplayLayoutMetrics) in page coordinates.
        let last = self
            .rec
            .items
            .iter()
            .filter(|r| r.tag == "drawer" || r.tag == "rows" || r.tag == "section")
            .map(|r| r.rect.bottom())
            .fold(f32::MIN, f32::max);
        if last > f32::MIN {
            self.rec.add(
                "last-block",
                Rect::from_min_max(pos2(x, page.top()), pos2(x + wdt, last)),
                "",
                "",
            );
        }
    }

    // ------------------------------------------------------------------ map

    fn map_rects(&self, size: (i32, i32)) -> Vec<IRect> {
        let e = self.editor;
        let bounds: Vec<IRect> = e.monitors.iter().map(|m| m.bounds).collect();
        let mut rects = e.backend().map_layout(&bounds, size.0, size.1, 14);
        rects.truncate(e.monitors.len());
        boundary::ensure_minimum_footprints(&mut rects, size.0, size.1);
        rects
    }

    fn draw_map(&mut self, ui: &mut Ui, map: Rect) {
        let e = self.editor;
        let t = *self.theme;
        let map_id = Id::new("monitor-map");
        let size = (map.width().round() as i32, map.height().round() as i32);
        let rects = self.map_rects(size);
        let mut views: Vec<BoundaryView> = e.boundaries.clone();
        boundary::position_boundaries(&mut views, &e.monitors, &rects);
        let to_abs = |r: IRect| {
            Rect::from_min_max(
                pos2(map.left() + r.l as f32, map.top() + r.t as f32),
                pos2(map.left() + r.r as f32, map.top() + r.b as f32),
            )
        };
        let to_pos = |p: (i32, i32)| pos2(map.left() + p.0 as f32, map.top() + p.1 as f32);

        let response = ui.interact(map, map_id, Sense::click());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, e.tr.t("TabScreens")));
        let names = |id: &str| e.monitor_name_for(id);
        let texts = MapTexts {
            tr: e.tr,
            settings: &e.draft,
            name_for_id: &names,
        };

        // Hover (C# OnMouseMove / OnMouseLeave).
        let local = |p: Pos2| ((p.x - map.left()).floor() as i32, (p.y - map.top()).floor() as i32);
        if let Some(p) = response.hover_pos() {
            let lp = local(p);
            let hb = boundary::hit_boundaries(&views, lp).first().copied();
            let hm = if hb.is_none() {
                boundary::hit_monitor(&rects, lp, false).map(|i| e.monitors[i].stable_id.clone())
            } else {
                None
            };
            self.map.hovered_boundary = hb;
            self.map.hovered_monitor = hm;
        } else {
            self.map.hovered_boundary = None;
            self.map.hovered_monitor = None;
        }

        // Clicks (C# OnMouseDown) and keyboard (C# OnKeyDown) -------------------------------------------
        let mouse_click = response.clicked_by(egui::PointerButton::Primary);
        if mouse_click && let Some(p) = response.interact_pointer_pos() {
            let lp = local(p);
            let hits = boundary::hit_boundaries(&views, lp);
            if !hits.is_empty() {
                let now = ui.input(|i| i.time);
                let cycle = self
                    .map
                    .last_click
                    .is_some_and(|(q, at)| now - at <= 0.9 && (q - p).length_sq() <= 16.0);
                self.map.cluster_index = if cycle {
                    (self.map.cluster_index + 1) % hits.len()
                } else {
                    0
                };
                self.map.last_click = Some((p, now));
                self.actions.push(Action::OpenBoundary(hits[self.map.cluster_index]));
            } else if let Some(i) = boundary::hit_monitor(&rects, lp, true) {
                self.actions
                    .push(Action::MonitorChosen(e.monitors[i].stable_id.clone()));
            }
        } else if response.clicked() {
            // Enter/Space on the focused map, or an AccessKit click on the map itself.
            if let Some(i) = e.map_selected_boundary {
                self.actions.push(Action::OpenBoundary(i));
            } else if let Some(m) = e
                .map_selected_monitor
                .as_ref()
                .and_then(|id| e.monitors.iter().find(|m| same(&m.stable_id, id)))
                .or(e.monitors.first())
            {
                self.actions.push(Action::MonitorChosen(m.stable_id.clone()));
            }
        }
        if response.has_focus() {
            ui.memory_mut(|mem| {
                mem.set_focus_lock_filter(
                    map_id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        tab: false,
                        escape: false,
                    },
                )
            });
            let keys: Vec<Key> = ui.input(|i| {
                i.events
                    .iter()
                    .filter_map(|ev| match ev {
                        Event::Key { key, pressed: true, .. } => Some(*key),
                        _ => None,
                    })
                    .collect()
            });
            for key in keys {
                self.map_key(key, &views);
            }
        }

        // Paint (C# OnPaint) -----------------------------------------------------------------------------
        let painter = ui.painter().with_clip_rect(map.intersect(ui.clip_rect()));
        painter.rect_filled(map, CornerRadius::same(8), t.sidebar());
        let mut sub = ui.new_child(UiBuilder::new().max_rect(map).id_salt("map-paint"));
        sub.set_clip_rect(map.intersect(ui.clip_rect()));
        let selected_b = e.map_selected_boundary;
        let hovered_b = self.map.hovered_boundary;
        let pill_rect = |b: &BoundaryView, ui: &Ui| -> Rect {
            let text = texts.pair_text(b);
            let measured = (w::text_width(ui, &text, regular(9.0)) + 12.0).min(190.0);
            let lx = (b.handle.0 as f32 - measured / 2.0).clamp(3.0, (map.width() - measured - 3.0).max(3.0));
            let ly = (b.handle.1 as f32 - 36.0).clamp(3.0, (map.height() - 25.0).max(3.0));
            Rect::from_min_size(pos2(map.left() + lx, map.top() + ly), vec2(measured, 22.0))
        };
        let mut pills: Vec<(usize, Rect)> = Vec::new();
        for i in [selected_b, hovered_b].into_iter().flatten() {
            if i < views.len() && !pills.iter().any(|(j, _)| *j == i) {
                pills.push((i, pill_rect(&views[i], &sub)));
            }
        }
        for (i, m) in e.monitors.iter().enumerate() {
            if i >= rects.len() {
                break;
            }
            let clones = e.monitors.iter().filter(|o| o.bounds == m.bounds).count();
            self.draw_monitor(&sub, m, to_abs(rects[i]), clones, &pills, &views, &texts);
            self.rec
                .add(format!("map.monitor.{i}"), to_abs(rects[i]), m.stable_id.clone(), "");
        }
        let mut standing = boundary::standing_boundaries(&views, selected_b, hovered_b, e.monitors.len());
        let always = |i: usize| {
            let b = &views[i];
            b.customized() || b.safety_demoted() || Some(i) == selected_b || Some(i) == hovered_b
        };
        standing.sort_by(|&a, &b| {
            (!always(a))
                .cmp(&!always(b))
                .then(
                    (views[b].corridor_end - views[b].corridor_start)
                        .cmp(&(views[a].corridor_end - views[a].corridor_start)),
                )
                .then(views[a].sort_order().cmp(&views[b].sort_order()))
        });
        let mut painted: Vec<(i32, i32)> = Vec::new();
        for i in standing {
            let b = &views[i];
            let paint_handle = always(i)
                || !painted.iter().any(|c| {
                    let (dx, dy) = (c.0 - b.handle.0, c.1 - b.handle.1);
                    dx * dx + dy * dy < 18 * 18
                });
            self.draw_boundary(
                &sub,
                b,
                Some(i) == selected_b,
                Some(i) == hovered_b,
                paint_handle,
                &to_pos,
                &texts,
                map,
            );
            if paint_handle {
                painted.push(b.handle);
            }
        }
        for (i, b) in views.iter().enumerate() {
            self.rec.add(
                format!("map.boundary.{i}"),
                to_abs(b.hit_target),
                texts.pair_text(b),
                if Some(i) == selected_b { "selected" } else { "" },
            );
        }
        // "Positions come from Windows" chip (C# DrawWindowsLockChip).
        let chip_text = e.tr.t("PositionsFromWindows");
        let cw = (w::text_width(&sub, &chip_text, regular(9.0)) + 31.0).min(230.0);
        let chip = Rect::from_min_size(
            pos2(map.left() + (map.width() - cw - 10.0).max(8.0), map.top() + 10.0),
            vec2(cw, 27.0),
        );
        w::rounded(&sub, chip, 5, t.surface, Stroke::new(1.0, t.border));
        let lock = Rect::from_min_size(chip.min + vec2(9.0, 12.0), vec2(8.0, 8.0));
        sub.painter()
            .rect_stroke(lock, CornerRadius::ZERO, Stroke::new(1.3, t.subtle), StrokeKind::Middle);
        sub.painter()
            .circle_stroke(pos2(lock.center().x, lock.top()), 3.0, Stroke::new(1.3, t.subtle));
        w::label(
            &sub,
            Rect::from_min_max(chip.min + vec2(23.0, 0.0), chip.max - vec2(4.0, 0.0)),
            &chip_text,
            regular(9.0),
            t.subtle,
            HAlign::Left,
            true,
            false,
        );
        if response.has_focus() {
            sub.painter().rect_stroke(
                map.shrink(2.0),
                CornerRadius::ZERO,
                Stroke::new(1.0, t.accent),
                StrokeKind::Inside,
            );
        }
        ui.painter().rect_stroke(
            map,
            CornerRadius::same(8),
            Stroke::new(1.0, t.border),
            StrokeKind::Inside,
        );

        // Tooltip (C# _toolTip).
        if let Some(i) = hovered_b {
            response.clone().on_hover_text(texts.boundary_description(&views[i]));
        } else if let Some(id) = &self.map.hovered_monitor
            && let Some(m) = e.monitors.iter().find(|m| same(&m.stable_id, id))
        {
            response.clone().on_hover_text(texts.screen_description(m));
        }
        self.rec.add(
            "map",
            map,
            e.tr.t("TabScreens"),
            if response.has_focus() { "focused" } else { "" },
        );

        // Accessible virtual children: one per screen, then one per boundary (C# MapAccessibleObject).
        for (i, m) in e.monitors.iter().enumerate() {
            let rect = rects.get(i).map_or(map, |r| to_abs(*r));
            self.a11y_child(
                ui,
                map_id,
                Id::new(("map-screen", i)),
                rect,
                texts.screen_description(m),
                e.tr.t("IdentifyLabel"),
            );
            if ui.input(|inp| inp.has_accesskit_action_request(Id::new(("map-screen", i)), accesskit::Action::Click)) {
                self.actions.push(Action::MonitorChosen(m.stable_id.clone()));
            }
        }
        for (i, b) in views.iter().enumerate() {
            self.a11y_child(
                ui,
                map_id,
                Id::new(("map-boundary", i)),
                to_abs(b.hit_target),
                texts.boundary_description(b),
                e.tr.t("OpenBoundary"),
            );
            if ui.input(|inp| inp.has_accesskit_action_request(Id::new(("map-boundary", i)), accesskit::Action::Click))
            {
                self.actions.push(Action::OpenBoundary(i));
            }
        }
    }

    fn a11y_child(&self, ui: &mut Ui, parent: Id, id: Id, rect: Rect, name: String, action: String) {
        let _child = ui.new_child(UiBuilder::new().id(id).max_rect(rect).accessibility_parent(parent));
        let ppp = ui.ctx().pixels_per_point() as f64;
        ui.ctx().accesskit_node_builder(id, |node| {
            node.set_role(accesskit::Role::Button);
            node.set_label(name);
            node.set_description(action);
            node.add_action(accesskit::Action::Click);
            node.add_action(accesskit::Action::Focus);
            node.set_bounds(accesskit::Rect {
                x0: rect.left() as f64 * ppp,
                y0: rect.top() as f64 * ppp,
                x1: rect.right() as f64 * ppp,
                y1: rect.bottom() as f64 * ppp,
            });
        });
    }

    /// C# `MonitorMapControl.OnKeyDown`.
    fn map_key(&mut self, key: Key, views: &[BoundaryView]) {
        let e = self.editor;
        let sorted: Vec<&MonitorInfo> = {
            let mut v: Vec<&MonitorInfo> = e.monitors.iter().collect();
            v.sort_by_key(|m| m.index);
            v
        };
        match key {
            Key::ArrowLeft | Key::ArrowRight => {
                if e.monitors.is_empty() {
                    return;
                }
                let current = e
                    .map_selected_monitor
                    .as_ref()
                    .and_then(|id| e.monitors.iter().position(|m| same(&m.stable_id, id)))
                    .unwrap_or(0) as i32;
                let delta = if key == Key::ArrowLeft { -1 } else { 1 };
                let n = e.monitors.len() as i32;
                let next = ((current + delta + n) % n.max(1)) as usize;
                self.actions
                    .push(Action::SelectMonitorKeyboard(sorted[next].stable_id.clone()));
            }
            Key::ArrowUp | Key::ArrowDown => {
                let anchor = e
                    .map_selected_boundary
                    .and_then(|i| views.get(i))
                    .map(|b| b.first.clone())
                    .or_else(|| {
                        e.map_selected_monitor
                            .as_ref()
                            .and_then(|id| e.monitors.iter().find(|m| same(&m.stable_id, id)).cloned())
                    })
                    .or_else(|| e.monitors.first().cloned());
                let Some(anchor) = anchor else { return };
                let mut connected: Vec<usize> = (0..views.len())
                    .filter(|&i| {
                        same(&views[i].first.stable_id, &anchor.stable_id)
                            || same(&views[i].second.stable_id, &anchor.stable_id)
                    })
                    .collect();
                connected.sort_by(|&a, &b| {
                    boundary::boundary_angle(&views[a], &anchor)
                        .partial_cmp(&boundary::boundary_angle(&views[b], &anchor))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                if connected.is_empty() {
                    return;
                }
                let n = connected.len() as i32;
                let i = e
                    .map_selected_boundary
                    .and_then(|s| connected.iter().position(|&c| c == s))
                    .unwrap_or(0) as i32;
                let next = ((i + if key == Key::ArrowUp { -1 } else { 1 } + n) % n) as usize;
                self.actions.push(Action::SelectBoundaryKeyboard(connected[next]));
            }
            // Escape is the window's (C# ProcessCmdKey/CancelButton ran before the map's OnKeyDown saw it).
            _ => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_monitor(
        &self,
        ui: &Ui,
        m: &MonitorInfo,
        rect: Rect,
        clones: usize,
        pills: &[(usize, Rect)],
        views: &[BoundaryView],
        texts: &MapTexts<'_>,
    ) {
        let e = self.editor;
        let t = self.theme;
        let excluded = contains_id(&e.draft.excluded_devices, &m.stable_id);
        let selected = e.map_selected_monitor.as_deref().is_some_and(|s| same(s, &m.stable_id));
        let hovered = self
            .map
            .hovered_monitor
            .as_deref()
            .is_some_and(|s| same(s, &m.stable_id))
            || self
                .map
                .external_hover
                .as_deref()
                .is_some_and(|s| same(s, &m.stable_id));
        let shadow = Color32::from_black_alpha(if t.is_dark() { 42 } else { 25 });
        ui.painter()
            .rect_filled(rect.translate(vec2(0.0, 2.0)), CornerRadius::same(7), shadow);
        ui.painter().rect_filled(rect, CornerRadius::same(7), t.surface2);
        if excluded {
            let a = if t.is_dark() { 72 } else { 42 };
            let c = Color32::from_rgba_unmultiplied(t.danger.r(), t.danger.g(), t.danger.b(), a);
            w::hatch(ui, rect.shrink(1.0), c);
        }
        let border = if selected || hovered { t.accent } else { t.border_strong };
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(7),
            Stroke::new(if selected { 2.5 } else { 1.5 }, border),
            StrokeKind::Inside,
        );
        let chip_size = (rect.height() / 3.0).floor().clamp(18.0, 24.0);
        let chip = Rect::from_min_size(rect.min + vec2(5.0, 5.0), vec2(chip_size.max(18.0), chip_size));
        ui.painter().rect_filled(
            chip,
            CornerRadius::same(5),
            if m.primary { t.accent_weak } else { t.chip },
        );
        w::label(
            ui,
            chip,
            &m.index.to_string(),
            semibold(9.0),
            if m.primary { t.accent } else { t.chip_text },
            HAlign::Center,
            true,
            false,
        );
        if m.primary {
            let main = format!("★ {}", e.tr.t("MainWord"));
            let bw = (w::text_width(ui, &main, regular(9.0)) + 10.0)
                .max(48.0)
                .min((rect.width() - 10.0).max(48.0));
            let badge = Rect::from_min_size(pos2(rect.right() - bw - 5.0, rect.bottom() - 23.0), vec2(bw, 18.0));
            ui.painter().rect_filled(badge, CornerRadius::same(4), t.accent_weak);
            w::label(ui, badge, &main, regular(9.0), t.accent, HAlign::Center, true, false);
        }
        if excluded && rect.width() >= 64.0 && rect.height() >= 34.0 {
            let c = pos2(rect.right() - 13.0, rect.bottom() - 13.0);
            let s = Stroke::new(2.0, t.danger);
            ui.painter().line_segment([c - vec2(4.0, 4.0), c + vec2(4.0, 4.0)], s);
            ui.painter().line_segment([c + vec2(4.0, -4.0), c + vec2(-4.0, 4.0)], s);
        }
        if clones > 1 {
            w::text_at(
                ui,
                pos2(rect.right() - 5.0, rect.bottom() - 13.0),
                Align2::RIGHT_CENTER,
                &format!("×{clones}"),
                regular(9.0),
                t.subtle,
            );
        }
        if rect.width() >= 64.0 && rect.height() >= 34.0 {
            let name = texts.name_for(m);
            let mut label = Rect::from_min_size(
                rect.min + vec2(6.0, 20.0),
                vec2(
                    rect.width() - 12.0,
                    (rect.height() - if m.primary { 42.0 } else { 34.0 }).max(15.0),
                ),
            );
            let covering = pills.iter().find(|(i, r)| {
                r.intersects(label)
                    && (same(&views[*i].first.stable_id, &m.stable_id)
                        || same(&views[*i].second.stable_id, &m.stable_id))
            });
            let mut covered = covering.is_some();
            if let Some((_, pill)) = covering
                && let Some(above) = name_above_pill_with_fixes(label, *pill, e.fixes)
            {
                label = above;
                covered = false;
            }
            if !covered {
                w::label(
                    ui,
                    label,
                    &name,
                    semibold(9.0),
                    if excluded { t.subtle } else { t.text },
                    HAlign::Center,
                    true,
                    false,
                );
                if rect.width() >= 104.0
                    && rect.height() >= 88.0
                    && covering.is_none_or(|(_, pill)| label.bottom() + 17.0 <= pill.top())
                {
                    let res = Rect::from_min_size(
                        pos2(rect.left() + 8.0, (rect.bottom() - 20.0).min(label.bottom())),
                        vec2(rect.width() - 16.0, 17.0),
                    );
                    w::label(
                        ui,
                        res,
                        &format!("{} × {}", m.bounds.width(), m.bounds.height()),
                        regular(8.0),
                        t.subtle,
                        HAlign::Center,
                        false,
                        false,
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_boundary(
        &self,
        ui: &Ui,
        b: &BoundaryView,
        selected: bool,
        hovered: bool,
        paint_handle: bool,
        to_pos: &dyn Fn((i32, i32)) -> Pos2,
        texts: &MapTexts<'_>,
        map: Rect,
    ) {
        let t = self.theme;
        let line = if selected || hovered { t.accent } else { t.border_strong };
        let width = if selected { 2.4 } else { 2.0 };
        let gap = b.gap_px > self.editor.draft.seam_tolerance_px;
        let h = to_pos(b.handle);
        let (s, en) = (to_pos(b.line_start), to_pos(b.line_end));
        let p = ui.painter();
        let pen = Stroke::new(width, line);
        let seg = |a: Pos2, c: Pos2| {
            if gap {
                // C# DashStyle.Dot.
                let len = (c - a).length();
                let dir = if len > 0.0 { (c - a) / len } else { vec2(0.0, 0.0) };
                let mut d = 0.0;
                while d < len {
                    let e2 = (d + width).min(len);
                    p.line_segment([a + dir * d, a + dir * e2], pen);
                    d += width * 2.0;
                }
            } else {
                p.line_segment([a, c], pen);
            }
        };
        let (half, post) = (14.0, 6.0);
        if b.vertical_connection() {
            seg(s, pos2(s.x.max(h.x - half), h.y));
            seg(pos2(en.x.min(h.x + half), h.y), en);
            p.line_segment([pos2(h.x - half, h.y - post), pos2(h.x - half, h.y + post)], pen);
            p.line_segment([pos2(h.x + half, h.y - post), pos2(h.x + half, h.y + post)], pen);
        } else {
            seg(s, pos2(h.x, s.y.max(h.y - half)));
            seg(pos2(h.x, en.y.min(h.y + half)), en);
            p.line_segment([pos2(h.x - post, h.y - half), pos2(h.x + post, h.y - half)], pen);
            p.line_segment([pos2(h.x - post, h.y + half), pos2(h.x + post, h.y + half)], pen);
        }
        if gap {
            for q in [s, en] {
                let tick = Stroke::new(1.5, line);
                if b.vertical_connection() {
                    p.line_segment([q - vec2(0.0, 4.0), q + vec2(0.0, 4.0)], tick);
                } else {
                    p.line_segment([q - vec2(4.0, 0.0), q + vec2(4.0, 0.0)], tick);
                }
            }
        }
        if hovered {
            p.circle_filled(h, 10.0, t.accent_weak);
        }
        if b.customized() && paint_handle {
            let first = b.first_to_second.as_ref().map_or(Behavior::Free, |x| x.behavior);
            let second = b.second_to_first.as_ref().map_or(Behavior::Free, |x| x.behavior);
            if first == second {
                w::mode_glyph(ui, h, first, t.accent, 1.0, t.text);
            } else if b.vertical_connection() {
                w::mode_glyph(ui, h - vec2(9.0, 0.0), first, t.accent, 0.75, t.text);
                w::mode_glyph(ui, h + vec2(9.0, 0.0), second, t.accent, 0.75, t.text);
            } else {
                w::mode_glyph(ui, h - vec2(0.0, 9.0), first, t.accent, 0.75, t.text);
                w::mode_glyph(ui, h + vec2(0.0, 9.0), second, t.accent, 0.75, t.text);
            }
        }
        if b.safety_demoted() {
            let st = Stroke::new(1.5, t.accent);
            p.line_segment([h + vec2(-7.0, 7.0), h + vec2(0.0, 11.0)], st);
            p.line_segment([h + vec2(0.0, 11.0), h + vec2(7.0, 7.0)], st);
        }
        if selected || hovered {
            if b.vertical_connection() {
                w::triangle(
                    ui,
                    h + vec2(-4.0, -13.0),
                    h + vec2(4.0, -13.0),
                    h + vec2(0.0, -18.0),
                    t.accent,
                );
                w::triangle(
                    ui,
                    h + vec2(-4.0, 13.0),
                    h + vec2(4.0, 13.0),
                    h + vec2(0.0, 18.0),
                    t.accent,
                );
            } else {
                w::triangle(
                    ui,
                    h + vec2(-13.0, -4.0),
                    h + vec2(-13.0, 4.0),
                    h + vec2(-18.0, 0.0),
                    t.accent,
                );
                w::triangle(
                    ui,
                    h + vec2(13.0, -4.0),
                    h + vec2(13.0, 4.0),
                    h + vec2(18.0, 0.0),
                    t.accent,
                );
            }
            let text = texts.pair_text(b);
            let measured = (w::text_width(ui, &text, regular(9.0)) + 12.0).min(190.0);
            let lx = (b.handle.0 as f32 - measured / 2.0).clamp(3.0, (map.width() - measured - 3.0).max(3.0));
            let ly = (b.handle.1 as f32 - 36.0).clamp(3.0, (map.height() - 25.0).max(3.0));
            let pill = Rect::from_min_size(pos2(map.left() + lx, map.top() + ly), vec2(measured, 22.0));
            w::rounded(ui, pill, 8, t.surface, Stroke::new(1.0, t.border));
            w::label(
                ui,
                pill.shrink2(vec2(4.0, 1.0)),
                &text,
                regular(9.0),
                t.text,
                HAlign::Center,
                true,
                false,
            );
        }
    }

    // ------------------------------------------------------------------ drawer

    fn draw_drawer(&mut self, ui: &mut Ui, card: Rect, g: &DisplaysGeometry) {
        let e = self.editor;
        let t = *self.theme;
        let d = &e.drawer;
        ui.painter().rect_filled(card, CornerRadius::ZERO, t.surface2);
        ui.painter().rect_stroke(
            card,
            CornerRadius::ZERO,
            Stroke::new(1.0, t.accent_line),
            StrokeKind::Inside,
        );
        self.rec.add("drawer", card, e.tr.t("DrawerTitle"), "");
        let cw = 430.max(g.m.width - 28) as f32;
        let x = card.left() + DRAWER_PAD_X as f32;
        let mut y = card.top() + DRAWER_PAD_Y as f32;

        // Header: endpoint chips, meta, binding note, close.
        let chip_from = Rect::from_min_size(pos2(x, y), vec2(135.0, 31.0));
        let sep = Rect::from_min_size(pos2(x + 136.0, y), vec2(28.0, 31.0));
        let chip_to = Rect::from_min_size(pos2(x + 165.0, y), vec2(135.0, 31.0));
        for (r, text, tag) in [
            (chip_from, &d.from_chip, "drawer.from"),
            (chip_to, &d.to_chip, "drawer.to"),
        ] {
            w::rounded(ui, r, 5, t.surface2, Stroke::new(1.0, t.border_strong));
            w::label(
                ui,
                r.shrink2(vec2(6.0, 0.0)),
                text,
                regular(9.0),
                t.text,
                HAlign::Center,
                true,
                false,
            );
            self.rec.add(tag, r, text.clone(), "");
        }
        w::label(
            ui,
            sep,
            d.separator,
            regular(9.0),
            t.subtle,
            HAlign::Center,
            true,
            false,
        );
        let meta_w = (cw - 316.0 - 44.0).clamp(80.0, 430.0);
        w::label(
            ui,
            Rect::from_min_size(pos2(x + 316.0, y), vec2(meta_w, 20.0)),
            &d.meta,
            semibold(8.5),
            t.text,
            HAlign::Left,
            true,
            false,
        );
        w::label(
            ui,
            Rect::from_min_size(pos2(x + 316.0, y + 21.0), vec2(meta_w, 25.0)),
            &e.tr.t("DrawerBinding"),
            regular(9.0),
            t.subtle,
            HAlign::Left,
            false,
            false,
        );
        let close = Rect::from_min_size(pos2(x + cw - 32.0, y), vec2(31.0, 31.0));
        if w::close_button(ui, self.rec, "drawer.close", close, &e.tr.t("CloseDrawer"), &t).clicked() {
            self.actions.push(Action::CloseDrawer);
        }
        y += D_HEADER as f32;

        // Question row: the direction caption and the overall-behavior link (C# LayoutDrawerQuestionText).
        let link_font = regular(9.0);
        let min_left = 260f32.min(cw / 2.0).floor();
        let available = (cw - min_left - 4.0).max(80.0);
        let measured = w::text_width(ui, &d.overall_text, link_font.clone()).ceil() + 4.0;
        let link_w = measured.min(available);
        let link_left = min_left.max(cw - 4.0 - link_w);
        let link_h = (w::text_height(ui, &d.overall_text, link_font, link_w, false).ceil() + 2.0).max(20.0);
        let link_rect = Rect::from_min_size(pos2(x + link_left, y + 4.0), vec2(link_w, link_h));
        let caption_w = (link_left - 12.0).max(120.0);
        let caption = Rect::from_min_size(pos2(x, y + 4.0), vec2(caption_w, 22.0));
        w::label(
            ui,
            caption,
            &d.direction_caption,
            semibold(10.5),
            t.text,
            HAlign::Left,
            true,
            false,
        );
        self.rec.add("drawer.caption", caption, d.direction_caption.clone(), "");
        if w::link(
            ui,
            self.rec,
            "drawer.overall",
            link_rect,
            &d.overall_text,
            d.overall_enabled,
            &t,
            HAlign::Right,
        )
        .clicked()
        {
            self.actions.push(Action::UseOverall);
        }
        y += D_QUESTION as f32;

        // The three tiles.
        let texts = e.tile_texts();
        let narrow = g.m.narrow;
        for (i, (title, help)) in texts.iter().enumerate() {
            let r = if narrow {
                Rect::from_min_size(
                    pos2(x, y + i as f32 * 78.0),
                    vec2((g.m.width - 28).max(220) as f32, 78.0),
                )
            } else {
                let tw = ((cw - 16.0) / 3.0).floor().max(150.0);
                Rect::from_min_size(pos2(x + i as f32 * (tw + 8.0), y), vec2(tw, 78.0))
            };
            let tag = ["drawer.tile.free", "drawer.tile.push", "drawer.tile.blocked"][i];
            if w::tile(
                ui,
                self.rec,
                tag,
                r,
                TILE_BEHAVIORS[i],
                title,
                help,
                d.tiles_enabled,
                d.tiles_selected[i],
                &t,
            )
            .clicked()
            {
                self.actions.push(Action::ChoosePrimary(TILE_BEHAVIORS[i]));
            }
        }
        y += g.tiles_h as f32;

        let ret = Rect::from_min_size(pos2(x, y), vec2(cw, D_RETURN as f32));
        w::label(
            ui,
            ret,
            &d.return_line,
            regular(9.0),
            if d.return_enabled { t.subtle } else { t.faint },
            HAlign::Left,
            true,
            false,
        );
        self.rec.add("drawer.return", ret, d.return_line.clone(), "");
        y += D_RETURN as f32;

        let status = Rect::from_min_size(pos2(x, y), vec2(cw, g.status_h as f32));
        w::italic_label(ui, status, &d.status, regular(8.5), t.warning);
        self.rec.add("drawer.status", status, d.status.clone(), "");
        y += g.status_h as f32;

        let half = (cw / 2.0).floor();
        let sep_link = Rect::from_min_size(pos2(x, y), vec2(half, D_FOLDS as f32));
        if w::link(
            ui,
            self.rec,
            "drawer.separate",
            sep_link,
            &d.separate_link_text,
            d.separate_link_enabled,
            &t,
            HAlign::Left,
        )
        .clicked()
        {
            self.actions.push(Action::ToggleSeparate);
        }
        let when_text = e.tr.t("WhenThisApplies");
        let when_link = Rect::from_min_size(pos2(x + half, y), vec2(half, D_FOLDS as f32));
        if w::link(
            ui,
            self.rec,
            "drawer.when",
            when_link,
            &when_text,
            d.when_enabled,
            &t,
            HAlign::Left,
        )
        .clicked()
        {
            self.actions.push(Action::ToggleWhen);
        }
        y += D_FOLDS as f32;

        for (row_i, row) in d.separate_rows.iter().enumerate() {
            let rr = Rect::from_min_size(pos2(x, y), vec2(cw, D_SEPARATE_ROW as f32));
            w::label(
                ui,
                Rect::from_min_size(rr.min, vec2(190.0, 72.0)),
                &row.label,
                semibold(8.75),
                t.text,
                HAlign::Left,
                true,
                true,
            );
            for (i, &behavior) in TILE_BEHAVIORS.iter().enumerate() {
                let r = Rect::from_min_size(pos2(x + 198.0 + i as f32 * 195.0, y + 5.0), vec2(188.0, 66.0));
                let tag = format!("drawer.row.{row_i}.{i}");
                let title = e.separate_tile_title(i);
                if w::tile(
                    ui,
                    self.rec,
                    &tag,
                    r,
                    behavior,
                    &title,
                    "",
                    d.separate_panel_enabled,
                    row.selected[i],
                    &t,
                )
                .clicked()
                {
                    self.actions.push(Action::ChooseSeparate(row_i, behavior));
                }
            }
            y += D_SEPARATE_ROW as f32;
        }
        if e.when_expanded {
            let auto = Rect::from_min_size(pos2(x, y), vec2(cw.min(760.0), 24.0));
            if w::radio(
                ui,
                self.rec,
                "drawer.automatic",
                auto,
                &e.tr.t("AutomaticControl"),
                d.automatic_checked,
                d.radios_enabled,
                &t,
            )
            .clicked()
            {
                self.actions.push(Action::SetControl(Control::Automatic));
            }
            w::label(
                ui,
                Rect::from_min_size(pos2(x + 24.0, y + 24.0), vec2(302.0, 24.0)),
                &e.tr.t("AutomaticControlHelp"),
                regular(9.0),
                t.subtle,
                HAlign::Left,
                true,
                false,
            );
            let always = Rect::from_min_size(pos2(x, y + 48.0), vec2(cw.min(760.0), 24.0));
            if w::radio(
                ui,
                self.rec,
                "drawer.always",
                always,
                &e.tr.t("AlwaysControl"),
                d.always_checked,
                d.radios_enabled,
                &t,
            )
            .clicked()
            {
                self.actions.push(Action::SetControl(Control::Always));
            }
            w::label(
                ui,
                Rect::from_min_size(pos2(x + 24.0, y + 72.0), vec2(302.0, 22.0)),
                &e.tr.t("AlwaysControlHelp"),
                regular(9.0),
                t.subtle,
                HAlign::Left,
                true,
                false,
            );
            y += D_WHEN as f32;
        }
        let allow = Rect::from_min_size(pos2(x + 3.0, y + 3.0), vec2(190.0, 32.0));
        let r = w::button(
            ui,
            self.rec,
            "drawer.allow",
            allow,
            &e.tr.t("AllowThisScreen"),
            d.allow_role,
            d.allow_enabled,
            &t,
            false,
        );
        if !d.allow_description.is_empty() {
            r.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    d.allow_enabled,
                    format!("{}. {}", e.tr.t("AllowThisScreen"), d.allow_description),
                )
            });
        }
        if r.clicked() {
            self.actions.push(Action::AllowScreen);
        }
        self.rec
            .add("drawer.allow.description", allow, d.allow_description.clone(), "");
    }

    // ------------------------------------------------------------------ screen list

    fn draw_section(&mut self, ui: &mut Ui, section: Rect) {
        let e = self.editor;
        let t = self.theme;
        let title = e.tr.t("ScreensSectionTitle");
        let tw = (w::text_width(ui, &title, semibold(11.0)).ceil() + 4.0).min((section.width() / 2.0).max(80.0));
        let tr = Rect::from_min_size(section.min + vec2(0.0, 8.0), vec2(tw, 24.0));
        w::label(ui, tr, &title, semibold(11.0), t.text, HAlign::Left, true, false);
        let help = e.tr.t("ScreensSectionHelp");
        let min_left = tw + 12.0;
        let available = (section.width() - min_left - 10.0).max(80.0);
        let hw = (w::text_width(ui, &help, regular(9.0)).ceil() + 4.0).min(available);
        let hl = min_left.max(section.width() - 10.0 - hw);
        let hr = Rect::from_min_size(section.min + vec2(hl, 10.0), vec2(hw, 22.0));
        w::label(ui, hr, &help, regular(9.0), t.subtle, HAlign::Right, true, true);
        self.rec.add("section", section, title, "");
    }

    fn draw_rows(&mut self, ui: &mut Ui, rows: Rect, g: &DisplaysGeometry) {
        let e = self.editor;
        let t = *self.theme;
        self.rec.add("rows", rows, "", "");
        if self.names.version != Some(e.rows_version) {
            self.names.version = Some(e.rows_version);
            self.names.text = e
                .monitors
                .iter()
                .map(|m| (m.stable_id.clone(), e.monitor_name_for(&m.stable_id)))
                .collect();
        }
        let narrow = g.m.narrow;
        let mut y = rows.top();
        let (c0, c2, c3) = (70.0, 128.0, 190.0);
        let inner = rows.shrink2(vec2(8.0, 0.0));
        if !narrow {
            let hdr = |ui: &Ui, x: f32, wd: f32, key: &str| {
                w::label(
                    ui,
                    Rect::from_min_size(pos2(x, y), vec2(wd, ROW_HEADER as f32)),
                    &e.tr.t(key),
                    semibold(7.5),
                    t.faint,
                    HAlign::Left,
                    true,
                    false,
                )
            };
            let c1 = inner.width() - c0 - c2 - c3;
            hdr(ui, inner.left(), c0, "ScreenColumn");
            hdr(ui, inner.left() + c0, c1, "NameColumn");
            hdr(ui, inner.left() + c0 + c1, c2, "ResolutionColumn");
            hdr(ui, inner.left() + c0 + c1 + c2, c3, "PointerAccessColumn");
        }
        y += ROW_HEADER as f32;
        let pitch = g.row_pitch as f32;
        for (i, m) in e.rows().iter().enumerate() {
            let row = Rect::from_min_size(pos2(rows.left(), y), vec2(rows.width(), pitch));
            ui.painter()
                .line_segment([row.left_top(), row.right_top()], Stroke::new(1.0, t.border));
            let hover = ui.interact(row, Id::new(("row-hover", i)), Sense::click());
            if hover.contains_pointer() {
                self.map.external_hover = Some(m.stable_id.clone());
            }
            if hover.clicked() {
                self.actions.push(Action::MonitorChosen(m.stable_id.clone()));
            }
            let inner = row.shrink2(vec2(8.0, 0.0));
            let (col0, col3) = if narrow { (62.0, 176.0) } else { (c0, c3) };
            let col2 = if narrow { 0.0 } else { c2 };
            let col1 = inner.width() - col0 - col2 - col3;
            let chip = Rect::from_min_size(pos2(inner.left(), row.center().y - 13.0), vec2(26.0, 26.0));
            w::number_chip(ui, chip, m.index, m.primary, &t);
            let name_frame = if narrow {
                Rect::from_min_size(pos2(inner.left() + col0, row.top() + 6.0), vec2(col1 - 12.0, 34.0))
            } else {
                Rect::from_min_size(pos2(inner.left() + col0, row.top() + 8.0), vec2(col1 - 12.0, 36.0))
            };
            self.name_box(ui, i, m, name_frame);
            let res_rect = if narrow {
                Rect::from_min_size(pos2(inner.left() + col0, row.top() + 42.0), vec2(col1, 26.0))
            } else {
                Rect::from_min_size(pos2(inner.left() + col0 + col1, row.top()), vec2(col2, pitch))
            };
            w::label(
                ui,
                res_rect,
                &format!("{} × {}", m.bounds.width(), m.bounds.height()),
                regular(9.0),
                t.subtle,
                HAlign::Left,
                true,
                false,
            );
            let access = Rect::from_min_size(pos2(inner.left() + col0 + col1 + col2, row.top()), vec2(col3, pitch));
            if m.primary {
                w::label(
                    ui,
                    access,
                    &e.tr.t("MainAlwaysV2"),
                    regular(9.0),
                    t.accent,
                    HAlign::Left,
                    true,
                    false,
                );
                self.rec
                    .add(format!("row.{i}.keep"), access, e.tr.t("MainAlwaysV2"), "main");
            } else {
                let checked = e.is_excluded(&m.stable_id);
                let cb = Rect::from_min_size(pos2(access.left(), access.center().y - 16.0), vec2(col3, 32.0));
                if w::checkbox(
                    ui,
                    self.rec,
                    &format!("row.{i}.keep"),
                    cb,
                    &e.tr.t("KeepPointerOff"),
                    checked,
                    !(e.fixes.j11_segment_and_identity && m.stable_id.contains("#ambiguous:")),
                    &t,
                )
                .clicked()
                {
                    self.actions.push(Action::SetKeepOut(m.stable_id.clone(), !checked));
                }
            }
            self.rec
                .add(format!("row.{i}"), row, e.screen_name(m), m.stable_id.clone());
            y += pitch;
        }
        if !ui.rect_contains_pointer(rows) {
            self.map.external_hover = None;
        }
    }

    fn name_box(&mut self, ui: &mut Ui, i: usize, m: &MonitorInfo, rect: Rect) {
        let e = self.editor;
        let t = *self.theme;
        let id = Id::new(("name-box", &m.stable_id));
        let focused = ui.memory(|mem| mem.has_focus(id));
        let hovered = ui.rect_contains_pointer(rect);
        let can_edit = !(e.fixes.j11_segment_and_identity && m.stable_id.contains("#ambiguous:"));
        w::frame(ui, rect, focused, hovered, false, can_edit, &t);
        let buf = self.names.text.entry(m.stable_id.clone()).or_default();
        let inner = rect.shrink2(vec2(7.0, 5.0));
        let r = ui.put(
            inner,
            TextEdit::singleline(buf)
                .interactive(can_edit)
                .id(id)
                .frame(egui::Frame::NONE)
                .hint_text(e.tr.t("OptionalName"))
                .font(regular(9.0))
                .text_color(t.text)
                .desired_width(inner.width())
                .vertical_align(egui::Align::Center),
        );
        if r.changed() {
            self.actions.push(Action::SetName(m.stable_id.clone(), buf.clone()));
        }
        self.rec.add(
            format!("row.{i}.name"),
            rect,
            buf.clone(),
            if can_edit { "" } else { "disabled" },
        );
    }
}
