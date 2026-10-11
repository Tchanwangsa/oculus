//! Oculus Studio's scene renderer. A component is a TSX file run in a
//! QuickJS sandbox; its element tree is laid out with taffy, typeset with
//! parley and drawn with vello_cpu. Every frame is a pure function of the
//! project and `t`, so renders are deterministic.
//!
//! ```no_run
//! use studio_core::{Project, RenderOptions, Renderer, Time};
//! let project = Project::load("my-video")?;
//! let mut r = Renderer::new(&project, RenderOptions { scale: 1.0 })?;
//! r.frame(Time::Global(12.4))?.write_png("frame.png")?;
//! println!("{}", project.lint()?);
//! # Ok::<(), studio_core::StudioError>(())
//! ```

mod easing;
mod error;
mod fonts;
mod host;
mod images;
mod layout;
mod morph;
mod paint;
mod project;
mod render;
mod scene;
mod schema;
mod suggest;
mod theme;
mod transition;

pub use error::{BudgetKind, Result, StudioError, TsxLoc};
pub use fonts::bundled_families;
pub use host::ComponentInfo;
pub use project::{
    ComponentSource, FolderComponents, LintIssue, LintReport, Project, SceneSpec, Timeline, Video,
};
pub use render::{Engine, Frame, RenderOptions, Renderer, SceneCall, Size, Time};
pub use scene::STYLE_KEYS;
pub use schema::{PropKind, PropSpec, PropsSchema};
pub use theme::Theme;
