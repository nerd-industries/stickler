use nelko_core::geometry::{LabelSize, PrintArea, print_area};
use nelko_core::protocol::*;

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// Build a 16-byte label block from 14 bytes, appending a valid BE CRC.
fn block(first14: &[u8]) -> Vec<u8> {
    let mut b = first14.to_vec();
    let crc = crc16_modbus(&b);
    b.extend(crc.to_be_bytes());
    b
}

#[test]
fn crc_matches_example() {
    let b = hex("000c011203000301121215280f0e");
    assert_eq!(crc16_modbus(&b), 0xED03);
}

#[test]
fn parses_p21_example_tag() {
    let r = parse_label_block(&hex("000c011203000301121215280f0eed03"))
        .unwrap()
        .unwrap();
    assert_eq!(r.size, LabelSize { width_mm: 14, length_mm: 40 });
    assert_eq!(r.gap_mm, None);
    assert_eq!(r.paper_type, PaperType::Gapped);
    assert_eq!(r.color, PaperColor::White);
    assert_eq!(r.encoding_version, 1);
    assert_eq!(r.status, PrinterStatus::Ready);
    assert_eq!(r.paper_code, "011203000301121215280f0e");
}

#[test]
fn v2_tag_reads_gap_from_byte8() {
    let b = block(&[0x00, 0x0c, 0x01, 0x21, 0x03, 0x00, 0x03, 0x01, 0x03, 0x12, 0x15, 0x28, 0x0f, 0x0e]);
    let r = parse_label_block(&b).unwrap().unwrap();
    assert_eq!(r.gap_mm, Some(3));
    assert_eq!(r.paper_type, PaperType::Gapped);
    assert_eq!(r.encoding_version, 2);
    assert_eq!(r.size, LabelSize { width_mm: 14, length_mm: 40 });
}

#[test]
fn legacy_02_tag_uses_byte12_width() {
    let b = block(&[0x00, 0x0c, 0x01, 0x02, 0x03, 0x00, 0x03, 0x01, 0x00, 0x12, 0x15, 0x28, 0x0f, 0x00]);
    let r = parse_label_block(&b).unwrap().unwrap();
    assert_eq!(r.size, LabelSize { width_mm: 15, length_mm: 40 });
    assert_eq!(r.gap_mm, None);
}

#[test]
fn blank_tag_is_none() {
    let mut first = vec![0x04, 0x0c];
    first.extend([0u8; 12]);
    assert!(parse_label_block(&block(&first)).unwrap().is_none());
}

#[test]
fn bad_crc_is_error() {
    let mut b = hex("000c011203000301121215280f0eed03");
    b[15] ^= 0xff;
    assert!(matches!(
        parse_label_block(&b),
        Err(nelko_core::PrinterError::BadReply(_))
    ));
}

#[test]
fn battery_bcd() {
    assert_eq!(
        parse_battery(b"BATTERY\x99\x01\r\n").unwrap(),
        Battery { percent: 99, charging: true }
    );
    assert_eq!(parse_battery(b"BATTERY\x45\x00\r\n").unwrap().percent, 45);
}

#[test]
fn config_parses() {
    let c = parse_config(b"CONFIG\x00\xcb\x00\x00\x03\x04\x02\x04\x01\x01\r\n").unwrap();
    assert_eq!((c.dpi, c.hw, c.fw, c.shutdown, c.beep), (203, [0, 0, 3], [4, 2, 4], 1, true));
}

#[test]
fn status_values() {
    assert_eq!(parse_status(0x00), PrinterStatus::Ready);
    assert_eq!(parse_status(0x01), PrinterStatus::LidOpen);
    assert_eq!(parse_status(0x24), PrinterStatus::OutOfPaper);
    assert_eq!(parse_status(0x77), PrinterStatus::Unknown(0x77));
}

#[test]
fn print_area_values() {
    assert_eq!(
        print_area("14x40".parse().unwrap()),
        PrintArea { width_dots: 96, length_dots: 284, offset_dots: 18 }
    );
    assert_eq!(
        print_area("14x30".parse().unwrap()),
        PrintArea { width_dots: 96, length_dots: 200, offset_dots: 20 }
    );
    assert_eq!(
        print_area("14x50".parse().unwrap()),
        PrintArea { width_dots: 96, length_dots: 384, offset_dots: 8 }
    );
}

#[test]
fn label_size_parse_and_display() {
    let s: LabelSize = "15x22".parse().unwrap();
    assert_eq!(s, LabelSize { width_mm: 15, length_mm: 22 });
    assert_eq!(s.to_string(), "15x22");
    assert!("banana".parse::<LabelSize>().is_err());
}

fn job(density: u8, copies: u16, data: Vec<u8>) -> PrintJob {
    PrintJob {
        size: "14x40".parse().unwrap(),
        gap_mm: 5,
        density,
        copies,
        bitmap: PrintBitmap { width_bytes: 12, height: 284, y_offset: 18, data },
    }
}

#[test]
fn encodes_full_job() {
    let data = vec![0xFFu8; 12 * 284];
    let mut want =
        b"SIZE 14 mm,40 mm\r\nGAP 5 mm,0 mm\r\nDIRECTION 1,1\r\nDENSITY 15\r\nCLS\r\nBITMAP 0,18,12,284,1,"
            .to_vec();
    want.extend(&data);
    want.extend(b"\r\nPRINT 2\r\n");
    assert_eq!(encode_job(&job(15, 2, data)), want);
}

#[test]
fn density_is_clamped() {
    let low = String::from_utf8_lossy(&encode_job(&job(0, 1, vec![]))).to_string();
    assert!(low.contains("DENSITY 1\r\n"));
    let high = String::from_utf8_lossy(&encode_job(&job(99, 1, vec![]))).to_string();
    assert!(high.contains("DENSITY 15\r\n"));
}

// Real replies captured from the user's P21 (fw 4.2.14), spec §9.1.
#[test]
fn real_battery_reply_with_space() {
    assert_eq!(
        parse_battery(&hex("424154544552592099000d0a")).unwrap(),
        Battery { percent: 99, charging: false }
    );
}

#[test]
fn real_config_reply_with_space() {
    let c = parse_config(&hex("434f4e4649472000cb00020204020e02010d0a")).unwrap();
    assert_eq!((c.dpi, c.hw, c.fw, c.shutdown, c.beep), (203, [0, 2, 2], [4, 2, 14], 2, true));
}

#[test]
fn query_commands_end_with_crlf() {
    assert_eq!(CMD_STATUS, &[0x1B, 0x21, 0x3F, 0x0D, 0x0A]);
    assert_eq!(CMD_LABEL_INFO, &[0x1B, 0x21, 0x6F, 0x0D, 0x0A]);
}
