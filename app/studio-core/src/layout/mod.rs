//! Element tree -> absolute boxes. taffy does flexbox with rounding off (so
//! positions stay fractional), parley measures and wraps text, images size
//! from their pixels. The root fills the frame unless it sets its own size.

mod flex;
mod text;

pub use text::{TextKey, TextLayout, Typeset};

use crate::images::ImageCache;
use crate::scene::{Dim, Kind, Node, Style};
use std::sync::Arc;
use taffy::{AvailableSpace, NodeId, Size, TaffyTree};
use vello_cpu::Pixmap;
use vello_cpu::kurbo::Rect;

/// A laid-out node. Rects are absolute, before any transform.
pub struct LNode {
    pub style: Style,
    /// Border box.
    pub rect: Rect,
    /// Content box (inside padding and border): where text and images go.
    pub content: Rect,
    pub text: Option<(TextKey, TextLayout)>,
    pub image: Option<Arc<Pixmap>>,
    pub children: Vec<LNode>,
}

pub enum LayoutError {
    Font {
        family: String,
        loc: Option<(u32, u32)>,
    },
    Image {
        src: String,
        message: String,
        loc: Option<(u32, u32)>,
    },
    Taffy(String),
}

enum Leaf {
    Text(Option<Box<TextLayout>>, TextKey),
    Image(Arc<Pixmap>),
}

pub struct Layouter<'a> {
    pub typeset: &'a mut Typeset,
    pub images: &'a mut ImageCache,
    /// Family for `Text` without `fontFamily` (the theme's body font).
    pub default_family: &'a str,
}

impl Layouter<'_> {
    pub fn layout(&mut self, root: Node, width: f32, height: f32) -> Result<LNode, LayoutError> {
        let mut tree: TaffyTree<Leaf> = TaffyTree::new();
        tree.disable_rounding();
        let mut root = root;
        root.style.width.get_or_insert(Dim::Px(width));
        root.style.height.get_or_insert(Dim::Px(height));
        let id = self.build(&mut tree, &root)?;
        let available = Size {
            width: AvailableSpace::Definite(width),
            height: AvailableSpace::Definite(height),
        };
        tree.compute_layout_with_measure(id, available, |inputs, _id, leaf, style| {
            taffy::compute_leaf_layout(
                inputs,
                style,
                |_, _| 0.0,
                |known, avail| measure(known, avail, leaf),
            )
        })
        .map_err(|e| LayoutError::Taffy(e.to_string()))?;
        extract(&mut tree, id, root, (0.0, 0.0))
    }

    fn build(&mut self, tree: &mut TaffyTree<Leaf>, n: &Node) -> Result<NodeId, LayoutError> {
        let style = flex::to_taffy(&n.style);
        let taffy = |e: taffy::TaffyError| LayoutError::Taffy(e.to_string());
        match n.kind {
            Kind::Text => {
                let family = n
                    .style
                    .font_family
                    .as_deref()
                    .unwrap_or(self.default_family);
                if !self.typeset.families.iter().any(|f| f == family) {
                    return Err(LayoutError::Font {
                        family: family.to_owned(),
                        loc: n.loc,
                    });
                }
                let key = TextKey::new(&n.text, family, &n.style);
                let layout = self.typeset.take(&key);
                tree.new_leaf_with_context(style, Leaf::Text(Some(Box::new(layout)), key))
                    .map_err(taffy)
            }
            Kind::Image => {
                let pixmap = self
                    .images
                    .get(&n.src)
                    .map_err(|message| LayoutError::Image {
                        src: n.src.clone(),
                        message,
                        loc: n.loc,
                    })?;
                tree.new_leaf_with_context(style, Leaf::Image(pixmap))
                    .map_err(taffy)
            }
            Kind::Box => {
                let kids = n
                    .children
                    .iter()
                    .map(|c| self.build(tree, c))
                    .collect::<Result<Vec<_>, _>>()?;
                tree.new_with_children(style, &kids).map_err(taffy)
            }
        }
    }
}

fn measure(
    known: Size<Option<f32>>,
    available: Size<AvailableSpace>,
    leaf: Option<&mut Leaf>,
) -> Size<f32> {
    match leaf {
        Some(Leaf::Text(Some(layout), _)) => {
            let max = match (known.width, available.width) {
                (Some(w), _) | (None, AvailableSpace::Definite(w)) => Some(w),
                (None, AvailableSpace::MinContent) => Some(0.0),
                (None, AvailableSpace::MaxContent) => None,
            };
            layout.break_all_lines(max);
            Size {
                width: known.width.unwrap_or(layout.full_width()),
                height: known.height.unwrap_or(layout.height()),
            }
        }
        Some(Leaf::Image(p)) => {
            let (iw, ih) = (f32::from(p.width()), f32::from(p.height()));
            match (known.width, known.height) {
                (Some(w), Some(h)) => Size {
                    width: w,
                    height: h,
                },
                (Some(w), None) => Size {
                    width: w,
                    height: w * ih / iw.max(1.0),
                },
                (None, Some(h)) => Size {
                    width: h * iw / ih.max(1.0),
                    height: h,
                },
                (None, None) => Size {
                    width: iw,
                    height: ih,
                },
            }
        }
        _ => Size::ZERO,
    }
}

fn extract(
    tree: &mut TaffyTree<Leaf>,
    id: NodeId,
    n: Node,
    origin: (f64, f64),
) -> Result<LNode, LayoutError> {
    let l = *tree
        .layout(id)
        .map_err(|e| LayoutError::Taffy(e.to_string()))?;
    let x = origin.0 + f64::from(l.location.x);
    let y = origin.1 + f64::from(l.location.y);
    let rect = Rect::new(
        x,
        y,
        x + f64::from(l.size.width),
        y + f64::from(l.size.height),
    );
    let content = Rect::new(
        rect.x0 + f64::from(l.padding.left + l.border.left),
        rect.y0 + f64::from(l.padding.top + l.border.top),
        rect.x1 - f64::from(l.padding.right + l.border.right),
        rect.y1 - f64::from(l.padding.bottom + l.border.bottom),
    );
    let mut out = LNode {
        style: n.style,
        rect,
        content,
        text: None,
        image: None,
        children: Vec::new(),
    };
    match tree.get_node_context_mut(id) {
        Some(Leaf::Text(layout, key)) => {
            if let Some(mut layout) = layout.take() {
                text::finish(&mut layout, content.width() as f32, out.style.text_align);
                out.text = Some((key.clone(), *layout));
            }
        }
        Some(Leaf::Image(p)) => out.image = Some(Arc::clone(p)),
        None => {
            let kids = tree
                .children(id)
                .map_err(|e| LayoutError::Taffy(e.to_string()))?;
            out.children = kids
                .into_iter()
                .zip(n.children)
                .map(|(k, c)| extract(tree, k, c, (x, y)))
                .collect::<Result<_, _>>()?;
        }
    }
    Ok(out)
}

impl Typeset {
    /// Returns a painted tree's text layouts to the pool.
    pub fn reclaim(&mut self, n: LNode) {
        if let Some((key, layout)) = n.text {
            self.give_back(key, layout);
        }
        for c in n.children {
            self.reclaim(c);
        }
    }
}
