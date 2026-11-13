use nelko_core::config::{Config, Rotation};
use nelko_core::lock::PrinterLock;
use nelko_core::printer::Printer;
use nelko_core::protocol::*;
use nelko_core::transport::{MockTransport, RfcommTransport};
use nelko_core::PrinterError;
use std::time::{Duration, Instant};

fn job() -> PrintJob {
    PrintJob {
        size: "14x40".parse().unwrap(),
        gap_mm: 5,
        density: 15,
        copies: 1,
        bitmap: PrintBitmap { width_bytes: 12, height: 284, y_offset: 18, data: vec![0xFF; 12 * 284] },
    }
}

#[tokio::test]
async fn print_sends_exact_job_bytes() {
    let mut p = Printer::new(MockTransport::with_replies(vec![vec![0x00], vec![0x00]]));
    p.print(&job()).await.unwrap();
    let mut want = CMD_STATUS.to_vec();
    want.extend(encode_job(&job()));
    want.extend(CMD_STATUS);
    assert_eq!(p.into_inner().written, want);
}

#[tokio::test]
async fn print_waits_while_busy() {
    let mut p = Printer::new(MockTransport::with_replies(vec![vec![0x00], vec![0x20], vec![0x20], vec![0x00]]));
    p.print(&job()).await.unwrap();
    assert!(p.into_inner().replies.is_empty());
}

#[tokio::test]
async fn print_refuses_when_lid_open() {
    let mut p = Printer::new(MockTransport::with_replies(vec![vec![0x01]]));
    let err = p.print(&job()).await.unwrap_err();
    assert!(matches!(err, PrinterError::Status(PrinterStatus::LidOpen)));
    assert_eq!(p.into_inner().written, CMD_STATUS.to_vec());
}

#[tokio::test]
async fn roll_parses_and_none_for_blank_tag() {
    let real = vec![0x00, 0x0c, 0x01, 0x12, 0x03, 0x00, 0x03, 0x01, 0x12, 0x12, 0x15, 0x28, 0x0f, 0x0e, 0xed, 0x03];
    let mut blank = vec![0x04, 0x0c];
    blank.extend([0u8; 12]);
    let crc = crc16_modbus(&blank);
    blank.extend(crc.to_be_bytes());
    let mut p = Printer::new(MockTransport::with_replies(vec![real, blank]));
    assert_eq!(p.roll().await.unwrap().unwrap().size.to_string(), "14x40");
    assert!(p.roll().await.unwrap().is_none());
    let t = p.into_inner();
    assert_eq!(t.written, [CMD_LABEL_INFO, CMD_LABEL_INFO].concat());
}

#[tokio::test]
async fn battery_and_config_queries() {
    let mut p = Printer::new(MockTransport::with_replies(vec![
        b"BATTERY \x99\x00\r\n".to_vec(),
        b"CONFIG \x00\xcb\x00\x02\x02\x04\x02\x0e\x02\x01\r\n".to_vec(),
    ]));
    assert_eq!(p.battery().await.unwrap().percent, 99);
    assert_eq!(p.config().await.unwrap().fw, [4, 2, 14]);
}

#[tokio::test]
async fn silent_printer_times_out() {
    let mut p = Printer::new(MockTransport::with_replies(vec![]));
    assert!(matches!(p.status().await, Err(PrinterError::Timeout(_))));
}

#[tokio::test]
#[ignore = "needs a Bluetooth adapter"]
async fn connect_failure_maps_to_unreachable() {
    let t0 = Instant::now();
    let r = RfcommTransport::connect("00:00:00:00:00:01", Duration::from_secs(2)).await;
    assert!(matches!(r, Err(PrinterError::Unreachable(_))));
    assert!(t0.elapsed() < Duration::from_secs(4));
}

#[test]
fn lock_second_acquire_times_out() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p21.lock");
    let first = PrinterLock::acquire_at(&path, Duration::from_millis(300)).unwrap();
    let second = PrinterLock::acquire_at(&path, Duration::from_millis(300));
    assert!(matches!(second, Err(PrinterError::Busy)));
    drop(first);
    assert!(PrinterLock::acquire_at(&path, Duration::from_millis(300)).is_ok());
}

#[test]
fn config_roundtrip_and_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let d = Config::load_from(&path).unwrap();
    assert_eq!(d.density, 15);
    assert_eq!(d.rotation, Rotation::Cw);
    let c = Config {
        printer_mac: Some("83:9D:F1:BB:5A:73".into()),
        printer_name: Some("P21".into()),
        density: 12,
        rotation: Rotation::Ccw,
        last_label: Some("14x40".parse().unwrap()),
        manual_label: None,
    };
    c.save_to(&path).unwrap();
    assert_eq!(Config::load_from(&path).unwrap(), c);
}

#[test]
fn not_connected_errors_are_retryable() {
    use nelko_core::transport::is_still_connecting;
    assert!(is_still_connecting(&std::io::Error::from_raw_os_error(107)));
    assert!(!is_still_connecting(&std::io::Error::from_raw_os_error(104)));
}
