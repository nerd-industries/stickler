//! P21 wire protocol: command encoding and reply parsing (spec §3, §3.4, §9.1).

use crate::error::PrinterError;
use crate::geometry::LabelSize;
use serde::{Deserialize, Serialize};

pub const CMD_STATUS: &[u8] = &[0x1B, 0x21, 0x3F, 0x0D, 0x0A];
pub const CMD_LABEL_INFO: &[u8] = &[0x1B, 0x21, 0x6F, 0x0D, 0x0A];
pub const CMD_BATTERY: &[u8] = b"BATTERY?\r\n";
pub const CMD_CONFIG: &[u8] = b"CONFIG?\r\n";
pub const CMD_SELFTEST: &[u8] = b"SELFTEST\r\n";

/// CRC-16/MODBUS (poly 0xA001 reflected, init 0xFFFF).
pub fn crc16_modbus(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in data {
        crc ^= b as u16;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xA001 } else { crc >> 1 };
        }
    }
    crc
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "code")]
pub enum PrinterStatus {
    Ready,
    LidOpen,
    Feeding,
    PaperError,
    NoPaper,
    Paused,
    Busy,
    OutOfPaper,
    LowBattery,
    Overheated,
    NoPower,
    Unknown(u8),
}

pub fn parse_status(b: u8) -> PrinterStatus {
    use PrinterStatus::*;
    match b {
        0x00 => Ready,
        0x01 => LidOpen,
        0x02 => Feeding,
        0x03 => PaperError,
        0x04 => NoPaper,
        0x10 => Paused,
        0x20 => Busy,
        0x24 => OutOfPaper,
        0x30 => LowBattery,
        0x80 => Overheated,
        0x90 => NoPower,
        other => Unknown(other),
    }
}

impl PrinterStatus {
    pub fn message(&self) -> &'static str {
        use PrinterStatus::*;
        match self {
            Ready => "Ready",
            LidOpen => "The lid is open. Close it and try again.",
            Feeding => "The printer is feeding paper.",
            PaperError => "There's a paper problem. Check the roll is seated properly.",
            NoPaper => "No labels loaded. Put a roll in.",
            Paused => "The printer is paused.",
            Busy => "The printer is busy.",
            OutOfPaper => "Out of labels. Load a new roll.",
            LowBattery => "The battery is too low. Charge the printer.",
            Overheated => "The printer is too hot. Let it cool down for a minute.",
            NoPower => "The printer has no power.",
            Unknown(_) => "The printer reported an unknown status.",
        }
    }
}

impl std::fmt::Display for PrinterStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrinterStatus::Unknown(c) => write!(f, "Unknown status {c:#04x}"),
            s => f.write_str(s.message()),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
}

/// Payload after `keyword`, skipping an optional space (real firmware sends one).
fn payload<'a>(reply: &'a [u8], keyword: &[u8], len: usize) -> Result<&'a [u8], PrinterError> {
    let bad = || PrinterError::BadReply(format!("expected {} reply, got {:02x?}", String::from_utf8_lossy(keyword), reply));
    let pos = reply.windows(keyword.len()).position(|w| w == keyword).ok_or_else(bad)?;
    let mut start = pos + keyword.len();
    // Only skip a space when the remaining bytes leave room for the payload + CRLF.
    if reply.get(start) == Some(&b' ') && reply.len() >= start + 1 + len {
        start += 1;
    }
    reply.get(start..start + len).ok_or_else(bad)
}

pub fn parse_battery(reply: &[u8]) -> Result<Battery, PrinterError> {
    let p = payload(reply, b"BATTERY", 2)?;
    let bcd = p[0];
    let percent = (bcd >> 4) * 10 + (bcd & 0x0F);
    Ok(Battery { percent: percent.min(100), charging: p[1] == 1 })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub dpi: u16,
    pub hw: [u8; 3],
    pub fw: [u8; 3],
    pub shutdown: u8,
    pub beep: bool,
}

pub fn parse_config(reply: &[u8]) -> Result<DeviceConfig, PrinterError> {
    let p = payload(reply, b"CONFIG", 10)?;
    Ok(DeviceConfig {
        dpi: u16::from_be_bytes([p[0], p[1]]),
        hw: [p[2], p[3], p[4]],
        fw: [p[5], p[6], p[7]],
        shutdown: p[8],
        beep: p[9] != 0,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaperType {
    Continuous,
    Gapped,
    BlackMark,
    Other(u8),
}

impl PaperType {
    fn from_code(c: u8) -> Self {
        match c {
            0 => Self::Continuous,
            1 => Self::Gapped,
            2 => Self::BlackMark,
            o => Self::Other(o),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaperColor {
    Transparent,
    White,
    Pink,
    Blue,
    Yellow,
    Other(u8),
}

impl PaperColor {
    fn from_code(c: u8) -> Self {
        match c {
            2 => Self::Transparent,
            3 => Self::White,
            4 => Self::Pink,
            5 => Self::Blue,
            6 => Self::Yellow,
            o => Self::Other(o),
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::Transparent => "Clear",
            Self::White => "White",
            Self::Pink => "Pink",
            Self::Blue => "Blue",
            Self::Yellow => "Yellow",
            Self::Other(_) => "Unknown colour",
        }
    }
}

/// What the printer read from the roll's NFC tag.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct RollInfo {
    pub size: LabelSize,
    pub gap_mm: Option<u8>,
    pub paper_type: PaperType,
    pub color: PaperColor,
    pub encoding_version: u8,
    pub status: PrinterStatus,
    pub paper_code: String,
}

/// Parse the 16-byte `ESC ! o` block. `Ok(None)` means no readable tag.
pub fn parse_label_block(b: &[u8]) -> Result<Option<RollInfo>, PrinterError> {
    if b.len() != 16 || b[1] != 0x0C {
        return Err(PrinterError::BadReply(format!("label block should be 16 bytes, got {:02x?}", b)));
    }
    let crc = u16::from_be_bytes([b[14], b[15]]);
    if crc16_modbus(&b[..14]) != crc {
        return Err(PrinterError::BadReply("label block checksum mismatch".into()));
    }
    if b[2..14].iter().all(|&x| x == 0) || b[11..14].iter().all(|&x| x == 0) {
        return Ok(None);
    }
    let length_mm = b[11];
    let mut paper_type = PaperType::from_code(b[7]);
    let mut gap_mm = None;
    let (width_mm, encoding_version) = if b[3] == 0x02 {
        (b[12], 0)
    } else {
        let version = b[3] >> 4;
        if version == 2 || version == 3 {
            paper_type = PaperType::from_code(b[7] & 0x0F);
            gap_mm = Some(b[8]);
        }
        (b[13], version)
    };
    let paper_code = b[2..14].iter().map(|x| format!("{x:02x}")).collect();
    Ok(Some(RollInfo {
        size: LabelSize { width_mm, length_mm },
        gap_mm,
        paper_type,
        color: PaperColor::from_code(b[4]),
        encoding_version,
        status: parse_status(b[0]),
        paper_code,
    }))
}

/// 1-bpp print image: MSB first, 0 bit = black, `width_bytes` per row.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PrintBitmap {
    pub width_bytes: u16,
    pub height: u16,
    pub y_offset: u16,
    pub data: Vec<u8>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PrintJob {
    pub size: LabelSize,
    pub gap_mm: u8,
    pub density: u8,
    pub copies: u16,
    pub bitmap: PrintBitmap,
}

pub fn encode_job(job: &PrintJob) -> Vec<u8> {
    let bm = &job.bitmap;
    let mut out = format!(
        "SIZE {} mm,{} mm\r\nGAP {} mm,0 mm\r\nDIRECTION 1,1\r\nDENSITY {}\r\nCLS\r\nBITMAP 0,{},{},{},1,",
        job.size.width_mm,
        job.size.length_mm,
        job.gap_mm,
        job.density.clamp(1, 15),
        bm.y_offset,
        bm.width_bytes,
        bm.height
    )
    .into_bytes();
    out.extend_from_slice(&bm.data);
    out.extend_from_slice(format!("\r\nPRINT {}\r\n", job.copies.max(1)).as_bytes());
    out
}
