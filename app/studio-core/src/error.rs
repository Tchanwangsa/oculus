//! Every failure in studio-core. Messages are read by agents, so each one
//! says what is wrong, where, and what would be valid.

use std::fmt;
use std::path::PathBuf;

/// A position in a component's TSX source (1-based), with that line's text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TsxLoc {
    pub line: u32,
    pub col: u32,
    pub snippet: String,
}

/// `line:col`; the snippet goes on its own line (see `quote`).
impl fmt::Display for TsxLoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

/// Which sandbox budget a component ran out of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetKind {
    Steps,
    Memory,
}

#[derive(Debug, thiserror::Error)]
pub enum StudioError {
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    /// `video.json` or a theme file is malformed or inconsistent.
    #[error("{}: {message}", path.display())]
    Project { path: PathBuf, message: String },

    /// No component file for a name used by a scene.
    #[error("no component `{name}` ({message})")]
    ComponentNotFound { name: String, message: String },

    /// The TSX did not parse or transform, or uses something not allowed yet.
    #[error("{} does not compile: {message}{}", file(component, loc), quote(loc))]
    Transpile {
        component: String,
        message: String,
        loc: Option<TsxLoc>,
    },

    /// The component threw, returned something that is not an element, or
    /// broke the component contract.
    #[error("{}: {message}{}", head(component, loc), quote(loc))]
    Component {
        component: String,
        message: String,
        loc: Option<TsxLoc>,
    },

    /// A scene's props do not match the component's `props` schema.
    #[error("{component} prop `{path}`: expected {expected}, got {got}")]
    Props {
        component: String,
        path: String,
        expected: String,
        got: String,
    },

    /// A node's `style` has an unknown key or a bad value.
    #[error("{}: {node}: {message}{}", head(component, loc), quote(loc))]
    Style {
        component: String,
        node: String,
        message: String,
        loc: Option<TsxLoc>,
    },

    #[error("{}: image `{src}` {message}{}", head(component, loc), quote(loc))]
    Image {
        component: String,
        src: String,
        message: String,
        loc: Option<TsxLoc>,
    },

    #[error("{}: unknown fontFamily `{family}`; bundled families: {}{}", head(component, loc), available.join(", "), quote(loc))]
    Font {
        component: String,
        family: String,
        available: Vec<String>,
        loc: Option<TsxLoc>,
    },

    #[error("{component} ran out of its {} budget{}", budget_name(*kind), budget_hint(*kind))]
    Budget { component: String, kind: BudgetKind },

    #[error(
        "t = {t} s is outside the video, which is {length:.3} s long; t must be in [0, {length:.3})"
    )]
    TimeOutOfRange { t: f64, length: f64 },

    #[error("scene {index} does not exist; the video has {count} scenes (0-based)")]
    SceneOutOfRange { index: usize, count: usize },

    #[error(
        "t = {t} s is outside scene {index}, which lasts {length:.3} s; t must be in [0, {length:.3})"
    )]
    SceneTimeOutOfRange { index: usize, t: f64, length: f64 },

    #[error("frame {frame} does not exist; the video has {frames} frames at {fps} fps (0-based)")]
    FrameOutOfRange { frame: u64, frames: u64, fps: u32 },

    /// A narrated scene's clip is missing from `narration/manifest.json` or
    /// was made for other text; `scene` is its id.
    #[error("scene `{scene}`: {message}")]
    Narration { scene: String, message: String },

    /// A problem in `script.md` text at a 1-based line.
    #[error("script.md:{line}: {message}")]
    Script { line: usize, message: String },

    /// A bad scene transition value; `level` names where it came from, as
    /// `scene 3 transition.kind` or `theme motion.transition.easing`.
    #[error("{level}: {message}")]
    Transition { level: String, message: String },

    /// One frame of a contact sheet failed; `kind`, `component` and `loc`
    /// are the inner error's.
    #[error("frame at {t:.3} s (scene {scene}, {scene_t:.3} s in): {source}")]
    Frame {
        t: f64,
        scene: usize,
        scene_t: f64,
        source: Box<StudioError>,
    },

    /// A problem lint reports as a sentence, with no TSX position.
    #[error("{0}")]
    Lint(String),

    #[error("JavaScript host: {0}")]
    Host(String),

    #[error("{0}")]
    Render(String),

    #[error("PNG {}: {message}", path.display())]
    Png { path: PathBuf, message: String },
}

/// Errors with a TSX position read like a compiler's:
///
/// ```text
/// Typo.tsx:4:5: Box > Box[0]: unknown style key `pading` (did you mean `padding`?)
///     4 | <Box style={{ pading: 12 }} />
/// ```
///
/// `file:line:col` first, so it can be opened or grepped; the line itself
/// below. Without a position the component's name leads.
fn head(component: &str, loc: &Option<TsxLoc>) -> String {
    match loc {
        Some(_) => file(component, loc),
        None => component.to_owned(),
    }
}

fn file(component: &str, loc: &Option<TsxLoc>) -> String {
    match loc {
        Some(l) => format!("{component}.tsx:{l}"),
        None => format!("{component}.tsx"),
    }
}

fn quote(loc: &Option<TsxLoc>) -> String {
    match loc {
        Some(l) if !l.snippet.trim().is_empty() => {
            format!("\n    {} | {}", l.line, l.snippet.trim())
        }
        _ => String::new(),
    }
}

fn budget_name(kind: BudgetKind) -> &'static str {
    match kind {
        BudgetKind::Steps => "step",
        BudgetKind::Memory => "memory (256 MB)",
    }
}

fn budget_hint(kind: BudgetKind) -> &'static str {
    match kind {
        BudgetKind::Steps => {
            " (an endless or very long loop?); a frame must be a quick pure function of t"
        }
        BudgetKind::Memory => " (an unbounded array or string?)",
    }
}

impl StudioError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// The variant as a stable snake_case word, for JSON output.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Io { .. } => "io",
            Self::Project { .. } => "project",
            Self::ComponentNotFound { .. } => "component_not_found",
            Self::Transpile { .. } => "transpile",
            Self::Component { .. } => "component",
            Self::Props { .. } => "props",
            Self::Style { .. } => "style",
            Self::Image { .. } => "image",
            Self::Font { .. } => "font",
            Self::Budget { .. } => "budget",
            Self::TimeOutOfRange { .. } => "time_out_of_range",
            Self::SceneOutOfRange { .. } => "scene_out_of_range",
            Self::SceneTimeOutOfRange { .. } => "scene_time_out_of_range",
            Self::FrameOutOfRange { .. } => "frame_out_of_range",
            Self::Transition { .. } => "transition",
            Self::Narration { .. } => "narration",
            Self::Script { .. } => "script",
            Self::Lint(_) => "lint",
            Self::Frame { source, .. } => source.kind(),
            Self::Host(_) => "host",
            Self::Render(_) => "render",
            Self::Png { .. } => "png",
        }
    }

    /// The component this error is about, when there is one.
    pub fn component(&self) -> Option<&str> {
        match self {
            Self::Transpile { component, .. }
            | Self::Component { component, .. }
            | Self::Props { component, .. }
            | Self::Style { component, .. }
            | Self::Image { component, .. }
            | Self::Font { component, .. }
            | Self::Budget { component, .. } => Some(component),
            Self::ComponentNotFound { name, .. } => Some(name),
            Self::Frame { source, .. } => source.component(),
            _ => None,
        }
    }

    /// The TSX position this error points at, when known.
    pub fn loc(&self) -> Option<&TsxLoc> {
        match self {
            Self::Transpile { loc, .. }
            | Self::Component { loc, .. }
            | Self::Style { loc, .. }
            | Self::Image { loc, .. }
            | Self::Font { loc, .. } => loc.as_ref(),
            Self::Frame { source, .. } => source.loc(),
            _ => None,
        }
    }
}

impl From<crate::transition::TransitionError> for StudioError {
    fn from(e: crate::transition::TransitionError) -> Self {
        Self::Transition {
            level: e.level,
            message: e.message,
        }
    }
}

pub type Result<T, E = StudioError> = std::result::Result<T, E>;
