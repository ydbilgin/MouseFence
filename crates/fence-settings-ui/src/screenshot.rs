//! `--screenshot` and `--screenshot-demo` (C# Program.cs:16-41 and `Ui/Screenshot.cs`): render the settings
//! window to a PNG and print one metrics line.
//!
//! Renderer: the real eframe/glow window, placed beyond the virtual desktop, never activated and kept out of the
//! taskbar; egui's `ViewportCommand::Screenshot` reads the back buffer after a full paint. When the window cannot
//! be created or the capture comes back blank, the same frame is painted by the CPU rasterizer instead and a
//! `renderer=` line on stderr says which path ran (so a fallback is never silent). The demo layout is synthetic
//! and never reads the live monitors or the owner's settings.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use eframe::egui::{self, ColorImage, Event, Vec2, ViewportCommand, vec2};
use fence_config::{Settings, Stored, StoredRule};
use fence_types::{Control, Direction, MonitorInfo, Rect, RuleBehavior};

use crate::backend::{Backend, RealBackend};
use crate::editor::{Dialogs, Editor};
use crate::headless::Headless;
use crate::text::Tr;
use crate::theme::Theme;
use crate::ui::{LayoutMetrics, Services, SettingsUi, SystemTheme};

/// Parsed screenshot arguments (C# `Main` before `CaptureSettings`).
#[derive(Clone, Debug, PartialEq)]
pub struct ShotArgs {
    pub path: String,
    pub theme: Option<String>,
    /// 0 Displays, 1 Advanced overview, 2 Shortcuts, 3 General, -1 selected boundary, -2 Advanced fold open.
    pub tab: i32,
    pub lang: String,
    pub scale: f32,
    pub client: Option<(i32, i32)>,
    pub demo: bool,
}

/// C# `TryParseClientSize`: "WxH" with x, X or ×; both sides 320..=10000.
pub fn parse_client_size(value: &str) -> Option<(i32, i32)> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let (i, sep) = value.char_indices().find(|(_, c)| matches!(c, 'x' | 'X' | '×'))?;
    let (left, right) = (&value[..i], &value[i + sep.len_utf8()..]);
    if left.is_empty() || right.is_empty() {
        return None;
    }
    let w: i32 = left.trim().parse().ok()?;
    let h: i32 = right.trim().parse().ok()?;
    if !(320..=10000).contains(&w) || !(320..=10000).contains(&h) {
        return None;
    }
    Some((w, h))
}

/// `None` when `args` are not a screenshot request (C# needs the flag and a path); `Some(Err)` for a 7th argument
/// that is not WxH (C# threw ArgumentException).
pub fn parse_args(args: &[String]) -> Option<Result<ShotArgs, String>> {
    let demo = match args.first().map(String::as_str) {
        Some("--screenshot") => false,
        Some("--screenshot-demo") => true,
        _ => return None,
    };
    if args.len() < 2 {
        return None;
    }
    let tab = args.get(3).and_then(|t| t.trim().parse::<i32>().ok()).unwrap_or(0);
    let lang = args.get(4).cloned().unwrap_or_else(|| "en".to_string());
    let mut scale = 1.0f32;
    let mut client = None;
    if let Some(a) = args.get(5) {
        if let Some(size) = parse_client_size(a) {
            client = Some(size);
        } else if let Ok(s) = a.trim().parse::<f32>() {
            scale = s;
        }
    }
    if let Some(a) = args.get(6) {
        match parse_client_size(a) {
            Some(size) => client = Some(size),
            None => {
                return Some(Err(
                    "Screenshot client size must use WxH, for example 1600x1040.".to_string()
                ));
            }
        }
    }
    Some(Ok(ShotArgs {
        path: args[1].clone(),
        theme: args.get(2).filter(|t| !t.is_empty()).cloned(),
        tab,
        lang,
        scale,
        client,
        demo,
    }))
}

/// C# `Screenshot.DemoMonitors()`: Main 2560x1440 at the origin, a centred upper screen, a laptop to the right.
/// The indices are the ones C# `SortAndNumber` gives this layout (asserted by parity row P223).
pub fn demo_monitors() -> Vec<MonitorInfo> {
    let m = |device: &str, id: &str, primary: bool, index: u32, r: Rect| MonitorInfo {
        device: device.to_string(),
        stable_id: id.to_string(),
        bounds: r,
        primary,
        index,
    };
    vec![
        m("DEMO_MAIN", "demo-main", true, 1, Rect::new(0, 0, 2560, 1440)),
        m("DEMO_TOP", "demo-top", false, 2, Rect::new(320, -1080, 2240, 0)),
        m("DEMO_LAPTOP", "demo-laptop", false, 3, Rect::new(2560, 360, 4480, 1440)),
    ]
}

/// C# `Screenshot.DemoSettings(theme, lang)`.
pub fn demo_settings(theme: Option<&str>, lang: &str, backend: &dyn Backend) -> Settings {
    let turkish = lang.eq_ignore_ascii_case("tr");
    let mut s = Settings {
        language: if turkish { "tr" } else { "en" }.to_string(),
        theme: theme.filter(|t| !t.is_empty()).unwrap_or("Light").to_string(),
        boundary_tip_seen: true,
        monitor_names: vec![
            (
                "demo-main".to_string(),
                if turkish { "Ana" } else { "Main" }.to_string(),
            ),
            ("demo-top".to_string(), if turkish { "Üst" } else { "Top" }.to_string()),
            (
                "demo-laptop".to_string(),
                if turkish { "Dizüstü" } else { "Laptop" }.to_string(),
            ),
        ],
        excluded_devices: vec!["demo-laptop".to_string()],
        boundary_rules: vec![StoredRule {
            from_stable_id: "demo-main".to_string(),
            to_stable_id: "demo-top".to_string(),
            direction: Stored::Known(Direction::Up),
            behavior: Stored::Known(RuleBehavior::Push),
            control: Stored::Known(Control::Automatic),
        }],
        ..Settings::default()
    };
    backend.normalize(&mut s);
    s
}

/// C# `SelectPreviewState(form, tabIndex)`.
pub fn select_preview_state(e: &mut Editor, tab: i32) {
    match tab {
        -2 => e.open_advanced_for_preview(),
        t if t < 0 => e.open_first_boundary_for_preview(true),
        1 => e.open_advanced_overview_for_preview(),
        t => e.select_tab(t as usize),
    }
}

/// The client size of a capture (C# 1064x820 for the demo, 1064x712 otherwise, or the WxH override).
pub fn client_size(a: &ShotArgs) -> (i32, i32) {
    a.client.unwrap_or(if a.demo { (1064, 820) } else { (1064, 712) })
}

/// C# metrics line.
pub fn metrics_line(path: &str, client: (i32, i32), m: &LayoutMetrics) -> String {
    format!(
        "capture={} client={}x{} mode={} body={}px map={}px band={}px bandPct={:.2}%",
        path,
        client.0,
        client.1,
        m.mode,
        m.body,
        m.map,
        m.band,
        m.band_percent()
    )
}

/// Services for a capture: no dialogs, no overlays, the theme facts given.
pub struct ShotServices {
    pub system: SystemTheme,
    dialogs: NoDialogs,
}

impl ShotServices {
    pub fn new(system: SystemTheme) -> Self {
        Self {
            system,
            dialogs: NoDialogs,
        }
    }
}

struct NoDialogs;

impl Dialogs for NoDialogs {
    fn pick_backup_path(&mut self, _title: &str, _filter: &str) -> Option<PathBuf> {
        None
    }
    fn pick_restore_path(&mut self, _title: &str, _filter: &str) -> Option<PathBuf> {
        None
    }
    fn confirm_restore(&mut self, _text: &str, _caption: &str) -> bool {
        false
    }
}

impl Services for ShotServices {
    fn dialogs(&mut self) -> &mut dyn Dialogs {
        &mut self.dialogs
    }
    fn identify(&mut self, _monitors: &[MonitorInfo], _names: &[(String, String)], _theme: &Theme) {}
    fn system_theme(&self) -> SystemTheme {
        self.system
    }
}

/// Build the window state for a capture: demo data, or the live settings and monitors for `--screenshot`.
pub fn build_ui(
    a: &ShotArgs,
    backend: Rc<dyn Backend>,
    system: SystemTheme,
    live: Option<(Settings, Vec<MonitorInfo>)>,
) -> SettingsUi {
    let (mut settings, monitors) = if a.demo {
        (demo_settings(a.theme.as_deref(), &a.lang, &*backend), demo_monitors())
    } else {
        live.unwrap_or_default()
    };
    settings.language = if a.lang.is_empty() {
        "en".to_string()
    } else {
        a.lang.clone()
    };
    if let Some(t) = &a.theme {
        settings.theme = t.clone();
    }
    let tr: Tr = crate::text::resolve(&settings.language, fence_win::os_ui_is_turkish());
    let mut editor = Editor::new(&settings, monitors, backend, tr, None);
    select_preview_state(&mut editor, a.tab);
    SettingsUi::new(editor, system)
}

/// The glow window capture. Returns the image and the metrics, or why it failed.
fn capture_glow(
    ui: SettingsUi,
    services: ShotServices,
    client: (i32, i32),
    scale: f32,
) -> Result<(Arc<ColorImage>, LayoutMetrics), String> {
    type Slot = Rc<RefCell<Option<(Arc<ColorImage>, LayoutMetrics)>>>;
    struct ShotApp {
        ui: SettingsUi,
        services: ShotServices,
        frames: u32,
        requested: Option<u32>,
        out: Slot,
    }
    impl eframe::App for ShotApp {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            let ctx = ui.ctx().clone();
            let shot = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            self.ui.show(ui, &mut self.services);
            if let Some(image) = shot {
                if let Some(m) = self.ui.layout_metrics() {
                    *self.out.borrow_mut() = Some((image, m));
                }
                ctx.send_viewport_cmd(ViewportCommand::Close);
                return;
            }
            self.frames += 1;
            if self.requested.is_none() && self.frames >= 12 {
                ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::UserData::default()));
                self.requested = Some(self.frames);
            } else if self.requested.is_some_and(|r| self.frames > r + 60) {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            ctx.request_repaint();
        }
    }

    let out: Slot = Rc::new(RefCell::new(None));
    let (vl, vt, vr, _vb) = crate::win::virtual_screen();
    let title = ui.editor.tr.t("SettingsTitle");
    let app = ShotApp {
        ui,
        services,
        frames: 0,
        requested: None,
        out: out.clone(),
    };
    let size = vec2(client.0 as f32 * scale, client.1 as f32 * scale);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size(size)
            .with_position(egui::pos2((vr + 400) as f32, vt as f32))
            .with_active(false)
            .with_taskbar(false)
            .with_resizable(false),
        renderer: eframe::Renderer::Glow,
        run_and_return: true,
        centered: false,
        ..Default::default()
    };
    let _ = vl;
    let result = eframe::run_native(
        "MouseFence screenshot",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_fonts(crate::ui::font_definitions(true));
            cc.egui_ctx.set_zoom_factor(scale);
            Ok(Box::new(app))
        }),
    );
    if let Err(e) = result {
        return Err(format!("eframe: {e}"));
    }
    let taken = out.borrow_mut().take();
    let (image, metrics) = taken.ok_or_else(|| "no screenshot event arrived".to_string())?;
    if is_blank(&image) {
        return Err("the captured frame is blank".to_string());
    }
    Ok((image, metrics))
}

/// A capture that is one flat colour (or nearly) did not render.
pub fn is_blank(image: &ColorImage) -> bool {
    let Some(first) = image.pixels.first() else { return true };
    let differing = image.pixels.iter().filter(|p| *p != first).count();
    differing * 100 < image.pixels.len()
}

/// The CPU path: the same frames run headless and the last one is rasterized.
pub fn capture_cpu(
    ui: SettingsUi,
    mut services: ShotServices,
    client: (i32, i32),
    scale: f32,
) -> (Vec<u8>, (usize, usize), LayoutMetrics) {
    let mut h = Headless::new(ui, Vec2::new(client.0 as f32, client.1 as f32), scale, true);
    h.settle(&mut services, 8);
    let metrics = h.ui.layout_metrics().unwrap_or(LayoutMetrics {
        mode: "list",
        body: 0,
        map: 0,
        band: 0,
    });
    let r = h.render();
    let flat: Vec<u8> = r.pixels.iter().flat_map(|p| *p).collect();
    (flat, (r.width, r.height), metrics)
}

/// The owner's settings for the non-demo `--screenshot`, read without writing anything: no v2 file, no import
/// save, no folder (JUDGE-FINAL M1). C# 1.1.1 `--screenshot` never created a 2.0 file, and 2.0's one-time import
/// of the 1.x file must happen at its first real start, not during an agent's QA run. `None` (no `%APPDATA%`)
/// gives defaults.
pub(crate) fn live_settings(appdata: Option<&std::path::Path>, monitors: &[MonitorInfo], intel_gpu: bool) -> Settings {
    appdata
        .map(|d| fence_config::load_read_only(&fence_config::Paths::under_appdata(d), monitors, intel_gpu).settings)
        .unwrap_or_default()
}

/// Run a screenshot request; returns the process exit code.
pub fn run(a: &ShotArgs) -> i32 {
    crate::win::set_dpi_awareness();
    let backend: Rc<dyn Backend> = Rc::new(RealBackend);
    let system = crate::win::system_theme();
    let live = if a.demo {
        None
    } else {
        let monitors = fence_win::enumerate_monitors();
        let settings = live_settings(
            fence_win::appdata_dir().as_deref(),
            &monitors,
            fence_win::has_intel_gpu(),
        );
        Some((settings, monitors))
    };
    let client = client_size(a);
    let scale = if a.scale > 1.0 && a.scale <= 3.0 { a.scale } else { 1.0 };
    if let Some(dir) = std::path::Path::new(&a.path).parent()
        && !dir.as_os_str().is_empty()
    {
        let _ = std::fs::create_dir_all(dir);
    }
    let ui = build_ui(a, backend.clone(), system, live.clone());
    let (rgba, size, metrics, renderer) = match capture_glow(ui, ShotServices::new(system), client, scale) {
        Ok((image, m)) => {
            let flat: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
            (flat, (image.size[0], image.size[1]), m, "glow".to_string())
        }
        Err(why) => {
            let ui = build_ui(a, backend, system, live);
            let (flat, size, m) = capture_cpu(ui, ShotServices::new(system), client, scale);
            (flat, size, m, format!("cpu-fallback ({why})"))
        }
    };
    // Opaque output, as the C# PNG.
    let rgba: Vec<u8> = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[0], p[1], p[2], 255])
        .collect();
    if let Err(e) = crate::png::write_rgba(std::path::Path::new(&a.path), size.0 as u32, size.1 as u32, &rgba) {
        eprintln!("screenshot: could not write {}: {e}", a.path);
        return 2;
    }
    eprintln!("renderer={renderer} pixels={}x{} scale={scale}", size.0, size.1);
    println!(
        "{}",
        metrics_line(&a.path, (size.0 as i32, size.1 as i32), &metrics.in_pixels(scale))
    );
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::FakeBackend;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // Parity row B035 (Program.cs:16-41,80-92). The window capture itself runs through low_run on the invisible
    // desktop (evidence in TRACK-D.txt); this test proves the argument forms, the sizes and the metrics line.
    #[test]
    fn b035_screenshot_argument_forms_sizes_and_metrics_line() {
        assert_eq!(
            parse_args(&args(&["--screenshot"])),
            None,
            "a path is required, as in C#"
        );
        assert_eq!(parse_args(&args(&["--identify", "x"])), None);
        let a = parse_args(&args(&["--screenshot-demo", "out.png"]))
            .expect("some")
            .expect("ok");
        assert_eq!(
            (a.theme.clone(), a.tab, a.lang.as_str(), a.scale, a.client, a.demo),
            (None, 0, "en", 1.0, None, true)
        );
        assert_eq!(client_size(&a), (1064, 820));
        let a = parse_args(&args(&["--screenshot", "o.png", "Dark", "-2", "tr", "1.5"]))
            .expect("some")
            .expect("ok");
        assert_eq!(
            (a.theme.as_deref(), a.tab, a.lang.as_str(), a.scale, a.demo),
            (Some("Dark"), -2, "tr", 1.5, false)
        );
        assert_eq!(client_size(&a), (1064, 712));
        let a = parse_args(&args(&["--screenshot-demo", "o.png", "Light", "x", "en", "1600x1040"]))
            .expect("some")
            .expect("ok");
        assert_eq!(
            (a.tab, a.client, a.scale),
            (0, Some((1600, 1040)), 1.0),
            "a bad tab is 0; WxH in the 6th slot"
        );
        let a = parse_args(&args(&[
            "--screenshot-demo",
            "o.png",
            "Light",
            "0",
            "en",
            "2",
            "2560×1352",
        ]))
        .expect("some")
        .expect("ok");
        assert_eq!((a.scale, a.client), (2.0, Some((2560, 1352))));
        assert!(
            parse_args(&args(&["--screenshot-demo", "o.png", "Light", "0", "en", "1", "big"]))
                .expect("some")
                .is_err()
        );
        for (text, want) in [
            ("1600x1040", Some((1600, 1040))),
            ("1600X1040", Some((1600, 1040))),
            ("320×320", Some((320, 320))),
            ("10000x10000", Some((10000, 10000))),
            ("319x400", None),
            ("400x10001", None),
            ("x400", None),
            ("400x", None),
            ("", None),
            ("1.5", None),
        ] {
            assert_eq!(parse_client_size(text), want, "{text}");
        }
        let m = LayoutMetrics {
            mode: "drawer",
            body: 976,
            map: 488,
            band: 49,
        };
        assert_eq!(
            metrics_line(&std::env::temp_dir().join("x.png").to_string_lossy(), (1600, 1040), &m),
            format!(
                "capture={} client=1600x1040 mode=drawer body=976px map=488px band=49px bandPct=5.02%",
                std::env::temp_dir().join("x.png").display()
            )
        );
        assert_eq!(LayoutMetrics { body: 0, ..m }.band_percent(), 0.0);
        assert_eq!(
            m.in_pixels(1.5),
            LayoutMetrics {
                mode: "drawer",
                body: 1464,
                map: 732,
                band: 74
            }
        );
    }

    // Parity row P223 (tests/Tests.cs:2824).
    #[test]
    fn p223_docs_demo_fixed_three_screen_geometry_and_neutral_identities() {
        let demo = demo_monitors();
        let s = demo_settings(Some("Dark"), "en", &FakeBackend);
        assert_eq!(demo.len(), 3);
        assert_eq!(
            demo.iter().map(|m| m.stable_id.as_str()).collect::<Vec<_>>(),
            ["demo-main", "demo-top", "demo-laptop"]
        );
        assert_eq!(demo.iter().map(|m| m.index).collect::<Vec<_>>(), [1, 2, 3]);
        assert!(demo[0].primary && demo[0].bounds.width() == 2560 && demo[0].bounds.height() == 1440);
        assert_eq!(demo[1].bounds.b, demo[0].bounds.t);
        assert_eq!(demo[2].bounds.l, demo[0].bounds.r);
        let name = |id: &str| {
            s.monitor_names
                .iter()
                .find(|(k, _)| k == id)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        assert_eq!(
            (name("demo-main"), name("demo-top"), name("demo-laptop")),
            ("Main".into(), "Top".into(), "Laptop".into())
        );
        assert_eq!(s.excluded_devices, ["demo-laptop"]);
        assert_eq!(s.theme, "Dark");
        assert!(s.boundary_tip_seen);
        let tr = demo_settings(None, "TR", &FakeBackend);
        assert_eq!((tr.language.as_str(), tr.theme.as_str()), ("tr", "Light"));
        assert!(tr.monitor_names.iter().any(|(k, v)| k == "demo-top" && v == "Üst"));
        // Never the live list: building the demo window with a "live" layout ignores it.
        let a = parse_args(&args(&["--screenshot-demo", "o.png", "Dark", "0", "en"]))
            .expect("some")
            .expect("ok");
        let live = Some((
            Settings::default(),
            vec![crate::testkit::mon("live", 1, true, 0, 0, 100, 100)],
        ));
        let ui = build_ui(&a, crate::testkit::fake(), SystemTheme::default(), live);
        assert_eq!(ui.editor.monitors, demo);
    }

    // JUDGE-FINAL M1: the non-demo --screenshot reads the settings of the %APPDATA% it is given and writes
    // nothing there. A temp folder stands in for %APPDATA%; the owner's profile is never touched.
    #[test]
    fn s_ui_screenshot_live_settings_never_write_a_v2_file_or_touch_the_1x_file() {
        use std::time::{Duration, SystemTime};
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let temp = std::env::temp_dir();
        let root = temp.join(format!("mf-ui-shot-test-{}-{nanos}", std::process::id()));
        let v1 = root.join("MouseFence").join("settings.json");
        let v2_dir = root.join("MouseFence").join("v2");
        std::fs::create_dir_all(v1.parent().unwrap()).unwrap();
        // A 1.x file that load() would import and save to the v2 file (a v4 file still saves on import), and a
        // legacy v3 one that load() would migrate and save.
        let cases: [(&str, &[u8]); 2] = [
            ("v4", b"{\"SettingsVersion\":4,\"HotKey\":71}"),
            (
                "v3",
                b"{\"SettingsVersion\":3,\"HotKey\":71,\"Mode\":\"Manual\",\"ManualMonitors\":[\"\\\\\\\\.\\\\DISPLAY2\"]}",
            ),
        ];
        let screen = |device: &str, id: &str, index: u32, primary: bool, r: Rect| MonitorInfo {
            device: device.into(),
            stable_id: id.into(),
            bounds: r,
            primary,
            index,
        };
        let monitors = vec![
            screen("\\\\.\\DISPLAY1", "edid-main", 1, true, Rect::new(0, 0, 100, 100)),
            screen("\\\\.\\DISPLAY2", "edid-top", 2, false, Rect::new(0, -100, 100, 0)),
        ];
        let mut failure = None;
        for (tag, bytes) in cases {
            std::fs::write(&v1, bytes).unwrap();
            let stamp = SystemTime::UNIX_EPOCH + Duration::from_secs(1_600_000_000);
            std::fs::File::options()
                .write(true)
                .open(&v1)
                .unwrap()
                .set_modified(stamp)
                .unwrap();
            let s = live_settings(Some(&root), &monitors, true);
            let check = (|| {
                if s.hot_key != 71 {
                    return Err(format!("{tag}: the 1.x file was not read (hot key {})", s.hot_key));
                }
                if s.mod_shift {
                    return Err(format!("{tag}: an imported file is not a fresh install"));
                }
                if tag == "v3" && s.manual_monitors != ["edid-top"] {
                    return Err(format!("{tag}: not migrated in memory: {:?}", s.manual_monitors));
                }
                if v2_dir.exists() {
                    return Err(format!("{tag}: {} appeared", v2_dir.display()));
                }
                if std::fs::read(&v1).unwrap() != bytes {
                    return Err(format!("{tag}: the 1.x bytes changed"));
                }
                if std::fs::metadata(&v1).unwrap().modified().unwrap() != stamp {
                    return Err(format!("{tag}: the 1.x modified time changed"));
                }
                let mut names: Vec<String> = std::fs::read_dir(v1.parent().unwrap())
                    .unwrap()
                    .filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect();
                names.sort();
                if names != ["settings.json"] {
                    return Err(format!("{tag}: the folder now holds {names:?}"));
                }
                Ok(())
            })();
            if let Err(e) = check {
                failure = Some(e);
                break;
            }
        }
        assert_eq!(live_settings(None, &monitors, false), Settings::default());
        // Remove only this test's own folder (a direct child of the temp folder with this test's prefix).
        let own = root.parent() == Some(temp.as_path())
            && root
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("mf-ui-shot-test-"));
        if own {
            let _ = std::fs::remove_dir_all(&root);
        }
        if let Some(e) = failure {
            panic!("{e}");
        }
    }
}
