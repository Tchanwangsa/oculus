mod blocks;
mod fixture;
mod geometry;
mod lint;
mod make;
mod snap;

use std::path::{Path, PathBuf};

use crate::test_support::Scratch;

/// A data dir holding the fixture PDF at `courses/TEST101/files/lecture.pdf`,
/// and an empty video folder at 1920x1080.
pub(super) struct Setup {
    pub scratch: Scratch,
    pub source: &'static str,
}

impl Setup {
    pub fn new(name: &str) -> Setup {
        let scratch = Scratch::new(name);
        let source = "courses/TEST101/files/lecture.pdf";
        let pdf = scratch.join(source);
        std::fs::create_dir_all(pdf.parent().unwrap()).unwrap();
        fixture::write(&pdf);
        let video = scratch.join("video");
        std::fs::create_dir_all(video.join("assets")).unwrap();
        std::fs::write(
            video.join("video.json"),
            r#"{ "size": { "width": 1920, "height": 1080 }, "scenes": [] }"#,
        )
        .unwrap();
        Setup { scratch, source }
    }

    pub fn data(&self) -> &Path {
        &self.scratch
    }

    pub fn video(&self) -> PathBuf {
        self.scratch.join("video")
    }

    pub fn pdf(&self) -> PathBuf {
        self.scratch.join(self.source)
    }
}
