use thiserror::Error;

/// Errors talking to the printer. Display strings are shown to users.
#[derive(Debug, Error)]
pub enum PrinterError {
    #[error("No printer is set up yet. Pair your P21 first.")]
    NotConfigured,
    #[error("{0}")]
    Unreachable(String),
    #[error("The printer is busy with another job. Try again in a moment.")]
    Busy,
    #[error("The printer didn't answer in time ({0}).")]
    Timeout(&'static str),
    #[error("The printer sent an unexpected reply: {0}")]
    BadReply(String),
    #[error("Printer status: {0:#04x}")]
    Status(u8),
    #[error("Connection error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Bluetooth error: {0}")]
    Bluetooth(String),
}

/// Errors while drawing a label.
#[derive(Debug, Error)]
pub enum RenderError {
    #[error("{0}")]
    InvalidData(String),
    #[error("Missing or unreadable asset: {0}")]
    Asset(String),
}

/// Top-level error returned by the service facade.
#[derive(Debug, Error)]
pub enum NelkoError {
    #[error(transparent)]
    Printer(#[from] PrinterError),
    #[error(transparent)]
    Render(#[from] RenderError),
    #[error("Template problem: {0}")]
    Template(String),
    #[error("Settings problem: {0}")]
    Config(String),
    #[error("I don't know what size labels are loaded. Pick a label size, or load a Nelko roll so the printer can read it.")]
    UnknownLabelSize,
}
