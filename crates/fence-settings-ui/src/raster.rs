//! A small CPU rasterizer for egui's tessellated meshes, so headless tests can look at pixels without a window or
//! a GPU (parity rows P003 and P224). It follows egui's glow painter: colours stay in sRGB gamma space, texture
//! and vertex colours multiply, and blending is premultiplied (`ONE, ONE_MINUS_SRC_ALPHA`).

use std::collections::HashMap;

use eframe::egui::{ClippedPrimitive, Color32, TextureId, TexturesDelta, epaint::Primitive};

struct Texture {
    w: usize,
    h: usize,
    px: Vec<Color32>,
}

pub struct Raster {
    pub width: usize,
    pub height: usize,
    /// Opaque RGBA rows, top to bottom.
    pub pixels: Vec<[u8; 4]>,
    textures: HashMap<TextureId, Texture>,
}

impl Raster {
    pub fn new(width: usize, height: usize, clear: Color32) -> Self {
        Self {
            width,
            height,
            pixels: vec![[clear.r(), clear.g(), clear.b(), 255]; width * height],
            textures: HashMap::new(),
        }
    }

    /// Clear the canvas to a new size; the uploaded textures stay.
    pub fn reset(&mut self, width: usize, height: usize, clear: Color32) {
        self.width = width;
        self.height = height;
        self.pixels = vec![[clear.r(), clear.g(), clear.b(), 255]; width * height];
    }

    /// Apply a frame's texture changes (the font atlas arrives here).
    pub fn update_textures(&mut self, delta: &TexturesDelta) {
        for (id, d) in delta.set.iter().flat_map(|(id, ds)| ds.iter().map(move |d| (id, d))) {
            let eframe::egui::ImageData::Color(img) = &d.image;
            match d.pos {
                None => {
                    self.textures.insert(
                        *id,
                        Texture {
                            w: img.size[0],
                            h: img.size[1],
                            px: img.pixels.clone(),
                        },
                    );
                }
                Some([x0, y0]) => {
                    if let Some(t) = self.textures.get_mut(id) {
                        for y in 0..img.size[1] {
                            for x in 0..img.size[0] {
                                let (tx, ty) = (x0 + x, y0 + y);
                                if tx < t.w && ty < t.h {
                                    t.px[ty * t.w + tx] = img.pixels[y * img.size[0] + x];
                                }
                            }
                        }
                    }
                }
            }
        }
        for id in &delta.free {
            self.textures.remove(id);
        }
    }

    fn sample(&self, id: TextureId, u: f32, v: f32) -> [f32; 4] {
        let Some(t) = self.textures.get(&id) else {
            return [1.0; 4];
        };
        if t.w == 0 || t.h == 0 {
            return [1.0; 4];
        }
        // Bilinear, clamped (egui's default TextureOptions::LINEAR).
        let fx = (u * t.w as f32 - 0.5).clamp(0.0, (t.w - 1) as f32);
        let fy = (v * t.h as f32 - 0.5).clamp(0.0, (t.h - 1) as f32);
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(t.w - 1), (y0 + 1).min(t.h - 1));
        let (ax, ay) = (fx - x0 as f32, fy - y0 as f32);
        let get = |x: usize, y: usize| {
            let c = t.px[y * t.w + x];
            [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32]
        };
        let (a, b, c, d) = (get(x0, y0), get(x1, y0), get(x0, y1), get(x1, y1));
        let mut out = [0.0; 4];
        for i in 0..4 {
            let top = a[i] + (b[i] - a[i]) * ax;
            let bottom = c[i] + (d[i] - c[i]) * ax;
            out[i] = (top + (bottom - top) * ay) / 255.0;
        }
        out
    }

    /// Paint one frame's tessellated primitives at `ppp` physical pixels per point.
    pub fn paint(&mut self, primitives: &[ClippedPrimitive], ppp: f32) {
        for p in primitives {
            let Primitive::Mesh(mesh) = &p.primitive else { continue };
            let clip_x0 = (p.clip_rect.min.x * ppp).round().max(0.0) as i64;
            let clip_y0 = (p.clip_rect.min.y * ppp).round().max(0.0) as i64;
            let clip_x1 = ((p.clip_rect.max.x * ppp).round() as i64).min(self.width as i64);
            let clip_y1 = ((p.clip_rect.max.y * ppp).round() as i64).min(self.height as i64);
            if clip_x0 >= clip_x1 || clip_y0 >= clip_y1 {
                continue;
            }
            for tri in mesh.indices.as_chunks::<3>().0 {
                let v = [
                    &mesh.vertices[tri[0] as usize],
                    &mesh.vertices[tri[1] as usize],
                    &mesh.vertices[tri[2] as usize],
                ];
                let pts = v.map(|x| (x.pos.x * ppp, x.pos.y * ppp));
                let area =
                    (pts[1].0 - pts[0].0) * (pts[2].1 - pts[0].1) - (pts[2].0 - pts[0].0) * (pts[1].1 - pts[0].1);
                if area.abs() < 1e-6 {
                    continue;
                }
                let min_x = pts
                    .iter()
                    .map(|p| p.0)
                    .fold(f32::MAX, f32::min)
                    .floor()
                    .max(clip_x0 as f32) as i64;
                let max_x = pts
                    .iter()
                    .map(|p| p.0)
                    .fold(f32::MIN, f32::max)
                    .ceil()
                    .min(clip_x1 as f32) as i64;
                let min_y = pts
                    .iter()
                    .map(|p| p.1)
                    .fold(f32::MAX, f32::min)
                    .floor()
                    .max(clip_y0 as f32) as i64;
                let max_y = pts
                    .iter()
                    .map(|p| p.1)
                    .fold(f32::MIN, f32::max)
                    .ceil()
                    .min(clip_y1 as f32) as i64;
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                        let w0 = ((pts[1].0 - px) * (pts[2].1 - py) - (pts[2].0 - px) * (pts[1].1 - py)) / area;
                        let w1 = ((pts[2].0 - px) * (pts[0].1 - py) - (pts[0].0 - px) * (pts[2].1 - py)) / area;
                        let w2 = 1.0 - w0 - w1;
                        if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                            continue;
                        }
                        let u = v[0].uv.x * w0 + v[1].uv.x * w1 + v[2].uv.x * w2;
                        let t = v[0].uv.y * w0 + v[1].uv.y * w1 + v[2].uv.y * w2;
                        let tex = self.sample(mesh.texture_id, u, t);
                        let mut src = [0.0f32; 4];
                        for (i, s) in src.iter_mut().enumerate() {
                            let c = |k: usize| v[k].color.to_array()[i] as f32 / 255.0;
                            *s = (c(0) * w0 + c(1) * w1 + c(2) * w2) * tex[i];
                        }
                        let d = &mut self.pixels[y as usize * self.width + x as usize];
                        for i in 0..3 {
                            let dst = d[i] as f32 / 255.0;
                            d[i] = ((src[i] + dst * (1.0 - src[3])).clamp(0.0, 1.0) * 255.0).round() as u8;
                        }
                    }
                }
            }
        }
    }

    pub fn get(&self, x: usize, y: usize) -> [u8; 4] {
        self.pixels[y * self.width + x]
    }

    pub fn to_png(&self) -> Vec<u8> {
        let flat: Vec<u8> = self.pixels.iter().flat_map(|p| [p[0], p[1], p[2], p[3]]).collect();
        crate::png::encode_rgba(self.width as u32, self.height as u32, &flat)
    }
}
