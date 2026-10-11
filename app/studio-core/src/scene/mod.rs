//! The element tree a component returns: `Box`, `Text` and `Image` nodes
//! with a typed `style`. It arrives from JS as JSON and is checked here, so
//! layout and paint only see valid trees.

mod color;
mod parse;
mod style;
mod values;

pub use color::Color;
pub use parse::parse_tree;
pub use style::{STYLE_KEYS, Style};
pub use values::{
    Align, Dim, Fill, FlexDirection, FlexWrap, Justify, ObjectFit, Position, Sides, TextAlign,
    describe,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Box,
    Text,
    Image,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Box => "Box",
            Kind::Text => "Text",
            Kind::Image => "Image",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Node {
    pub kind: Kind,
    pub style: Style,
    /// A `Text` node's string.
    pub text: String,
    /// An `Image` node's path, relative to the project's `assets/`.
    pub src: String,
    pub children: Vec<Node>,
    /// The JSX element's TSX line and column, from the `__source` transform.
    pub loc: Option<(u32, u32)>,
}
