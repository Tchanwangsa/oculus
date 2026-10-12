// NumberLine: a line from `from` to `to` that draws on left to right, its
// ticks and numbers arriving as the line passes them; points that pop in
// and glide along it; an interval drawn on as a bar. Ticks are keyed
// `<id>/t<value>`, numbers `<id>/n<value>` and points `<id>/p<i>` (the dot
// `/dot`, its name `/name`), so two NumberLines with one id morph. Copy
// `NumberLine` into a scene component to compose it.
export const props = {
  from: num().default(0),
  to: num().default(10),
  step: num().default(1),
  // Number every this many ticks.
  labelEvery: num().default(1),
  width: num().default(1500),
  // The line draws on from here.
  at: num().default(0.2),
  points: list(num()).default([]),
  // A TeX name per point, written above it.
  names: list(latex()).default([]),
  // Where each point glides to, from moveAt.
  moveTo: list(num()).optional(),
  moveAt: num().default(2),
  // An interval [from, to], drawn on from spanAt, with an optional label.
  span: list(num()).optional(),
  spanAt: num().default(2),
  spanLabel: latex().optional(),
  tone: token("color").default("accent"),
};

export const examples = {
  hop: {
    points: [3],
    names: ["a"],
    moveTo: [8],
    moveAt: 2.2,
    span: [3, 8],
    spanAt: 2.2,
    spanLabel: "+5",
  },
  signed: {
    from: -5,
    to: 5,
    points: [-3, 2],
    names: ["x", "y"],
    span: [-3, 2],
    spanAt: 2.4,
    spanLabel: "|x - y| = 5",
  },
};

const fmt = (v) => String(Number(v.toFixed(4)));

// Where a point is at t: `value`, then each [time, value] of `moves` in
// turn, gliding for `dur`.
const glide = (t, value, moves, dur) => {
  let v = value;
  for (const [at, to] of moves) {
    v = lerp(v, to, easeInOutCubic(clamp((t - at) / dur, 0, 1)));
  }
  return v;
};

/**
 * A number line `width` px long. `points`: `[{ value, name (TeX), at, moves:
 * [[time, value]], color }]`. `span`: `{ from, to, at, label (TeX), color }`.
 * `at` and `dur` draw the line on. Lays out `width + 4 * size` wide, the line
 * 180 px down; `size` is the numbers' font size.
 */
const NumberLine = (
  { from, to, step = 1, labelEvery = 1, width, at = 0, dur, points = [], span, id = "nl", tone, size = 56 },
  { t, theme },
) => {
  const { color, motion } = theme;
  const pad = 2 * size;
  const y = 180;
  const w = size / 9;
  const x = (v) => pad + ((v - from) / (to - from)) * width;
  const q = easeInOutCubic(clamp((t - at) / (dur ?? 2 * motion.duration.slow), 0, 1));
  // A tick shows as the line's head passes it, its number just after; the
  // head runs past the end so the last ones finish with the line.
  const head = q * (width + 3 * size);
  const passed = (v, lag) => clamp((head - (x(v) - pad)) / 60 - lag, 0, 1);
  const centred = (cx, top, w, kid, opacity, key) => (
    <Box
      key={key}
      style={{ position: "absolute", left: cx - w / 2, top, width: w, justifyContent: "center", opacity }}
    >
      {kid}
    </Box>
  );
  const ticks = [];
  const count = Math.round((to - from) / step);
  for (let k = 0; k <= count; k++) {
    const v = from + k * step;
    ticks.push(
      <Box
        key={`${id}/t${fmt(v)}`}
        style={{
          position: "absolute",
          left: x(v) - w / 2,
          top: y - size * 0.35,
          width: w,
          height: size * 0.7,
          radius: w / 2,
          fill: color.text,
          opacity: passed(v, 0),
        }}
      />,
    );
    if (k % labelEvery === 0) {
      ticks.push(
        centred(
          x(v),
          y + size * 0.6,
          4 * size,
          <Math key={`${id}/n${fmt(v)}`} tex={fmt(v)} style={{ fontSize: size, color: color.text }} />,
          passed(v, 0.6),
        ),
      );
    }
  }
  let bar = null;
  if (span) {
    const sp = easeInOutCubic(clamp((t - span.at) / motion.duration.slow, 0, 1));
    const [a, b] = [x(Math.min(span.from, span.to)), x(Math.max(span.from, span.to))];
    const ink = span.color ?? tone;
    bar = [
      <Box
        key={`${id}/span`}
        style={{
          position: "absolute",
          left: a,
          top: y - size * 0.25,
          width: b - a,
          height: size * 0.5,
          radius: size * 0.25,
          fill: mix(color.bg, ink, 0.6),
          clipInset: [0, 1 - sp, 0, 0],
        }}
      />,
      span.label
        ? centred(
            (a + b) / 2,
            y - size * 2.6,
            b - a + 4 * size,
            <Math key={`${id}/spanLabel`} tex={span.label} style={{ fontSize: size * 1.1, color: ink }} />,
            clamp((t - span.at) / motion.duration.slow - 0.5, 0, 1) * 2,
          )
        : null,
    ];
  }
  const marks = points.map((p, i) => {
    const pop = t < (p.at ?? 0) ? 0 : spring(t - (p.at ?? 0), motion.spring.snappy);
    const cx = x(glide(t, p.value, p.moves ?? [], motion.duration.slow));
    const ink = p.color ?? tone;
    const d = size * 0.8;
    return [
      <Box
        key={`${id}/p${i}/dot`}
        style={{
          position: "absolute",
          left: cx - d / 2,
          top: y - d / 2,
          width: d,
          height: d,
          radius: theme.radius.pill,
          fill: ink,
          border: { width: size * 0.14, color: color.bg },
          scale: pop,
          opacity: Math.min(1, 3 * pop),
        }}
      />,
      p.name
        ? centred(
            cx,
            y - size * 2.1,
            4 * size,
            <Math key={`${id}/p${i}/name`} tex={p.name} style={{ fontSize: size * 1.1, color: ink }} />,
            clamp(pop * 2, 0, 1),
          )
        : null,
    ];
  });
  return (
    <Box style={{ width: width + 2 * pad, height: y + 2.2 * size }}>
      {bar}
      <Box
        key={`${id}/line`}
        style={{
          position: "absolute",
          left: pad - size * 0.3,
          top: y - w / 2,
          width: width + size * 0.6,
          height: w,
          radius: w / 2,
          fill: color.text,
          clipInset: [0, 1 - q, 0, 0],
        }}
      />
      {ticks}
      {marks}
    </Box>
  );
};

export default (
  { from, to, step, labelEvery, width, at, points, names, moveTo, moveAt, span, spanAt, spanLabel, tone },
  { theme },
) => (
  <Stack justify="center" align="center" style={{ flexGrow: 1, fill: theme.color.bg }}>
    <NumberLine
      from={from}
      to={to}
      step={step}
      labelEvery={labelEvery}
      width={width}
      at={at}
      tone={tone}
      points={points.map((value, i) => ({
        value,
        name: names[i],
        at: at + 1.2 + 0.2 * i,
        moves: moveTo && moveTo[i] !== undefined ? [[moveAt, moveTo[i]]] : [],
      }))}
      span={span ? { from: span[0], to: span[1], at: spanAt, label: spanLabel } : undefined}
    />
  </Stack>
);
