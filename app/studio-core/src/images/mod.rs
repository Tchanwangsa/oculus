//! Images from the project's `assets/`, decoded once per renderer (not per
//! frame) into premultiplied pixmaps. A missing or undecodable file is an
//! error, never a placeholder.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use vello_cpu::peniko::ImageAlphaType;
use vello_cpu::{PixelMetadata, Pixmap};

#[derive(Default)]
pub struct ImageCache {
    assets: Option<PathBuf>,
    decoded: HashMap<String, Arc<Pixmap>>,
}

impl ImageCache {
    pub fn new(assets: Option<PathBuf>) -> ImageCache {
        ImageCache {
            assets,
            decoded: HashMap::new(),
        }
    }

    /// The decoded image for an `<Image src>`; the error says why not.
    pub fn get(&mut self, src: &str) -> Result<Arc<Pixmap>, String> {
        if let Some(p) = self.decoded.get(src) {
            return Ok(Arc::clone(p));
        }
        let path = self.resolve(src)?;
        let bytes = std::fs::read(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!("not found: no file {}", path.display())
            } else {
                format!("could not be read ({}): {e}", path.display())
            }
        })?;
        let img = image::load_from_memory(&bytes)
            .map_err(|e| format!("could not be decoded as PNG or JPEG: {e}"))?
            .to_rgba8();
        let (w, h) = img.dimensions();
        let (Ok(w16), Ok(h16)) = (u16::try_from(w), u16::try_from(h)) else {
            return Err(format!("is {w}x{h}; images may be at most 65535 px a side"));
        };
        let pixmap = Arc::new(Pixmap::from_parts(
            img.into_raw(),
            w16,
            h16,
            PixelMetadata {
                may_have_transparency: true,
                alpha_type: ImageAlphaType::Alpha,
            },
        ));
        self.decoded.insert(src.to_owned(), Arc::clone(&pixmap));
        Ok(pixmap)
    }

    /// `src` inside `assets/`: relative, without `..`.
    fn resolve(&self, src: &str) -> Result<PathBuf, String> {
        let rel = Path::new(src);
        if !rel
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
        {
            return Err(
                "must be a path inside the project's assets/ folder (relative, no `..`)".to_owned(),
            );
        }
        let Some(dir) = &self.assets else {
            return Err("cannot be loaded: this render has no assets folder".to_owned());
        };
        Ok(dir.join(rel))
    }
}
