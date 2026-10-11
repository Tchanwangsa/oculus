//! Inline images: a body to Markdown, its images downloaded beside it.

use crate::library::paths;
use crate::pages::md::{self, ImageMap};
use crate::sync::render::{content_type_of, up_to_course_root};
use crate::sync::{Engine, Subject, IMAGE_EXT};

impl Engine {
    /// Convert a body to Markdown with its inline images downloaded to the
    /// course's `images/`. Image `src` resolves against the document at
    /// `out_path`, so nested documents climb back to the course root.
    pub(in crate::sync) fn convert(&self, html: &str, c: &Subject, out_path: &str) -> String {
        let up = up_to_course_root(out_path);
        let mut images = ImageMap::new();
        for (endpoint, src) in md::image_refs(html) {
            if src.is_empty() || images.contains_key(&src) {
                continue;
            }
            match self.fetch_image(c, &endpoint) {
                Ok(Some(path)) => {
                    images.insert(src, format!("{up}{path}"));
                }
                Ok(None) => {}
                Err(e) => self
                    .reporter
                    .log("warning", &c.code, &format!("image {endpoint}: {e}")),
            }
        }
        md::to_markdown(html, &images)
    }

    /// Returns `images/…`, not yet adjusted for the document's depth.
    fn fetch_image(&self, c: &Subject, endpoint: &str) -> Result<Option<String>, String> {
        let r = self.canvas.get(endpoint)?;
        if !r.ok() {
            return Ok(None);
        }
        let info = r.json()?;

        let ct = content_type_of(&info);
        let ext = IMAGE_EXT
            .iter()
            .find(|(k, _)| *k == ct)
            .map(|(_, v)| *v)
            .unwrap_or("png");
        // Without an id every image would be `images/0.png`.
        let fid = info["id"].as_i64().unwrap_or_else(|| {
            endpoint
                .rsplit('/')
                .find_map(|seg| seg.parse::<i64>().ok())
                .unwrap_or(0)
        });
        let path = format!("images/{fid}.{ext}");

        // Same unchanged-skip as fetch_file: metadata match + on disk.
        let modified = info["modified_at"]
            .as_str()
            .or_else(|| info["updated_at"].as_str())
            .unwrap_or("")
            .to_string();
        let meta_size = info["size"].as_u64().unwrap_or(0);
        if !modified.is_empty() {
            let known = self
                .manifest
                .borrow()
                .get(&fid.to_string())
                .is_some_and(|(m, s)| *m == modified && *s == meta_size);
            let on_disk = paths::course_rel_path(&c.code, &path)
                .map_or(false, |rel| self.data_dir.join(rel).is_file());
            if known && on_disk {
                return Ok(Some(path));
            }
        }

        let Some(url) = self.download_url(&info, fid)? else {
            return Ok(None);
        };
        let bytes = self.fetch_bytes(&url)?;
        self.write(c, &path, &bytes, Some(fid))?;
        if !modified.is_empty() {
            self.manifest
                .borrow_mut()
                .insert(fid.to_string(), (modified, meta_size));
        }
        Ok(Some(path))
    }
}
