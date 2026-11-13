//! Label sizes and the printable area on the P21 (203 dpi, 96-dot head).

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

pub const DOTS_PER_MM: u32 = 8;
pub const HEAD_DOTS: u32 = 96;

/// A label: `width_mm` across the tape, `length_mm` along the feed.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct LabelSize {
    pub width_mm: u8,
    pub length_mm: u8,
}

impl LabelSize {
    pub const fn new(width_mm: u8, length_mm: u8) -> Self {
        Self { width_mm, length_mm }
    }
}

impl fmt::Display for LabelSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width_mm, self.length_mm)
    }
}

impl FromStr for LabelSize {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || format!("'{s}' isn't a label size; use WIDTHxLENGTH in mm, like 14x40");
        let lower = s.trim().to_lowercase();
        let (w, l) = lower.split_once(['x', '×']).ok_or_else(bad)?;
        let num = |p: &str| p.trim().trim_end_matches("mm").trim().parse::<u8>().map_err(|_| bad());
        let (w, l) = (num(w)?, num(l)?);
        if w == 0 || !(10..=75).contains(&l) {
            return Err(format!("{w}x{l} is out of range: length must be 10–75 mm"));
        }
        Ok(LabelSize::new(w, l))
    }
}

/// The printed rectangle, in dots. `offset_dots` is the gap from the label start.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PrintArea {
    pub width_dots: u32,
    pub length_dots: u32,
    pub offset_dots: u32,
}

/// Printable area per spec §3.4 (matches the official app).
pub fn print_area(size: LabelSize) -> PrintArea {
    let len = size.length_mm as f32;
    let printable = match size.length_mm {
        40 => 35.5,
        30 => 25.0,
        _ => len - 2.0,
    };
    let offset = (len - printable) / 2.0;
    PrintArea {
        width_dots: (size.width_mm.min(12) as u32) * DOTS_PER_MM,
        length_dots: (printable * DOTS_PER_MM as f32).round() as u32,
        offset_dots: (offset * DOTS_PER_MM as f32).round() as u32,
    }
}

/// Known P21 roll sizes, most common first.
pub const PRESETS: &[LabelSize] = &[
    LabelSize::new(14, 40),
    LabelSize::new(14, 30),
    LabelSize::new(14, 50),
    LabelSize::new(14, 75),
    LabelSize::new(12, 40),
    LabelSize::new(15, 22),
    LabelSize::new(15, 30),
    LabelSize::new(15, 40),
    LabelSize::new(15, 50),
    LabelSize::new(15, 75),
];
