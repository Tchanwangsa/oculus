//! A transition frame as layers for one canvas: each scene's laid-out tree
//! wrapped in a full-frame node whose style carries the transition's
//! opacity, translate, scale or clip, so `paint` draws it with nothing new.
//! Where B covers A it fills the theme background first, so A does not show
//! through B's transparent areas.

use super::cascade::{Direction, Look};
use crate::layout::LNode;
use crate::scene::{Color, Fill, Inset, Kind, Style};
use vello_cpu::kurbo::Rect;

/// The layers for `look` at `p` (eased), painted in order. Magic is not a
/// layering; the caller morphs the trees instead.
pub fn layers(
    look: Look,
    direction: Direction,
    p: f32,
    a: LNode,
    b: LNode,
    size: (f64, f64),
    bg: Color,
) -> (Vec<LNode>, Option<LNode>) {
    let frame = |style: Style, child: LNode| wrap(size, style, vec![child]);
    let cover = |style: Style| Style {
        fill: Some(Fill::Solid(bg)),
        ..style
    };
    match look {
        Look::Cut | Look::Magic => (vec![b], Some(a)),
        Look::Dissolve => (vec![a, frame(cover(opacity(p)), b)], None),
        Look::Push => {
            let (w, h) = (size.0 as f32, size.1 as f32);
            let (dx, dy) = match direction {
                Direction::Left => (-w, 0.0),
                Direction::Right => (w, 0.0),
                Direction::Up => (0.0, -h),
                Direction::Down => (0.0, h),
            };
            let at = |k: f32| Style {
                translate_x: Some(dx * k),
                translate_y: Some(dy * k),
                ..Style::default()
            };
            (vec![frame(at(p), a), frame(at(p - 1.0), b)], None)
        }
        Look::Wipe => {
            // B shows from the edge it comes in from: `left` comes in from
            // the right, as push does.
            let k = 1.0 - p;
            let inset = match direction {
                Direction::Left => [0.0, 0.0, 0.0, k],
                Direction::Right => [0.0, k, 0.0, 0.0],
                Direction::Up => [k, 0.0, 0.0, 0.0],
                Direction::Down => [0.0, 0.0, k, 0.0],
            };
            let clip = Style {
                clip_inset: Some(Inset(inset)),
                ..Style::default()
            };
            (vec![a, frame(cover(clip), b)], None)
        }
        Look::Zoom => {
            // A grows and fades; B settles in from smaller, over A from
            // p = 0.3. B's own root (its background) stays full-frame and only
            // what is inside it scales, so no rim shows at the frame's edge.
            let scaled = |s: f32, op: f32| Style {
                scale: Some(s),
                ..opacity(op)
            };
            let fade_in = ((p - 0.3) / 0.7).clamp(0.0, 1.0);
            let b = scale_inside(b, 0.94 + 0.06 * p, size);
            (
                vec![
                    frame(scaled(1.0 + 0.08 * p, 1.0 - p), a),
                    frame(cover(opacity(fade_in)), b),
                ],
                None,
            )
        }
    }
}

/// The root's children scaled about the frame centre; a root that draws
/// content of its own scales whole.
fn scale_inside(mut root: LNode, s: f32, size: (f64, f64)) -> LNode {
    let style = Style {
        scale: Some(s),
        ..Style::default()
    };
    if root.text.is_some() || root.image.is_some() || root.path.is_some() {
        return wrap(size, style, vec![root]);
    }
    let kids = std::mem::take(&mut root.children);
    root.children = vec![wrap(size, style, kids)];
    root
}

fn opacity(k: f32) -> Style {
    Style {
        opacity: (k < 1.0).then_some(k),
        ..Style::default()
    }
}

fn wrap(size: (f64, f64), style: Style, children: Vec<LNode>) -> LNode {
    let r = Rect::new(0.0, 0.0, size.0, size.1);
    LNode {
        kind: Kind::Box,
        style,
        rect: r,
        content: r,
        text: None,
        image: None,
        path: None,
        children,
        key: None,
        loc: None,
        scope: None,
    }
}
