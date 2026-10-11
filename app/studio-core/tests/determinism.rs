//! The same frame from two independent renderers is byte-identical.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

mod common;

use common::fixture;
use studio_core::{Project, RenderOptions, Renderer, Time};

#[test]
fn two_renderers_agree_byte_for_byte() {
    for (name, t) in [("title-card", 1.0), ("layout", 0.0), ("text-motion", 0.45)] {
        let project = Project::load(fixture(name)).unwrap();
        let mut a = Renderer::new(&project, RenderOptions::default()).unwrap();
        let mut b = Renderer::new(&project, RenderOptions::default()).unwrap();
        // Render something else first in one of them, so pooled layouts
        // and cached images are exercised.
        b.frame(Time::Global(0.2)).unwrap();
        let fa = a.frame(Time::Global(t)).unwrap();
        let fb = b.frame(Time::Global(t)).unwrap();
        assert!(fa.rgba == fb.rgba, "{name}: frames differ");
        assert_eq!(fa.encode_png().unwrap(), fb.encode_png().unwrap());
    }
}
