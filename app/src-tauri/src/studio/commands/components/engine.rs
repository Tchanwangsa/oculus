//! The engines the playground draws with: one per call, made on the thread
//! that uses it (QuickJS is not Send).

use studio_core::{Engine, RenderOptions, Size, Theme};

/// A component is drawn in a 16:9 frame; a still's width scales it.
pub const WIDTH: u32 = 1920;
pub const HEIGHT: u32 = 1080;

/// An engine on `theme` drawing at `scale` over the full-size frame.
pub fn new(theme: Theme, scale: f64) -> Result<Engine, String> {
    let size = Size {
        width: WIDTH,
        height: HEIGHT,
    };
    let options = RenderOptions {
        scale,
        motion_blur: None,
    };
    Engine::new(theme, size, None, options).map_err(|e| e.to_string())
}

/// An engine that only loads components: the bundled `default` theme at a
/// tenth of full size, since a load draws nothing.
pub fn loader() -> Result<Engine, String> {
    let theme = Theme::bundled("default").map_err(|e| e.to_string())?;
    new(theme, 0.1)
}
