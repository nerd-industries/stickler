//! Core library for the Nelko P21 label printer: protocol, transport,
//! rendering, templates and the service facade shared by the app and MCP server.

pub mod error;
pub mod geometry;
pub mod protocol;

pub use error::{NelkoError, PrinterError, RenderError};
