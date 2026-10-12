// Axes and plots: `axes()` maps data to px, `Axes` draws the two axes on with
// ticks, numbers and titles, and inside it `Plot` draws any function as a
// stroke, `Dot` marks a point, `Scatter` pops points in (with a least-squares
// line), `Area` sweeps a shaded region in, `Riemann` pops rectangles in and
// refines them, and `Tangent` slides a tangent or a shrinking secant along a
// curve. `formula("sin(x)/x")` makes a function from text, since video.json
// cannot hold one. Keys: axes `<id>/x`, `<id>/y`, numbers `<id>/xn<v>`,
// `<id>/yn<v>`; each helper takes its own `key` and suffixes its pieces
// (`/label`, `/head`, `/dot`, `/fit`, `/r<i>`). Copy `formula`, `axes`,
// `Axes` and the helpers you use into a scene component to compose them.
// `endLabels` nudges curve names apart so they never sit on each other.
// Formulas read implicit products like `*`: `1/2x` is `(1/2)*x`, so write
// `1/(2x)` for one over two x.
export const props = {
  x: list(num()).default([-1, 4, 1]),
  y: list(num()).default([-1, 10, 2]),
  // Where the axes cross, [x, y]; default [0, 0], held inside the ranges.
  origin: list(num()).optional(),
  xTitle: latex().optional(),
  yTitle: latex().optional(),
  // Curves as formulas in x: "x^3 - 2x", "sin(x)/x", "exp(-x^2/2)/sqrt(2*pi)".
  fns: list(str()).default(["x^2"]),
  // A colour and a TeX name per curve, read with fns by index.
  tones: list(token("color")).default([]),
  labels: list(latex()).default([]),
  // The axes draw on from here; the curves from plotAt, one after another.
  at: num().default(0.2),
  plotAt: num().default(1.4),
  // x values marked on the first curve, with guides and coordinates, from markAt.
  marks: list(num()).default([]),
  markAt: num().default(3),
  // TeX for each mark's label (default its coordinates), and which way it sits from its dot, [ux, uy] in -1..1
  // (default: above the curve, clear of it).
  markLabels: list(latex()).optional(),
  markPlace: list(num()).optional(),
  // A dot rides the first curve's drawn end.
  head: bool().default(false),
  // Scatter points (parallel lists) pop in one by one from pointsAt.
  px: list(num()).default([]),
  py: list(num()).default([]),
  pointsAt: num().default(1.4),
  // A least-squares line through the points, drawn on from fitAt, with its equation.
  fit: bool().default(false),
  fitAt: num().default(4),
  // Shade the first curve over [a, b], down to the x-axis or to shadeVs (a formula), sweeping in from shadeAt.
  shade: list(num()).optional(),
  shadeVs: str().optional(),
  shadeAt: num().default(2.4),
  shadeLabel: latex().optional(),
  shadeTone: token("color").optional(),
  // Rectangles instead of a fill: a count each time, refined one count after another.
  rects: list(num()).default([]),
  // A tangent to the first curve at tangent[0], gliding to tangent[1] when there is one, from tangentAt.
  tangent: list(num()).default([]),
  tangentAt: num().default(2.6),
  // A secant h wide instead: the second point closes in on the first as the glide runs.
  secant: num().default(0),
  tone: token("color").default("accent"),
};

export const examples = {
  parabola: {
    x: [-1, 4, 1],
    y: [-1, 10, 2],
    fns: ["x^2"],
    labels: ["y = x^2"],
    marks: [1, 2, 3],
    markAt: 3,
  },
  sine: { x: [0, 7, 1], y: [-1.25, 1.25, 0.5], fns: ["sin(x)"], labels: ["y = \\sin x"], head: true },
  tangent: {
    x: [-3, 3, 1],
    y: [-2, 2, 1],
    fns: ["x^3/3 - x"],
    labels: ["y = \\tfrac{x^3}{3} - x"],
    tangent: [-2, 2],
    tangentAt: 2,
  },
  limit: {
    x: [0, 4, 1],
    y: [0, 4, 1],
    fns: ["x^2/4"],
    labels: ["y = \\tfrac{x^2}{4}"],
    tangent: [2],
    secant: 1.6,
    tangentAt: 2,
  },
  riemann: {
    x: [0, 5, 1],
    y: [0, 2.5, 0.5],
    fns: ["1 + sin(x)"],
    shade: [0, 3.1416],
    rects: [4, 8, 16],
    shadeAt: 2,
    shadeLabel: "\\int_0^{\\pi} (1 + \\sin x)\\,dx = \\pi + 2",
  },
  tail: {
    x: [-4, 4, 1],
    y: [0, 0.5, 0.1],
    origin: [-4, 0],
    fns: ["exp(-x^2/2)/sqrt(2*pi)"],
    labels: ["\\varphi(z)"],
    shade: [1.5, 4],
    shadeAt: 2.6,
    shadeLabel: "P(Z > 1.5) \\approx 0.067",
    shadeTone: "negative",
  },
  regression: {
    x: [0, 9, 1],
    y: [40, 90, 10],
    xTitle: "\\text{hours studied}",
    yTitle: "\\text{score}",
    fns: [],
    px: [1, 2, 3, 4, 5, 6, 7, 8],
    py: [52, 55, 61, 60, 68, 72, 70, 79],
    pointsAt: 1.2,
    fit: true,
    fitAt: 3.6,
  },
  projectile: {
    x: [0, 4, 1],
    y: [0, 25, 5],
    xTitle: "t\\ (\\text{s})",
    yTitle: "h\\ (\\text{m})",
    fns: ["20x - 5x^2", "15x - 5x^2"],
    tones: ["accent", "warning"],
    labels: ["v_0 = 20", "v_0 = 15"],
    plotAt: 1.2,
    marks: [2],
    markAt: 4,
  },
  market: {
    x: [0, 10, 1],
    y: [0, 10, 1],
    xTitle: "Q",
    yTitle: "P",
    fns: ["9 - 0.8x", "1 + 0.8x"],
    tones: ["accent", "positive"],
    labels: ["D", "S"],
    plotAt: 1,
    marks: [5],
    markAt: 3.6,
    markLabels: ["E"],
    markPlace: [0, -1],
  },
};

const fmt = (v, d = 2) => String(Number(v.toFixed(d)));

/** Decimals that show a tick `step` exactly: two at least, six at most. */
const decimals = (step) => {
  for (let d = 2; d < 6; d++) if (Math.abs(step * 10 ** d - Math.round(step * 10 ** d)) < 1e-6) return d;
  return 6;
};

/**
 * A function of x from a formula: numbers, x, pi, e, + - * / ^, unary minus,
 * implicit products (2x, 2(x+1), (x+1)(x-1)), parentheses and sin cos tan
 * asin acos atan exp ln log (base 10) sqrt abs floor min max. An implicit
 * product binds like `*`, so `1/2x` is `(1/2)*x`; write `1/(2x)`. A bad
 * formula throws an error that names it.
 */
const formula = (src) => {
  const FN = { sin: Math.sin, cos: Math.cos, tan: Math.tan, asin: Math.asin, acos: Math.acos, atan: Math.atan };
  Object.assign(FN, { exp: Math.exp, ln: Math.log, log: Math.log10, sqrt: Math.sqrt, abs: Math.abs, floor: Math.floor });
  Object.assign(FN, { min: Math.min, max: Math.max });
  const CONST = { pi: Math.PI, e: Math.E };
  const has = (o, k) => Object.prototype.hasOwnProperty.call(o, k);
  const toks = String(src).match(/\d+\.?\d*|\.\d+|[A-Za-z]+|\S/g) ?? [];
  let i = 0;
  const fail = (why) => {
    throw new Error(`formula "${src}": ${why}`);
  };
  const eat = (s) => toks[i] === s && ++i > 0;
  const expect = (s) => eat(s) || fail(`expected "${s}" ${i < toks.length ? `before "${toks[i]}"` : "at the end"}`);
  const atom = () => {
    const tok = toks[i++];
    if (tok === undefined) return fail("it ends too soon");
    if (/^[\d.]/.test(tok)) return () => Number(tok);
    if (tok === "(") {
      const inner = sum();
      expect(")");
      return inner;
    }
    if (tok === "x") return (x) => x;
    if (has(CONST, tok)) return () => CONST[tok];
    if (has(FN, tok)) {
      expect("(");
      const args = [sum()];
      while (eat(",")) args.push(sum());
      expect(")");
      return (x) => FN[tok](...args.map((a) => a(x)));
    }
    return fail(/^[A-Za-z]/.test(tok) ? `unknown name "${tok}" (x, pi, e, ${Object.keys(FN).join(" ")})` : `unexpected "${tok}"`);
  };
  const unary = () => {
    if (eat("-")) {
      const inner = unary();
      return (x) => -inner(x);
    }
    eat("+");
    const base = atom();
    if (!eat("^")) return base;
    const power = unary();
    return (x) => base(x) ** power(x);
  };
  const product = () => {
    let f = unary();
    for (;;) {
      const [a, div] = [f, eat("/")];
      if (!div && !eat("*") && !/^[\d.A-Za-z(]/.test(toks[i] ?? "")) return f;
      const b = unary();
      f = div ? (x) => a(x) / b(x) : (x) => a(x) * b(x);
    }
  };
  const sum = () => {
    let f = product();
    for (;;) {
      const [a, minus] = [f, toks[i] === "-"];
      if (!minus && !eat("+")) return f;
      if (minus) i++;
      const b = product();
      f = minus ? (x) => a(x) - b(x) : (x) => a(x) + b(x);
    }
  };
  const f = sum();
  if (i < toks.length) fail(`unexpected "${toks[i]}"`);
  return f;
};

/** Data -> px for an axes `width` by `height` px, `pad` px round it for numbers; `origin` is where the axes cross. */
const axes = ({ x, y, origin = [0, 0], width = 1300, height = 760, pad = 96 }) => {
  for (const [name, r] of [["x", x], ["y", y]]) {
    if (!Array.isArray(r) || r.length !== 3 || !r.every(Number.isFinite) || !(r[0] < r[1]) || !(r[2] > 0)) {
      throw new Error(`Axes: ${name} = ${JSON.stringify(r)} must be [lo, hi, step] with lo < hi and step > 0`);
    }
  }
  const X = (v) => pad + ((v - x[0]) / (x[1] - x[0])) * width;
  const Y = (v) => pad + height - ((v - y[0]) / (y[1] - y[0])) * height;
  const [ox, oy] = [clamp(origin[0], x[0], x[1]), clamp(origin[1], y[0], y[1])];
  return { x, y, origin, pad, w: width + 2 * pad, h: height + 2 * pad, X, Y, ox: X(ox), oy: Y(oy) };
};

/** A full-box absolute layer, so children draw in the axes' px. */
const layer = (ax) => ({ position: "absolute", left: 0, top: 0, width: ax.w, height: ax.h });

/** A TeX label in a wide box with its [ux, uy] corner (signs: -1, 0, 1) at (px, py), so it grows away from there. */
const tag = (px, py, [ux, uy], tex, key, style, size) => {
  const [w, h] = [size * 14, size * 2];
  return (
    <Box
      style={{
        position: "absolute",
        left: ux > 0 ? px : ux < 0 ? px - w : px - w / 2,
        top: uy > 0 ? py : uy < 0 ? py - h : py - h / 2,
        width: w,
        height: h,
        justifyContent: ux > 0 ? "flex-start" : ux < 0 ? "flex-end" : "center",
        alignItems: uy > 0 ? "flex-start" : uy < 0 ? "flex-end" : "center",
        opacity: style.opacity,
      }}
    >
      <Math key={key} tex={tex} style={{ fontSize: size, color: style.color }} />
    </Box>
  );
};

/** The axes through `ax.origin`, drawn on from `at` over `dur`, numbered every step but the origin's. */
const Axes = ({ ax, at = 0, dur, id = "ax", size = 40, xTitle, yTitle, children }, { t, theme }) => {
  const { color, motion } = theme;
  const q = easeInOutCubic(clamp((t - at) / (dur ?? 2 * motion.duration.slow), 0, 1));
  const w = size / 10;
  const { ox, oy } = ax;
  const over = size * 0.6;
  const line = (key, left, top, width, height, inset, opacity = 1) => (
    <Box
      key={key}
      style={{ position: "absolute", left, top, width, height, radius: w / 2, fill: color.text, clipInset: inset, opacity }}
    />
  );
  const parts = [
    line(`${id}/x`, ax.X(ax.x[0]) - over, oy - w / 2, ax.X(ax.x[1]) - ax.X(ax.x[0]) + 2 * over, w, [0, 1 - q, 0, 0]),
    line(`${id}/y`, ox - w / 2, ax.Y(ax.y[1]) - over, w, ax.Y(ax.y[0]) - ax.Y(ax.y[1]) + 2 * over, [1 - q, 0, 0, 0]),
  ];
  const numbers = (axis) => {
    const [lo, hi, step] = ax[axis];
    const zero = ax.origin[axis === "x" ? 0 : 1];
    const [d, eps] = [decimals(step), step * 1e-6];
    for (let k = Math.ceil(lo / step - 1e-6); k * step <= hi + eps; k++) {
      const v = k * step;
      if (Math.abs(v - zero) < eps) continue;
      const along = axis === "x" ? (ax.X(v) - ax.X(lo)) / (ax.X(hi) - ax.X(lo)) : (ax.Y(lo) - ax.Y(v)) / (ax.Y(lo) - ax.Y(hi));
      const shown = clamp((q - along) * 8 + 1, 0, 1);
      const [cx, cy] = axis === "x" ? [ax.X(v), oy] : [ox, ax.Y(v)];
      const tick = size * 0.3;
      parts.push(
        axis === "x"
          ? line(`${id}/xt${fmt(v, d)}`, cx - w / 2, cy - tick, w, 2 * tick, [0, 0, 0, 0], shown)
          : line(`${id}/yt${fmt(v, d)}`, cx - tick, cy - w / 2, 2 * tick, w, [0, 0, 0, 0], shown),
      );
      const text = fmt(v, d);
      const box = 3 * size;
      const boxW = Math.max(box, size * 0.7 * text.length);
      parts.push(
        <Box
          style={{
            position: "absolute",
            left: axis === "x" ? cx - boxW / 2 : cx - boxW - size * 0.5,
            top: axis === "x" ? cy + size * 0.5 : cy - box / 2,
            width: boxW,
            height: box,
            alignItems: axis === "x" ? "flex-start" : "center",
            justifyContent: axis === "x" ? "center" : "flex-end",
            opacity: shown,
          }}
        >
          <Math key={`${id}/${axis}n${fmt(v, d)}`} tex={text} style={{ fontSize: size, color: color.muted }} />
        </Box>,
      );
    }
  };
  numbers("x");
  numbers("y");
  const title = { opacity: clamp(q * 3 - 2, 0, 1), color: color.text };
  if (xTitle) parts.push(tag((ax.X(ax.x[0]) + ax.X(ax.x[1])) / 2, ax.Y(ax.y[0]) + 1.9 * size, [0, 1], xTitle, `${id}/xtitle`, title, size));
  if (yTitle) parts.push(tag(ox, ax.Y(ax.y[1]) - 1.5 * over, [0, -1], yTitle, `${id}/ytitle`, title, size));
  return (
    <Box style={{ width: ax.w, height: ax.h }}>
      {parts}
      {children}
    </Box>
  );
};

/**
 * The visible pieces of `f` over [lo, hi] as lists of px points. A piece
 * ends where f leaves the y range (at the range's edge, found by bisection)
 * or is not finite, and breaks at a jump or an asymptote (1/x, tan, floor),
 * so no vertical spikes are drawn.
 */
const trace = (ax, f, lo, hi, samples) => {
  const m = (ax.y[1] - ax.y[0]) * 0.02;
  const seen = (x) => f(x) >= ax.y[0] - m && f(x) <= ax.y[1] + m;
  // The last x that is seen between a seen x and an unseen one.
  const edge = (seenX, unseenX) => {
    for (let k = 0; k < 24; k++) {
      const mid = (seenX + unseenX) / 2;
      if (seen(mid)) seenX = mid;
      else unseenX = mid;
    }
    return seenX;
  };
  // A jump is a gap between two seen x that bisection cannot close.
  const jump = (a, b) => {
    const first = Math.abs(f(b) - f(a));
    for (let k = 0; k < 8; k++) {
      const mid = (a + b) / 2;
      if (!seen(mid)) return true;
      if (Math.abs(f(mid) - f(a)) > Math.abs(f(b) - f(mid))) b = mid;
      else a = mid;
    }
    return Math.abs(f(b) - f(a)) > first / 2;
  };
  const pieces = [];
  let piece = null;
  const add = (x) => piece.push([ax.X(x), ax.Y(clamp(f(x), ax.y[0] - m, ax.y[1] + m))]);
  let [prev, prevSeen] = [lo, seen(lo)];
  if (prevSeen) {
    piece = [];
    pieces.push(piece);
    add(lo);
  }
  for (let i = 1; i <= samples; i++) {
    const x = lo + ((hi - lo) * i) / samples;
    const now = seen(x);
    if (now && prevSeen && jump(prev, x)) {
      piece = [];
      pieces.push(piece);
    } else if (now && !prevSeen) {
      piece = [];
      pieces.push(piece);
      add(edge(x, prev));
    } else if (!now && prevSeen) {
      add(edge(prev, x));
    }
    if (now) add(x);
    [prev, prevSeen] = [x, now];
  }
  return pieces.filter((p) => p.length > 1);
};

/** The point a fraction q of the stroke's length along `pieces`, and whether the stroke is rising there. */
const tip = (pieces, q) => {
  const lengths = pieces.map((p) => p.slice(1).map(([px, py], i) => Math.hypot(px - p[i][0], py - p[i][1])));
  let left = q * lengths.flat().reduce((s, l) => s + l, 0);
  let end = pieces[0]?.[0] ?? [0, 0];
  let rising = true;
  pieces.forEach((p, k) =>
    lengths[k].forEach((l, i) => {
      if (left <= 0) return;
      const r = Math.min(1, left / l);
      end = [lerp(p[i][0], p[i + 1][0], r), lerp(p[i][1], p[i + 1][1], r)];
      rising = p[i + 1][1] <= p[i][1];
      left -= l;
    }),
  );
  return { end, rising };
};

/** Rough px size [w, h] of `tex` set at font size `size`. */
const texBox = (tex, size) => [
  (tex.replace(/\\[A-Za-z]+/g, "xxx").replace(/[ {}^_]/g, "").length * 0.625 + 0.6 * (tex.match(/[=+<>-]/g) ?? []).length) * size,
  size * (/\\t?frac/.test(tex) ? 2.1 : 1.25),
];

/** The name of a stroke ending at `end`: above-right of it, or left of it (on the free side) when it would pass `room` px right of the axes. */
const endLabel = (ax, end, rising, label, width, size, room) => {
  const [w, h] = texBox(label, size);
  const place = end[0] + w > ax.w + room ? [-1, rising ? -1 : 1] : [1, -1];
  return { place, px: end[0] + place[0] * width, py: end[1] + place[1] * 2 * width, w, h };
};

/** The [left, top, right, bottom] px of a `w` by `h` label whose [ux, uy] corner is at (px, py). */
const boxOf = (px, py, [ux, uy], w, h) => {
  const [l, t] = [ux > 0 ? px : ux < 0 ? px - w : px - w / 2, uy > 0 ? py : uy < 0 ? py - h : py - h / 2];
  return [l, t, l + w, t + h];
};

/** How many 6 px steps of the polylines `pieces` lie inside `box`. */
const through = (box, pieces) => {
  const inside = (x, y) => x >= box[0] && x <= box[2] && y >= box[1] && y <= box[3];
  return pieces.reduce(
    (sum, p) =>
      sum +
      p.reduce((n, [x, y], j) => {
        if (!j) return n + (inside(x, y) ? 1 : 0);
        const [x0, y0] = p[j - 1];
        const steps = Math.ceil(Math.hypot(x - x0, y - y0) / 6);
        return n + Array.from({ length: steps }, (_, k) => inside(lerp(x0, x, (k + 1) / steps), lerp(y0, y, (k + 1) / steps))).filter(Boolean).length;
      }, 0),
    0,
  );
};

/**
 * The [dx, dy] px offsets (`nudge` for `Plot`) that keep the curves' end
 * labels clear of each other and of every curve: `curves` is `[{ f, label }]`,
 * the first label stays put and each later one moves the least (up or down,
 * then right) that clears the earlier labels and the curves while staying in
 * the plot; `room` is as for `Plot`.
 */
const endLabels = (ax, curves, size = 48, width = 7, room = ax.pad) => {
  const lines = curves.map((c) => trace(ax, c.f, ax.x[0], ax.x[1], 240));
  const taken = [];
  return curves.map((c, i) => {
    if (!c.label) return [0, 0];
    const { end, rising } = tip(lines[i], 1);
    const { place, px, py, w, h } = endLabel(ax, end, rising, c.label, width, size, room);
    const grown = (bx, m) => [bx[0] - m, bx[1] - m, bx[2] + m, bx[3] + m];
    const cost = ([dx, dy]) => {
      const bx = boxOf(px + dx, py + dy, place, w, h);
      const out = bx[1] < -ax.pad / 2 || bx[3] > ax.Y(ax.y[0]) || bx[0] < 0 || bx[2] > ax.w + room;
      const overlap = (o) => Math.max(0, Math.min(bx[2], o[2]) - Math.max(bx[0], o[0]) + 6) * Math.max(0, Math.min(bx[3], o[3]) - Math.max(bx[1], o[1]) + 6);
      const hit = taken.reduce((sum, o) => sum + (overlap(o) > 36 ? overlap(o) / 300 : 0), 0);
      return (out ? 1000 : 0) + hit + lines.reduce((sum, l, j) => sum + (j === i ? 0.25 : 1) * through(grown(bx, 4), l), 0);
    };
    const ups = [0, ...Array.from({ length: 16 }, (_, k) => (k % 2 ? 1 : -1) * Math.ceil((k + 1) / 2) * h * 0.4)];
    const tries = [0, 0.3, 0.6, 0.9].flatMap((r) => ups.map((dy) => [r * w, dy]));
    const best = cost([0, 0]) < 2 ? [0, 0] : tries.reduce((pick, d) => (cost(d) < cost(pick) ? d : pick));
    taken.push(boxOf(px + best[0], py + best[1], place, w, h));
    return best;
  });
};

/**
 * `f` (any function of x) stroked from `from` to `to` (default the x range),
 * drawn on from `at` over `dur`; it breaks where f leaves the y range, jumps
 * or is not finite. `label` (TeX, font `size`) arrives at the drawn end, `nudge`
 * px over (see `endLabels`), left of it if it would pass `room` px right of
 * the axes; `head` rides it as a dot.
 */
const Plot = ({ ax, f, from, to, at = 0, dur, color, width = 7, label, head, nudge = [0, 0], key, size = 48, room = ax.pad, samples = 240 }, { t, theme }) => {
  const { motion } = theme;
  const pieces = trace(ax, f, from ?? ax.x[0], to ?? ax.x[1], samples);
  const d = pieces.map((p) => p.map(([px, py], i) => `${i ? "L" : "M"}${px.toFixed(2)} ${py.toFixed(2)}`).join(" ")).join(" ");
  const q = easeInOutCubic(clamp((t - at) / (dur ?? 2 * motion.duration.slow), 0, 1));
  const ink = color ?? theme.color.accent;
  const { end, rising } = tip(pieces, q);
  const name = label ? endLabel(ax, end, rising, label, width, size, room) : null;
  const r = width * 1.6;
  return [
    <Path
      key={key}
      d={d || "M0 0"}
      viewBox={[0, 0, ax.w, ax.h]}
      stroke={{ width, color: ink, cap: "round", join: "round" }}
      trim={[0, q]}
      style={layer(ax)}
    />,
    head && q > 0 ? (
      <Box
        key={key ? `${key}/head` : undefined}
        style={{ position: "absolute", left: end[0] - r, top: end[1] - r, width: 2 * r, height: 2 * r, radius: r, fill: ink }}
      />
    ) : null,
    name
      ? tag(name.px + nudge[0], name.py + nudge[1], name.place, label, key ? `${key}/label` : undefined, { opacity: clamp(q * 8 - 7, 0, 1), color: ink }, size)
      : null,
  ];
};

/**
 * A point at (x, y) that pops in at `at`, with dashed guides to the axes and
 * `label` (TeX) beside it: `place` is the [ux, uy] direction it sits in
 * (default [1, 0], to the right).
 */
const Dot = ({ ax, x, y, at = 0, color, label, guides = true, place = [1, 0], key, size = 40 }, { t, theme }) => {
  const { motion } = theme;
  const ink = color ?? theme.color.accent;
  const pop = t < at ? 0 : spring(t - at, motion.spring.snappy);
  const fade = clamp((t - at) / motion.duration.base, 0, 1);
  const [px, py] = [ax.X(x), ax.Y(y)];
  const r = size * 0.4;
  const dash = size / 4;
  const [ux, uy] = place.map(Math.sign);
  return [
    guides ? (
      <Path
        d={`M${px} ${ax.oy} L${px} ${py} L${ax.ox} ${py}`}
        viewBox={[0, 0, ax.w, ax.h]}
        stroke={{ width: size / 14, color: theme.color.muted, dash: [dash, dash] }}
        trim={[0, fade]}
        style={layer(ax)}
      />
    ) : null,
    <Box
      key={key}
      style={{
        position: "absolute",
        left: px - r,
        top: py - r,
        width: 2 * r,
        height: 2 * r,
        radius: r,
        fill: ink,
        scale: pop,
        opacity: Math.min(1, 3 * pop),
      }}
    />,
    label ? tag(px + ux * 2 * r, py + uy * 2 * r, [ux, uy], label, key ? `${key}/label` : undefined, { opacity: fade, color: ink }, size) : null,
  ];
};

/**
 * The least-squares line through (xs, ys): slope m, intercept b, and r squared
 * (null for a flat line, which explains nothing). Null when the points do not
 * span two x values.
 */
const leastSquares = (xs, ys) => {
  const n = xs.length;
  const [mx, my] = [xs.reduce((s, v) => s + v, 0) / n, ys.reduce((s, v) => s + v, 0) / n];
  const sxx = xs.reduce((s, v) => s + (v - mx) ** 2, 0);
  const syy = ys.reduce((s, v) => s + (v - my) ** 2, 0);
  const sxy = xs.reduce((s, v, i) => s + (v - mx) * (ys[i] - my), 0);
  if (n < 2 || sxx < 1e-12) return null;
  const flat = syy < 1e-12;
  return { m: flat ? 0 : sxy / sxx, b: flat ? my : my - (sxy / sxx) * mx, r2: flat ? null : (sxy * sxy) / (sxx * syy) };
};

/** The part of the segment p0 -> p1 inside the box [x0, y0, x1, y1] (Liang-Barsky), or null when none is. */
const clipLine = ([ax_, ay], [bx, by], [x0, y0, x1, y1]) => {
  let [lo, hi] = [0, 1];
  const [dx, dy] = [bx - ax_, by - ay];
  for (const [p, q] of [[-dx, ax_ - x0], [dx, x1 - ax_], [-dy, ay - y0], [dy, y1 - ay]]) {
    if (p === 0) {
      if (q < 0) return null;
    } else {
      const r = q / p;
      if (p < 0) lo = Math.max(lo, r);
      else hi = Math.min(hi, r);
    }
  }
  return lo < hi ? [[ax_ + lo * dx, ay + lo * dy], [ax_ + hi * dx, ay + hi * dy]] : null;
};

/**
 * Points (xs[i], ys[i]) popping in one after another from `at`, `each` s
 * apart, keyed `<key>/p<i>`. With `fit`, the least-squares line is drawn on
 * from `fitAt` over `fitDur` as `<key>/fit` (cut at the plot's edges), with
 * its equation and r squared as `<key>/fit/label` in the empty top corner.
 * Fewer than two distinct x values have no line; a flat one has no r squared.
 */
const Scatter = ({ ax, xs, ys, at = 0, each = 0.2, color, fit, fitAt = 0, fitDur, fitColor, key = "pts", size = 28 }, { t, theme }) => {
  if (xs.length !== ys.length) throw new Error(`Scatter: px has ${xs.length} values but py has ${ys.length}; they must pair up`);
  const { motion, color: palette } = theme;
  const ink = color ?? palette.accent;
  const out = xs.map((x, i) => <Dot ax={ax} x={x} y={ys[i]} at={at + i * each} color={ink} guides={false} size={size} key={`${key}/p${i}`} />);
  const ls = fit ? leastSquares(xs, ys) : null;
  if (!ls) return out;
  const { m, b, r2 } = ls;
  const [lo, hi] = [Math.min(...xs), Math.max(...xs)];
  const [a, c] = [lo - 0.04 * (ax.x[1] - ax.x[0]), hi + 0.04 * (ax.x[1] - ax.x[0])];
  const seg = clipLine([ax.X(a), ax.Y(m * a + b)], [ax.X(c), ax.Y(m * c + b)], [ax.X(ax.x[0]), ax.Y(ax.y[1]), ax.X(ax.x[1]), ax.Y(ax.y[0])]);
  const q = easeInOutCubic(clamp((t - fitAt) / (fitDur ?? 2 * motion.duration.slow), 0, 1));
  const line = fitColor ?? palette.negative;
  const sign = b < 0 ? "-" : "+";
  const tex = r2 === null ? `y = ${b.toFixed(2)}` : `y = ${m.toFixed(2)}x ${sign} ${Math.abs(b).toFixed(2)} \\quad r^2 = ${r2.toFixed(2)}`;
  // The equation sits in the top corner the line leans away from, or the other one if the line runs through it.
  const [lw, lh] = texBox(tex, (size * 10) / 7);
  const corners = (m >= 0 ? [-1, 1] : [1, -1]).map((side) => [side < 0 ? ax.X(ax.x[0]) + size : ax.X(ax.x[1]) - size, ax.Y(ax.y[1]) + size * 0.5, [-side, 1]]);
  const [cx, cy, lean] = corners.find((c) => !seg || !through(boxOf(c[0], c[1], c[2], lw, lh), [seg])) ?? corners[0];
  out.push(
    seg ? (
      <Path
        key={`${key}/fit`}
        d={`M${seg[0].join(" ")} L${seg[1].join(" ")}`}
        viewBox={[0, 0, ax.w, ax.h]}
        stroke={{ width: size / 4, color: line, cap: "round" }}
        trim={[0, q]}
        style={layer(ax)}
      />
    ) : null,
    tag(cx, cy, lean, tex, `${key}/fit/label`, { opacity: clamp(q * 3 - 2, 0, 1), color: line }, (size * 10) / 7),
  );
  return out;
};

/**
 * The region between `f` and `g` (default the x-axis) over [from, to],
 * sweeping in left to right from `at` over `dur`, keyed `<key>`. `label`
 * (TeX) arrives above the region once it is full, as `<key>/label`.
 */
const Area = ({ ax, f, g = () => 0, from, to, at = 0, dur, color, label, key = "area", size = 40, samples = 160 }, { t, theme }) => {
  const { motion } = theme;
  const [a, b] = [from ?? ax.x[0], to ?? ax.x[1]];
  const q = easeInOutCubic(clamp((t - at) / (dur ?? 2 * motion.duration.slow), 0, 1));
  const ink = color ?? theme.color.accent;
  const base = clamp(0, ax.y[0], ax.y[1]);
  const Y = (h, x) => ax.Y(Number.isFinite(h(x)) ? clamp(h(x), ax.y[0], ax.y[1]) : base);
  const xs = Array.from({ length: samples + 1 }, (_, i) => a + ((a + (b - a) * q - a) * i) / samples);
  const edge = (h) => xs.map((x) => `${ax.X(x).toFixed(2)} ${Y(h, x).toFixed(2)}`);
  const d = `M${edge(f).join(" L")} L${edge(g).reverse().join(" L")} Z`;
  const full = Array.from({ length: samples + 1 }, (_, i) => a + ((b - a) * i) / samples);
  const top = Math.min(...full.map((x) => Y(f, x)), ...full.map((x) => Y(g, x)));
  return [
    <Path key={key} d={d} viewBox={[0, 0, ax.w, ax.h]} fill={ink} style={{ ...layer(ax), opacity: 0.3 }} />,
    label ? tag(ax.X(a) + size * 0.4, top - size * 0.4, [1, -1], label, `${key}/label`, { opacity: clamp(q * 3 - 2, 0, 1), color: ink }, size) : null,
  ];
};

/**
 * Rectangles under `f` over [from, to] (left-hand heights), the first
 * `counts[0]` popping in from `at`, `each` s apart; then each next count
 * splits them from `refineAt`, `gap` s after one another: a rectangle keeps
 * its left edge and its new neighbours grow out of its right edge, so the
 * pieces always tile the span without overlap. Keyed `<key>/r<i>` for the
 * last count's rectangles; an earlier count's rectangles are the same
 * pieces, the ones that split waiting with no width at the right of the one
 * that is their first. `label` (TeX) sits above them as `<key>/label`.
 */
const Riemann = ({ ax, f, from, to, counts, at = 0, refineAt, each = 0.12, gap = 1.4, color, label, key = "rects", size = 40 }, { t, theme }) => {
  const { motion } = theme;
  const [a, b] = [from ?? ax.x[0], to ?? ax.x[1]];
  const ink = color ?? theme.color.accent;
  const N = counts[counts.length - 1];
  const base = ax.Y(clamp(0, ax.y[0], ax.y[1]));
  const first = refineAt ?? at + counts[0] * each + 0.8;
  // Rectangle i as part of the counts[k] split: left px, right px, top px.
  const part = (k, i) => {
    const n = counts[k];
    const c = Math.floor((i * n) / N);
    const x = a + ((b - a) * c) / n;
    const y = f(x);
    const right = ax.X(x + (b - a) / n);
    const top = ax.Y(Number.isFinite(y) ? clamp(y, ax.y[0], ax.y[1]) : 0);
    return [Math.ceil((c * N) / n) === i ? ax.X(x) : right, right, top];
  };
  const top = Math.min(...Array.from({ length: N }, (_, i) => part(counts.length - 1, i)[2]));
  const rects = Array.from({ length: N }, (_, i) => {
    let p = part(0, i);
    for (let k = 1; k < counts.length; k++) {
      const q = easeInOutCubic(clamp((t - first - (k - 1) * gap) / (gap * 0.75), 0, 1));
      p = part(k, i).map((v, j) => lerp(p[j], v, q));
    }
    const pop = easeOutCubic(clamp((t - at - Math.floor((i * counts[0]) / N) * each) / motion.duration.base, 0, 1));
    const edge = base + (p[2] - base) * pop;
    const wide = p[1] - p[0];
    return (
      <Box
        key={`${key}/r${i}`}
        style={{
          position: "absolute",
          left: p[0],
          top: Math.min(edge, base),
          width: wide,
          height: Math.abs(edge - base),
          fill: ink,
          border: { width: size * 0.075, color: ink },
          opacity: 0.45 * clamp(wide / 4, 0, 1) * Math.min(1, 3 * pop),
        }}
      />
    );
  });
  const shown = clamp((t - at - counts[0] * each) / motion.duration.base, 0, 1);
  return [...rects, label ? tag(ax.X(a) + size * 0.4, top - size * 0.4, [1, -1], label, `${key}/label`, { opacity: shown, color: ink }, size) : null];
};

/**
 * The corner of the plot area (inset `gap` px) that the curve `f` stays
 * furthest from, for a `w` by `h` px label: [px, py, [ux, uy]] for `tag`.
 */
const quiet = (ax, f, w, h, gap) => {
  const [x0, x1, y0, y1] = [ax.X(ax.x[0]) + gap, ax.X(ax.x[1]) - gap, ax.Y(ax.y[1]) + gap, ax.Y(ax.y[0]) - gap];
  const dots = Array.from({ length: 80 }, (_, i) => ax.x[0] + ((ax.x[1] - ax.x[0]) * i) / 79)
    .filter((x) => f(x) >= ax.y[0] && f(x) <= ax.y[1])
    .map((x) => [ax.X(x), ax.Y(f(x))]);
  const clearance = ([px, py, [ux, uy]]) => {
    const [l, t] = [ux > 0 ? px : px - w, uy > 0 ? py : py - h];
    return Math.min(...dots.map(([cx, cy]) => Math.hypot(Math.max(l - cx, 0, cx - l - w), Math.max(t - cy, 0, cy - t - h))));
  };
  const corners = [[x0, y0, [1, 1]], [x1, y0, [-1, 1]], [x0, y1, [1, -1]], [x1, y1, [-1, -1]]];
  return corners.reduce((best, c) => (clearance(c) > clearance(best) ? c : best));
};

/**
 * A line tangent to `f` at x0, or through (x0, f(x0)) and (x0 + h, f(x0 + h))
 * when `h` is set (a secant), drawn on from `at`. From `moveAt` over `dur` the
 * point glides to x1 and h shrinks to h1 (default 0, the tangent). Keyed
 * `<key>` (the line), `<key>/dot`, `<key>/dot2` (the secant's second point)
 * and `<key>/label` (the slope, with h for a secant, in the corner of the
 * plot the curve keeps clear of). `reach` is the line's half-length in x.
 */
const Tangent = ({ ax, f, x0, x1 = x0, h = 0, h1 = 0, at = 0, moveAt, dur, reach, color, label = true, key = "tan", size = 40 }, { t, theme }) => {
  const { motion } = theme;
  const p = easeInOutCubic(clamp((t - (moveAt ?? at + motion.duration.slow)) / (dur ?? 2 * motion.duration.slow), 0, 1));
  const [x, hh] = [lerp(x0, x1, p), lerp(h, h1, p)];
  const y = f(x);
  if (!Number.isFinite(y)) return [];
  const span = ax.x[1] - ax.x[0];
  const slope = hh > 1e-6 ? (f(x + hh) - y) / hh : (f(x + span * 1e-5) - f(x - span * 1e-5)) / (2e-5 * span);
  let [lo, hi] = [x - (reach ?? 0.12 * span), x + hh + (reach ?? 0.12 * span)];
  if (Math.abs(slope) > 1e-9) {
    const [u, v] = [(ax.y[0] - y) / slope + x, (ax.y[1] - y) / slope + x];
    [lo, hi] = [Math.max(lo, Math.min(u, v)), Math.min(hi, Math.max(u, v))];
  }
  if (hi <= lo) return [];
  const q = clamp((t - at) / motion.duration.slow, 0, 1);
  const ink = color ?? theme.color.warning;
  const at_ = (v) => [ax.X(v), ax.Y(y + slope * (v - x))];
  const r = size * 0.3;
  const dot = (k, [px, py], opacity) => (
    <Box key={k} style={{ position: "absolute", left: px - r, top: py - r, width: 2 * r, height: 2 * r, radius: r, fill: ink, opacity }} />
  );
  const m = Math.abs(slope) < 0.005 ? 0 : slope;
  const step = hh < 0.005 ? "h \\to 0" : `h = ${hh.toFixed(2)}`;
  const tex = `${h > 0 ? `${step} \\quad ` : ""}m = ${m.toFixed(2)}`;
  const end = at_(hi);
  return [
    <Path
      key={key}
      d={`M${at_(lo).join(" ")} L${end.join(" ")}`}
      viewBox={[0, 0, ax.w, ax.h]}
      stroke={{ width: size / 7, color: ink, cap: "round" }}
      trim={[0, q]}
      style={layer(ax)}
    />,
    dot(`${key}/dot`, [ax.X(x), ax.Y(y)], q),
    h > 0 ? dot(`${key}/dot2`, [ax.X(x + hh), ax.Y(y + slope * hh)], q * clamp(hh / (0.02 * span), 0, 1)) : null,
    label ? tag(...quiet(ax, f, size * 10, size * 1.6, size), tex, `${key}/label`, { opacity: q, color: ink }, size) : null,
  ];
};

export default (props, { theme, size }) => {
  const { x, y, fns, tones, labels, marks, px, py, shade, rects, tangent, tone } = props;
  const { color, space } = theme;
  // The 1300 x 760 plot and its 96 px of numbers shrink with the frame, the plot taller when the frame is.
  const [availW, availH] = [size.width - 2 * space.xl, size.height - 2 * space.xl];
  const k = Math.min(1, availW / 1492, availH / 952);
  const [pad, width] = [96 * k, 1300 * k];
  const height = Math.max(760 * k, Math.min(availH - 2 * pad, width));
  const ax = axes({ x, y, origin: props.origin, width, height, pad });
  const fs = fns.map(formula);
  const palette = [tone, color.warning, color.positive, color.negative];
  const f = fs[0];
  const room = (size.width - ax.w) / 2;
  const nudges = endLabels(ax, fs.map((g, i) => ({ f: g, label: labels[i] })), 48 * k, 7 * k, room);
  // A mark's label sits above the curve: up-left on a rising one, up-right on a falling one, above a flat one.
  const placed = (m) => {
    const slope = (f(m + 1e-4) - f(m - 1e-4)) * (x[1] - x[0]) / (2e-4 * (y[1] - y[0]));
    return Math.abs(slope) < 0.05 ? [0, -1] : [slope > 0 ? -1 : 1, -1];
  };
  return (
    <Stack justify="center" align="center" style={{ flexGrow: 1, fill: color.bg }}>
      <Axes ax={ax} at={props.at} size={40 * k} xTitle={props.xTitle} yTitle={props.yTitle}>
        {shade && rects.length ? (
          <Riemann ax={ax} f={f} from={shade[0]} to={shade[1]} counts={rects} at={props.shadeAt} color={props.shadeTone ?? tone} label={props.shadeLabel} size={40 * k} key="ax/rects" />
        ) : null}
        {shade && !rects.length ? (
          <Area
            ax={ax}
            f={f}
            g={props.shadeVs ? formula(props.shadeVs) : undefined}
            from={shade[0]}
            to={shade[1]}
            at={props.shadeAt}
            color={props.shadeTone ?? tone}
            label={props.shadeLabel}
            size={40 * k}
            key="ax/area"
          />
        ) : null}
        {fs.map((g, i) => (
          <Plot
            ax={ax}
            f={g}
            at={props.plotAt + i * 0.6}
            color={tones[i] ?? palette[i % palette.length]}
            width={7 * k}
            size={48 * k}
            room={room}
            label={labels[i]}
            nudge={nudges[i]}
            head={props.head && i === 0}
            key={i ? `curve${i}` : "curve"}
          />
        ))}
        {tangent.length ? (
          <Tangent ax={ax} f={f} x0={tangent[0]} x1={tangent[1] ?? tangent[0]} h={props.secant} at={props.tangentAt} size={40 * k} key="ax/tangent" />
        ) : null}
        {px.length ? <Scatter ax={ax} xs={px} ys={py} at={props.pointsAt} color={tone} fit={props.fit} fitAt={props.fitAt} size={28 * k} key="ax/pts" /> : null}
        {marks.map((m, i) => (
          <Dot ax={ax} x={m} y={f(m)} at={props.markAt + i * 0.3} color={tones[0] ?? tone} label={props.markLabels?.[i] ?? `(${fmt(m)}, ${fmt(f(m))})`} place={props.markPlace ?? placed(m)} size={40 * k} key={`m${i}`} />
        ))}
      </Axes>
    </Stack>
  );
};
