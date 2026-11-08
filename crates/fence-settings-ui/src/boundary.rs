//! The map's model: one clickable boundary per pair of touching screens (port of C# `Ui/MonitorMapControl.cs`:
//! `BoundaryView`, `BuildBoundaries`, `TryBoundary`, the map geometry and hit tests, and the accessible texts).
//! Integer geometry as in the C# (map pixels relative to the map's top-left corner).

use fence_config::Settings;
use fence_types::{Behavior, CompileResult, Direction, MonitorInfo, Rect, RuntimeTransition};

use crate::text::Tr;

/// C# `Same` in the UI: ordinal, case-insensitive.
pub fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b) || a.to_lowercase() == b.to_lowercase()
}

pub fn contains_id(list: &[String], id: &str) -> bool {
    list.iter().any(|x| same(x, id))
}

/// A map point (x, y) in map pixels.
pub type Pt = (i32, i32);

/// C# `BoundaryView`.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundaryView {
    pub first: MonitorInfo,
    pub second: MonitorInfo,
    pub first_to_second_direction: Direction,
    pub corridor_start: i32,
    pub corridor_end: i32,
    pub gap_px: i32,
    pub first_to_second: Option<RuntimeTransition>,
    pub second_to_first: Option<RuntimeTransition>,
    pub has_excluded_screen: bool,
    pub hit_target: Rect,
    pub handle: Pt,
    pub line_start: Pt,
    pub line_end: Pt,
}

pub fn opposite(d: Direction) -> Direction {
    match d {
        Direction::Up => Direction::Down,
        Direction::Down => Direction::Up,
        Direction::Left => Direction::Right,
        Direction::Right => Direction::Left,
    }
}

impl BoundaryView {
    pub fn second_to_first_direction(&self) -> Direction {
        opposite(self.first_to_second_direction)
    }
    pub fn vertical_connection(&self) -> bool {
        self.first_to_second_direction.is_vertical()
    }
    pub fn customized(&self) -> bool {
        self.first_to_second.as_ref().is_some_and(|t| t.explicit_rule)
            || self.second_to_first.as_ref().is_some_and(|t| t.explicit_rule)
    }
    pub fn safety_demoted(&self) -> bool {
        self.first_to_second.as_ref().is_some_and(|t| t.safety_demoted)
            || self.second_to_first.as_ref().is_some_and(|t| t.safety_demoted)
    }
    pub fn sort_order(&self) -> i32 {
        let a = self.first_to_second.as_ref().map_or(i32::MAX, |t| t.sort_order);
        let b = self.second_to_first.as_ref().map_or(i32::MAX, |t| t.sort_order);
        a.min(b)
    }
    pub fn same_pair(&self, other: &BoundaryView) -> bool {
        (same(&self.first.stable_id, &other.first.stable_id) && same(&self.second.stable_id, &other.second.stable_id))
            || (same(&self.first.stable_id, &other.second.stable_id)
                && same(&self.second.stable_id, &other.first.stable_id))
    }
}

/// C# `MonitorMapControl.TryBoundary(a, b, ...)`: the direction from `a` to `b`, the corridor and the gap.
pub fn try_boundary(a: &MonitorInfo, b: &MonitorInfo) -> Option<(Direction, i32, i32, i32)> {
    let (a, b) = (a.bounds, b.bounds);
    if b.b <= a.t {
        let (start, end) = (a.l.max(b.l), a.r.min(b.r));
        if end > start {
            return Some((Direction::Up, start, end, a.t - b.b));
        }
    }
    if b.t >= a.b {
        let (start, end) = (a.l.max(b.l), a.r.min(b.r));
        if end > start {
            return Some((Direction::Down, start, end, b.t - a.b));
        }
    }
    if b.r <= a.l {
        let (start, end) = (a.t.max(b.t), a.b.min(b.b));
        if end > start {
            return Some((Direction::Left, start, end, a.l - b.r));
        }
    }
    if b.l >= a.r {
        let (start, end) = (a.t.max(b.t), a.b.min(b.b));
        if end > start {
            return Some((Direction::Right, start, end, b.l - a.r));
        }
    }
    None
}

/// C# `MonitorMapControl.BuildBoundaries`: geometric pairs, split by the compiler's corridor segments, sorted
/// vertical first, then corridor, then ordinal ids.
pub fn build_boundaries(monitors: &[MonitorInfo], settings: &Settings, compiled: &CompileResult) -> Vec<BoundaryView> {
    let mut out = Vec::new();
    let excluded = &settings.excluded_devices;
    for i in 0..monitors.len() {
        for j in i + 1..monitors.len() {
            let (a, b) = (&monitors[i], &monitors[j]);
            let Some((direction, start, end, gap)) = try_boundary(a, b) else {
                continue;
            };
            let pair: Vec<&RuntimeTransition> = compiled
                .transitions
                .iter()
                .filter(|t| {
                    (same(&t.from_stable_id, &a.stable_id) && same(&t.to_stable_id, &b.stable_id))
                        || (same(&t.from_stable_id, &b.stable_id) && same(&t.to_stable_id, &a.stable_id))
                })
                .collect();
            let mut segments: Vec<(i32, i32, i32)> = Vec::new();
            for t in &pair {
                let s = (t.corridor_start, t.corridor_end, t.gap_px);
                if !segments.contains(&s) {
                    segments.push(s);
                }
            }
            let has_excluded = contains_id(excluded, &a.stable_id) || contains_id(excluded, &b.stable_id);
            if segments.is_empty() && has_excluded {
                segments.push((start, end, gap));
            }
            for (cs, ce, g) in segments {
                let back = opposite(direction);
                let find = |from: &MonitorInfo, to: &MonitorInfo, d: Direction| {
                    pair.iter()
                        .find(|t| {
                            same(&t.from_stable_id, &from.stable_id)
                                && same(&t.to_stable_id, &to.stable_id)
                                && t.direction == d
                                && t.corridor_start == cs
                                && t.corridor_end == ce
                        })
                        .map(|t| (*t).clone())
                };
                out.push(BoundaryView {
                    first: a.clone(),
                    second: b.clone(),
                    first_to_second_direction: direction,
                    corridor_start: cs,
                    corridor_end: ce,
                    gap_px: g,
                    first_to_second: find(a, b, direction),
                    second_to_first: find(b, a, back),
                    has_excluded_screen: has_excluded,
                    hit_target: Rect::default(),
                    handle: (0, 0),
                    line_start: (0, 0),
                    line_end: (0, 0),
                });
            }
        }
    }
    out.sort_by(|a, b| {
        let o = |v: &BoundaryView| if v.vertical_connection() { 0 } else { 1 };
        o(a).cmp(&o(b))
            .then(a.corridor_start.cmp(&b.corridor_start))
            .then(a.corridor_end.cmp(&b.corridor_end))
            .then(a.first.stable_id.cmp(&b.first.stable_id))
            .then(a.second.stable_id.cmp(&b.second.stable_id))
    });
    out
}

/// C# `MonitorMapControl.FindBoundary(from, to, direction)`.
pub fn find_boundary(boundaries: &[BoundaryView], from: &str, to: &str, direction: Direction) -> Option<usize> {
    boundaries.iter().position(|b| {
        (same(&b.first.stable_id, from) && same(&b.second.stable_id, to) && b.first_to_second_direction == direction)
            || (same(&b.second.stable_id, from)
                && same(&b.first.stable_id, to)
                && b.second_to_first_direction() == direction)
    })
}

/// C# `EnsureMinimumFootprints`: every screen is at least 12x12 map pixels and stays inside the map.
pub fn ensure_minimum_footprints(rects: &mut [Rect], width: i32, height: i32) {
    for r in rects.iter_mut() {
        let (rw, rh) = (r.r - r.l, r.b - r.t);
        let (w, h) = (rw.max(12), rh.max(12));
        let x = (r.l - (w - rw) / 2).clamp(2, (width - w - 2).max(2));
        let y = (r.t - (h - rh) / 2).clamp(2, (height - h - 2).max(2));
        *r = Rect::new(x, y, x + w, y + h);
    }
}

/// C# `PositionBoundaries`: the seam line, its handle and the 24x24 hit target of every boundary.
pub fn position_boundaries(boundaries: &mut [BoundaryView], monitors: &[MonitorInfo], rects: &[Rect]) {
    let round = |v: f64| v.round() as i32;
    for b in boundaries.iter_mut() {
        let ai = monitors.iter().position(|m| *m == b.first);
        let bi = monitors.iter().position(|m| *m == b.second);
        let (Some(ai), Some(bi)) = (ai, bi) else { continue };
        if ai >= rects.len() || bi >= rects.len() {
            continue;
        }
        let (ar, br) = (rects[ai], rects[bi]);
        if b.vertical_connection() {
            let w = b.first.bounds.width();
            let scale = if w == 0 { 1.0 } else { (ar.r - ar.l) as f32 / w as f32 } as f64;
            let x1 = ar.l + round((b.corridor_start - b.first.bounds.l) as f64 * scale);
            let x2 = ar.l + round((b.corridor_end - b.first.bounds.l) as f64 * scale);
            let up = b.first_to_second_direction == Direction::Up;
            let ya = if up { ar.t } else { ar.b };
            let yb = if up { br.b } else { br.t };
            let y = (ya + yb) / 2;
            b.line_start = (x1, y);
            b.line_end = ((x1 + 1).max(x2), y);
        } else {
            let h = b.first.bounds.height();
            let scale = if h == 0 { 1.0 } else { (ar.b - ar.t) as f32 / h as f32 } as f64;
            let y1 = ar.t + round((b.corridor_start - b.first.bounds.t) as f64 * scale);
            let y2 = ar.t + round((b.corridor_end - b.first.bounds.t) as f64 * scale);
            let left = b.first_to_second_direction == Direction::Left;
            let xa = if left { ar.l } else { ar.r };
            let xb = if left { br.r } else { br.l };
            let x = (xa + xb) / 2;
            b.line_start = (x, y1);
            b.line_end = (x, (y1 + 1).max(y2));
        }
        b.handle = ((b.line_start.0 + b.line_end.0) / 2, (b.line_start.1 + b.line_end.1) / 2);
        b.hit_target = Rect::new(b.handle.0 - 12, b.handle.1 - 12, b.handle.0 + 12, b.handle.1 + 12);
    }
}

fn distance_squared(a: Pt, b: Pt) -> i64 {
    let (x, y) = ((a.0 - b.0) as i64, (a.1 - b.1) as i64);
    x * x + y * y
}

/// WinForms `Rectangle.Contains`: half-open.
fn rect_contains(r: Rect, p: Pt) -> bool {
    p.0 >= r.l && p.0 < r.r && p.1 >= r.t && p.1 < r.b
}

fn distance_to_segment_squared(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = ((b.0 - a.0) as f64, (b.1 - a.1) as f64);
    if dx == 0.0 && dy == 0.0 {
        return distance_squared(p, a) as f64;
    }
    let t = ((((p.0 - a.0) as f64) * dx + ((p.1 - a.1) as f64) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    let (x, y) = (a.0 as f64 + t * dx - p.0 as f64, a.1 as f64 + t * dy - p.1 as f64);
    x * x + y * y
}

/// C# `HitBoundaries(p)`: boundaries under the point, best first.
pub fn hit_boundaries(boundaries: &[BoundaryView], p: Pt) -> Vec<usize> {
    let mut hits: Vec<usize> = (0..boundaries.len())
        .filter(|&i| {
            let b = &boundaries[i];
            rect_contains(b.hit_target, p) || distance_to_segment_squared(p, b.line_start, b.line_end) <= 36.0
        })
        .collect();
    hits.sort_by(|&x, &y| {
        let (a, b) = (&boundaries[x], &boundaries[y]);
        (!rect_contains(a.hit_target, p))
            .cmp(&!rect_contains(b.hit_target, p))
            .then(distance_squared(p, a.handle).cmp(&distance_squared(p, b.handle)))
            .then((a.corridor_end - a.corridor_start).cmp(&(b.corridor_end - b.corridor_start)))
            .then(a.sort_order().cmp(&b.sort_order()))
    });
    hits
}

/// C# `HitMonitor(p, inflate)`: the topmost screen rectangle under the point.
pub fn hit_monitor(rects: &[Rect], p: Pt, inflate: bool) -> Option<usize> {
    for i in (0..rects.len()).rev() {
        let mut r = rects[i];
        let (w, h) = (r.r - r.l, r.b - r.t);
        if inflate && (w < 24 || h < 24) {
            let (ix, iy) = (((24 - w) / 2).max(0), ((24 - h) / 2).max(0));
            r = Rect::new(r.l - ix, r.t - iy, r.r + ix, r.b + iy);
        }
        if rect_contains(r, p) {
            return Some(i);
        }
    }
    None
}

/// C# `StandingBoundaries()`: customized, demoted, selected and hovered boundaries, then the widest neutral ones.
pub fn standing_boundaries(
    boundaries: &[BoundaryView],
    selected: Option<usize>,
    hovered: Option<usize>,
    monitor_count: usize,
) -> Vec<usize> {
    let special = |i: usize| {
        let b = &boundaries[i];
        b.customized() || b.safety_demoted() || Some(i) == selected || Some(i) == hovered
    };
    let mut out: Vec<usize> = (0..boundaries.len()).filter(|&i| special(i)).collect();
    let mut neutral: Vec<usize> = (0..boundaries.len()).filter(|&i| !special(i)).collect();
    neutral.sort_by(|&x, &y| {
        let (a, b) = (&boundaries[x], &boundaries[y]);
        (b.corridor_end - b.corridor_start)
            .cmp(&(a.corridor_end - a.corridor_start))
            .then(a.gap_px.cmp(&b.gap_px))
            .then(a.sort_order().cmp(&b.sort_order()))
    });
    let take = (monitor_count * 2).clamp(2, 12);
    out.extend(neutral.into_iter().take(take));
    out
}

/// Accessible and visible texts of the map (C# `NameFor`, `BoundaryPairText`, `ScreenDescription`,
/// `BoundaryDescription`).
pub struct MapTexts<'a> {
    pub tr: Tr,
    pub settings: &'a Settings,
    pub name_for_id: &'a dyn Fn(&str) -> String,
}

impl MapTexts<'_> {
    pub fn name_for(&self, m: &MonitorInfo) -> String {
        let name = (self.name_for_id)(&m.stable_id);
        if !name.is_empty() {
            name
        } else if m.primary {
            self.tr.t("MainWord")
        } else {
            self.tr.f("DefaultScreenName", &[&m.index.to_string()])
        }
    }

    pub fn pair_text(&self, b: &BoundaryView) -> String {
        let sep = if b.vertical_connection() { "  ↕  " } else { "  ↔  " };
        format!("{}{}{}", self.name_for(&b.first), sep, self.name_for(&b.second))
    }

    pub fn screen_description(&self, m: &MonitorInfo) -> String {
        let tail = if m.primary {
            self.tr.t("MainScreenAvailable")
        } else if contains_id(&self.settings.excluded_devices, &m.stable_id) {
            self.tr.t("BlockedByKeepOut")
        } else {
            String::new()
        };
        self.tr.f(
            "ScreenAccessible",
            &[
                &m.index.to_string(),
                &self.name_for(m),
                &m.bounds.width().to_string(),
                &m.bounds.height().to_string(),
                &tail,
            ],
        )
    }

    pub fn behavior_text(&self, b: Option<Behavior>) -> String {
        match b {
            Some(Behavior::Blocked) => self.tr.t("NeverCrosses"),
            Some(Behavior::Push) => self.tr.t("NeedsFirmPush"),
            _ => self.tr.t("CrossesFreely"),
        }
    }

    pub fn boundary_description(&self, b: &BoundaryView) -> String {
        let geometry = if b.gap_px > self.settings.seam_tolerance_px {
            self.tr.f("GapFormat", &[&b.gap_px.to_string()])
        } else {
            self.tr.t("TouchingBoundary")
        };
        let excluded = if b.has_excluded_screen {
            self.tr.t("BlockedByKeepOut")
        } else {
            String::new()
        };
        self.tr.f(
            "BoundaryAccessible",
            &[
                &self.pair_text(b),
                &self.tr.t("DrawerTitle"),
                &geometry,
                &self.tr.t("FirstDirection"),
                &self.behavior_text(b.first_to_second.as_ref().map(|t| t.behavior)),
                &self.tr.t("ReturnDirection"),
                &self.behavior_text(b.second_to_first.as_ref().map(|t| t.behavior)),
                &excluded,
            ],
        )
    }
}

/// C# `BoundaryAngle(b, anchor)` for the Up/Down keyboard walk around a screen.
pub fn boundary_angle(b: &BoundaryView, anchor: &MonitorInfo) -> f32 {
    let other = if same(&b.first.stable_id, &anchor.stable_id) {
        &b.second
    } else {
        &b.first
    };
    let c = |r: Rect| ((r.l as f64 + r.r as f64) / 2.0, (r.t as f64 + r.b as f64) / 2.0);
    let (ax, ay) = c(anchor.bounds);
    let (ox, oy) = c(other.bounds);
    (ox - ax).atan2(ay - oy) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(id: &str, index: u32, primary: bool, r: Rect) -> MonitorInfo {
        MonitorInfo {
            device: String::new(),
            stable_id: id.into(),
            bounds: r,
            primary,
            index,
        }
    }

    #[test]
    fn s_ui_try_boundary_matches_the_four_csharp_cases() {
        let main = mon("main", 1, true, Rect::new(0, 0, 100, 100));
        let top = mon("top", 2, false, Rect::new(10, -50, 90, -5));
        let right = mon("right", 3, false, Rect::new(103, 20, 200, 80));
        assert_eq!(try_boundary(&main, &top), Some((Direction::Up, 10, 90, 5)));
        assert_eq!(try_boundary(&top, &main), Some((Direction::Down, 10, 90, 5)));
        assert_eq!(try_boundary(&main, &right), Some((Direction::Right, 20, 80, 3)));
        assert_eq!(try_boundary(&right, &main), Some((Direction::Left, 20, 80, 3)));
        let diagonal = mon("d", 4, false, Rect::new(100, 100, 200, 200));
        assert_eq!(try_boundary(&main, &diagonal), None, "corner contact is not a boundary");
    }

    #[test]
    fn s_ui_excluded_pair_keeps_a_geometric_boundary_without_transitions() {
        let a = mon("left", 1, true, Rect::new(0, 0, 100, 100));
        let b = mon("right", 2, false, Rect::new(100, 0, 200, 100));
        let settings = Settings {
            excluded_devices: vec!["RIGHT".into()],
            ..Settings::default()
        };
        let views = build_boundaries(&[a, b], &settings, &CompileResult::default());
        assert_eq!(views.len(), 1);
        assert!(views[0].has_excluded_screen && views[0].first_to_second.is_none());
        assert_eq!((views[0].corridor_start, views[0].corridor_end), (0, 100));
        let none = build_boundaries(
            &views.iter().map(|v| v.first.clone()).collect::<Vec<_>>(),
            &settings,
            &CompileResult::default(),
        );
        assert!(none.is_empty());
    }

    #[test]
    fn s_ui_hit_target_is_24_by_24_around_the_handle() {
        let a = mon("top", 1, false, Rect::new(0, -100, 100, 0));
        let b = mon("main", 2, true, Rect::new(0, 0, 100, 100));
        let settings = Settings {
            excluded_devices: vec!["top".into()],
            ..Settings::default()
        };
        let mut views = build_boundaries(&[a.clone(), b.clone()], &settings, &CompileResult::default());
        let rects = vec![Rect::new(10, 10, 60, 60), Rect::new(10, 60, 60, 110)];
        position_boundaries(&mut views, &[a, b], &rects);
        let v = &views[0];
        assert_eq!(v.handle, (35, 60));
        assert_eq!(
            (v.hit_target.r - v.hit_target.l, v.hit_target.b - v.hit_target.t),
            (24, 24)
        );
        assert_eq!(hit_boundaries(&views, (35, 60)), vec![0]);
        assert_eq!(hit_monitor(&rects, (20, 20), true), Some(0));
    }
}
