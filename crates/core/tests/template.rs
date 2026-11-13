use nelko_core::geometry::{LabelSize, PRESETS};
use nelko_core::template::*;
use std::path::{Path, PathBuf};

fn fixture() -> String {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gradient.png").display().to_string()
}

fn store() -> (tempfile::TempDir, TemplateStore) {
    let dir = tempfile::tempdir().unwrap();
    let s = TemplateStore::load(dir.path().to_path_buf()).unwrap();
    (dir, s)
}

fn defaults_for(t: &Template) -> FieldValues {
    let mut v = FieldValues::new();
    if t.layout == Layout::Image {
        for f in &t.fields {
            if f.kind == FieldKind::ImagePath {
                v.insert(f.key.clone(), fixture());
            }
        }
    }
    v
}

#[test]
fn has_expected_builtins() {
    let (_d, s) = store();
    let ids: Vec<&str> = s.list().iter().map(|t| t.id.as_str()).collect();
    for want in [
        "plain-text", "bold-title", "two-line", "boxed-text", "kitchen-jar", "spice", "cable-tag", "storage-bin",
        "toolbox", "medicine", "wifi-qr", "link-qr", "asset-tag", "barcode-plain", "leftovers", "opened-on", "image",
    ] {
        assert!(ids.contains(&want), "missing built-in {want}");
    }
    assert!(s.list().iter().all(|t| t.builtin));
}

#[test]
fn all_builtins_render_on_all_presets() {
    let (_d, s) = store();
    for t in s.list() {
        for &size in PRESETS {
            let l = render_template(t, &defaults_for(t), size)
                .unwrap_or_else(|e| panic!("{} at {size}: {e}", t.id));
            assert!(l.mono.iter().any(|&b| b), "{} at {size} is blank", t.id);
        }
    }
}

#[test]
fn snapshots_match() {
    let (_d, s) = store();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");
    let update = std::env::var("UPDATE_SNAPSHOTS").is_ok();
    for t in s.list() {
        if t.layout == Layout::DateStamp || t.layout == Layout::Image {
            continue; // date changes daily; image uses a fixture path
        }
        let png = render_template(t, &defaults_for(t), "14x40".parse().unwrap()).unwrap().preview_png(2);
        let path = dir.join(format!("{}.png", t.id));
        if update {
            std::fs::write(&path, &png).unwrap();
        } else {
            let want = std::fs::read(&path).unwrap_or_else(|_| panic!("no snapshot for {}; run with UPDATE_SNAPSHOTS=1", t.id));
            assert!(want == png, "snapshot mismatch for {}", t.id);
        }
    }
}

#[test]
fn date_today_substitution() {
    let (_d, s) = store();
    let t = s.get("leftovers").unwrap();
    let size: LabelSize = "14x40".parse().unwrap();
    let date_key = t.fields.iter().find(|f| f.kind == FieldKind::Date).unwrap().key.clone();
    let mut today = FieldValues::new();
    today.insert(date_key.clone(), "today".into());
    let mut explicit = FieldValues::new();
    explicit.insert(date_key, chrono::Local::now().format(&t.style.date_format).to_string());
    assert_eq!(render_template(t, &today, size).unwrap().mono, render_template(t, &explicit, size).unwrap().mono);
}

#[test]
fn missing_field_uses_default() {
    let (_d, s) = store();
    let t = s.get("kitchen-jar").unwrap();
    let size: LabelSize = "14x40".parse().unwrap();
    let mut explicit = FieldValues::new();
    for f in &t.fields {
        explicit.insert(f.key.clone(), f.default.clone());
    }
    assert_eq!(render_template(t, &FieldValues::new(), size).unwrap().mono, render_template(t, &explicit, size).unwrap().mono);
}

#[test]
fn invert_flips_pixels() {
    let (_d, s) = store();
    let mut t = s.get("plain-text").unwrap().clone();
    let size: LabelSize = "14x40".parse().unwrap();
    let normal = render_template(&t, &FieldValues::new(), size).unwrap();
    t.style.invert = true;
    let inv = render_template(&t, &FieldValues::new(), size).unwrap();
    assert!(normal.mono.iter().zip(&inv.mono).all(|(a, b)| a != b));
}

#[test]
fn store_save_slugifies_and_avoids_builtin_ids() {
    let (dir, mut s) = store();
    let mut t = s.get("plain-text").unwrap().clone();
    t.id = String::new();
    t.name = "Plain Text".into();
    let saved = s.save_user(t).unwrap();
    assert_eq!(saved.id, "plain-text-2");
    assert!(!saved.builtin);
    assert!(dir.path().join("plain-text-2.json").exists());
    assert!(s.delete_user("plain-text").is_err());
    // Reload from disk sees it.
    let s2 = TemplateStore::load(dir.path().to_path_buf()).unwrap();
    assert!(s2.get("plain-text-2").is_some());
    s.delete_user("plain-text-2").unwrap();
    assert!(s.get("plain-text-2").is_none());
}

#[test]
fn duplicate_makes_user_copy() {
    let (_d, mut s) = store();
    let t = s.duplicate("wifi-qr", "Guest Wi-Fi").unwrap();
    assert_eq!(t.id, "guest-wi-fi");
    assert_eq!(t.layout, Layout::QrText);
    assert!(s.list().iter().any(|x| x.id == "guest-wi-fi" && !x.builtin));
}

#[test]
fn image_template_without_path_errors() {
    let (_d, s) = store();
    let t = s.get("image").unwrap();
    assert!(render_template(t, &FieldValues::new(), "14x40".parse().unwrap()).is_err());
}
