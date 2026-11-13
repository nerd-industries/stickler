//! Templates: a fixed layout with named fields and style options.

use crate::error::{NelkoError, RenderError};
use crate::geometry::{DOTS_PER_MM, LabelSize, print_area};
use crate::render::elements::{BarcodeKind, Border, Dither, draw_barcode, draw_border, draw_icon, draw_image, draw_qr};
use crate::render::text::{TextStyle, draw_text_fit};
use crate::render::{Align, Canvas, FontChoice, Rect, RenderedLabel, finish};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/templates.rs"));
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    Text,
    IconText,
    QrText,
    Barcode,
    DateStamp,
    Image,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Text,
    Multiline,
    Date,
    QrData,
    BarcodeData,
    ImagePath,
    Icon,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct FieldDef {
    pub key: String,
    pub label: String,
    pub kind: FieldKind,
    #[serde(default)]
    pub default: String,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Style {
    pub font: FontChoice,
    pub bold: bool,
    pub align: Align,
    pub border: Border,
    pub invert: bool,
    pub barcode: BarcodeKind,
    pub date_format: String,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            font: FontChoice::Sans,
            bold: true,
            align: Align::Center,
            border: Border::None,
            invert: false,
            barcode: BarcodeKind::Auto,
            date_format: "%b %-d, %Y".into(),
        }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Template {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub category: String,
    pub layout: Layout,
    pub fields: Vec<FieldDef>,
    #[serde(default)]
    pub style: Style,
    #[serde(default)]
    pub builtin: bool,
}

pub type FieldValues = BTreeMap<String, String>;

/// A field's value: the given one, else the template default. Trimmed.
fn value<'a>(f: &'a FieldDef, values: &'a FieldValues) -> &'a str {
    values.get(&f.key).map(String::as_str).unwrap_or(&f.default).trim()
}

fn first<'a>(t: &'a Template, values: &'a FieldValues, kinds: &[FieldKind]) -> &'a str {
    t.fields.iter().find(|f| kinds.contains(&f.kind)).map(|f| value(f, values)).unwrap_or("")
}

/// Text lines from all text fields (split on newlines), empty ones dropped, max 3.
fn text_lines<'a>(t: &'a Template, values: &'a FieldValues) -> Vec<&'a str> {
    t.fields
        .iter()
        .filter(|f| matches!(f.kind, FieldKind::Text | FieldKind::Multiline))
        .flat_map(|f| value(f, values).lines())
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(3)
        .collect()
}

fn format_date(v: &str, fmt: &str) -> String {
    if v.is_empty() || v.eq_ignore_ascii_case("today") {
        return chrono::Local::now().format(fmt).to_string();
    }
    match chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d") {
        Ok(d) => d.format(fmt).to_string(),
        Err(_) => v.to_owned(),
    }
}

const GAP: f32 = DOTS_PER_MM as f32; // 1 mm between side-by-side elements

pub fn render_template(t: &Template, values: &FieldValues, size: LabelSize) -> Result<RenderedLabel, NelkoError> {
    let mut c = Canvas::new(&print_area(size));
    let inner = draw_border(&mut c, t.style.border).inset(2.0);
    let ts = TextStyle { font: t.style.font, bold: t.style.bold, align: t.style.align, ..Default::default() };
    let right_of = |used: Rect| Rect { x: used.x + used.w + GAP, y: inner.y, w: (inner.x + inner.w - used.x - used.w - GAP).max(0.0), h: inner.h };

    match t.layout {
        Layout::Text => {
            draw_text_fit(&mut c, inner, &text_lines(t, values), &ts)?;
        }
        Layout::IconText => {
            let icon = first(t, values, &[FieldKind::Icon]);
            let lines = text_lines(t, values);
            if icon.is_empty() {
                draw_text_fit(&mut c, inner, &lines, &ts)?;
            } else {
                let side = inner.h.min(inner.w);
                let used = draw_icon(&mut c, Rect { x: inner.x, y: inner.y, w: side, h: inner.h }, icon)?;
                draw_text_fit(&mut c, right_of(used), &lines, &ts)?;
            }
        }
        Layout::QrText => {
            let data = first(t, values, &[FieldKind::QrData]);
            let lines = text_lines(t, values);
            if data.is_empty() && !lines.is_empty() {
                // An empty field is left out: text only.
                draw_text_fit(&mut c, inner, &lines, &ts)?;
            } else if lines.is_empty() {
                let side = inner.h.min(inner.w);
                draw_qr(&mut c, Rect { x: inner.x + (inner.w - side) / 2.0, y: inner.y, w: side, h: inner.h }, data)?;
            } else {
                let used = draw_qr(&mut c, inner, data)?;
                draw_text_fit(&mut c, right_of(used), &lines, &ts)?;
            }
        }
        Layout::Barcode => {
            let data = first(t, values, &[FieldKind::BarcodeData]);
            let lines = text_lines(t, values);
            if data.is_empty() && !lines.is_empty() {
                draw_text_fit(&mut c, inner, &lines, &ts)?;
            } else if lines.is_empty() {
                draw_barcode(&mut c, inner, data, t.style.barcode)?;
            } else {
                let bar_h = (inner.h * 0.72).floor();
                draw_barcode(&mut c, Rect { h: bar_h, ..inner }, data, t.style.barcode)?;
                let cap = Rect { y: inner.y + bar_h + 1.0, h: inner.h - bar_h - 1.0, ..inner };
                draw_text_fit(&mut c, cap, &lines[..1], &TextStyle { align: Align::Center, ..ts })?;
            }
        }
        Layout::DateStamp => {
            let label = first(t, values, &[FieldKind::Text, FieldKind::Multiline]);
            let date = format_date(first(t, values, &[FieldKind::Date]), &t.style.date_format);
            if label.is_empty() {
                draw_text_fit(&mut c, inner, &[&date], &ts)?;
            } else {
                let top_h = (inner.h * 0.4).floor();
                draw_text_fit(&mut c, Rect { h: top_h, ..inner }, &[label], &TextStyle { bold: false, ..ts.clone() })?;
                let bottom = Rect { y: inner.y + top_h, h: inner.h - top_h, ..inner };
                draw_text_fit(&mut c, bottom, &[&date], &TextStyle { bold: true, ..ts })?;
            }
        }
        Layout::Image => {
            let path = first(t, values, &[FieldKind::ImagePath]);
            if path.is_empty() {
                return Err(RenderError::InvalidData("Pick an image to print.".into()).into());
            }
            draw_image(&mut c, inner, Path::new(path), Dither::FloydSteinberg)?;
        }
    }

    let mut label = finish(c, size, 128);
    if t.style.invert {
        label.invert();
    }
    Ok(label)
}

/// Lowercase ASCII words joined by '-'.
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() { "template".into() } else { out }
}

pub struct TemplateStore {
    builtins: Vec<Template>,
    user_dir: PathBuf,
    user: Vec<Template>,
}

fn sort(v: &mut [Template]) {
    v.sort_by(|a, b| (&a.category, &a.name).cmp(&(&b.category, &b.name)));
}

impl TemplateStore {
    pub fn load(user_dir: PathBuf) -> Result<Self, NelkoError> {
        let mut builtins: Vec<Template> = embedded::TEMPLATES
            .iter()
            .map(|(id, bytes)| {
                let mut t: Template = serde_json::from_slice(bytes).expect("built-in template JSON is valid");
                t.id = (*id).to_owned();
                t.builtin = true;
                t
            })
            .collect();
        sort(&mut builtins);
        let mut user = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&user_dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "json") {
                    // Skip unreadable files rather than refusing to start.
                    if let Ok(mut t) = std::fs::read(&p).map_err(|_| ()).and_then(|b| serde_json::from_slice::<Template>(&b).map_err(|_| ())) {
                        t.id = p.file_stem().unwrap().to_string_lossy().into_owned();
                        t.builtin = false;
                        user.push(t);
                    }
                }
            }
        }
        sort(&mut user);
        Ok(Self { builtins, user_dir, user })
    }

    pub fn list(&self) -> Vec<&Template> {
        self.builtins.iter().chain(&self.user).collect()
    }

    pub fn get(&self, id: &str) -> Option<&Template> {
        self.list().into_iter().find(|t| t.id == id)
    }

    /// Save (or overwrite) a user template. Ids never collide with built-ins.
    pub fn save_user(&mut self, mut t: Template) -> Result<Template, NelkoError> {
        let base = slugify(if t.id.trim().is_empty() { &t.name } else { &t.id });
        let mut id = base.clone();
        let mut n = 2;
        while self.builtins.iter().any(|b| b.id == id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        t.id = id;
        t.builtin = false;
        std::fs::create_dir_all(&self.user_dir).map_err(|e| NelkoError::Template(e.to_string()))?;
        let json = serde_json::to_vec_pretty(&t).map_err(|e| NelkoError::Template(e.to_string()))?;
        std::fs::write(self.user_dir.join(format!("{}.json", t.id)), json).map_err(|e| NelkoError::Template(e.to_string()))?;
        self.user.retain(|u| u.id != t.id);
        self.user.push(t.clone());
        sort(&mut self.user);
        Ok(t)
    }

    pub fn delete_user(&mut self, id: &str) -> Result<(), NelkoError> {
        if self.builtins.iter().any(|b| b.id == id) {
            return Err(NelkoError::Template("Built-in templates can't be deleted.".into()));
        }
        if !self.user.iter().any(|u| u.id == id) {
            return Err(NelkoError::Template(format!("No template called '{id}'.")));
        }
        let _ = std::fs::remove_file(self.user_dir.join(format!("{id}.json")));
        self.user.retain(|u| u.id != id);
        Ok(())
    }

    pub fn duplicate(&mut self, id: &str, new_name: &str) -> Result<Template, NelkoError> {
        let mut t = self.get(id).cloned().ok_or_else(|| NelkoError::Template(format!("No template called '{id}'.")))?;
        t.id = String::new();
        t.name = new_name.to_owned();
        self.save_user(t)
    }
}
