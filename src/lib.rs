//! gsp-rs compiles `.gsp` inputs into self-contained HTML documents.

mod config;
pub mod export;
mod format;
pub(crate) mod runtime;
pub(crate) mod util;

pub mod pipeline;

pub use config::{Config, RenderJob};

pub mod gsp {
    pub use crate::format::GspFile;

    pub fn parse(data: &[u8]) -> Result<GspFile, String> {
        GspFile::parse(data)
    }
}
