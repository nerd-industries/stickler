//! Non-text label elements: QR codes, barcodes, icons, images and borders.

use super::{Canvas, Rect};
use crate::error::RenderError;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tiny_skia::Transform;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/icons.rs"));
}

fn snap(r: Rect) -> Rect {
    let x = r.x.ceil();
    let y = r.y.ceil();
    Rect { x, y, w: (r.x + r.w - x).floor().max(0.0), h: (r.y + r.h - y).floor().max(0.0) }
}

/// QR code as the largest square that fits, whole-dot modules, 1-module quiet zone.
/// Left-aligned and vertically centred in `rect`; returns the area used.
pub fn draw_qr(c: &mut Canvas, rect: Rect, data: &str) -> Result<Rect, RenderError> {
    let data = data.trim();
    if data.is_empty() {
        return Err(RenderError::InvalidData("The QR code needs some text or a link.".into()));
    }
    let code = qrcode::QrCode::new(data.as_bytes())
        .map_err(|e| RenderError::InvalidData(format!("Can't make a QR code from that: {e}")))?;
    let n = code.width() as f32;
    let r = snap(rect);
    let m = (r.w.min(r.h) / (n + 2.0)).floor();
    if m < 1.0 {
        return Err(RenderError::InvalidData("Too much data for a QR code on this label. Use shorter text.".into()));
    }
    let side = m * (n + 2.0);
    let x0 = r.x;
    let y0 = r.y + ((r.h - side) / 2.0).floor();
    for (i, col) in code.to_colors().iter().enumerate() {
        if *col == qrcode::Color::Dark {
            let (cx, cy) = ((i % code.width()) as f32, (i / code.width()) as f32);
            c.fill_rect(Rect { x: x0 + (cx + 1.0) * m, y: y0 + (cy + 1.0) * m, w: m, h: m });
        }
    }
    Ok(Rect { x: x0, y: y0, w: side, h: side })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BarcodeKind {
    #[default]
    Auto,
    Code128,
    Ean13,
}

/// `Auto` → EAN-13 for 12–13 digits with a valid checksum, else Code128.
pub fn choose_kind(data: &str, kind: BarcodeKind) -> BarcodeKind {
    match kind {
        BarcodeKind::Auto => {
            let d = data.trim();
            let digits = (12..=13).contains(&d.len()) && d.bytes().all(|b| b.is_ascii_digit());
            if digits && barcoders::sym::ean13::EAN13::new(d).is_ok() {
                BarcodeKind::Ean13
            } else {
                BarcodeKind::Code128
            }
        }
        k => k,
    }
}

/// Barcode filling `rect`'s height, centred horizontally, bars on whole dots.
pub fn draw_barcode(c: &mut Canvas, rect: Rect, data: &str, kind: BarcodeKind) -> Result<Rect, RenderError> {
    let d = data.trim();
    if d.is_empty() {
        return Err(RenderError::InvalidData("The barcode needs some data.".into()));
    }
    let modules = match choose_kind(d, kind) {
        BarcodeKind::Ean13 => barcoders::sym::ean13::EAN13::new(d)
            .map_err(|e| RenderError::InvalidData(format!("Not a valid EAN-13 number: {e}")))?
            .encode(),
        _ => {
            if !d.is_ascii() {
                return Err(RenderError::InvalidData(
                    "Barcodes can only hold plain letters, numbers and symbols (no accents or emoji).".into(),
                ));
            }
            barcoders::sym::code128::Code128::new(format!("\u{0181}{d}"))
                .map_err(|e| RenderError::InvalidData(format!("Can't make a barcode from that: {e}")))?
                .encode()
        }
    };
    let r = snap(rect);
    let quiet = 8.0;
    let m = (r.w / (modules.len() as f32 + quiet)).floor();
    if m < 1.0 {
        return Err(RenderError::InvalidData("That barcode is too long for this label. Use shorter data.".into()));
    }
    let w = m * modules.len() as f32;
    let x0 = r.x + ((r.w - w) / 2.0).floor();
    for (i, &bit) in modules.iter().enumerate() {
        if bit == 1 {
            c.fill_rect(Rect { x: x0 + i as f32 * m, y: r.y, w: m, h: r.h });
        }
    }
    Ok(Rect { x: x0, y: r.y, w, h: r.h })
}

pub fn icon_names() -> Vec<&'static str> {
    embedded::ICONS.iter().map(|(n, _)| *n).collect()
}

/// Icon as the largest square that fits, centred in `rect`.
pub fn draw_icon(c: &mut Canvas, rect: Rect, name: &str) -> Result<Rect, RenderError> {
    let svg = embedded::ICONS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, b)| String::from_utf8_lossy(b).replace("currentColor", "#000"))
        .ok_or_else(|| RenderError::Asset(format!("There's no icon called '{name}'.")))?;
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default())
        .map_err(|e| RenderError::Asset(format!("icon {name}: {e}")))?;
    let r = snap(rect);
    let side = r.w.min(r.h);
    let (x, y) = ((r.x + (r.w - side) / 2.0).floor(), (r.y + (r.h - side) / 2.0).floor());
    let scale = side / tree.size().width().max(tree.size().height());
    resvg::render(&tree, Transform::from_translate(x, y).pre_scale(scale, scale), &mut c.pixmap.as_mut());
    Ok(Rect { x, y, w: side, h: side })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dither {
    Threshold(u8),
    #[default]
    FloydSteinberg,
}

/// Image fitted inside `rect` (aspect kept, centred), converted to pure black/white.
pub fn draw_image(c: &mut Canvas, rect: Rect, path: &Path, dither: Dither) -> Result<Rect, RenderError> {
    let img = image::open(path)
        .map_err(|e| RenderError::Asset(format!("Can't open image {}: {e}", path.display())))?
        .to_luma8();
    let r = snap(rect);
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    let s = (r.w / iw).min(r.h / ih);
    let (tw, th) = ((iw * s).floor().max(1.0) as u32, (ih * s).floor().max(1.0) as u32);
    let small = image::imageops::resize(&img, tw, th, image::imageops::FilterType::Triangle);
    let mut buf: Vec<f32> = small.pixels().map(|p| p.0[0] as f32).collect();
    let (w, h) = (tw as usize, th as usize);
    let mut out = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let black = match dither {
                Dither::Threshold(t) => buf[i] < t as f32,
                Dither::FloydSteinberg => buf[i] < 128.0,
            };
            out[i] = black;
            if let Dither::FloydSteinberg = dither {
                let err = buf[i] - if black { 0.0 } else { 255.0 };
                let mut spread = |dx: isize, dy: usize, f: f32| {
                    let nx = x as isize + dx;
                    if nx >= 0 && (nx as usize) < w && y + dy < h {
                        buf[(y + dy) * w + nx as usize] += err * f;
                    }
                };
                spread(1, 0, 7.0 / 16.0);
                spread(-1, 1, 3.0 / 16.0);
                spread(0, 1, 5.0 / 16.0);
                spread(1, 1, 1.0 / 16.0);
            }
        }
    }
    let (x0, y0) = ((r.x + (r.w - tw as f32) / 2.0).floor() as u32, (r.y + (r.h - th as f32) / 2.0).floor() as u32);
    let pw = c.pixmap.width();
    let data = c.pixmap.data_mut();
    for y in 0..th {
        for x in 0..tw {
            let i = (((y0 + y) * pw + x0 + x) * 4) as usize;
            if i + 3 < data.len() {
                let v = if out[(y * tw + x) as usize] { 0 } else { 255 };
                data[i..i + 3].fill(v);
                data[i + 3] = 255;
            }
        }
    }
    Ok(Rect { x: x0 as f32, y: y0 as f32, w: tw as f32, h: th as f32 })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Border {
    #[default]
    None,
    Thin,
    Thick,
    Rounded,
    Double,
}

fn frame(c: &mut Canvas, r: Rect, t: f32) {
    c.fill_rect(Rect { x: r.x, y: r.y, w: r.w, h: t });
    c.fill_rect(Rect { x: r.x, y: r.y + r.h - t, w: r.w, h: t });
    c.fill_rect(Rect { x: r.x, y: r.y, w: t, h: r.h });
    c.fill_rect(Rect { x: r.x + r.w - t, y: r.y, w: t, h: r.h });
}

/// Draw a border around the whole canvas; returns the content rect inside it.
pub fn draw_border(c: &mut Canvas, b: Border) -> Rect {
    let outer = c.bounds();
    match b {
        Border::None => outer,
        Border::Thin => {
            frame(c, outer, 2.0);
            outer.inset(3.0)
        }
        Border::Thick => {
            frame(c, outer, 4.0);
            outer.inset(5.0)
        }
        Border::Rounded => {
            c.stroke_round_rect(outer.inset(1.5), 16.0, 3.0);
            // Keep content clear of the 2 mm corners.
            outer.inset(5.0)
        }
        Border::Double => {
            frame(c, outer, 1.0);
            frame(c, outer.inset(2.0), 1.0);
            outer.inset(4.0)
        }
    }
}
