//! User settings in ~/.config/nelko/config.toml.

use crate::error::NelkoError;
use crate::geometry::LabelSize;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// How the landscape design is turned into print rows. `Cw` verified on hardware.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rotation {
    #[default]
    Cw,
    Ccw,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub printer_mac: Option<String>,
    pub printer_name: Option<String>,
    pub density: u8,
    pub rotation: Rotation,
    pub last_label: Option<LabelSize>,
    pub manual_label: Option<LabelSize>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            printer_mac: None,
            printer_name: None,
            density: 15,
            rotation: Rotation::Cw,
            last_label: None,
            manual_label: None,
        }
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("nelko")
}

impl Config {
    pub fn path() -> PathBuf {
        config_dir().join("config.toml")
    }

    pub fn templates_dir() -> PathBuf {
        config_dir().join("templates")
    }

    pub fn load() -> Result<Self, NelkoError> {
        Self::load_from(&Self::path())
    }

    pub fn save(&self) -> Result<(), NelkoError> {
        self.save_to(&Self::path())
    }

    pub fn load_from(path: &Path) -> Result<Self, NelkoError> {
        match std::fs::read_to_string(path) {
            Ok(s) => toml::from_str(&s).map_err(|e| NelkoError::Config(format!("{}: {e}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(NelkoError::Config(format!("{}: {e}", path.display()))),
        }
    }

    pub fn save_to(&self, path: &Path) -> Result<(), NelkoError> {
        let err = |e: String| NelkoError::Config(format!("{}: {e}", path.display()));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| err(e.to_string()))?;
        }
        let s = toml::to_string_pretty(self).map_err(|e| err(e.to_string()))?;
        std::fs::write(path, s).map_err(|e| err(e.to_string()))
    }
}
