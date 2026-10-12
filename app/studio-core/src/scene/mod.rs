//! The element tree a component returns: `Box`, `Text`, `Image` and `Path`
//! nodes with a typed `style`, `Morph` nodes holding two layouts, `Math`
//! nodes holding LaTeX and `Overlay` nodes bound to keys. It arrives from JS as JSON and is checked here, so
//! layout and paint only see valid trees.

mod color;
mod motion;
mod overlay;
mod parse;
mod path;
mod select;
mod style;
mod timing;
mod values;

pub use color::Color;
pub use motion::{Inset, Origin, Reveal, RevealBy, RevealFrom};
pub use overlay::{End, OverlayKind, OverlaySpec, Placed, Side};
pub use parse::{NodeError, parse_tree};
pub use path::{LineCap, LineJoin, PathSpec, StrokeSpec};
pub use select::{Scope, select};
pub use style::{STYLE_KEYS, Style};
pub use timing::{Curve, FADES, Fade, Timing};
pub use values::{
    Align, Border, Dim, Fill, FlexDirection, FlexWrap, Justify, NumericVariant, ObjectFit,
    Position, Shadow, Sides, TextAlign, describe,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Box,
    Text,
    Image,
    Path,
    /// Two layouts and a `p` between them; replaced by one of them before
    /// layout (see `select`).
    Morph,
    /// LaTeX; replaced by a Box of glyphs before layout (`crate::math`).
    Math,
    /// Drawn from keyed nodes' boxes after layout and morphing (a Brace, a
    /// Surround, …; `crate::overlay`), then a Box.
    Overlay,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Box => "Box",
            Kind::Text => "Text",
            Kind::Image => "Image",
            Kind::Path => "Path",
            Kind::Morph => "Morph",
            Kind::Math => "Math",
            Kind::Overlay => "Overlay",
        }
    }
}

/// `<Math tex display>`.
#[derive(Clone, Debug, PartialEq)]
pub struct MathSpec {
    pub tex: String,
    /// Display style (big operators, limits above and below), the default;
    /// `false` sets it as inline maths.
    pub display: bool,
    /// Set by a `<Write>` around it.
    pub write: Option<WriteSpec>,
}

/// `<Write>` on a Math: each written group's linear progress (a group name
/// is a `\k` group; `None` is the whole formula), the lag between pieces in
/// a group (`None`: manim's `min(4 / (n + 1), 0.2)`), and the outline's
/// stroke width in em.
#[derive(Clone, Debug, PartialEq)]
pub struct WriteSpec {
    pub groups: Vec<(Option<String>, f32)>,
    pub lag: Option<f32>,
    pub stroke: f64,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub kind: Kind,
    pub style: Style,
    /// A `Text` node's string.
    pub text: String,
    /// An `Image` node's path, relative to the project's `assets/`.
    pub src: String,
    /// A `Path` node's geometry and paint.
    pub path: Option<Box<PathSpec>>,
    /// A `Math` node's source.
    pub math: Option<Box<MathSpec>>,
    /// An `Overlay` node's spec.
    pub overlay: Option<Box<OverlaySpec>>,
    pub children: Vec<Node>,
    /// The element's `key`, kept so a later frame or scene can match it.
    pub key: Option<String>,
    /// The JSX element's TSX line and column, from the `__source` transform.
    pub loc: Option<(u32, u32)>,
    /// A `Morph` node's progress: 0 is its first child, 1 its second; an
    /// easing that overshoots may take it past either.
    pub p: f32,
    /// A `Morph` node's lag, fade and ease.
    pub timing: Option<Box<Timing>>,
    /// Set on the root of a Morph's chosen layout by `select`.
    pub scope: Option<Scope>,
}
