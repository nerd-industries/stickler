//! Byte transports to the printer: real RFCOMM and an in-memory mock.

use crate::error::PrinterError;
use async_trait::async_trait;
use std::collections::VecDeque;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const DEFAULT_IDLE: Duration = Duration::from_millis(150);
pub const DEFAULT_MAX: Duration = Duration::from_secs(2);

#[async_trait]
pub trait Transport: Send {
    async fn write_all(&mut self, b: &[u8]) -> Result<(), PrinterError>;
    /// Collect bytes until none arrive for `idle`, or `max` elapses.
    /// Errors with `Timeout` if nothing arrives at all.
    async fn read_reply(&mut self, idle: Duration, max: Duration) -> Result<Vec<u8>, PrinterError>;
}

#[async_trait]
impl Transport for Box<dyn Transport> {
    async fn write_all(&mut self, b: &[u8]) -> Result<(), PrinterError> {
        (**self).write_all(b).await
    }
    async fn read_reply(&mut self, idle: Duration, max: Duration) -> Result<Vec<u8>, PrinterError> {
        (**self).read_reply(idle, max).await
    }
}

const UNREACHABLE: &str = "Printer is off, out of range, or connected to another device";

/// Bluetooth Classic RFCOMM, channel 1.
pub struct RfcommTransport {
    stream: bluer::rfcomm::Stream,
    /// Until the first write succeeds, the link may still be coming up.
    connect_deadline: Option<tokio::time::Instant>,
}

/// bluer's connect can return while the RFCOMM link is still being set up;
/// writes then fail with ENOTCONN until it is (verified on hardware, ~3 s).
pub fn is_still_connecting(e: &std::io::Error) -> bool {
    e.raw_os_error() == Some(107)
}

impl RfcommTransport {
    pub async fn connect(mac: &str, timeout: Duration) -> Result<Self, PrinterError> {
        let addr: bluer::Address = mac
            .parse()
            .map_err(|_| PrinterError::Bluetooth(format!("'{mac}' isn't a Bluetooth address")))?;
        let target = bluer::rfcomm::SocketAddr::new(addr, 1);
        // One deadline covers both the connect call and the link coming up.
        let deadline = tokio::time::Instant::now() + timeout;
        let stream = match tokio::time::timeout_at(deadline, bluer::rfcomm::Stream::connect(target)).await {
            Ok(Ok(s)) => s,
            _ => return Err(PrinterError::Unreachable(UNREACHABLE.into())),
        };
        Ok(Self { stream, connect_deadline: Some(deadline) })
    }
}

#[async_trait]
impl Transport for RfcommTransport {
    async fn write_all(&mut self, b: &[u8]) -> Result<(), PrinterError> {
        if let Some(deadline) = self.connect_deadline {
            let first = &b[..b.len().min(1024)];
            loop {
                match self.stream.write_all(first).await {
                    Ok(()) => break,
                    Err(e) if is_still_connecting(&e) && tokio::time::Instant::now() < deadline => {
                        tokio::time::sleep(Duration::from_millis(100)).await
                    }
                    Err(e) if is_still_connecting(&e) => {
                        return Err(PrinterError::Unreachable(UNREACHABLE.into()));
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            self.connect_deadline = None;
            for chunk in b[first.len()..].chunks(1024) {
                self.stream.write_all(chunk).await?;
            }
            self.stream.flush().await?;
            return Ok(());
        }
        for chunk in b.chunks(1024) {
            self.stream.write_all(chunk).await?;
        }
        self.stream.flush().await?;
        Ok(())
    }

    async fn read_reply(&mut self, idle: Duration, max: Duration) -> Result<Vec<u8>, PrinterError> {
        let deadline = tokio::time::Instant::now() + max;
        let mut out = Vec::new();
        let mut buf = [0u8; 256];
        loop {
            let now = tokio::time::Instant::now();
            if now >= deadline {
                break;
            }
            // Before the first byte, wait up to the deadline; after it, only `idle`.
            let wait = if out.is_empty() { deadline - now } else { idle.min(deadline - now) };
            match tokio::time::timeout(wait, self.stream.read(&mut buf)).await {
                Ok(Ok(0)) => return Err(PrinterError::Unreachable("The printer closed the connection".into())),
                Ok(Ok(n)) => out.extend_from_slice(&buf[..n]),
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => break,
            }
        }
        if out.is_empty() {
            Err(PrinterError::Timeout("no reply"))
        } else {
            Ok(out)
        }
    }
}

/// Test transport: records writes, replays canned replies in order.
#[derive(Default, Debug)]
pub struct MockTransport {
    pub written: Vec<u8>,
    pub replies: VecDeque<Vec<u8>>,
}

impl MockTransport {
    pub fn with_replies(replies: Vec<Vec<u8>>) -> Self {
        Self { written: Vec::new(), replies: replies.into() }
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn write_all(&mut self, b: &[u8]) -> Result<(), PrinterError> {
        self.written.extend_from_slice(b);
        Ok(())
    }
    async fn read_reply(&mut self, _idle: Duration, _max: Duration) -> Result<Vec<u8>, PrinterError> {
        self.replies.pop_front().ok_or(PrinterError::Timeout("no reply"))
    }
}
