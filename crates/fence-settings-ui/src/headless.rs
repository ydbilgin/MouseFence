//! Drive the window without a window: an `egui::Context` fed raw input, frame by frame. The parity tests use it
//! (with synthetic pointer and key events), and the screenshot modes use it as the CPU fallback when the glow
//! window cannot render (see `screenshot`).

use eframe::egui::{self, Context, Event, FullOutput, Pos2, RawInput, Rect, Vec2, ViewportId, ViewportInfo, pos2};

use crate::raster::Raster;
use crate::ui::{Services, SettingsUi, font_definitions};

pub struct Headless {
    pub ctx: Context,
    pub ui: SettingsUi,
    /// Client size in points.
    pub size: Vec2,
    /// Physical pixels per point (the DPI scale).
    pub ppp: f32,
    pub time: f64,
    pub last: Option<FullOutput>,
    /// Every texture the frames uploaded (the font atlas), for `render`.
    pub raster: Raster,
    pub modifiers: egui::Modifiers,
}

impl Headless {
    pub fn new(ui: SettingsUi, size: Vec2, ppp: f32, windows_fonts: bool) -> Self {
        let ctx = Context::default();
        ctx.set_fonts(font_definitions(windows_fonts));
        ctx.enable_accesskit();
        let (w, h) = ((size.x * ppp).round() as usize, (size.y * ppp).round() as usize);
        Self {
            ctx,
            ui,
            size,
            ppp,
            time: 0.0,
            last: None,
            raster: Raster::new(w.max(1), h.max(1), egui::Color32::BLACK),
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Run one frame with these events (16 ms of simulated time apart).
    pub fn frame(&mut self, services: &mut dyn Services, events: Vec<Event>) -> &FullOutput {
        self.time += 0.016;
        let mut raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.time),
            events,
            focused: true,
            ..Default::default()
        };
        raw.viewports.insert(
            ViewportId::ROOT,
            ViewportInfo {
                native_pixels_per_point: Some(self.ppp),
                focused: Some(true),
                ..Default::default()
            },
        );
        let ui = &mut self.ui;
        let mut out = self.ctx.run_ui(raw, |root| ui.show(root, services));
        self.raster.update_textures(&out.textures_delta);
        // Applied: epaint asserts that a dropped delta was handled.
        out.textures_delta.clear();
        self.last = Some(out);
        self.last.as_ref().expect("just set")
    }

    /// Run `n` frames without input (lets layout settle: page heights, scroll targets).
    pub fn settle(&mut self, services: &mut dyn Services, n: usize) {
        for _ in 0..n {
            self.frame(services, Vec::new());
        }
    }

    /// Paint the last frame into an RGBA image at the physical size.
    pub fn render(&mut self) -> &Raster {
        let (w, h) = (
            (self.size.x * self.ppp).round() as usize,
            (self.size.y * self.ppp).round() as usize,
        );
        self.raster.reset(w.max(1), h.max(1), egui::Color32::BLACK);
        if let Some(out) = &self.last {
            let prims = self.ctx.tessellate(out.shapes.clone(), out.pixels_per_point);
            self.raster.paint(&prims, out.pixels_per_point);
        }
        &self.raster
    }

    /// A pointer click at a point in points: move, press, release over three frames (egui needs them apart).
    pub fn click_at(&mut self, services: &mut dyn Services, p: Pos2) {
        let m = self.modifiers;
        self.frame(services, vec![Event::PointerMoved(p)]);
        self.frame(
            services,
            vec![Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: m,
            }],
        );
        self.frame(
            services,
            vec![Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: m,
            }],
        );
        self.frame(services, Vec::new());
    }

    /// Press and release a key.
    pub fn key(&mut self, services: &mut dyn Services, key: egui::Key, modifiers: egui::Modifiers) {
        self.frame(
            services,
            vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed: true,
                repeat: false,
                modifiers,
            }],
        );
        self.frame(
            services,
            vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed: false,
                repeat: false,
                modifiers,
            }],
        );
    }

    pub fn hover(&mut self, services: &mut dyn Services, p: Pos2) {
        self.frame(services, vec![Event::PointerMoved(p)]);
    }

    pub fn pointer_gone(&mut self, services: &mut dyn Services) {
        self.frame(services, vec![Event::PointerGone]);
    }

    pub fn center_of(&self, tag: &str) -> Option<Pos2> {
        self.ui.rec.get(tag).map(|r| r.rect.center())
    }

    pub fn origin() -> Pos2 {
        pos2(0.0, 0.0)
    }
}
