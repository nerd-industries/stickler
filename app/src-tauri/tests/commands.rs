use async_trait::async_trait;
use base64::Engine;
use nelko_app::commands::*;
use nelko_core::config::Config;
use nelko_core::service::{Connector, Nelko};
use nelko_core::template::{FieldValues, TemplateStore};
use nelko_core::transport::Transport;
use nelko_core::PrinterError;

struct Offline;

#[async_trait]
impl Connector for Offline {
    async fn open(&self, _mac: &str) -> Result<Box<dyn Transport>, PrinterError> {
        Err(PrinterError::Unreachable("Printer is off, out of range, or connected to another device".into()))
    }
}

fn nelko(cfg: Config) -> (Nelko, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = TemplateStore::load(dir.path().to_path_buf()).unwrap();
    (Nelko::with(cfg, store, Box::new(Offline)), dir)
}

#[tokio::test]
async fn preview_returns_png_and_default_size() {
    let (n, _d) = nelko(Config::default());
    let p = preview_impl(&n, "kitchen-jar".into(), FieldValues::new(), None, None).await.unwrap();
    let png = base64::engine::general_purpose::STANDARD.decode(p.png_base64).unwrap();
    assert_eq!(&png[..4], b"\x89PNG");
    assert_eq!((p.size.as_str(), p.source.as_str()), ("14x40", "default"));
}

#[tokio::test]
async fn preview_uses_explicit_size() {
    let (n, _d) = nelko(Config::default());
    let p = preview_impl(&n, "plain-text".into(), FieldValues::new(), None, Some("15x22".into())).await.unwrap();
    assert_eq!((p.size.as_str(), p.source.as_str()), ("15x22", "explicit"));
}

#[tokio::test]
async fn preview_bad_size_is_error_message() {
    let (n, _d) = nelko(Config::default());
    let e = preview_impl(&n, "plain-text".into(), FieldValues::new(), None, Some("huge".into())).await.unwrap_err();
    assert!(e.message.contains("label size"));
}

#[tokio::test]
async fn print_error_is_plain_message() {
    let (n, _d) = nelko(Config { printer_mac: Some("83:9D:F1:BB:5A:73".into()), ..Config::default() });
    let req = PrintRequestDto { template_id: "plain-text".into(), values: FieldValues::new(), style: None, size: None, copies: 1, density: None };
    let e = print_impl(&n, req).await.unwrap_err();
    assert!(e.message.contains("out of range"));
}
