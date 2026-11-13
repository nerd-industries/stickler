//! MCP tools for printing labels on a Nelko P21, backed by `nelko_core::service::Nelko`.

use base64::Engine;
use nelko_core::NelkoError;
use nelko_core::geometry::LabelSize;
use nelko_core::service::{Nelko, PrintRequest, ResolvedSize, SizeSource};
use nelko_core::template::{FieldValues, Style, Template};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ErrorData, ServerHandler, schemars, tool, tool_handler, tool_router};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

pub const MAX_COPIES: u16 = 10;

pub const INSTRUCTIONS: &str = "Prints to the user's Nelko P21 label maker (12 mm printable width). \
Call printer_status or preview_label before printing when unsure; labels cost money. Keep text short. \
Use list_templates to see layouts and their field keys.";

type ToolResult = Result<CallToolResult, ErrorData>;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PreviewParams {
    /// Template id from list_templates, e.g. "kitchen-jar".
    pub template: String,
    /// Field values by key; omitted fields use the template default.
    pub fields: Option<BTreeMap<String, String>>,
    /// Optional style override: {font: sans|sans_bold|condensed|rounded, bold, align: left|center|right,
    /// border: none|thin|thick|rounded|double, invert, barcode: auto|code128|ean13, date_format}.
    pub style: Option<serde_json::Value>,
    /// Label size as WIDTHxLENGTH in mm, e.g. "14x40". Normally read from the printer.
    pub label_size: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PrintParams {
    /// Template id from list_templates.
    pub template: String,
    /// Field values by key; omitted fields use the template default.
    pub fields: Option<BTreeMap<String, String>>,
    /// Optional style override (same shape as in preview_label).
    pub style: Option<serde_json::Value>,
    /// Label size like "14x40"; only used if the printer can't read the roll.
    pub label_size: Option<String>,
    /// Number of copies (1–10, default 1).
    pub copies: Option<u16>,
    /// Print darkness 1–15 (default from settings, usually 15).
    pub density: Option<u8>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PrintTextParams {
    /// Text to print; use \n for up to 3 lines.
    pub text: String,
    /// Label size like "14x40"; only used if the printer can't read the roll.
    pub label_size: Option<String>,
    /// Number of copies (1–10, default 1).
    pub copies: Option<u16>,
    /// Bold text (default true).
    pub bold: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PrintImageParams {
    /// Absolute path to a local image file (PNG, JPEG, WebP, GIF, BMP).
    pub path: String,
    /// Label size like "14x40"; only used if the printer can't read the roll.
    pub label_size: Option<String>,
    /// Number of copies (1–10, default 1).
    pub copies: Option<u16>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SaveTemplateParams {
    /// Human name; the id is derived from it.
    pub name: String,
    /// One of: text, icon_text, qr_text, barcode, date_stamp, image.
    pub layout: String,
    /// Array of {key, label, kind: text|multiline|date|qr_data|barcode_data|image_path|icon, default}.
    pub fields: serde_json::Value,
    /// Optional style (same shape as in preview_label).
    pub style: Option<serde_json::Value>,
    /// Gallery category, e.g. "Kitchen".
    pub category: Option<String>,
}

#[derive(Clone)]
pub struct NelkoServer {
    nelko: Arc<Nelko>,
    #[allow(dead_code)] // read by the #[tool_handler] macro
    tool_router: ToolRouter<Self>,
}

fn fail(msg: impl Into<String>) -> ToolResult {
    Ok(CallToolResult::error(vec![ContentBlock::text(msg)]))
}

fn parse_size(s: &Option<String>) -> Result<Option<LabelSize>, String> {
    s.as_deref().map(str::parse).transpose()
}

fn parse_style(v: Option<serde_json::Value>) -> Result<Option<Style>, String> {
    v.map(|v| serde_json::from_value::<Style>(v).map_err(|e| format!("Bad style: {e}"))).transpose()
}

fn copies(c: Option<u16>) -> Result<u16, String> {
    match c.unwrap_or(1) {
        0 => Err("Copies must be at least 1.".into()),
        n if n > MAX_COPIES => Err(format!("At most {MAX_COPIES} copies per request.")),
        n => Ok(n),
    }
}

fn describe_print(copies: u16, name: &str, r: &ResolvedSize) -> String {
    let mut s = format!(
        "Printed {copies} × {name} on {}×{} mm (size {}).",
        r.size.width_mm,
        r.size.length_mm,
        r.source.describe()
    );
    if r.source != SizeSource::Roll {
        s += &format!(" The printer couldn't read the roll, so {} was assumed; ask the user if that's wrong.", r.size);
    }
    s
}

#[tool_router]
impl NelkoServer {
    pub fn new(nelko: Arc<Nelko>) -> Self {
        Self { nelko, tool_router: Self::tool_router() }
    }

    async fn do_print(&self, req: PrintRequest) -> ToolResult {
        let name = self.nelko.template(&req.template_id).map(|t| t.name).unwrap_or_else(|| req.template_id.clone());
        let n = req.copies;
        match self.nelko.print(req).await {
            Ok(r) => Ok(CallToolResult::success(vec![ContentBlock::text(describe_print(n, &name, &r))])),
            Err(e) => fail(e.to_string()),
        }
    }

    #[tool(description = "Check the label printer: connection, battery, status, and the loaded roll's size and colour.")]
    pub async fn printer_status(&self) -> ToolResult {
        let report = self.nelko.report().await;
        Ok(CallToolResult::success(vec![ContentBlock::text(serde_json::to_string_pretty(&report).unwrap_or_default())]))
    }

    #[tool(description = "List label templates with their ids, layouts and field keys/defaults.")]
    pub async fn list_templates(&self) -> ToolResult {
        let list: Vec<serde_json::Value> = self
            .nelko
            .templates()
            .into_iter()
            .map(|t| {
                serde_json::json!({
                    "id": t.id, "name": t.name, "category": t.category, "layout": t.layout,
                    "fields": t.fields, "builtin": t.builtin,
                })
            })
            .collect();
        let mut out = serde_json::to_string_pretty(&list).unwrap_or_default();
        out += &format!("\n\nIcon names: {}", self.nelko.icon_names().join(", "));
        Ok(CallToolResult::success(vec![ContentBlock::text(out)]))
    }

    #[tool(description = "Render a label to a PNG preview without printing. Shows exactly what will print.")]
    pub async fn preview_label(&self, Parameters(p): Parameters<PreviewParams>) -> ToolResult {
        let (explicit, style) = match (parse_size(&p.label_size), parse_style(p.style)) {
            (Ok(s), Ok(st)) => (s, st),
            (Err(e), _) | (_, Err(e)) => return fail(e),
        };
        if self.nelko.template(&p.template).is_none() {
            return fail(format!("No template called '{}'. Use list_templates.", p.template));
        }
        let (size, note) = match self.nelko.resolve_size(explicit, true).await {
            Ok(r) => (r.size, format!("Label {} (size {}).", r.size, r.source.describe())),
            Err(_) => {
                let s: LabelSize = "14x40".parse().expect("valid");
                (s, format!("Label {s} (assumed; the roll size is unknown)."))
            }
        };
        let values: FieldValues = p.fields.unwrap_or_default();
        match self.nelko.preview(&p.template, &values, style, size) {
            Ok(png) => Ok(CallToolResult::success(vec![
                ContentBlock::image(base64::engine::general_purpose::STANDARD.encode(png), "image/png"),
                ContentBlock::text(note),
            ])),
            Err(e) => fail(e.to_string()),
        }
    }

    #[tool(description = "Print a label from a template. Size comes from the loaded roll when possible.")]
    pub async fn print_label(&self, Parameters(p): Parameters<PrintParams>) -> ToolResult {
        let parsed = (|| Ok::<_, String>((parse_size(&p.label_size)?, parse_style(p.style)?, copies(p.copies)?)))();
        let (size, style, copies) = match parsed {
            Ok(v) => v,
            Err(e) => return fail(e),
        };
        self.do_print(PrintRequest {
            template_id: p.template,
            values: p.fields.unwrap_or_default(),
            style,
            size,
            copies,
            density: p.density,
        })
        .await
    }

    #[tool(description = "Quickly print plain text (auto-sized to fit; \\n for up to 3 lines).")]
    pub async fn print_text(&self, Parameters(p): Parameters<PrintTextParams>) -> ToolResult {
        let (size, copies) = match (parse_size(&p.label_size), copies(p.copies)) {
            (Ok(s), Ok(c)) => (s, c),
            (Err(e), _) | (_, Err(e)) => return fail(e),
        };
        let mut style = self.nelko.template("plain-text").map(|t| t.style).unwrap_or_default();
        style.bold = p.bold.unwrap_or(true);
        let mut values = FieldValues::new();
        values.insert("text".into(), p.text.replace("\\n", "\n"));
        self.do_print(PrintRequest { template_id: "plain-text".into(), values, style: Some(style), size, copies, density: None })
            .await
    }

    #[tool(description = "Print a local image file, dithered to black and white.")]
    pub async fn print_image(&self, Parameters(p): Parameters<PrintImageParams>) -> ToolResult {
        let (size, copies) = match (parse_size(&p.label_size), copies(p.copies)) {
            (Ok(s), Ok(c)) => (s, c),
            (Err(e), _) | (_, Err(e)) => return fail(e),
        };
        match self.nelko.print_image(&PathBuf::from(&p.path), size, copies).await {
            Ok(r) => Ok(CallToolResult::success(vec![ContentBlock::text(describe_print(copies, "image", &r))])),
            Err(e) => fail(e.to_string()),
        }
    }

    #[tool(description = "Save a new user template the user can reuse in the app. Returns its id.")]
    pub async fn save_template(&self, Parameters(p): Parameters<SaveTemplateParams>) -> ToolResult {
        let t: Result<Template, String> = serde_json::from_value(serde_json::json!({
            "name": p.name, "layout": p.layout, "fields": p.fields,
            "style": p.style.unwrap_or(serde_json::json!({})), "category": p.category.unwrap_or_else(|| "Mine".into()),
        }))
        .map_err(|e| format!("Bad template: {e}"));
        match t.and_then(|t| self.nelko.save_template(t).map_err(|e: NelkoError| e.to_string())) {
            Ok(t) => Ok(CallToolResult::success(vec![ContentBlock::text(format!("Saved template '{}' (id: {}).", t.name, t.id))])),
            Err(e) => fail(e),
        }
    }
}

#[tool_handler]
impl ServerHandler for NelkoServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("stickler", env!("CARGO_PKG_VERSION")).with_title("Stickler label printer"))
            .with_instructions(INSTRUCTIONS)
    }
}
