//! Oculus Studio's scene renderer. A component is a TSX file run in a
//! QuickJS sandbox; its element tree is laid out with taffy, typeset with
//! parley and drawn with vello_cpu. Every frame is a pure function of the
//! project and `t`, so renders are deterministic.
//!
//! ```no_run
//! use studio_core::{Project, RenderOptions, Renderer, StripOptions, Time, strip};
//! let project = Project::load("my-video")?;
//! let mut r = Renderer::new(&project, RenderOptions::default())?;
//! r.frame(Time::Global(12.4))?.write_png("frame.png")?;
//! strip(&project, StripOptions::default())?.write_png("strip.png")?;
//! println!("{}", project.lint()?);
//! # Ok::<(), studio_core::StudioError>(())
//! ```

mod easing;
mod error;
mod fonts;
mod host;
mod images;
mod layout;
mod library;
mod morph;
mod paint;
mod project;
mod render;
mod scene;
mod schema;
mod strip;
mod suggest;
mod theme;
mod transition;

pub use error::{BudgetKind, Result, StudioError, TsxLoc};
pub use fonts::bundled_families;
pub mod gallery;
pub use host::ComponentInfo;
pub use library::{
    STARTER_COMPONENTS, STARTER_THEMES, Starter, seed as seed_library, write as write_library,
};
pub use project::{
    CascadeComponents, ComponentSource, FRAME_RATES, FolderComponents, LOW_CONFIDENCE, LintIssue,
    LintReport, MANIFEST_PATH, Manifest, ManifestGap, ManifestScene, ManifestWord, Project,
    SceneSpec, Script, ScriptSection, Timeline, Video, Voice, WordCues,
};
pub use render::{
    Engine, Frame, MotionBlur, Purpose, RenderOptions, Renderer, SceneCall, Size, Time,
};
pub use scene::STYLE_KEYS;
pub use schema::{PropKind, PropSpec, PropsSchema};
pub use strip::{Strip, StripOptions, strip};
pub use theme::{NarrationTokens, Theme};
