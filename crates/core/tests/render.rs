use nelko_core::config::Rotation;
use nelko_core::geometry::{LabelSize, print_area};
use nelko_core::render::text::{TextStyle, draw_text_fit};
use nelko_core::render::{Align, Canvas, FontChoice, Rect, finish};

fn size() -> LabelSize {
    "14x40".parse().unwrap()
}

fn canvas() -> Canvas {
    Canvas::new(&print_area(size()))
}

fn full(c: &Canvas) -> Rect {
    Rect { x: 0.0, y: 0.0, w: c.pixmap.width() as f32, h: c.pixmap.height() as f32 }
}

fn style() -> TextStyle {
    TextStyle { font: FontChoice::Sans, bold: true, align: Align::Center, max_px: 90.0, min_px: 6.0 }
}

#[test]
fn canvas_is_landscape_print_area() {
    let c = canvas();
    assert_eq!((c.pixmap.width(), c.pixmap.height()), (284, 96));
}

#[test]
fn packs_black_as_zero_bits() {
    let mut c = canvas();
    c.set_black(0, 0);
    let bm = finish(c, size(), 128).to_print_bitmap(Rotation::Cw);
    // CW: landscape (x=0,y=0) -> print row 0, column 95 -> byte 11, lowest bit.
    assert_eq!(bm.data[11], 0xFE);
    assert_eq!(bm.data.iter().filter(|&&b| b != 0xFF).count(), 1);
}

#[test]
fn ccw_maps_origin_to_last_row() {
    let mut c = canvas();
    c.set_black(0, 0);
    let bm = finish(c, size(), 128).to_print_bitmap(Rotation::Ccw);
    // CCW: landscape (x=0,y=0) -> print row 283, column 0 -> first byte of last row, top bit.
    assert_eq!(bm.data[283 * 12], 0x7F);
}

#[test]
fn rotation_dimensions() {
    let bm = finish(canvas(), size(), 128).to_print_bitmap(Rotation::Cw);
    assert_eq!((bm.width_bytes, bm.height, bm.y_offset), (12, 284, 18));
    assert_eq!(bm.data.len(), 12 * 284);
    assert!(bm.data.iter().all(|&b| b == 0xFF));
}

#[test]
fn autofit_picks_smaller_for_longer_text() {
    let mut c = canvas();
    let r = full(&c);
    let short = draw_text_fit(&mut c, r, &["Hi"], &style()).unwrap();
    let mut c2 = canvas();
    let long = draw_text_fit(&mut c2, r, &["Hello there friend"], &style()).unwrap();
    assert!(short > long, "short {short} long {long}");
}

#[test]
fn autofit_truncates_with_ellipsis() {
    let mut c = canvas();
    let r = full(&c);
    let text = "word ".repeat(40);
    let px = draw_text_fit(&mut c, r, &[text.as_str()], &style()).unwrap();
    assert_eq!(px, 6.0);
    let label = finish(c, size(), 128);
    assert!(label.mono.iter().any(|&b| b));
}

#[test]
fn renders_unicode_without_panic() {
    let mut c = canvas();
    let r = full(&c);
    draw_text_fit(&mut c, r, &["Café ✓ 日本 🍕"], &style()).unwrap();
    assert!(finish(c, size(), 128).mono.iter().any(|&b| b));
}

#[test]
fn text_stays_inside_rect() {
    let mut c = canvas();
    let r = Rect { x: 100.0, y: 20.0, w: 80.0, h: 40.0 };
    draw_text_fit(&mut c, r, &["Kitchen"], &style()).unwrap();
    let l = finish(c, size(), 128);
    for y in 0..l.h {
        for x in 0..l.w {
            if l.mono[(y * l.w + x) as usize] {
                assert!((99..=181).contains(&x) && (19..=61).contains(&y), "pixel outside rect at {x},{y}");
            }
        }
    }
}

#[test]
fn preview_png_is_valid() {
    let png = finish(canvas(), size(), 128).preview_png(2);
    let img = image::load_from_memory(&png).unwrap();
    assert_eq!((img.width(), img.height()), (40 * 8 * 2, 14 * 8 * 2));
}
