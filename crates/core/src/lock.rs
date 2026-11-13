//! Cross-process lock so the app and the MCP server never talk to the printer at once.

use crate::error::PrinterError;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const DEFAULT_LOCK_WAIT: Duration = Duration::from_secs(10);

/// Held for the duration of one printer connection; released on drop.
#[derive(Debug)]
pub struct PrinterLock {
    _file: File,
}

pub fn default_lock_path() -> PathBuf {
    dirs::runtime_dir().unwrap_or_else(std::env::temp_dir).join("nelko-p21.lock")
}

impl PrinterLock {
    pub fn acquire(timeout: Duration) -> Result<Self, PrinterError> {
        Self::acquire_at(&default_lock_path(), timeout)
    }

    pub fn acquire_at(path: &Path, timeout: Duration) -> Result<Self, PrinterError> {
        let file = OpenOptions::new().create(true).truncate(false).write(true).open(path)?;
        let start = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(TryLockError::WouldBlock) if start.elapsed() < timeout => {
                    std::thread::sleep(Duration::from_millis(100))
                }
                Err(TryLockError::WouldBlock) => return Err(PrinterError::Busy),
                Err(TryLockError::Error(e)) => return Err(e.into()),
            }
        }
    }
}
