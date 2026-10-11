//! A rendered frame: straight-alpha RGBA8, row-major, plus whatever the
//! component logged while making it.

use crate::error::{Result, StudioError};
use image::{ImageEncoder, codecs::png::PngEncoder};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// `console.log` output from the component call, in order.
    pub logs: Vec<String>,
}

impl Frame {
    pub fn encode_png(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        PngEncoder::new(&mut out)
            .write_image(
                &self.rgba,
                self.width,
                self.height,
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| StudioError::Png {
                path: "<memory>".into(),
                message: e.to_string(),
            })?;
        Ok(out)
    }

    pub fn write_png(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let bytes = self.encode_png()?;
        std::fs::write(path, bytes).map_err(|e| StudioError::io(path, e))
    }
}
