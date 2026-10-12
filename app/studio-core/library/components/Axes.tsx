// Axes and plot: `axes()` maps data to px, `Axes` draws the two axes on
// with their ticks and numbers, and inside it `Plot` draws a function on as
// a stroke and `Dot` marks a point, with guides to the axes. Keys: the axes
// `<id>/x`, `<id>/y`, numbers `<id>/xn<v>`, `<id>/yn<v>`; a Plot or Dot
// takes its own `key` (a Dot's label `<key>/label`). Copy `axes`, `Axes`,
// `Plot` and `Dot` into a scene component to compose them.
export const props = {
  x: list(num()).default([-1, 4, 1]),
  y: list(num()).default([-1, 10, 2]),
  fn: oneOf(["x^2", "\\sqrt{x}", "\\sin x", "2^x", "\\frac{1}{x}"]).default("x^2"),
  // The axes draw on from here; the curve from plotAt.
  at: num().default(0.2),
  plotAt: num().default(1.4),
  // x values marked on the curve, with guides and coordinates, from markAt.
  marks: list(num()).default([]),
  markAt: num().default(3),
  // A dot rides the curve's drawn end.
  head: bool().default(false),
  tone: token("color").default("accent"),
};

export const examples = {
  parabola: { x: [-1, 4, 1], y: [-1, 10, 2], fn: "x^2", marks: [1, 2, 3], markAt: 3 },
  sine: { x: [0, 7, 1], y: [-1.5, 1.5, 0.5], fn: "\\sin x", head: true },
};

const FNS = {
  "x^2": (x) => x * x,
  "\\sqrt{x}": (x) => Math.sqrt(x),
  "\\sin x": (x) => Math.sin(x),
  "2^x": (x) => 2 ** x,
  "\\frac{1}{x}": (x) => 1 / x,
};

const fmt = (v) => String(Number(v.toFixed(2)));

/** Data -> px for an axes `width` by `height` px, `pad` px round it for numbers. */
const axes = ({ x, y, width = 1300, height = 760, pad = 96 }) => ({
  x,
  y,
  pad,
  w: width + 2 * pad,
  h: height + 2 * pad,
  X: (v) => pad + ((v - x[0]) / (x[1] - x[0])) * width,
  Y: (v) => pad + height - ((v - y[0]) / (y[1] - y[0])) * height,
});

/** A full-box absolute layer, so children draw in the axes' px. */
const layer = (ax) => ({ position: "absolute", left: 0, top: 0, width: ax.w, height: ax.h });

/** The axes through 0 (or the low end), drawn on from `at` over `dur`, numbered every step but 0. */
const Axes = ({ ax, at = 0, dur, id = "ax", size = 40, children }, { t, theme }) => {
  const { color, motion } = theme;
  const q = easeInOutCubic(clamp((t - at) / (dur ?? 2 * motion.duration.slow), 0, 1));
  const w = size / 10;
  const ox = ax.X(clamp(0, ax.x[0], ax.x[1]));
  const oy = ax.Y(clamp(0, ax.y[0], ax.y[1]));
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
    for (let k = Math.ceil(lo / step); k * step <= hi + 1e-9; k++) {
      const v = k * step;
      if (Math.abs(v) < 1e-9) continue;
      const along = axis === "x" ? (ax.X(v) - ax.X(lo)) / (ax.X(hi) - ax.X(lo)) : (ax.Y(lo) - ax.Y(v)) / (ax.Y(lo) - ax.Y(hi));
      const shown = clamp((q - along) * 8 + 1, 0, 1);
      const [cx, cy] = axis === "x" ? [ax.X(v), oy] : [ox, ax.Y(v)];
      const tick = size * 0.3;
      parts.push(
        axis === "x"
          ? line(`${id}/xt${fmt(v)}`, cx - w / 2, cy - tick, w, 2 * tick, [0, 0, 0, 0], shown)
          : line(`${id}/yt${fmt(v)}`, cx - tick, cy - w / 2, 2 * tick, w, [0, 0, 0, 0], shown),
      );
      const box = 3 * size;
      parts.push(
        <Box
          style={{
            position: "absolute",
            left: axis === "x" ? cx - box / 2 : cx - box - size * 0.5,
            top: axis === "x" ? cy + size * 0.5 : cy - box / 2,
            width: box,
            height: box,
            alignItems: axis === "x" ? "flex-start" : "center",
            justifyContent: axis === "x" ? "center" : "flex-end",
            opacity: shown,
          }}
        >
          <Math key={`${id}/${axis}n${fmt(v)}`} tex={fmt(v)} style={{ fontSize: size, color: color.muted }} />
        </Box>,
      );
    }
  };
  numbers("x");
  numbers("y");
  return (
    <Box style={{ width: ax.w, height: ax.h }}>
      {parts}
      {children}
    </Box>
  );
};

/**
 * `f` stroked from `from` to `to` (default the x range), drawn on from `at`
 * over `dur`; it breaks where f leaves the y range or is not finite.
 * `label` (TeX) arrives at the drawn end; `head` rides it as a dot.
 */
const Plot = ({ ax, f, from, to, at = 0, dur, color, width = 7, label, head, key, samples = 240 }, { t, theme }) => {
  const { motion } = theme;
  const [lo, hi] = [from ?? ax.x[0], to ?? ax.x[1]];
  const margin = (ax.y[1] - ax.y[0]) * 0.02;
  const pts = [];
  let d = "";
  let pen = false;
  for (let i = 0; i <= samples; i++) {
    const x = lo + ((hi - lo) * i) / samples;
    const y = f(x);
    if (!Number.isFinite(y) || y < ax.y[0] - margin || y > ax.y[1] + margin) {
      pen = false;
      continue;
    }
    d += `${pen ? "L" : "M"}${ax.X(x).toFixed(2)} ${ax.Y(y).toFixed(2)} `;
    pts.push([ax.X(x), ax.Y(y)]);
    pen = true;
  }
  const q = clamp((t - at) / (dur ?? 2 * motion.duration.slow), 0, 1);
  const ink = color ?? theme.color.accent;
  const end = pts[Math.min(pts.length - 1, Math.round(easeInOutCubic(q) * (pts.length - 1)))] ?? [0, 0];
  const r = width * 1.6;
  return [
    <Path
      key={key}
      d={d || "M0 0"}
      viewBox={[0, 0, ax.w, ax.h]}
      stroke={{ width, color: ink, cap: "round", join: "round" }}
      trim={[0, easeInOutCubic(q)]}
      style={layer(ax)}
    />,
    head && q > 0 ? (
      <Box
        key={key ? `${key}/head` : undefined}
        style={{ position: "absolute", left: end[0] - r, top: end[1] - r, width: 2 * r, height: 2 * r, radius: r, fill: ink }}
      />
    ) : null,
    label ? (
      <Box style={{ position: "absolute", left: end[0] + width * 2, top: end[1] - 120, height: 100, alignItems: "flex-end", opacity: clamp(q * 3 - 2, 0, 1) }}>
        <Math key={key ? `${key}/label` : undefined} tex={label} style={{ fontSize: 48, color: ink }} />
      </Box>
    ) : null,
  ];
};

/** A point at (x, y) that pops in at `at`, with dashed guides to the axes and `label` (TeX) to its right. */
const Dot = ({ ax, x, y, at = 0, color, label, guides = true, key, size = 40 }, { t, theme }) => {
  const { motion } = theme;
  const ink = color ?? theme.color.accent;
  const pop = t < at ? 0 : spring(t - at, motion.spring.snappy);
  const fade = clamp((t - at) / motion.duration.base, 0, 1);
  const [px, py] = [ax.X(x), ax.Y(y)];
  const [ox, oy] = [ax.X(clamp(0, ax.x[0], ax.x[1])), ax.Y(clamp(0, ax.y[0], ax.y[1]))];
  const r = size * 0.4;
  const dash = size / 4;
  return [
    guides ? (
      <Path
        d={`M${px} ${oy} L${px} ${py} L${ox} ${py}`}
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
    label ? (
      <Box style={{ position: "absolute", left: px + 2 * r, top: py - size, height: 2 * size, alignItems: "center", opacity: fade }}>
        <Math key={key ? `${key}/label` : undefined} tex={label} style={{ fontSize: size, color: ink }} />
      </Box>
    ) : null,
  ];
};

export default ({ x, y, fn, at, plotAt, marks, markAt, head, tone }, { theme }) => {
  const ax = axes({ x, y });
  const f = FNS[fn];
  return (
    <Stack justify="center" align="center" style={{ flexGrow: 1, fill: theme.color.bg }}>
      <Axes ax={ax} at={at}>
        <Plot ax={ax} f={f} at={plotAt} color={tone} label={`y = ${fn}`} head={head} key="curve" />
        {marks.map((m, i) => (
          <Dot
            ax={ax}
            x={m}
            y={f(m)}
            at={markAt + i * 0.3}
            color={tone}
            label={`(${fmt(m)}, ${fmt(f(m))})`}
            key={`m${i}`}
          />
        ))}
      </Axes>
    </Stack>
  );
};
