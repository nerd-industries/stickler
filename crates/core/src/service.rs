//! `Nelko`: the one entry point the desktop app and the MCP server use.

use crate::config::Config;
use crate::error::{NelkoError, PrinterError};
use crate::geometry::LabelSize;
use crate::lock::{DEFAULT_LOCK_WAIT, PrinterLock, default_lock_path};
use crate::printer::Printer;
use crate::protocol::{Battery, PaperType, PrintJob, PrinterStatus, RollInfo};
use crate::render::elements::icon_names;
use crate::template::{FieldValues, Style, Template, TemplateStore, render_template};
use crate::transport::{RfcommTransport, Transport};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};
use std::time::Duration;

pub const DEFAULT_GAP_MM: u8 = 5;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

#[async_trait]
pub trait Connector: Send + Sync {
    async fn open(&self, mac: &str) -> Result<Box<dyn Transport>, PrinterError>;
}

pub struct RfcommConnector;

#[async_trait]
impl Connector for RfcommConnector {
    async fn open(&self, mac: &str) -> Result<Box<dyn Transport>, PrinterError> {
        Ok(Box::new(RfcommTransport::connect(mac, CONNECT_TIMEOUT).await?))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SizeSource {
    Roll,
    Explicit,
    LastKnown,
    Manual,
}

impl SizeSource {
    pub fn describe(&self) -> &'static str {
        match self {
            Self::Roll => "read from the loaded roll",
            Self::Explicit => "as requested",
            Self::LastKnown => "assumed from the last roll used",
            Self::Manual => "from your saved default size",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ResolvedSize {
    pub size: LabelSize,
    pub gap_mm: u8,
    pub continuous: bool,
    pub source: SizeSource,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PrinterReport {
    pub connected: bool,
    pub name: Option<String>,
    pub status: Option<PrinterStatus>,
    pub status_text: Option<String>,
    pub battery: Option<Battery>,
    pub roll: Option<RollInfo>,
    pub firmware: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrintRequest {
    pub template_id: String,
    #[serde(default)]
    pub values: FieldValues,
    #[serde(default)]
    pub style: Option<Style>,
    #[serde(default)]
    pub size: Option<LabelSize>,
    #[serde(default = "one")]
    pub copies: u16,
    #[serde(default)]
    pub density: Option<u8>,
}

fn one() -> u16 {
    1
}

pub struct Nelko {
    /// Last good copy; re-read from `config_path` on every access when set.
    config: Mutex<Config>,
    config_path: Option<PathBuf>,
    /// When set, user templates are re-read on access (the app and MCP share them).
    templates_dir: Option<PathBuf>,
    lock_path: PathBuf,
    store: RwLock<TemplateStore>,
    /// The roll most recently read from the printer this session.
    known_roll: Mutex<Option<RollInfo>>,
    connector: Box<dyn Connector>,
}

/// Pick the size: roll, then explicit, then last known, then manual default.
/// The roll wins so preview and print agree with what's physically loaded.
fn resolve(roll: Option<&RollInfo>, explicit: Option<LabelSize>, cfg: &Config) -> Result<ResolvedSize, NelkoError> {
    let (size, source) = if let Some(r) = roll {
        (r.size, SizeSource::Roll)
    } else if let Some(s) = explicit {
        (s, SizeSource::Explicit)
    } else if let Some(s) = cfg.last_label {
        (s, SizeSource::LastKnown)
    } else if let Some(s) = cfg.manual_label {
        (s, SizeSource::Manual)
    } else {
        return Err(NelkoError::UnknownLabelSize);
    };
    let continuous = roll.is_some_and(|r| r.paper_type == PaperType::Continuous);
    let gap_mm = if continuous { 0 } else { roll.and_then(|r| r.gap_mm).unwrap_or(DEFAULT_GAP_MM) };
    Ok(ResolvedSize { size, gap_mm, continuous, source })
}

impl Nelko {
    /// Real service: user config, user templates, Bluetooth.
    pub fn new() -> Result<Self, NelkoError> {
        Self::at(Config::path(), Config::templates_dir(), Box::new(RfcommConnector))
    }

    /// Service backed by files on disk, shared live with other processes.
    /// A broken config file falls back to defaults instead of failing.
    pub fn at(config_path: PathBuf, templates_dir: PathBuf, connector: Box<dyn Connector>) -> Result<Self, NelkoError> {
        let config = Config::load_from(&config_path).unwrap_or_else(|e| {
            eprintln!("stickler: {e}; using default settings");
            Config::default()
        });
        Ok(Self {
            config: Mutex::new(config),
            config_path: Some(config_path),
            store: RwLock::new(TemplateStore::load(templates_dir.clone())?),
            templates_dir: Some(templates_dir),
            lock_path: default_lock_path(),
            known_roll: Mutex::new(None),
            connector,
        })
    }

    /// For tests: config is not persisted, and the lock lives in a private temp file.
    pub fn with(config: Config, store: TemplateStore, connector: Box<dyn Connector>) -> Self {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let lock_path = std::env::temp_dir().join(format!("nelko-test-{}-{n}.lock", std::process::id()));
        Self {
            config: Mutex::new(config),
            config_path: None,
            templates_dir: None,
            lock_path,
            store: RwLock::new(store),
            known_roll: Mutex::new(None),
            connector,
        }
    }

    /// Current settings, re-read from disk so changes made by the other app show up.
    pub fn config(&self) -> Config {
        let mut cached = self.config.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(p) = &self.config_path {
            if let Ok(fresh) = Config::load_from(p) {
                *cached = fresh;
            }
        }
        cached.clone()
    }

    /// Read-modify-write against the file on disk, so concurrent edits aren't lost.
    pub fn update_config(&self, f: impl FnOnce(&mut Config)) -> Result<Config, NelkoError> {
        let mut cached = self.config.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(p) = &self.config_path {
            if let Ok(fresh) = Config::load_from(p) {
                *cached = fresh;
            }
        }
        f(&mut cached);
        if let Some(p) = &self.config_path {
            cached.save_to(p)?;
        }
        Ok(cached.clone())
    }

    fn reload_templates(&self) {
        if let Some(dir) = &self.templates_dir {
            if let Ok(fresh) = TemplateStore::load(dir.clone()) {
                *self.store.write().unwrap_or_else(|p| p.into_inner()) = fresh;
            }
        }
    }

    async fn connect(&self) -> Result<(PrinterLock, Printer<Box<dyn Transport>>), PrinterError> {
        let mac = self.config().printer_mac.ok_or(PrinterError::NotConfigured)?;
        let lock_path = self.lock_path.clone();
        let lock = tokio::task::spawn_blocking(move || PrinterLock::acquire_at(Path::new(&lock_path), DEFAULT_LOCK_WAIT))
            .await
            .map_err(|e| PrinterError::Bluetooth(e.to_string()))??;
        let t = self.connector.open(&mac).await?;
        Ok((lock, Printer::new(t)))
    }

    fn remember_roll(&self, roll: Option<&RollInfo>) {
        *self.known_roll.lock().unwrap_or_else(|p| p.into_inner()) = roll.cloned();
        if let Some(r) = roll {
            if self.config().last_label != Some(r.size) {
                let size = r.size;
                let _ = self.update_config(|c| c.last_label = Some(size));
            }
        }
    }

    /// Everything about the printer. Never fails; problems go in `error`.
    pub async fn report(&self) -> PrinterReport {
        let mut rep = PrinterReport { name: self.config().printer_name, ..Default::default() };
        let result: Result<(), PrinterError> = async {
            let (_lock, mut p) = self.connect().await?;
            rep.connected = true;
            let status = p.status().await?;
            rep.status = Some(status);
            rep.status_text = Some(status.to_string());
            rep.battery = p.battery().await.ok();
            rep.firmware = p.config().await.ok().map(|c| format!("{}.{}.{}", c.fw[0], c.fw[1], c.fw[2]));
            rep.roll = p.roll().await.ok().flatten();
            Ok(())
        }
        .await;
        if let Err(e) = result {
            rep.error = Some(e.to_string());
        }
        self.remember_roll(rep.roll.as_ref());
        rep
    }

    /// Resolve the label size; with `query_printer`, asks the printer for the roll first.
    pub async fn resolve_size(&self, explicit: Option<LabelSize>, query_printer: bool) -> Result<ResolvedSize, NelkoError> {
        let roll = if query_printer {
            let roll = match self.connect().await {
                Ok((_lock, mut p)) => p.roll().await.ok().flatten(),
                Err(_) => None,
            };
            self.remember_roll(roll.as_ref());
            roll
        } else {
            // Use what the printer last reported, so a preview matches what will print.
            self.known_roll.lock().unwrap_or_else(|p| p.into_inner()).clone()
        };
        resolve(roll.as_ref(), explicit, &self.config())
    }

    pub fn templates(&self) -> Vec<Template> {
        self.reload_templates();
        self.store.read().unwrap_or_else(|p| p.into_inner()).list().into_iter().cloned().collect()
    }

    pub fn template(&self, id: &str) -> Option<Template> {
        self.reload_templates();
        self.store.read().unwrap_or_else(|p| p.into_inner()).get(id).cloned()
    }

    pub fn icon_names(&self) -> Vec<&'static str> {
        icon_names()
    }

    pub fn save_template(&self, t: Template) -> Result<Template, NelkoError> {
        self.reload_templates();
        self.store.write().unwrap_or_else(|p| p.into_inner()).save_user(t)
    }

    pub fn delete_template(&self, id: &str) -> Result<(), NelkoError> {
        self.reload_templates();
        self.store.write().unwrap_or_else(|p| p.into_inner()).delete_user(id)
    }

    pub fn duplicate_template(&self, id: &str, name: &str) -> Result<Template, NelkoError> {
        self.reload_templates();
        self.store.write().unwrap_or_else(|p| p.into_inner()).duplicate(id, name)
    }

    fn template_with_style(&self, id: &str, style: Option<Style>) -> Result<Template, NelkoError> {
        let mut t = self.template(id).ok_or_else(|| NelkoError::Template(format!("No template called '{id}'.")))?;
        if let Some(s) = style {
            t.style = s;
        }
        Ok(t)
    }

    pub fn preview(&self, template_id: &str, values: &FieldValues, style: Option<Style>, size: LabelSize) -> Result<Vec<u8>, NelkoError> {
        let t = self.template_with_style(template_id, style)?;
        Ok(render_template(&t, values, size)?.preview_png(4))
    }

    /// Render and print, holding one lock and one connection throughout.
    pub async fn print(&self, req: PrintRequest) -> Result<ResolvedSize, NelkoError> {
        let t = self.template_with_style(&req.template_id, req.style.clone())?;
        let (_lock, mut p) = self.connect().await?;
        let roll = p.roll().await.ok().flatten();
        self.remember_roll(roll.as_ref());
        let resolved = resolve(roll.as_ref(), req.size, &self.config())?;
        let label = render_template(&t, &req.values, resolved.size)?;
        let cfg = self.config();
        let job = PrintJob {
            size: resolved.size,
            gap_mm: resolved.gap_mm,
            density: req.density.unwrap_or(cfg.density).clamp(1, 15),
            copies: req.copies.max(1),
            bitmap: label.to_print_bitmap(cfg.rotation),
        };
        p.print(&job).await?;
        if roll.is_none() {
            let size = resolved.size;
            let _ = self.update_config(|c| c.last_label = Some(size));
        }
        Ok(resolved)
    }

    pub async fn print_image(&self, path: &Path, size: Option<LabelSize>, copies: u16) -> Result<ResolvedSize, NelkoError> {
        let mut values = FieldValues::new();
        values.insert("image".into(), path.display().to_string());
        self.print(PrintRequest { template_id: "image".into(), values, style: None, size, copies, density: None }).await
    }

    pub async fn self_test(&self) -> Result<(), NelkoError> {
        let (_lock, mut p) = self.connect().await?;
        p.self_test().await?;
        Ok(())
    }
}
