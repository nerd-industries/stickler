use async_trait::async_trait;
use nelko_core::config::Config;
use nelko_core::protocol::crc16_modbus;
use nelko_core::service::*;
use nelko_core::template::{FieldValues, TemplateStore};
use nelko_core::transport::{MockTransport, Transport};
use nelko_core::{NelkoError, PrinterError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Transport whose state the test can inspect after the service drops it.
struct Shared(Arc<Mutex<MockTransport>>);

#[async_trait]
impl Transport for Shared {
    async fn write_all(&mut self, b: &[u8]) -> Result<(), PrinterError> {
        self.0.lock().unwrap().written.extend_from_slice(b);
        Ok(())
    }
    async fn read_reply(&mut self, _i: Duration, _m: Duration) -> Result<Vec<u8>, PrinterError> {
        self.0.lock().unwrap().replies.pop_front().ok_or(PrinterError::Timeout("no reply"))
    }
}

struct TestConnector {
    t: Option<Arc<Mutex<MockTransport>>>,
}

#[async_trait]
impl Connector for TestConnector {
    async fn open(&self, _mac: &str) -> Result<Box<dyn Transport>, PrinterError> {
        match &self.t {
            Some(t) => Ok(Box::new(Shared(t.clone()))),
            None => Err(PrinterError::Unreachable("Printer is off, out of range, or connected to another device".into())),
        }
    }
}

fn tag(width: u8, length: u8) -> Vec<u8> {
    let mut b = vec![0x00, 0x0c, 0x01, 0x12, 0x03, 0x00, 0x03, 0x01, 0x12, 0x12, 0x15, length, 0x0f, width];
    let crc = crc16_modbus(&b);
    b.extend(crc.to_be_bytes());
    b
}

fn blank_tag() -> Vec<u8> {
    let mut b = vec![0x00, 0x0c];
    b.extend([0u8; 12]);
    let crc = crc16_modbus(&b);
    b.extend(crc.to_be_bytes());
    b
}

fn service(replies: Option<Vec<Vec<u8>>>, cfg: Config) -> (Nelko, Option<Arc<Mutex<MockTransport>>>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = TemplateStore::load(dir.path().join("templates")).unwrap();
    let t = replies.map(|r| Arc::new(Mutex::new(MockTransport::with_replies(r))));
    let n = Nelko::with(cfg, store, Box::new(TestConnector { t: t.clone() }));
    (n, t, dir)
}

fn paired() -> Config {
    Config { printer_mac: Some("83:9D:F1:BB:5A:73".into()), ..Config::default() }
}

fn req(size: Option<&str>) -> PrintRequest {
    let mut values = FieldValues::new();
    values.insert("text".into(), "Hello".into());
    PrintRequest { template_id: "plain-text".into(), values, style: None, size: size.map(|s| s.parse().unwrap()), copies: 1, density: None }
}

fn written(t: &Option<Arc<Mutex<MockTransport>>>) -> String {
    String::from_utf8_lossy(&t.as_ref().unwrap().lock().unwrap().written).into_owned()
}

#[tokio::test]
async fn print_uses_roll_size_over_explicit() {
    let (n, t, _d) = service(Some(vec![tag(14, 30), vec![0], vec![0]]), paired());
    let r = n.print(req(Some("14x40"))).await.unwrap();
    assert_eq!(r.source, SizeSource::Roll);
    assert_eq!(r.size.to_string(), "14x30");
    let w = written(&t);
    assert!(w.contains("SIZE 14 mm,30 mm\r\nGAP 5 mm,0 mm"));
    assert!(w.contains("DENSITY 15"));
    assert_eq!(n.config().last_label.unwrap().to_string(), "14x30");
}

#[tokio::test]
async fn print_without_any_size_errors() {
    let (n, _t, _d) = service(Some(vec![blank_tag()]), paired());
    assert!(matches!(n.print(req(None)).await, Err(NelkoError::UnknownLabelSize)));
}

#[tokio::test]
async fn print_falls_back_to_explicit_then_last_known() {
    let (n, _t, _d) = service(Some(vec![blank_tag(), vec![0], vec![0]]), paired());
    assert_eq!(n.print(req(Some("14x50"))).await.unwrap().source, SizeSource::Explicit);
    let cfg = Config { last_label: Some("14x50".parse().unwrap()), ..paired() };
    let (n, _t, _d) = service(Some(vec![blank_tag(), vec![0], vec![0]]), cfg);
    assert_eq!(n.print(req(None)).await.unwrap().source, SizeSource::LastKnown);
}

#[tokio::test]
async fn print_needs_a_paired_printer() {
    let (n, _t, _d) = service(Some(vec![]), Config::default());
    assert!(matches!(n.print(req(None)).await, Err(NelkoError::Printer(PrinterError::NotConfigured))));
}

#[tokio::test]
async fn density_override_is_clamped() {
    let (n, t, _d) = service(Some(vec![tag(14, 40), vec![0], vec![0]]), paired());
    n.print(PrintRequest { density: Some(40), ..req(None) }).await.unwrap();
    assert!(written(&t).contains("DENSITY 15\r\n"));
}

#[tokio::test]
async fn report_never_errors_when_unreachable() {
    let (n, _t, _d) = service(None, paired());
    let r = n.report().await;
    assert!(!r.connected);
    assert!(r.error.unwrap().contains("out of range"));
}

#[tokio::test]
async fn report_reads_everything() {
    let replies = vec![
        vec![0x00],
        b"BATTERY \x87\x00\r\n".to_vec(),
        b"CONFIG \x00\xcb\x00\x02\x02\x04\x02\x0e\x02\x01\r\n".to_vec(),
        tag(14, 40),
    ];
    let (n, _t, _d) = service(Some(replies), paired());
    let r = n.report().await;
    assert!(r.connected, "{:?}", r.error);
    assert_eq!(r.battery.unwrap().percent, 87);
    assert_eq!(r.firmware.as_deref(), Some("4.2.14"));
    assert_eq!(r.roll.unwrap().size.to_string(), "14x40");
}

#[tokio::test]
async fn preview_returns_png() {
    let (n, _t, _d) = service(None, paired());
    let png = n.preview("kitchen-jar", &FieldValues::new(), None, "14x40".parse().unwrap()).unwrap();
    assert_eq!(&png[..4], b"\x89PNG");
}
