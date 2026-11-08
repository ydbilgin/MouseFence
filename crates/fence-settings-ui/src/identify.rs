//! "Identify all screens" (C# `Ui/IdentifyOverlay.cs`) and the `--identify` dev harness (C# Program.cs:46-69).
//!
//! Each overlay is a borderless, top-most, no-activate, layered window covering one monitor: accent fill at 62%
//! opacity with one white bold line of text (the MouseFence number, a star on the primary, the nickname). It
//! closes itself after 1.8 s. The windows are plain Win32 (windows-sys) so the overlay works in the settings
//! process and in the `--identify` harness alike; the logic around them (texts, sizes, the bounded pump) is
//! host-independent and tested with a fake host (parity rows P157 and B036).

use std::time::Duration;

use eframe::egui::Color32;
use fence_types::MonitorInfo;

/// C# `DurationMs`.
pub const DURATION_MS: u32 = 1800;
/// C# `Opacity = 0.62`.
pub const OPACITY: f32 = 0.62;
/// C# harness: at most 280 pumps of 10 ms (about 2.8 s) before stragglers are closed.
pub const HARNESS_ITERS: u32 = 280;
pub const HARNESS_SLEEP_MS: u64 = 10;

/// C# `IdentifyOverlay.DisplayText(monitor, name)`: "2", "1  ★", "2 — Studio"; the name is normalized first.
pub fn display_text(m: &MonitorInfo, name: &str, normalize: &dyn Fn(&str) -> String) -> String {
    let number = if m.primary {
        format!("{}  ★", m.index)
    } else {
        m.index.to_string()
    };
    let name = normalize(name);
    if name.is_empty() {
        number
    } else {
        format!("{number} — {name}")
    }
}

/// One overlay to show (C# constructor of `IdentifyOverlay`).
#[derive(Clone, Debug, PartialEq)]
pub struct OverlaySpec {
    pub bounds: fence_types::Rect,
    pub text: String,
    pub back: Color32,
    pub opacity: f32,
    /// Bold Segoe UI size in points: clamp(min(side / 14, width / max(1, len * 0.72)), 24, 150).
    pub font_pt: f32,
    /// C# frame thickness max(10, side / 45) (computed by 1.x but not drawn; kept for parity of the numbers).
    pub frame: i32,
    pub duration_ms: u32,
}

pub fn overlay_spec(m: &MonitorInfo, name: &str, accent: Color32, normalize: &dyn Fn(&str) -> String) -> OverlaySpec {
    let (w, h) = (m.bounds.width() as i32, m.bounds.height() as i32);
    let side = w.min(h);
    let text = display_text(m, name, normalize);
    // C# string.Length counts UTF-16 code units.
    let len = text.encode_utf16().count() as f32;
    let font_pt = (side as f32 / 14.0)
        .min(w as f32 / (len * 0.72).max(1.0))
        .clamp(24.0, 150.0);
    OverlaySpec {
        bounds: m.bounds,
        text,
        back: accent,
        opacity: OPACITY,
        font_pt,
        frame: 10.max(side / 45),
        duration_ms: DURATION_MS,
    }
}

/// C# `Flash(monitors, names, theme, owner)`: one overlay per monitor, its name looked up case-insensitively.
pub fn overlay_specs(
    monitors: &[MonitorInfo],
    names: &[(String, String)],
    accent: Color32,
    normalize: &dyn Fn(&str) -> String,
) -> Vec<OverlaySpec> {
    monitors
        .iter()
        .map(|m| {
            let name = names
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(&m.stable_id))
                .map(|(_, v)| v.as_str())
                .unwrap_or("");
            overlay_spec(m, name, accent, normalize)
        })
        .collect()
}

/// Where overlays live: the Win32 host in the product, a fake in tests.
pub trait OverlayHost {
    fn open(&mut self, spec: &OverlaySpec) -> bool;
    /// Dispatch pending window messages (the overlays' own timers close them).
    fn pump(&mut self);
    fn open_count(&self) -> usize;
    /// Close whatever is still open.
    fn close_all(&mut self);
}

/// C# `--identify` harness: flash every overlay, pump until they have closed themselves or 280 pumps have
/// passed, close stragglers, and report `(iters, remaining)`.
pub fn run_harness(host: &mut dyn OverlayHost, specs: &[OverlaySpec], sleep: &mut dyn FnMut(Duration)) -> (u32, usize) {
    for s in specs {
        host.open(s);
    }
    let mut iters = 0;
    while iters < HARNESS_ITERS && host.open_count() > 0 {
        host.pump();
        sleep(Duration::from_millis(HARNESS_SLEEP_MS));
        iters += 1;
    }
    let remaining = host.open_count();
    host.close_all();
    (iters, remaining)
}

/// The text the harness writes to its file.
pub fn harness_report(iters: u32, remaining: usize) -> String {
    format!("iters={iters} remaining={remaining}")
}

/// Show the overlays from the settings window without blocking it: a short-lived thread owns the windows and
/// pumps their messages until they close (at most about 2.8 s, like the harness).
pub fn flash_in_background(specs: Vec<OverlaySpec>) {
    if specs.is_empty() {
        return;
    }
    let _ = std::thread::Builder::new().name("identify".into()).spawn(move || {
        let mut host = crate::win::Win32Overlays::new();
        let _ = run_harness(&mut host, &specs, &mut |d| std::thread::sleep(d));
    });
}

/// `--identify [file]` (C# Program.cs:46-69). Uses fence-win's monitor list; while that is still a pre-join stub
/// (it returns no monitors) the harness falls back to a read-only Win32 enumeration so the overlays can be
/// exercised. Returns the process exit code (0).
pub fn run_identify_cli(args: &[String]) -> i32 {
    crate::win::set_dpi_awareness();
    let mut monitors = fence_win::enumerate_monitors();
    if monitors.is_empty() {
        monitors = crate::win::enumerate_monitors_fallback();
    }
    let system = crate::win::system_theme();
    let theme = crate::theme::Theme::for_kind(
        crate::theme::resolve("System", system.high_contrast, system.system_dark),
        &system.colors,
    );
    let backend = crate::backend::RealBackend;
    let normalize = |s: &str| crate::backend::Backend::normalize_monitor_name(&backend, s);
    let specs = overlay_specs(&monitors, &[], theme.accent, &normalize);
    let mut host = crate::win::Win32Overlays::new();
    let (iters, remaining) = run_harness(&mut host, &specs, &mut |d| std::thread::sleep(d));
    // Extra evidence on stderr (the file keeps the 1.x format): how many overlays were asked for.
    eprintln!(
        "identify: overlays={} {}",
        specs.len(),
        harness_report(iters, remaining)
    );
    if let Some(path) = args.get(1) {
        let _ = std::fs::write(path, harness_report(iters, remaining));
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{fake_normalize_name, mon};

    // Parity row P157 (tests/Tests.cs:2126).
    #[test]
    fn p157_identify_text_is_one_line_with_number_and_name_and_number_only_fallback() {
        let mut m = mon("stable-main", 1, true, 0, 0, 1920, 1080);
        m.index = 1;
        let named = display_text(&m, "BenQ\r\nDesk", &fake_normalize_name);
        let fallback = display_text(&m, "", &fake_normalize_name);
        assert_eq!(named, "1  ★ — BenQ  Desk");
        assert_eq!(fallback, "1  ★");
        assert!(!named.contains('\r') && !named.contains('\n'));
        let other = mon("b", 2, false, 1920, 0, 3840, 1080);
        assert_eq!(display_text(&other, "  ", &fake_normalize_name), "2");
    }

    struct FakeHost {
        open: Vec<(OverlaySpec, u32)>,
        close_after: u32,
        pumps: u32,
        closed_by_force: usize,
    }

    impl OverlayHost for FakeHost {
        fn open(&mut self, spec: &OverlaySpec) -> bool {
            self.open.push((spec.clone(), 0));
            true
        }
        fn pump(&mut self) {
            self.pumps += 1;
            for o in &mut self.open {
                o.1 += 1;
            }
            let limit = self.close_after;
            self.open.retain(|o| o.1 < limit);
        }
        fn open_count(&self) -> usize {
            self.open.len()
        }
        fn close_all(&mut self) {
            self.closed_by_force += self.open.len();
            self.open.clear();
        }
    }

    // Parity row B036 (Program.cs:46-69; Ui/IdentifyOverlay.cs). The window part runs only through low_run on the
    // invisible desktop (`--identify <file>`, evidence in TRACK-D.txt); this test proves the logic around it.
    #[test]
    fn b036_identify_overlays_cover_every_monitor_self_close_and_report_iters_and_remaining() {
        let ms = vec![
            mon("main", 1, true, 0, 0, 2560, 1440),
            mon("top", 2, false, 320, -1080, 2240, 0),
            mon("tiny", 3, false, 2560, 0, 2900, 300),
        ];
        let names = vec![("TOP".to_string(), "Studio".to_string())];
        let accent = Color32::from_rgb(1, 2, 3);
        let specs = overlay_specs(&ms, &names, accent, &fake_normalize_name);
        assert_eq!(specs.len(), 3, "one overlay per monitor");
        assert_eq!(specs[0].text, "1  ★");
        assert_eq!(specs[1].text, "2 — Studio", "names match case-insensitively");
        assert!(
            specs
                .iter()
                .all(|s| s.back == accent && (s.opacity - 0.62).abs() < 1e-6 && s.duration_ms == 1800)
        );
        assert_eq!(specs[0].bounds, ms[0].bounds);
        // pt = clamp(min(side/14, w/max(1, len*0.72)), 24, 150): 1440/14 = 102.86; 2560/(4*0.72) = 888.9.
        assert!((specs[0].font_pt - 1440.0 / 14.0).abs() < 0.01);
        assert_eq!(specs[2].font_pt, 24.0, "small screens clamp to 24 pt");
        assert_eq!(specs[0].frame, 32);

        // Self-close within the cap: 1.8 s timers close the overlays after 180 pumps of 10 ms.
        let mut host = FakeHost {
            open: vec![],
            close_after: 180,
            pumps: 0,
            closed_by_force: 0,
        };
        let mut slept = Duration::ZERO;
        let (iters, remaining) = run_harness(&mut host, &specs, &mut |d| slept += d);
        assert_eq!((iters, remaining, host.closed_by_force), (180, 0, 0));
        assert_eq!(slept, Duration::from_millis(1800));
        assert_eq!(harness_report(iters, remaining), "iters=180 remaining=0");

        // A stuck overlay is closed by force after 280 pumps and reported.
        let mut stuck = FakeHost {
            open: vec![],
            close_after: u32::MAX,
            pumps: 0,
            closed_by_force: 0,
        };
        let (iters, remaining) = run_harness(&mut stuck, &specs, &mut |_| {});
        assert_eq!(
            (iters, remaining, stuck.closed_by_force, stuck.open_count()),
            (280, 3, 3, 0)
        );
        // No monitors: nothing to pump.
        let mut none = FakeHost {
            open: vec![],
            close_after: 1,
            pumps: 0,
            closed_by_force: 0,
        };
        assert_eq!(run_harness(&mut none, &[], &mut |_| {}), (0, 0));
    }
}
