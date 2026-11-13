//! A printer session over any transport: queries and printing.

use crate::error::PrinterError;
use crate::protocol::*;
use crate::transport::{DEFAULT_IDLE, DEFAULT_MAX, Transport};
use std::time::{Duration, Instant};

const POLL_EVERY: Duration = Duration::from_millis(300);

pub struct Printer<T: Transport> {
    t: T,
}

impl<T: Transport> Printer<T> {
    pub fn new(t: T) -> Self {
        Self { t }
    }

    pub fn into_inner(self) -> T {
        self.t
    }

    async fn ask(&mut self, cmd: &[u8]) -> Result<Vec<u8>, PrinterError> {
        self.t.write_all(cmd).await?;
        self.t.read_reply(DEFAULT_IDLE, DEFAULT_MAX).await
    }

    pub async fn status(&mut self) -> Result<PrinterStatus, PrinterError> {
        let r = self.ask(CMD_STATUS).await?;
        // The status is the last byte; ignore any stray bytes before it.
        Ok(parse_status(*r.last().expect("read_reply never returns empty")))
    }

    pub async fn roll(&mut self) -> Result<Option<RollInfo>, PrinterError> {
        let r = self.ask(CMD_LABEL_INFO).await?;
        parse_label_block(&r)
    }

    pub async fn battery(&mut self) -> Result<Battery, PrinterError> {
        parse_battery(&self.ask(CMD_BATTERY).await?)
    }

    pub async fn config(&mut self) -> Result<DeviceConfig, PrinterError> {
        parse_config(&self.ask(CMD_CONFIG).await?)
    }

    pub async fn self_test(&mut self) -> Result<(), PrinterError> {
        self.t.write_all(CMD_SELFTEST).await
    }

    /// Check the printer is ready, send the job, then wait until it's ready again.
    pub async fn print(&mut self, job: &PrintJob) -> Result<(), PrinterError> {
        let s = self.status().await?;
        if s != PrinterStatus::Ready {
            return Err(PrinterError::Status(s));
        }
        self.t.write_all(&encode_job(job)).await?;
        let limit = Duration::from_secs(5 + 3 * job.copies.max(1) as u64);
        let start = Instant::now();
        loop {
            match self.status().await? {
                PrinterStatus::Ready => return Ok(()),
                PrinterStatus::Busy | PrinterStatus::Feeding => {}
                other => return Err(PrinterError::Status(other)),
            }
            if start.elapsed() > limit {
                return Err(PrinterError::Timeout("printing took too long"));
            }
            tokio::time::sleep(POLL_EVERY).await;
        }
    }
}
