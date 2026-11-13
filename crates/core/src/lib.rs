//! Core library for the Nelko P21 label printer: protocol, transport,
//! rendering, templates and the service facade shared by the app and MCP server.

pub mod config;
pub mod discovery;
pub mod error;
pub mod geometry;
pub mod lock;
pub mod printer;
pub mod protocol;
pub mod render;
pub mod transport;

pub use error::{NelkoError, PrinterError, RenderError};
