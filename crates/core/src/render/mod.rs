//! Label rendering. Templates draw on a landscape `Canvas` (length × width, in
//! dots); `finish` thresholds it to 1-bit; `RenderedLabel` produces both the
//! print bitmap and the preview PNG from the same pixels.

pub mod text;

use crate::geometry::{DOTS_PER_MM, LabelSize, PrintArea, print_area};
use crate::protocol::PrintBitmap;
use serde::{Deserialize, Serialize};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

pub use crate::config::Rotation;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn inset(&self, d: f32) -> Rect {
        Rect { x: self.x + d, y: self.y + d, w: (self.w - 2.0 * d).max(0.0), h: (self.h - 2.0 * d).max(0.0) }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontChoice {
    #[default]
    Sans,
    SansBold,
    Condensed,
    Rounded,
}

/// Landscape drawing surface: width = printable length, height = printable width.
pub struct Canvas {
    pub pixmap: Pixmap,
}

impl Canvas {
    pub fn new(area: &PrintArea) -> Self {
        let mut pixmap = Pixmap::new(area.length_dots.max(1), area.width_dots.max(1)).expect("non-zero canvas");
        pixmap.fill(Color::WHITE);
        Self { pixmap }
    }

    pub fn bounds(&self) -> Rect {
        Rect { x: 0.0, y: 0.0, w: self.pixmap.width() as f32, h: self.pixmap.height() as f32 }
    }

    pub fn set_black(&mut self, x: u32, y: u32) {
        self.fill_rect(Rect { x: x as f32, y: y as f32, w: 1.0, h: 1.0 });
    }

    pub fn fill_rect(&mut self, r: Rect) {
        if let Some(rect) = tiny_skia::Rect::from_xywh(r.x, r.y, r.w, r.h) {
            self.pixmap.fill_rect(rect, &black(), Transform::identity(), None);
        }
    }

    pub fn stroke_round_rect(&mut self, r: Rect, radius: f32, width: f32) {
        if let Some(path) = round_rect_path(r, radius) {
            let stroke = Stroke { width, ..Default::default() };
            self.pixmap.stroke_path(&path, &black(), &stroke, Transform::identity(), None);
        }
    }

    /// Blend `alpha` (0–255) of ink onto one pixel; used by glyph and image drawing.
    pub fn blend(&mut self, x: i32, y: i32, alpha: u8) {
        if x < 0 || y < 0 || x >= self.pixmap.width() as i32 || y >= self.pixmap.height() as i32 {
            return;
        }
        let i = (y as usize * self.pixmap.width() as usize + x as usize) * 4;
        let px = &mut self.pixmap.data_mut()[i..i + 3];
        for c in px {
            *c = ((*c as u32 * (255 - alpha as u32)) / 255) as u8;
        }
    }
}

pub(crate) fn black() -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(Color::BLACK);
    p.anti_alias = false;
    p
}

pub(crate) fn round_rect_path(r: Rect, radius: f32) -> Option<tiny_skia::Path> {
    let rad = radius.min(r.w / 2.0).min(r.h / 2.0);
    let (x0, y0, x1, y1) = (r.x, r.y, r.x + r.w, r.y + r.h);
    let mut pb = PathBuilder::new();
    pb.move_to(x0 + rad, y0);
    pb.line_to(x1 - rad, y0);
    pb.quad_to(x1, y0, x1, y0 + rad);
    pb.line_to(x1, y1 - rad);
    pb.quad_to(x1, y1, x1 - rad, y1);
    pb.line_to(x0 + rad, y1);
    pb.quad_to(x0, y1, x0, y1 - rad);
    pb.line_to(x0, y0 + rad);
    pb.quad_to(x0, y0, x0 + rad, y0);
    pb.close();
    pb.finish()
}

/// A finished 1-bit label in landscape orientation (`true` = black).
#[derive(Clone, Debug)]
pub struct RenderedLabel {
    pub size: LabelSize,
    pub area: PrintArea,
    pub mono: Vec<bool>,
    pub w: u32,
    pub h: u32,
}

/// Threshold the canvas: luminance below `threshold` prints black.
pub fn finish(c: Canvas, size: LabelSize, threshold: u8) -> RenderedLabel {
    let (w, h) = (c.pixmap.width(), c.pixmap.height());
    let mono = c
        .pixmap
        .data()
        .chunks_exact(4)
        .map(|p| {
            let lum = (p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000;
            lum < threshold as u32
        })
        .collect();
    RenderedLabel { size, area: print_area(size), mono, w, h }
}

impl RenderedLabel {
    fn at(&self, x: u32, y: u32) -> bool {
        self.mono[(y * self.w + x) as usize]
    }

    pub fn invert(&mut self) {
        self.mono.iter_mut().for_each(|b| *b = !*b);
    }

    /// Rotate to print rows (one row per dot along the feed) and pack MSB-first, 0 = black.
    pub fn to_print_bitmap(&self, rot: Rotation) -> PrintBitmap {
        let (lw, lh) = (self.w, self.h); // landscape: lw = length, lh = width
        let width_bytes = lh.div_ceil(8);
        let mut data = vec![0xFFu8; (width_bytes * lw) as usize];
        for r in 0..lw {
            for c in 0..lh {
                let black = match rot {
                    Rotation::Cw => self.at(r, lh - 1 - c),
                    Rotation::Ccw => self.at(lw - 1 - r, c),
                };
                if black {
                    data[(r * width_bytes + c / 8) as usize] &= !(0x80 >> (c % 8));
                }
            }
        }
        PrintBitmap { width_bytes: width_bytes as u16, height: lw as u16, y_offset: self.area.offset_dots as u16, data }
    }

    /// Landscape preview of the whole label at `scale`× (nearest neighbour).
    pub fn preview_png(&self, scale: u32) -> Vec<u8> {
        let s = scale.max(1);
        let lw = self.size.length_mm as u32 * DOTS_PER_MM;
        let lh = self.size.width_mm as u32 * DOTS_PER_MM;
        let mut pm = Pixmap::new(lw * s, lh * s).expect("non-zero preview");
        let body = Rect { x: 0.5, y: 0.5, w: (lw * s) as f32 - 1.0, h: (lh * s) as f32 - 1.0 };
        let radius = 2.0 * DOTS_PER_MM as f32 * s as f32;
        if let Some(path) = round_rect_path(body, radius) {
            let mut fill = Paint::default();
            fill.set_color_rgba8(255, 255, 255, 255);
            fill.anti_alias = true;
            pm.fill_path(&path, &fill, FillRule::Winding, Transform::identity(), None);
            let mut edge = Paint::default();
            edge.set_color_rgba8(200, 200, 205, 255);
            edge.anti_alias = true;
            pm.stroke_path(&path, &edge, &Stroke { width: s as f32, ..Default::default() }, Transform::identity(), None);
        }
        let ox = self.area.offset_dots;
        let oy = lh.saturating_sub(self.h) / 2;
        let ink = tiny_skia::PremultipliedColorU8::from_rgba(17, 17, 20, 255).expect("valid colour");
        let pw = pm.width();
        let pixels = pm.pixels_mut();
        for y in 0..self.h {
            for x in 0..self.w {
                if !self.at(x, y) {
                    continue;
                }
                for dy in 0..s {
                    for dx in 0..s {
                        let (px, py) = ((ox + x) * s + dx, (oy + y) * s + dy);
                        if px < lw * s && py < lh * s {
                            pixels[(py * pw + px) as usize] = ink;
                        }
                    }
                }
            }
        }
        pm.encode_png().expect("png encode")
    }
}
