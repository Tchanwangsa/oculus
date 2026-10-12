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
    /// How many sub-frames were averaged: 1 without motion blur, or when
    /// nothing moved over the shutter.
    pub samples: u32,
}

impl Frame {
    pub fn encode_png(&self) -> Result<Vec<u8>> {
        encode_png(&self.rgba, self.width, self.height)
    }

    pub fn write_png(&self, path: impl AsRef<Path>) -> Result<()> {
        write_png(&self.rgba, self.width, self.height, path.as_ref())
    }
}

pub(crate) fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    PngEncoder::new(&mut out)
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|e| StudioError::Png {
            path: "<memory>".into(),
            message: e.to_string(),
        })?;
    Ok(out)
}

pub(crate) fn write_png(rgba: &[u8], width: u32, height: u32, path: &Path) -> Result<()> {
    let bytes = encode_png(rgba, width, height)?;
    std::fs::write(path, bytes).map_err(|e| StudioError::io(path, e))
}
