//! One component call -> one laid-out tree: check props, run the component,
//! parse its element tree and lay it out. A tree with `<Morph>` lays out
//! twice (every Morph at its first child, then its second) and the morph
//! engine blends the two at each Morph's `p`.

use super::{Engine, SceneCall};
use crate::error::{Result, StudioError};
use crate::host::{CallCtx, seed, tsx_loc};
use crate::layout::{LNode, LayoutError, Layouter};
use crate::morph::{self, MorphError};
use crate::project::WordCues;
use crate::scene::{Node, NodeError, parse_tree, select};

/// A laid-out tree, not yet painted; its text layouts go back to the pool
/// when it is composed.
pub struct Prepared {
    pub tree: LNode,
    pub logs: Vec<String>,
}

impl Engine {
    pub(crate) fn prepare(&mut self, call: SceneCall<'_>) -> Result<Prepared> {
        let name = call.component;
        let props = self.check_props(name, call.props)?;
        let (json, logs) = self.host.call(
            name,
            &props,
            CallCtx {
                t: call.t,
                duration: call.duration,
                width: f64::from(self.size.width),
                height: f64::from(self.size.height),
                seed: seed(name, call.index),
                words: call.words.map(WordCues::json),
            },
        )?;
        let tree = parse_tree(&json).map_err(|e| self.node_error(name, e))?;
        let tree = match select(&tree).map_err(|e| self.node_error(name, e))? {
            None => self.lay_out(name, tree)?,
            Some(sides) => {
                let a = self.lay_out(name, sides.a)?;
                let b = match self.lay_out(name, sides.b) {
                    Ok(b) => b,
                    Err(e) => {
                        self.typeset.reclaim(a);
                        return Err(e);
                    }
                };
                let ink = self.ink(self.morph_color);
                morph::morph_scene(a, b, &mut self.typeset, ink)
                    .map_err(|e| self.morph_error(name, name, e))?
            }
        };
        Ok(Prepared { tree, logs })
    }

    fn lay_out(&mut self, name: &str, tree: Node) -> Result<LNode> {
        let mut layouter = Layouter {
            typeset: &mut self.typeset,
            images: &mut self.images,
            default_family: &self.theme.body_font,
        };
        let laid = layouter.layout(tree, self.size.width as f32, self.size.height as f32);
        laid.map_err(|e| match e {
            LayoutError::Font { family, loc } => StudioError::Font {
                component: name.to_owned(),
                family,
                available: self.typeset.families.clone(),
                loc: self.loc(name, loc),
            },
            LayoutError::Image { src, message, loc } => StudioError::Image {
                component: name.to_owned(),
                src,
                message,
                loc: self.loc(name, loc),
            },
            LayoutError::Taffy(m) => StudioError::Render(format!("{name}: layout failed: {m}")),
        })
    }

    fn loc(&self, name: &str, l: Option<(u32, u32)>) -> Option<crate::error::TsxLoc> {
        let tsx = self.host.source(name).unwrap_or_default();
        l.map(|l| tsx_loc(tsx, l))
    }

    fn node_error(&self, name: &str, e: NodeError) -> StudioError {
        if e.is_style {
            StudioError::Style {
                component: name.to_owned(),
                node: e.path,
                message: e.message,
                loc: self.loc(name, e.loc),
            }
        } else {
            StudioError::Component {
                component: name.to_owned(),
                message: format!("{}: {}", e.path, e.message),
                loc: self.loc(name, e.loc),
            }
        }
    }

    /// `a` and `b` name the components the two morphed trees came from.
    pub(crate) fn morph_error(&self, a: &str, b: &str, e: MorphError) -> StudioError {
        let name = if e.in_a { a } else { b };
        StudioError::Component {
            component: name.to_owned(),
            message: e.message,
            loc: self.loc(name, e.loc),
        }
    }
}
