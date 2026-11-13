//! Tauri commands. Each is a thin wrapper over a testable `*_impl` taking `&Nelko`.

use base64::Engine;
use nelko_core::config::Config;
use nelko_core::discovery::{self, FoundPrinter};
use nelko_core::geometry::LabelSize;
use nelko_core::service::{Nelko, PrintRequest, PrinterReport, SizeSource};
use nelko_core::template::{FieldValues, Style, Template};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tauri::State;

/// Errors cross to the UI as `{ message }` with a sentence fit to show.
#[derive(Debug, Serialize)]
pub struct UiError {
    pub message: String,
}

impl<E: std::fmt::Display> From<E> for UiError {
    fn from(e: E) -> Self {
        Self { message: e.to_string() }
    }
}

type Res<T> = Result<T, UiError>;
type Svc<'a> = State<'a, Arc<Nelko>>;

#[derive(Debug, Serialize)]
pub struct Preview {
    pub png_base64: String,
    pub size: String,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct PrintOutcome {
    pub size: String,
    pub source: String,
    pub source_text: String,
}

#[derive(Debug, Deserialize)]
pub struct PrintRequestDto {
    pub template_id: String,
    #[serde(default)]
    pub values: FieldValues,
    pub style: Option<Style>,
    pub size: Option<String>,
    pub copies: u16,
    pub density: Option<u8>,
}

fn source_name(s: SizeSource) -> String {
    serde_json::to_value(s).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default()
}

fn parse_size(s: Option<String>) -> Res<Option<LabelSize>> {
    s.filter(|s| !s.is_empty() && s != "auto").map(|s| s.parse::<LabelSize>()).transpose().map_err(UiError::from)
}

pub async fn preview_impl(n: &Nelko, template_id: String, values: FieldValues, style: Option<Style>, size: Option<String>) -> Res<Preview> {
    let explicit = parse_size(size)?;
    let (size, source) = match n.resolve_size(explicit, false).await {
        Ok(r) => (r.size, source_name(r.source)),
        Err(_) => ("14x40".parse().expect("valid"), "default".to_owned()),
    };
    let png = n.preview(&template_id, &values, style, size)?;
    Ok(Preview { png_base64: base64::engine::general_purpose::STANDARD.encode(png), size: size.to_string(), source })
}

pub async fn print_impl(n: &Nelko, req: PrintRequestDto) -> Res<PrintOutcome> {
    let r = n
        .print(PrintRequest {
            template_id: req.template_id,
            values: req.values,
            style: req.style,
            size: parse_size(req.size)?,
            copies: req.copies.clamp(1, 50),
            density: req.density,
        })
        .await?;
    Ok(PrintOutcome { size: r.size.to_string(), source: source_name(r.source), source_text: r.source.describe().into() })
}

#[tauri::command]
pub async fn printer_report(n: Svc<'_>) -> Res<PrinterReport> {
    Ok(n.report().await)
}

#[tauri::command]
pub async fn discover_printers() -> Res<Vec<FoundPrinter>> {
    Ok(discovery::discover(Duration::from_secs(12)).await?)
}

#[tauri::command]
pub async fn pair_printer(n: Svc<'_>, mac: String, name: Option<String>) -> Res<()> {
    discovery::pair_and_trust(&mac).await?;
    n.update_config(|c| {
        c.printer_mac = Some(mac);
        c.printer_name = Some(name.unwrap_or_else(|| "P21".into()));
    })?;
    Ok(())
}

#[tauri::command]
pub fn list_templates(n: Svc<'_>) -> Vec<Template> {
    n.templates()
}

#[tauri::command]
pub fn icon_names(n: Svc<'_>) -> Vec<String> {
    n.icon_names().into_iter().map(str::to_owned).collect()
}

#[tauri::command]
pub fn label_presets() -> Vec<String> {
    nelko_core::geometry::PRESETS.iter().map(ToString::to_string).collect()
}

#[tauri::command]
pub async fn preview(n: Svc<'_>, template_id: String, values: FieldValues, style: Option<Style>, size: Option<String>) -> Res<Preview> {
    preview_impl(&n, template_id, values, style, size).await
}

#[tauri::command]
pub async fn print(n: Svc<'_>, req: PrintRequestDto) -> Res<PrintOutcome> {
    print_impl(&n, req).await
}

#[tauri::command]
pub fn save_template(n: Svc<'_>, t: Template) -> Res<Template> {
    Ok(n.save_template(t)?)
}

#[tauri::command]
pub fn delete_template(n: Svc<'_>, id: String) -> Res<()> {
    Ok(n.delete_template(&id)?)
}

#[tauri::command]
pub fn duplicate_template(n: Svc<'_>, id: String, name: String) -> Res<Template> {
    Ok(n.duplicate_template(&id, &name)?)
}

#[tauri::command]
pub fn get_settings(n: Svc<'_>) -> Config {
    n.config()
}

#[tauri::command]
pub fn set_settings(n: Svc<'_>, c: Config) -> Res<Config> {
    Ok(n.update_config(|cfg| *cfg = c)?)
}

#[tauri::command]
pub async fn self_test(n: Svc<'_>) -> Res<()> {
    Ok(n.self_test().await?)
}
