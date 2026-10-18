//! Retained spatial queries and stroke simplification for the drawing surface.
//! The bridge transfers geometry on edits, then queries this BVH with scalar
//! coordinates while panning, zooming and selecting.
use wasm_bindgen::prelude::*;

#[derive(Clone, Copy, Debug)]
struct Point {
    x: f64,
    y: f64,
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    x: f64,
    y: f64,
    right: f64,
    bottom: f64,
}

impl Bounds {
    fn union(self, other: Self) -> Self {
        Self {
            x: self.x.min(other.x),
            y: self.y.min(other.y),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }
    fn expand(self, n: f64) -> Self {
        Self {
            x: self.x - n,
            y: self.y - n,
            right: self.right + n,
            bottom: self.bottom + n,
        }
    }
    fn intersects(self, other: Self) -> bool {
        self.x <= other.right
            && self.right >= other.x
            && self.y <= other.bottom
            && self.bottom >= other.y
    }
    fn contains(self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.right && p.y >= self.y && p.y <= self.bottom
    }
}

#[derive(Debug)]
struct Shape {
    kind: u8,
    origin: Point,
    bounds: Bounds,
    stroke: f64,
    filled: bool,
    heads: u8,
    points: Vec<Point>,
}

fn segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len = dx * dx + dy * dy;
    let t = if len > 0.0 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.x - a.x - t * dx).hypot(p.y - a.y - t * dy)
}

/// The chevron wing pair stands in for every arrowhead type, as in the JS reference.
fn head_hit(p: Point, tip: Point, prev: Point, pad: f64) -> bool {
    let angle = (tip.y - prev.y).atan2(tip.x - prev.x);
    let size = 16.0_f64.min((tip.x - prev.x).hypot(tip.y - prev.y) / 3.0);
    [-std::f64::consts::PI / 6.0, std::f64::consts::PI / 6.0]
        .iter()
        .any(|a| {
            let wing = Point {
                x: tip.x - (angle + a).cos() * size,
                y: tip.y - (angle + a).sin() * size,
            };
            segment_distance(p, wing, tip) <= pad
        })
}

impl Shape {
    fn visual_bounds(&self) -> Bounds {
        self.bounds
            .expand(self.stroke / 2.0 + if self.kind == 4 { 16.0 } else { 0.0 } + 2.0)
    }
    fn hit(&self, world: Point, tolerance: f64) -> bool {
        let pad = tolerance.max(0.0) + self.stroke / 2.0;
        if self.kind >= 3 && self.kind <= 5 {
            let p = Point {
                x: world.x - self.origin.x,
                y: world.y - self.origin.y,
            };
            if self.points.len() == 1 {
                return segment_distance(p, self.points[0], self.points[0]) <= pad;
            }
            if self
                .points
                .windows(2)
                .any(|s| segment_distance(p, s[0], s[1]) <= pad)
            {
                return true;
            }
            if self.kind == 4 && self.points.len() >= 2 {
                let n = self.points.len();
                let ends = [
                    (1, self.points[0], self.points[1]),
                    (2, self.points[n - 1], self.points[n - 2]),
                ];
                return ends
                    .iter()
                    .any(|&(bit, tip, prev)| self.heads & bit != 0 && head_hit(p, tip, prev, pad));
            }
            return false;
        }
        let b = self.bounds;
        if !b.expand(pad).contains(world) {
            return false;
        }
        if self.kind == 6 {
            return true;
        }
        let rx = (b.right - b.x) / 2.0;
        let ry = (b.bottom - b.y) / 2.0;
        let dx = world.x - b.x - rx;
        let dy = world.y - b.y - ry;
        if self.kind == 1 {
            let outer =
                (dx / (rx + pad).max(1e-9)).powi(2) + (dy / (ry + pad).max(1e-9)).powi(2) <= 1.0;
            return outer
                && (self.filled
                    || rx <= pad
                    || ry <= pad
                    || (dx / (rx - pad)).powi(2) + (dy / (ry - pad)).powi(2) >= 1.0);
        }
        if self.kind == 2 {
            if self.filled && rx > 0.0 && ry > 0.0 && (dx / rx).abs() + (dy / ry).abs() <= 1.0 {
                return true;
            }
            let v = [
                Point {
                    x: b.x + rx,
                    y: b.y,
                },
                Point {
                    x: b.right,
                    y: b.y + ry,
                },
                Point {
                    x: b.x + rx,
                    y: b.bottom,
                },
                Point {
                    x: b.x,
                    y: b.y + ry,
                },
            ];
            return (0..4).any(|i| segment_distance(world, v[i], v[(i + 1) % 4]) <= pad);
        }
        self.filled
            || world.x <= b.x + pad
            || world.x >= b.right - pad
            || world.y <= b.y + pad
            || world.y >= b.bottom - pad
    }
}

enum Node {
    Leaf {
        bounds: Bounds,
        indices: Vec<usize>,
    },
    Branch {
        bounds: Bounds,
        left: Box<Node>,
        right: Box<Node>,
    },
}

impl Node {
    fn build(shapes: &[Shape], mut indices: Vec<usize>) -> Option<Self> {
        let bounds = indices
            .iter()
            .map(|i| shapes[*i].visual_bounds())
            .reduce(Bounds::union)?;
        if indices.len() <= 8 {
            return Some(Self::Leaf { bounds, indices });
        }
        let horizontal = bounds.right - bounds.x >= bounds.bottom - bounds.y;
        indices.sort_unstable_by(|a, b| {
            let a = shapes[*a].bounds;
            let b = shapes[*b].bounds;
            let ca = if horizontal {
                a.x / 2.0 + a.right / 2.0
            } else {
                a.y / 2.0 + a.bottom / 2.0
            };
            let cb = if horizontal {
                b.x / 2.0 + b.right / 2.0
            } else {
                b.y / 2.0 + b.bottom / 2.0
            };
            ca.total_cmp(&cb)
        });
        let right = indices.split_off(indices.len() / 2);
        Some(Self::Branch {
            bounds,
            left: Box::new(Self::build(shapes, indices)?),
            right: Box::new(Self::build(shapes, right)?),
        })
    }
    fn query(&self, area: Bounds, candidates: &mut Vec<usize>) {
        match self {
            Self::Leaf { bounds, indices } => {
                if bounds.intersects(area) {
                    candidates.extend(indices);
                }
            }
            Self::Branch {
                bounds,
                left,
                right,
            } => {
                if bounds.intersects(area) {
                    left.query(area, candidates);
                    right.query(area, candidates);
                }
            }
        }
    }
}

#[wasm_bindgen]
pub struct CanvasScene {
    shapes: Vec<Shape>,
    root: Option<Node>,
}

#[wasm_bindgen]
impl CanvasScene {
    /// Each shape is kind,x,y,width,height,strokeWidth,filled,pointStart,pointCount,heads.
    /// Point offsets count pairs, not scalar values. `heads` is a bitfield: 1 marks an
    /// arrowhead at the first point, 2 at the last. Invalid records are skipped.
    #[wasm_bindgen(constructor)]
    pub fn new(records: &[f64], coordinates: &[f64]) -> Self {
        let mut shapes = Vec::with_capacity(records.len() / 10);
        for record in records.as_chunks::<10>().0 {
            if !record.iter().all(|v| v.is_finite()) {
                continue;
            }
            let kind = record[0] as u8;
            let origin = Point {
                x: record[1],
                y: record[2],
            };
            let mut points = Vec::new();
            if (3..=5).contains(&kind) {
                let start = record[7].max(0.0) as usize;
                let count = record[8].max(0.0) as usize;
                if let Some(slice) = start
                    .checked_mul(2)
                    .zip(start.checked_add(count).and_then(|n| n.checked_mul(2)))
                    .and_then(|(start, end)| coordinates.get(start..end))
                {
                    for p in slice.as_chunks::<2>().0 {
                        points.push(Point { x: p[0], y: p[1] });
                    }
                }
                if points.is_empty() {
                    points = vec![
                        Point { x: 0.0, y: 0.0 },
                        Point {
                            x: record[3],
                            y: record[4],
                        },
                    ];
                }
            }
            let bounds = if points.is_empty() {
                Bounds {
                    x: origin.x.min(origin.x + record[3]),
                    y: origin.y.min(origin.y + record[4]),
                    right: origin.x.max(origin.x + record[3]),
                    bottom: origin.y.max(origin.y + record[4]),
                }
            } else {
                points
                    .iter()
                    .map(|p| Bounds {
                        x: p.x + origin.x,
                        y: p.y + origin.y,
                        right: p.x + origin.x,
                        bottom: p.y + origin.y,
                    })
                    .reduce(Bounds::union)
                    .unwrap()
            };
            shapes.push(Shape {
                kind,
                origin,
                bounds,
                stroke: record[5].max(0.0),
                filled: record[6] != 0.0,
                heads: record[9] as u8,
                points,
            });
        }
        let root = Node::build(&shapes, (0..shapes.len()).collect());
        Self { shapes, root }
    }

    /// Return the topmost matching element's zero-based index, or -1.
    pub fn hit_test(&self, x: f64, y: f64, tolerance: f64) -> i32 {
        let p = Point { x, y };
        let area = Bounds {
            x,
            y,
            right: x,
            bottom: y,
        }
        .expand(tolerance.max(0.0));
        let mut candidates = Vec::new();
        if let Some(root) = &self.root {
            root.query(area, &mut candidates);
        }
        candidates
            .into_iter()
            .filter(|i| self.shapes[*i].hit(p, tolerance))
            .max()
            .map_or(-1, |i| i as i32)
    }

    /// Return visible elements in paint order.
    pub fn visible_indices(&self, x: f64, y: f64, width: f64, height: f64) -> Vec<u32> {
        let area = Bounds {
            x,
            y,
            right: x + width,
            bottom: y + height,
        };
        let mut candidates = Vec::new();
        if let Some(root) = &self.root {
            root.query(area, &mut candidates);
        }
        candidates.retain(|i| self.shapes[*i].visual_bounds().intersects(area));
        candidates.sort_unstable();
        candidates.into_iter().map(|i| i as u32).collect()
    }
}

/// Iterative Ramer–Douglas–Peucker. The endpoints and significant turns survive.
#[wasm_bindgen]
pub fn simplify_points(coordinates: &[f64], tolerance: f64) -> Vec<f64> {
    let points: Vec<Point> = coordinates
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| Point { x: p[0], y: p[1] })
        .collect();
    if points.len() <= 2 {
        return coordinates.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut stack = vec![(0, points.len() - 1)];
    while let Some((start, end)) = stack.pop() {
        let mut far = tolerance.max(0.0);
        let mut index = None;
        for i in start + 1..end {
            let d = segment_distance(points[i], points[start], points[end]);
            if d > far {
                far = d;
                index = Some(i);
            }
        }
        if let Some(i) = index {
            keep[i] = true;
            stack.push((start, i));
            stack.push((i, end));
        }
    }
    points
        .iter()
        .zip(keep)
        .filter(|(_, keep)| *keep)
        .flat_map(|(p, _)| [p.x, p.y])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scene(records: &[[f64; 10]], points: &[f64]) -> CanvasScene {
        CanvasScene::new(&records.concat(), points)
    }
    fn rect(x: f64, y: f64, filled: bool) -> [f64; 10] {
        [
            0.0,
            x,
            y,
            100.0,
            100.0,
            2.0,
            if filled { 1.0 } else { 0.0 },
            0.0,
            0.0,
            0.0,
        ]
    }
    #[test]
    fn hollow_shape_and_topmost() {
        let s = scene(&[rect(0.0, 0.0, true), rect(0.0, 0.0, false)], &[]);
        assert_eq!(s.hit_test(50.0, 50.0, 2.0), 0);
        assert_eq!(s.hit_test(0.0, 50.0, 2.0), 1);
        assert_eq!(s.hit_test(200.0, 50.0, 2.0), -1);
    }
    #[test]
    fn polyline_hits_segments_not_box() {
        let s = scene(
            &[[4.0, 20.0, 30.0, 100.0, 100.0, 2.0, 0.0, 0.0, 2.0, 2.0]],
            &[0.0, 0.0, 100.0, 100.0],
        );
        assert_eq!(s.hit_test(70.0, 80.0, 2.0), 0);
        assert_eq!(s.hit_test(30.0, 120.0, 2.0), -1);
        assert_eq!(s.hit_test(108.0, 126.0, 2.0), 0);
    }
    #[test]
    fn arrowheads_hit_only_at_ends_that_have_one() {
        for (heads, start, end) in [(0.0, -1, -1), (1.0, 0, -1), (2.0, -1, 0), (3.0, 0, 0)] {
            let s = scene(
                &[[4.0, 20.0, 30.0, 100.0, 100.0, 2.0, 0.0, 0.0, 2.0, heads]],
                &[0.0, 0.0, 100.0, 100.0],
            );
            assert_eq!(s.hit_test(22.0, 40.0, 2.0), start);
            assert_eq!(s.hit_test(108.0, 126.0, 2.0), end);
        }
    }
    #[test]
    fn ellipse_and_diamond_leave_corners_empty() {
        for kind in [1.0, 2.0] {
            let s = scene(
                &[[kind, 0.0, 0.0, 100.0, 100.0, 2.0, 1.0, 0.0, 0.0, 0.0]],
                &[],
            );
            assert_eq!(s.hit_test(2.0, 2.0, 0.0), -1);
            assert_eq!(s.hit_test(50.0, 50.0, 0.0), 0);
        }
    }
    #[test]
    fn bvh_queries_stay_in_paint_order_and_normalize_negative_sizes() {
        let mut records: Vec<_> = (0..200)
            .map(|i| rect(i as f64 * 200.0, 0.0, true))
            .collect();
        records[20][3] = -100.0;
        let s = scene(&records, &[]);
        assert_eq!(
            s.visible_indices(3900.0, 0.0, 400.0, 100.0),
            vec![19, 20, 21]
        );
        assert_eq!(s.hit_test(3950.0, 50.0, 0.0), 20);
        assert_eq!(s.hit_test(199.0 * 200.0 + 50.0, 50.0, 0.0), 199);
    }
    #[test]
    fn simplification_preserves_corners_endpoints_and_closed_strokes() {
        assert_eq!(
            simplify_points(&[0.0, 0.0, 1.0, 0.1, 2.0, 0.0, 2.0, 2.0], 0.2),
            vec![0.0, 0.0, 2.0, 0.0, 2.0, 2.0]
        );
        assert_eq!(
            simplify_points(&[0.0, 0.0, 2.0, 2.0, 0.0, 0.0], 0.2).len(),
            6
        );
    }
}
