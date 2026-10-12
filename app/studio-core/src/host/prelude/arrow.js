// Arrow: a stroke through `points` (frame px) drawn up to `progress`, and a
// head that fades and grows in over the last 15% of it. Both are absolute
// full-frame Paths with the frame as viewBox, so the parent must fill the frame.
const xy = (p) => `${p[0]} ${p[1]}`;
const towards = (a, b) => {
  const dx = b[0] - a[0], dy = b[1] - a[1], len = Math.hypot(dx, dy);
  return len > 1e-9 ? { u: [dx / len, dy / len], len } : null;
};

// Straight segments, or a Catmull-Rom spline through the points with its
// tangents scaled by `curve` and natural (zero-curvature) ends, so the first
// and last segments do not hook. `from` is where the end tangent comes from:
// the last cubic's second control point, or the previous point.
const pathThrough = (pts, curve) => {
  const n = pts.length;
  if (!curve) {
    return { d: `M ${pts.map(xy).join(' L ')}`, from: pts[n - 2] };
  }
  const add = (a, b, k) => [a[0] + b[0] * k, a[1] + b[1] * k];
  const sub = (a, b) => [a[0] - b[0], a[1] - b[1]];
  // Hermite tangents: interior ones from the neighbours; each end solves
  // m0 = 1.5 (p1 - p0) - 0.5 m1, which is zero curvature there.
  const m = pts.map(() => [0, 0]);
  for (let i = 1; i < n - 1; i++) m[i] = add([0, 0], sub(pts[i + 1], pts[i - 1]), curve / 2);
  if (n === 2) {
    m[0] = m[1] = sub(pts[1], pts[0]);
  } else {
    m[0] = add(add([0, 0], sub(pts[1], pts[0]), 1.5), m[1], -0.5);
    m[n - 1] = add(add([0, 0], sub(pts[n - 1], pts[n - 2]), 1.5), m[n - 2], -0.5);
  }
  let d = `M ${xy(pts[0])}`;
  let from = pts[n - 2];
  for (let i = 0; i < n - 1; i++) {
    const c1 = add(pts[i], m[i], 1 / 3);
    const c2 = add(pts[i + 1], m[i + 1], -1 / 3);
    d += ` C ${xy(c1)} ${xy(c2)} ${xy(pts[i + 1])}`;
    from = c2;
  }
  return { d, from };
};

// The unit end tangent of `pts` through `pathThrough`, falling back to the
// last distinct point when a handle is degenerate.
const endTangent = (pts, from) => {
  const end = pts[pts.length - 1];
  let dir = towards(from, end);
  for (let i = pts.length - 2; i >= 0 && !dir; i--) dir = towards(pts[i], end);
  return dir;
};

function Arrow(props, ctx) {
  const { points, curve = 0, progress: p = 1, head, stroke = {}, style, key, children, __source, __self, ...rest } = props;
  onlyProps('Arrow', rest, 'points, curve, progress, head, stroke, style, key');
  if (children && children.length) throw new TypeError('<Arrow> takes no children');
  if (!Array.isArray(points) || points.length < 2) {
    throw new TypeError(`<Arrow> points must be two or more [x, y] points, got ${describe(points)}`);
  }
  points.forEach((q, i) => {
    if (!Array.isArray(q) || q.length !== 2) throw new TypeError(`<Arrow> points[${i}] must be [x, y], got ${describe(q)}`);
    finiteArgs('<Arrow>', { [`points[${i}][0]`]: q[0], [`points[${i}][1]`]: q[1] });
  });
  const width = stroke.width ?? 4;
  const color = stroke.color ?? (ctx.theme.color && ctx.theme.color.text) ?? '#000000';
  const size = head ?? width * 4.5;
  finiteArgs('<Arrow>', { curve, progress: p, head: size, 'stroke.width': width });
  const { width: w, height: hgt } = ctx.size;
  const overlay = (extra) =>
    Object.assign({ position: 'absolute', left: 0, top: 0, width: '100%', height: '100%' }, style, extra);
  const viewBox = [0, 0, w, hgt];

  // The shaft stops inside the head, so its cap never pokes past the tip.
  const pts = points.map((q) => [q[0], q[1]]);
  const end = pts[pts.length - 1];
  const tip = [end[0], end[1]];
  let dir = endTangent(pts, pathThrough(pts, curve).from);
  if (size > 0 && dir) {
    const last = towards(pts[pts.length - 2], end);
    const back = Math.min(size * 0.6, (last ? last.len : 0) * 0.9);
    end[0] -= dir.u[0] * back;
    end[1] -= dir.u[1] * back;
  }
  // The head follows the tangent of the shaft as drawn.
  const drawn = pathThrough(pts, curve);
  dir = endTangent(pts, drawn.from) || dir;
  const shaftStroke = Object.assign({ cap: 'round', join: 'round' }, stroke, { width, color });
  const shaft = h('path', {
    key: key === undefined ? undefined : `${key}:shaft`,
    __source,
    d: drawn.d,
    viewBox,
    stroke: shaftStroke,
    trim: [0, clamp(p, 0, 1)],
    style: overlay({}),
  });
  if (!(size > 0 && dir)) return shaft;
  const u = dir.u, nrm = [-u[1], u[0]];
  const base = [tip[0] - u[0] * size, tip[1] - u[1] * size];
  const corner = (s) => [base[0] + nrm[0] * size * 0.5 * s, base[1] + nrm[1] * size * 0.5 * s];
  const shown = easeOutCubic(clamp((p - 0.85) / 0.15, 0, 1));
  const arrowHead = h('path', {
    key: key === undefined ? undefined : `${key}:head`,
    __source,
    d: `M ${xy(tip)} L ${xy(corner(1))} L ${xy(corner(-1))} Z`,
    viewBox,
    fill: color,
    style: overlay({ opacity: shown, scale: lerp(0.5, 1, shown), transformOrigin: [end[0] / w, end[1] / hgt] }),
  });
  return [shaft, arrowHead];
}
define('Arrow', Arrow);
