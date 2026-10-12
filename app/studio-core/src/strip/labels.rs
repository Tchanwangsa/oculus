//! The sheet's ground and label bars, as an element tree laid out and
//! painted like a scene (bundled Inter, tabular digits, no hinting), so the
//! labels use the normal text path. A bar holds the time on the left and
//! the scene on the right, or both stacked when a tile is too narrow.

use crate::error::{Result, StudioError};
use crate::images::ImageCache;
use crate::layout::{LNode, LayoutError, Layouter, Typeset};
use crate::paint::{Canvas, paint};
use crate::scene::{Color, parse_tree};
use serde_json::{Value, json};
use vello_cpu::kurbo::Affine;

const FONT_SIZE: f64 = 13.0;
const LINE: f64 = 1.2;
const PAD: u32 = 6;
/// The least space between the time and the scene on one line.
const SPACE: f64 = 10.0;
/// Neither theme's background, so a tile's edge always shows.
const GROUND: &str = "#5a5d66";
const BAR_FILL: &str = "#44464e";
const TIME_INK: &str = "#f2f3f6";
const SCENE_INK: &str = "#c3c6cf";
/// A two-line bar's height, the taller of the two layouts.
pub const TALL: u32 = 38;

pub struct Label {
    pub time: String,
    pub scene: Option<String>,
}

/// How the label bars are laid out, the same for every tile.
#[derive(Clone, Copy)]
pub struct Bars {
    pub height: u32,
    stacked: bool,
}

pub struct Painter {
    typeset: Typeset,
    images: ImageCache,
}

impl Painter {
    pub fn new() -> Painter {
        Painter {
            typeset: Typeset::new(),
            images: ImageCache::new(None),
        }
    }

    /// One line per bar when every label fits a tile `width` wide, else two.
    pub fn bars(&mut self, labels: &[Label], width: u32) -> Result<Bars> {
        let one = Bars {
            height: 22,
            stacked: false,
        };
        let probe: Vec<Value> = (0..labels.len())
            .map(|i| bar((0, i as u32 * one.height), width, &labels[i], one))
            .collect();
        let height = (labels.len() as u32 * one.height).max(1);
        let laid = self.lay_out(probe, width, height)?;
        let fits = laid.children.iter().all(|b| match b.children.as_slice() {
            [time, scene] => time.rect.x1 + SPACE <= scene.rect.x0,
            _ => true,
        });
        self.typeset.reclaim(laid);
        Ok(if fits {
            one
        } else {
            Bars {
                height: TALL,
                stacked: true,
            }
        })
    }

    /// The ground with a bar at each `(x, y)`, painted, as RGBA8.
    pub fn paint(
        &mut self,
        size: (u32, u32),
        at: &[(u32, u32)],
        labels: &[Label],
        width: u32,
        bars: Bars,
    ) -> Result<Vec<u8>> {
        let tree = at
            .iter()
            .zip(labels)
            .map(|(&p, l)| bar(p, width, l, bars))
            .collect();
        let laid = self.lay_out(tree, size.0, size.1)?;
        let ground = Color::parse(GROUND).map_err(fail)?;
        let mut canvas = Canvas::new(size.0 as u16, size.1 as u16);
        canvas.begin(ground);
        let painted = paint(&mut canvas, &laid, Affine::IDENTITY, Color::BLACK);
        let rgba = canvas.finish();
        self.typeset.reclaim(laid);
        painted.map_err(fail)?;
        Ok(rgba)
    }

    fn lay_out(&mut self, bars: Vec<Value>, width: u32, height: u32) -> Result<LNode> {
        let tree = json!({ "k": "box", "children": bars });
        let node = parse_tree(&tree).map_err(|e| fail(format!("{}: {}", e.path, e.message)))?;
        let mut layouter = Layouter {
            typeset: &mut self.typeset,
            images: &mut self.images,
            default_family: "Inter",
        };
        layouter
            .layout(node, width as f32, height as f32)
            .map_err(|e| match e {
                LayoutError::Font { family, .. } => fail(format!("no font `{family}`")),
                LayoutError::Image { src, .. } => fail(format!("unexpected image `{src}`")),
                LayoutError::Taffy(m) => fail(m),
            })
    }
}

fn fail(m: String) -> StudioError {
    StudioError::Render(format!("strip labels: {m}"))
}

/// A bar `width` wide with its top left at `(x, y)`. Text never wraps or
/// shrinks; whatever is too wide is clipped at the bar's edge.
fn bar((x, y): (u32, u32), width: u32, label: &Label, bars: Bars) -> Value {
    let text = |s: &str, ink: &str| {
        json!({
            "k": "text",
            "text": s,
            "style": {
                "fontFamily": "Inter",
                "fontSize": FONT_SIZE,
                "fontWeight": 550,
                "lineHeight": LINE,
                "fontVariantNumeric": "tabular-nums",
                "color": ink,
                "flexShrink": 0,
            },
        })
    };
    let mut children = vec![text(&label.time, TIME_INK)];
    if let Some(s) = &label.scene {
        children.push(text(s, SCENE_INK));
    }
    let (direction, justify, align) = if bars.stacked {
        ("column", "center", "flex-start")
    } else {
        ("row", "space-between", "center")
    };
    json!({
        "k": "box",
        "style": {
            "position": "absolute",
            "left": x,
            "top": y,
            "width": width,
            "height": bars.height,
            "padding": [0, PAD],
            "flexDirection": direction,
            "justifyContent": justify,
            "alignItems": align,
            "fill": BAR_FILL,
            "clip": true,
        },
        "children": children,
    })
}
