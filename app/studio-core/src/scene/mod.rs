//! The element tree a component returns: `Box`, `Text`, `Image` and `Path`
//! nodes with a typed `style`, `Morph` nodes holding two layouts and `Math`
//! nodes holding LaTeX. It arrives from JS as JSON and is checked here, so
//! layout and paint only see valid trees.

mod color;
mod motion;
mod parse;
mod path;
mod select;
mod style;
mod values;

pub use color::Color;
pub use motion::{Inset, Origin, Reveal, RevealBy, RevealFrom};
pub use parse::{NodeError, parse_tree};
pub use path::{LineCap, LineJoin, PathSpec, StrokeSpec};
pub use select::{Scope, select};
pub use style::{STYLE_KEYS, Style};
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
    pub children: Vec<Node>,
    /// The element's `key`, kept so a later frame or scene can match it.
    pub key: Option<String>,
    /// The JSX element's TSX line and column, from the `__source` transform.
    pub loc: Option<(u32, u32)>,
    /// A `Morph` node's progress, 0..=1.
    pub p: f32,
    /// Set on the root of a Morph's chosen layout by `select`.
    pub scope: Option<Scope>,
}
