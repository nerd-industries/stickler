use nelko_core::RenderError;
use nelko_core::geometry::{LabelSize, print_area};
use nelko_core::render::elements::*;
use nelko_core::render::{Canvas, Rect, finish};
use std::path::Path;

fn size() -> LabelSize {
    "14x40".parse().unwrap()
}
fn canvas() -> Canvas {
    Canvas::new(&print_area(size()))
}
fn any_black(c: Canvas) -> bool {
    finish(c, size(), 128).mono.iter().any(|&b| b)
}

#[test]
fn qr_decodes_back() {
    let mut c = canvas();
    let r = c.bounds();
    let used = draw_qr(&mut c, r, "https://example.com").unwrap();
    assert!((used.w - used.h).abs() < 0.01 && used.h <= 96.0);
    let l = finish(c, size(), 128);
    // Scale up 4x so the detector has comfortable module sizes.
    let s = 4u32;
    let img = image::GrayImage::from_fn(l.w * s, l.h * s, |x, y| {
        image::Luma([if l.mono[((y / s) * l.w + x / s) as usize] { 0 } else { 255 }])
    });
    let mut prep = rqrr::PreparedImage::prepare(img);
    let grids = prep.detect_grids();
    assert_eq!(grids.len(), 1);
    assert_eq!(grids[0].decode().unwrap().1, "https://example.com");
}

#[test]
fn qr_empty_is_error() {
    let mut c = canvas();
    let r = c.bounds();
    assert!(matches!(draw_qr(&mut c, r, " "), Err(RenderError::InvalidData(_))));
}

#[test]
fn ean_falls_back_to_code128() {
    assert_eq!(choose_kind("123", BarcodeKind::Auto), BarcodeKind::Code128);
    assert_eq!(choose_kind("5901234123457", BarcodeKind::Auto), BarcodeKind::Ean13);
    assert_eq!(choose_kind("5901234123458", BarcodeKind::Auto), BarcodeKind::Code128);
    assert_eq!(choose_kind("590123412345", BarcodeKind::Auto), BarcodeKind::Ean13);
}

#[test]
fn barcode_draws_on_whole_dots() {
    let mut c = canvas();
    let r = c.bounds();
    let used = draw_barcode(&mut c, r, "ASSET-0042", BarcodeKind::Auto).unwrap();
    assert_eq!(used.x.fract(), 0.0);
    assert_eq!(used.w.fract(), 0.0);
    assert!(any_black(c));
}

#[test]
fn barcode_invalid_data_errors() {
    let mut c = canvas();
    let r = c.bounds();
    assert!(matches!(draw_barcode(&mut c, r, "日本", BarcodeKind::Code128), Err(RenderError::InvalidData(_))));
    assert!(matches!(draw_barcode(&mut c, r, "", BarcodeKind::Auto), Err(RenderError::InvalidData(_))));
}

#[test]
fn icon_known_draws_black() {
    let mut c = canvas();
    draw_icon(&mut c, Rect { x: 4.0, y: 4.0, w: 88.0, h: 88.0 }, "coffee").unwrap();
    assert!(any_black(c));
    assert!(icon_names().len() >= 60);
    assert!(icon_names().contains(&"coffee"));
}

#[test]
fn icon_unknown_errors() {
    let mut c = canvas();
    let r = c.bounds();
    assert!(matches!(draw_icon(&mut c, r, "no-such-icon"), Err(RenderError::Asset(_))));
}

#[test]
fn border_inner_rect_shrinks() {
    let mut c = canvas();
    let inner = draw_border(&mut c, Border::Thin);
    assert_eq!(inner, Rect { x: 3.0, y: 3.0, w: 284.0 - 6.0, h: 96.0 - 6.0 });
    let mut c2 = canvas();
    assert_eq!(draw_border(&mut c2, Border::None), c2.bounds());
}

#[test]
fn dither_outputs_only_bw() {
    let mut c = canvas();
    let r = c.bounds();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gradient.png");
    draw_image(&mut c, r, &fixture, Dither::FloydSteinberg).unwrap();
    assert!(c.pixmap.data().chunks_exact(4).all(|p| p[0] == 0 || p[0] == 255));
    let l = finish(c, size(), 128);
    let black = l.mono.iter().filter(|&&b| b).count();
    assert!(black > 1000 && black < l.mono.len() - 1000);
}

#[test]
fn missing_image_is_asset_error() {
    let mut c = canvas();
    let r = c.bounds();
    assert!(matches!(draw_image(&mut c, r, Path::new("/nope.png"), Dither::FloydSteinberg), Err(RenderError::Asset(_))));
}
