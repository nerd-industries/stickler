//! Text drawing with bundled fonts and auto-fit.

use super::{Align, Canvas, FontChoice, Rect};
use crate::error::RenderError;
use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache, Weight, Wrap};
use std::sync::{Mutex, OnceLock};

const LINE_HEIGHT: f32 = 1.12;

#[derive(Clone, Debug)]
pub struct TextStyle {
    pub font: FontChoice,
    pub bold: bool,
    pub align: Align,
    pub max_px: f32,
    pub min_px: f32,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self { font: FontChoice::Sans, bold: false, align: Align::Center, max_px: f32::MAX, min_px: 6.0 }
    }
}

struct Fonts {
    system: FontSystem,
    cache: SwashCache,
}

fn fonts() -> &'static Mutex<Fonts> {
    static FONTS: OnceLock<Mutex<Fonts>> = OnceLock::new();
    FONTS.get_or_init(|| {
        let mut db = cosmic_text::fontdb::Database::new();
        for data in [
            &include_bytes!("../../assets/fonts/Roboto-Regular.ttf")[..],
            include_bytes!("../../assets/fonts/Roboto-Bold.ttf"),
            include_bytes!("../../assets/fonts/RobotoCondensed-Regular.ttf"),
            include_bytes!("../../assets/fonts/RobotoCondensed-Bold.ttf"),
            include_bytes!("../../assets/fonts/Nunito-Regular.ttf"),
            include_bytes!("../../assets/fonts/Nunito-Bold.ttf"),
        ] {
            db.load_font_data(data.to_vec());
        }
        // System fonts give fallback glyphs (CJK, symbols, emoji).
        db.load_system_fonts();
        let system = FontSystem::new_with_locale_and_db("en-US".into(), db);
        Mutex::new(Fonts { system, cache: SwashCache::new() })
    })
}

fn attrs(style: &TextStyle) -> Attrs<'static> {
    let family = match style.font {
        FontChoice::Sans | FontChoice::SansBold => "Roboto",
        FontChoice::Condensed => "Roboto Condensed",
        FontChoice::Rounded => "Nunito",
    };
    let bold = style.bold || style.font == FontChoice::SansBold;
    Attrs::new().family(Family::Name(family)).weight(if bold { Weight::BOLD } else { Weight::NORMAL })
}

fn shape(fs: &mut FontSystem, text: &str, px: f32, style: &TextStyle) -> (Buffer, f32) {
    let mut buf = Buffer::new(fs, Metrics::new(px, px * LINE_HEIGHT));
    buf.set_wrap(Wrap::None);
    buf.set_size(None, None);
    buf.set_text(text, &attrs(style), Shaping::Advanced, None);
    buf.shape_until_scroll(fs, false);
    let w = buf.layout_runs().map(|r| r.line_w).fold(0.0, f32::max);
    (buf, w)
}

fn fits(fs: &mut FontSystem, lines: &[String], px: f32, rect: Rect, style: &TextStyle) -> bool {
    if px * LINE_HEIGHT * lines.len() as f32 > rect.h + 0.5 {
        return false;
    }
    lines.iter().all(|l| shape(fs, l, px, style).1 <= rect.w)
}

/// Shorten `line` with "…" until it fits `max_w` at `px`.
fn truncate(fs: &mut FontSystem, line: &str, px: f32, max_w: f32, style: &TextStyle) -> String {
    if shape(fs, line, px, style).1 <= max_w {
        return line.to_owned();
    }
    let chars: Vec<char> = line.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let candidate: String = chars[..mid].iter().collect::<String>().trim_end().to_owned() + "…";
        if shape(fs, &candidate, px, style).1 <= max_w { lo = mid } else { hi = mid - 1 }
    }
    chars[..lo].iter().collect::<String>().trim_end().to_owned() + "…"
}

/// Draw `lines` as large as possible inside `rect`; returns the chosen pixel size.
/// At `min_px`, lines that still overflow are truncated with "…".
pub fn draw_text_fit(c: &mut Canvas, rect: Rect, lines: &[&str], style: &TextStyle) -> Result<f32, RenderError> {
    let lines: Vec<String> = lines.iter().map(|l| l.trim().to_owned()).filter(|l| !l.is_empty()).collect();
    if lines.is_empty() || rect.w < 1.0 || rect.h < 1.0 {
        return Ok(style.min_px);
    }
    let mut guard = fonts().lock().unwrap_or_else(|p| p.into_inner());
    let Fonts { system: fs, cache } = &mut *guard;

    let max_px = style.max_px.min(rect.h / (LINE_HEIGHT * lines.len() as f32)).max(style.min_px);
    let mut px = style.min_px;
    if fits(fs, &lines, max_px, rect, style) {
        px = max_px;
    } else {
        let (mut lo, mut hi) = (style.min_px, max_px);
        while hi - lo > 0.25 {
            let mid = (lo + hi) / 2.0;
            if fits(fs, &lines, mid, rect, style) { lo = mid } else { hi = mid }
        }
        if fits(fs, &lines, lo, rect, style) {
            px = lo;
        }
    }
    let px = if px <= style.min_px { style.min_px } else { (px * 4.0).floor() / 4.0 };
    let lines: Vec<String> = if fits(fs, &lines, px, rect, style) {
        lines
    } else {
        lines.iter().map(|l| truncate(fs, l, px, rect.w, style)).collect()
    };

    let line_h = px * LINE_HEIGHT;
    let total_h = line_h * lines.len() as f32;
    let top = rect.y + ((rect.h - total_h) / 2.0).max(0.0);
    let (x0, y0, x1, y1) = (rect.x.floor() as i32, rect.y.floor() as i32, (rect.x + rect.w).ceil() as i32, (rect.y + rect.h).ceil() as i32);
    for (i, line) in lines.iter().enumerate() {
        let (mut buf, w) = shape(fs, line, px, style);
        let dx = match style.align {
            Align::Left => 0.0,
            Align::Center => ((rect.w - w) / 2.0).max(0.0),
            Align::Right => (rect.w - w).max(0.0),
        };
        let ox = (rect.x + dx).round() as i32;
        let oy = (top + i as f32 * line_h).round() as i32;
        buf.draw(fs, cache, Color::rgb(0, 0, 0), |x, y, w, h, color| {
            let a = color.a();
            if a == 0 {
                return;
            }
            for yy in 0..h as i32 {
                for xx in 0..w as i32 {
                    let (px, py) = (ox + x + xx, oy + y + yy);
                    if px >= x0 && px < x1 && py >= y0 && py < y1 {
                        c.blend(px, py, a);
                    }
                }
            }
        });
    }
    Ok(px)
}
