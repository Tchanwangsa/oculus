//! The arithmetic of a page's window region on Windows, free of Win32: which
//! holes and covers touch one page, in its container's own physical pixels.

use serde::Deserialize;

/// An open slot in logical window points; `bl` and `br` round its bottom
/// corners where it meets the card's.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct Hole {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub bl: f64,
    pub br: f64,
}

/// A portal (popover, menu, dialog) over an open hole, in logical window
/// points, rounded by its largest corner radius.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct Cover {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub radius: f64,
}

/// A rect in physical pixels, right and bottom exclusive, as GDI takes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Px {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Px {
    fn width(&self) -> i32 {
        self.right - self.left
    }

    fn height(&self) -> i32 {
        self.bottom - self.top
    }

    fn overlaps(&self, other: &Px) -> bool {
        self.left < other.right
            && other.left < self.right
            && self.top < other.bottom
            && other.top < self.bottom
    }

    fn offset(self, dx: i32, dy: i32) -> Px {
        Px {
            left: self.left + dx,
            top: self.top + dy,
            right: self.right + dx,
            bottom: self.bottom + dy,
        }
    }
}

/// A rect with a radius per corner, in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    pub rect: Px,
    pub top_left: i32,
    pub top_right: i32,
    pub bottom_left: i32,
    pub bottom_right: i32,
}

/// One rounded corner as GDI cuts it: the corner's square, minus the circle
/// whose quarter rounds it (`circle` is that circle's bounding box).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cut {
    pub square: Px,
    pub circle: Px,
}

impl Shape {
    /// The pieces to take off the rect, one per rounded corner.
    pub fn cuts(&self) -> Vec<Cut> {
        let rect = self.rect;
        // `len` pixels in from the near (left or top) or far edge.
        let span = |near: bool, lo: i32, hi: i32, len: i32| {
            if near {
                (lo, lo + len)
            } else {
                (hi - len, hi)
            }
        };
        let corners = [
            (self.top_left, true, true),
            (self.top_right, false, true),
            (self.bottom_left, true, false),
            (self.bottom_right, false, false),
        ];
        corners
            .into_iter()
            .filter(|(radius, _, _)| *radius > 0)
            .map(|(n, west, north)| {
                let (left, right) = span(west, rect.left, rect.right, n);
                let (top, bottom) = span(north, rect.top, rect.bottom, n);
                let square = Px {
                    left,
                    top,
                    right,
                    bottom,
                };
                let (left, right) = span(west, rect.left, rect.right, 2 * n);
                let (top, bottom) = span(north, rect.top, rect.bottom, 2 * n);
                let circle = Px {
                    left,
                    top,
                    right,
                    bottom,
                };
                Cut { square, circle }
            })
            .collect()
    }
}

/// What one page shows: the union of its holes minus the union of its
/// covers. Empty holes hide the page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageRegion {
    pub holes: Vec<Shape>,
    pub covers: Vec<Shape>,
}

/// The region for a page whose container sits at `frame` (the window's client
/// area, physical pixels), with holes and covers in logical points and
/// `scale` physical pixels per point. Shapes come out in the container's
/// own coordinates; anything that misses the page is left out.
pub fn page_region(holes: &[Hole], covers: &[Cover], frame: Px, scale: f64) -> PageRegion {
    let local = Px {
        left: 0,
        top: 0,
        right: frame.width(),
        bottom: frame.height(),
    };
    let place = |left, top, right, bottom| {
        to_px(left, top, right, bottom, scale).offset(-frame.left, -frame.top)
    };
    let holes: Vec<Shape> = holes
        .iter()
        .map(|h| {
            let rect = place(h.left, h.top, h.right, h.bottom);
            // Square top corners, so a bottom one may take the full height.
            let max = (rect.width() / 2).min(rect.height());
            Shape {
                rect,
                top_left: 0,
                top_right: 0,
                bottom_left: fit(h.bl, scale, max),
                bottom_right: fit(h.br, scale, max),
            }
        })
        .filter(|s| s.rect.overlaps(&local))
        .collect();
    let covers = covers
        .iter()
        .map(|c| {
            let rect = place(c.left, c.top, c.right, c.bottom);
            let radius = fit(c.radius, scale, rect.width().min(rect.height()) / 2);
            Shape {
                rect,
                top_left: radius,
                top_right: radius,
                bottom_left: radius,
                bottom_right: radius,
            }
        })
        .filter(|c| holes.iter().any(|h| h.rect.overlaps(&c.rect)))
        .collect();
    PageRegion { holes, covers }
}

/// Logical points to physical pixels, rounded outward so a hole or cover
/// never comes up a pixel short of what the DOM drew.
fn to_px(left: f64, top: f64, right: f64, bottom: f64, scale: f64) -> Px {
    // Float noise (19.999999) must not tip an exact edge over a pixel.
    const SLACK: f64 = 1e-6;
    let lo = |v: f64| (v * scale + SLACK).floor() as i32;
    let hi = |v: f64| (v * scale - SLACK).ceil() as i32;
    Px {
        left: lo(left),
        top: lo(top),
        right: hi(right).max(lo(left)),
        bottom: hi(bottom).max(lo(top)),
    }
}

/// A corner radius in physical pixels, no larger than its shape allows.
fn fit(radius: f64, scale: f64, max: i32) -> i32 {
    if !radius.is_finite() {
        return 0;
    }
    ((radius * scale).round() as i32).clamp(0, max.max(0))
}

#[cfg(test)]
mod tests;
