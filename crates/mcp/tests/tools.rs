use async_trait::async_trait;
use nelko_core::config::Config;
use nelko_core::protocol::crc16_modbus;
use nelko_core::service::{Connector, Nelko};
use nelko_core::template::TemplateStore;
use nelko_core::transport::{MockTransport, Transport};
use nelko_core::PrinterError;
use nelko_mcp::*;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use std::sync::Arc;

struct OneShot(std::sync::Mutex<Option<MockTransport>>);

#[async_trait]
impl Connector for OneShot {
    async fn open(&self, _mac: &str) -> Result<Box<dyn Transport>, PrinterError> {
        self.0
            .lock()
            .unwrap()
            .take()
            .map(|t| Box::new(t) as Box<dyn Transport>)
            .ok_or(PrinterError::Unreachable("Printer is off, out of range, or connected to another device".into()))
    }
}

fn tag_14x40() -> Vec<u8> {
    let mut b = vec![0x00, 0x0c, 0x01, 0x12, 0x03, 0x00, 0x03, 0x01, 0x12, 0x12, 0x15, 0x28, 0x0f, 0x0e];
    let crc = crc16_modbus(&b);
    b.extend(crc.to_be_bytes());
    b
}

fn server(replies: Option<Vec<Vec<u8>>>) -> (NelkoServer, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = TemplateStore::load(dir.path().to_path_buf()).unwrap();
    let cfg = Config { printer_mac: Some("83:9D:F1:BB:5A:73".into()), ..Config::default() };
    let conn = OneShot(std::sync::Mutex::new(replies.map(MockTransport::with_replies)));
    (NelkoServer::new(Arc::new(Nelko::with(cfg, store, Box::new(conn)))), dir)
}

fn text(r: &CallToolResult) -> String {
    r.content.iter().filter_map(|c| c.as_text()).map(|t| t.text.clone()).collect::<Vec<_>>().join("\n")
}

fn is_error(r: &CallToolResult) -> bool {
    r.is_error == Some(true)
}

#[tokio::test]
async fn list_templates_includes_builtins() {
    let (s, _d) = server(None);
    let r = s.list_templates().await.unwrap();
    assert!(text(&r).contains("\"kitchen-jar\""));
    assert!(text(&r).contains("\"fields\""));
}

#[tokio::test]
async fn preview_returns_image() {
    let (s, _d) = server(None);
    let p = PreviewParams { template: "plain-text".into(), fields: None, style: None, label_size: Some("14x40".into()) };
    let r = s.preview_label(Parameters(p)).await.unwrap();
    assert!(!is_error(&r), "{}", text(&r));
    assert!(r.content.iter().any(|c| c.as_image().is_some_and(|i| i.mime_type == "image/png")));
    assert!(text(&r).contains("14x40"));
}

#[tokio::test]
async fn print_rejects_too_many_copies() {
    let (s, _d) = server(None);
    let p = PrintParams { template: "plain-text".into(), fields: None, style: None, label_size: None, copies: Some(11), density: None };
    let r = s.print_label(Parameters(p)).await.unwrap();
    assert!(is_error(&r));
    assert!(text(&r).contains("At most 10 copies"));
}

#[tokio::test]
async fn print_text_prints_and_reports_size_source() {
    let (s, _d) = server(Some(vec![tag_14x40(), vec![0], vec![0]]));
    let p = PrintTextParams { text: "Hello".into(), label_size: None, copies: None, bold: None };
    let r = s.print_text(Parameters(p)).await.unwrap();
    assert!(!is_error(&r), "{}", text(&r));
    assert!(text(&r).contains("Printed 1 × Plain text on 14×40 mm (size read from the loaded roll)"));
}

#[tokio::test]
async fn unreachable_printer_is_tool_error() {
    let (s, _d) = server(None);
    let p = PrintTextParams { text: "Hello".into(), label_size: None, copies: None, bold: None };
    let r = s.print_text(Parameters(p)).await.unwrap();
    assert!(is_error(&r));
    assert!(text(&r).contains("out of range"));
}

#[tokio::test]
async fn unknown_template_is_tool_error() {
    let (s, _d) = server(None);
    let p = PreviewParams { template: "nope".into(), fields: None, style: None, label_size: None };
    let r = s.preview_label(Parameters(p)).await.unwrap();
    assert!(is_error(&r));
}
