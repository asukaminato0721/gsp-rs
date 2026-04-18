//! gsp-rs compiles `.gsp` inputs into self-contained HTML documents.

mod config;
pub mod export;
mod format;
pub mod geometry_parity;
pub(crate) mod runtime;
pub mod upload;
pub(crate) mod util;

pub mod pipeline;

pub use config::{Config, RenderJob};

pub mod gsp {
    pub use crate::format::{GspFile, ParseError};

    pub fn parse(data: &[u8]) -> Result<GspFile, ParseError> {
        GspFile::parse(data)
    }
}
