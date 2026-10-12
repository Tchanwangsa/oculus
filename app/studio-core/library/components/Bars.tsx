// Bars: a bar chart or histogram. `bars()` maps data to px (nice value axis,
// bar slots, a sorted order that glides between states); `Bars` draws it:
// axis and gridlines, bars growing from the baseline on a spring, value
// labels counting up, states that glide into the next, a reference line, a
// highlighted subset, a difference arrow. Keys: bars `<id>/b<i>` (series 2
// and 3 `<id>/b<i>s2`), labels `<id>/l<i>` (histogram edges `<id>/e<j>`),
// values `<id>/v<i>`, ticks `<id>/n<value>`, gridlines `<id>/g<value>`,
// `<id>/base`, `<id>/line`, `<id>/linelabel`, `<id>/diff`. A key follows its
// bar when `sort` reorders, so one id morphs between scenes. Copy `bars`
// and `Bars` (and the small helpers above them) into a scene to compose.
export const props = {
  // One TeX label per bar; a histogram's are its n + 1 bin edges.
  labels: list(latex()),
  values: list(num()),
  // More series: grouped bars side by side, or `stacked`.
  values2: list(num()).optional(),
  values3: list(num()).optional(),
  // A TeX name per series, for the legend.
  names: list(latex()).default([]),
  // A colour per series, or per bar when there is one series.
  tones: list(token("color")).default([]),
  orientation: oneOf(["vertical", "horizontal"]).default("vertical"),
  // Bars touch; `labels` are the bin edges.
  histogram: bool().default(false),
  stacked: bool().default(false),
  // Rank bars by value (total, with several series): they glide to new ranks.
  sort: bool().default(false),
  // TeX after every number on the value axis and bar labels, e.g. "\\%".
  unit: latex().default(""),
  // The value axis tops out here, ticks every `step`; else both from the data.
  max: num().optional(),
  step: num().optional(),
  // Bar labels count up; `decimals` fixes their places (else from the data).
  valueLabels: bool().default(true),
  decimals: num().optional(),
  // The axis draws on from `at`, the bars grow from barsAt, each `each` after the last.
  at: num().default(0.2),
  barsAt: num().default(0.7),
  each: num().optional(),
  // State k+1 (to, to2, to3: one list of values per state) takes over at changeAt[k].
  to: list(list(num())).default([]),
  to2: list(list(num())).default([]),
  to3: list(list(num())).default([]),
  changeAt: list(num()).default([]),
  glide: num().optional(),
  // A reference line at this value (or, lineOn "category", this position
  // along the bars: a bar's index, a histogram's edge value), drawn on from lineAt.
  line: num().optional(),
  lineOn: oneOf(["value", "category"]).default("value"),
  lineLabel: latex().optional(),
  lineAt: num().default(2),
  lineTone: token("color").default("text"),
  // Bars (by index) recoloured from highlightAt while `dim` fades the others.
  highlight: list(num()).default([]),
  highlightAt: num().default(2.6),
  highlightTone: token("color").default("accentStrong"),
  dim: bool().default(true),
  // The gap between two bars (by index, first series) as an arrow and a label.
  diff: list(num()).default([]),
  diffLabel: latex().optional(),
  diffAt: num().default(3),
};

export const examples = {
  scores: {
    labels: ["40", "50", "60", "70", "80", "90", "100"],
    values: [4, 9, 21, 33, 24, 9],
    histogram: true,
    line: 72.4,
    lineOn: "category",
    lineLabel: "\\bar{x} = 72.4",
    lineAt: 2,
    highlight: [4, 5],
    highlightAt: 3.2,
  },
  growth: {
    labels: ["2019", "2020", "2021", "2022", "2023"],
    values: [2.3, -3.4, 5.9, 1.9, 2.5],
    values2: [1.6, -5.1, 6.5, 3.2, 0.4],
    names: ["\\text{Country A}", "\\text{Country B}"],
    unit: "\\%",
    decimals: 1,
    line: 2,
    lineLabel: "\\text{target}",
    lineAt: 2.6,
  },
  population: {
    labels: ["\\text{China}", "\\text{India}", "\\text{USA}", "\\text{Indonesia}", "\\text{Brazil}", "\\text{Nigeria}"],
    values: [1263, 1057, 282, 215, 176, 123],
    to: [[1410, 1450, 347, 283, 212, 232], [1317, 1670, 375, 317, 230, 377]],
    changeAt: [2, 3.8],
    orientation: "horizontal",
    sort: true,
    valueLabels: true,
    barsAt: 0.5,
  },
  sorting: {
    labels: [
      "\\text{bubble sort}",
      "\\text{insertion sort}",
      "\\text{merge sort}",
      "\\text{quicksort}",
    ],
    values: [500, 250, 9, 11],
    tones: ["negative", "warning", "positive", "positive"],
    unit: "\\text{k}",
    diff: [0, 2],
    diffLabel: "\\times 55",
    diffAt: 2.6,
  },
};

const fmt = (v) => String(Number(v.toFixed(4)));

const decimals = (v) => {
  const s = String(Number(v.toFixed(2)));
  const i = s.indexOf(".");
  return i < 0 ? 0 : s.length - i - 1;
};

/** Rough px width of `tex` set at font size `size` (its widest line). */
const texW = (tex, size) =>
  Math.max(
    ...tex.split("\\\\").map((line) => {
      const flat = line
        .replace(/\\(begin|end)\{[^}]*\}/g, "")
        .replace(/\\(text|mathrm|mathbf)\{/g, "")
        .replace(/\\[a-zA-Z]+/g, "x")
        .replace(/[{}^_$&]/g, "");
      let w = 0;
      for (const ch of flat) w += ch === "." || ch === "," ? 0.28 : ch === " " ? 0.3 : 0.56;
      return w * 1.21 * size;
    }),
  );

/** `\text{two words}` labels wider than `limit` px at `size` split over two lines. */
const wrap = (labels, limit, size) =>
  labels.map((tex) => {
    const m = /^\\text\{([^{}]*)\}$/.exec(tex.trim());
    if (!m || !m[1].includes(" ") || texW(tex, size) <= limit) return tex;
    const words = m[1].split(" ");
    let best = 1;
    for (let k = 1; k < words.length; k++) {
      const [a, b] = [words.slice(0, k).join(" ").length, words.slice(k).join(" ").length];
      const [c, d] = [words.slice(0, best).join(" ").length, words.slice(best).join(" ").length];
      if (Math.abs(a - b) < Math.abs(c - d)) best = k;
    }
    const [a, b] = [words.slice(0, best).join(" "), words.slice(best).join(" ")];
    return `\\begin{gathered}\\text{${a}}\\\\\\text{${b}}\\end{gathered}`;
  });

/** The smallest 1, 2 or 5 times a power of ten that puts `span` in at most six steps. */
const niceStep = (span) => {
  const p = 10 ** Math.floor(Math.log10(span / 6));
  for (const m of [1, 2, 5, 10]) if (span / (m * p) <= 6) return m * p;
  return 10 * p;
};

/**
 * Data -> px for a chart `width` by `height` px (margins for the numbers,
 * labels, legend and line label included). `series` is one list of n values
 * per series; `to[s][k]` the values of series s in state k + 1, taking over
 * at `changeAt[k]` over `glide` s. Returns the mapping: `V(v)` a value's px
 * along the value axis, `cross(p, s)` the [start, thickness] of the bar at
 * (fractional) slot p in series s, `rect`, `at(c, v)` a point, `catPos(x)`
 * a position along the bars, `pose(t)` the values and slots at t, plus the
 * ticks, the label texts and sizes. Unit and label fonts follow `size`.
 */
const bars = (o) => {
  const { labels, series, to = [], changeAt = [], histogram = false, stacked = false, unit = "", size = 40 } = o;
  const { width: W = 1700, height: H = 920, glide = 0.9, names = [], lineLabel } = o;
  const vertical = o.orientation !== "horizontal";
  const n = series[0].length;
  const S = series.length;
  const edges = histogram && labels.length === n + 1;
  const states = [series];
  for (let k = 0; k < changeAt.length; k++) states.push(series.map((_, s) => (to[s] && to[s][k]) || states[k][s]));
  const rankOf = (st) => {
    const total = st[0].map((_, i) => st.reduce((a, row) => a + row[i], 0));
    const rank = [];
    total
      .map((_, i) => i)
      .sort((a, b) => total[b] - total[a] || a - b)
      .forEach((i, r) => (rank[i] = r));
    return rank;
  };
  const ranks = states.map((st) => (o.sort && !histogram ? rankOf(st) : st[0].map((_, i) => i)));

  let hi = 0;
  let lo = 0;
  for (const st of states) {
    for (let i = 0; i < n; i++) {
      if (stacked) {
        let [up, down] = [0, 0];
        for (const row of st) {
          if (row[i] >= 0) up += row[i];
          else down += row[i];
        }
        [hi, lo] = [Math.max(hi, up), Math.min(lo, down)];
      } else {
        for (const row of st) [hi, lo] = [Math.max(hi, row[i]), Math.min(lo, row[i])];
      }
    }
  }
  const stepV = o.step ?? niceStep(hi - lo || 1);
  let top = o.max ?? Math.ceil(hi / stepV - 1e-9) * stepV;
  const bot = Math.floor(lo / stepV + 1e-9) * stepV;
  if (top <= bot) top = bot + stepV;
  const ticks = [];
  for (let k = Math.round(bot / stepV); k * stepV <= top + 1e-9; k++) ticks.push(Number((k * stepV).toFixed(6)));
  const dec = o.decimals ?? Math.min(2, Math.max(0, ...states.flat(2).map(decimals)));
  const valText = (v) => v.toFixed(dec) + unit;

  const catSize = size;
  const tickSize = size * 0.9;
  const gap = size * 0.6;
  const lineH = size * 1.7;
  const tickW = Math.max(...ticks.map((v) => texW(fmt(v) + unit, tickSize)));
  const valW0 = Math.max(texW(valText(hi), size * 0.9), texW(valText(lo), size * 0.9));
  const lineHoriz = o.line !== undefined && vertical === (o.lineOn !== "category");
  const lineLines = lineLabel ? lineLabel.split("\\\\").length : 1;
  const legendH = names.length ? size * 1.7 : 0;
  const lineRow = o.line !== undefined && lineLabel && !lineHoriz ? lineLines * lineH + gap * 0.5 : 0;

  // Margins: L, R, T, B px round the plot. Vertical bars: the numbers sit
  // left, the labels below; horizontal: the labels left, the numbers below.
  let [L, R, T, B] = [0, 0, 0, 0];
  let labelTex = labels;
  let catEff = catSize;
  let valEff = size * 0.9;
  let showVal = o.valueLabels !== false;
  const gapFrac = histogram ? 0 : 0.3;
  const group = (slot) => Math.min(slot * (1 - gapFrac), size * 5 * (stacked ? 1 : S));
  if (vertical) {
    L = tickW + gap * 1.4;
    R = gap;
    if (lineHoriz && lineLabel) R = Math.max(R, texW(lineLabel, size) + gap * 1.6);
    if (edges) {
      L = Math.max(L, texW(labels[0], catSize) / 2 + gap * 0.3);
      R = Math.max(R, texW(labels[n], catSize) / 2 + gap * 0.3);
    }
    const sw = (W - L - R) / n;
    const bt = group(sw) / (stacked ? 1 : S);
    const limit = sw * 0.94;
    labelTex = wrap(labels, limit, catSize);
    const widest = Math.max(...labelTex.map((l) => texW(l, catSize)));
    catEff = Math.max(catSize * 0.62, Math.min(catSize, (catSize * limit) / widest));
    if (!stacked) valEff = Math.min(valEff, (valEff * bt * 1.15) / valW0);
    if (valEff < size * 0.55 && !stacked) showVal = false;
    const lines = Math.max(...labelTex.map((l) => l.split("\\\\").length));
    T = legendH + lineRow + (showVal ? valEff * 1.9 : gap * 1.2);
    B = gap * 0.9 + lines * catEff * 1.6 + (showVal && lo < 0 ? valEff * 1.7 : 0);
  } else {
    T = legendH + lineRow + gap * 1.2;
    B = tickSize * 1.7 + gap;
    const wideLimit = W * 0.3;
    labelTex = wrap(labels, wideLimit, catSize);
    const widest = Math.max(...labelTex.map((l) => texW(l, catSize)));
    const sh = (H - T - B) / n;
    const lines = Math.max(...labelTex.map((l) => l.split("\\\\").length));
    catEff = Math.max(catSize * 0.62, Math.min(catSize, (catSize * wideLimit) / widest, (sh * 0.92) / (lines * 1.5)));
    L = Math.min(wideLimit, Math.max(...labelTex.map((l) => texW(l, catEff)))) + gap * 1.4;
    const bt = group(sh) / (stacked ? 1 : S);
    if (!stacked) valEff = Math.min(valEff, bt / 1.45);
    if (valEff < size * 0.55 && !stacked) showVal = false;
    R = Math.max(gap, tickW / 2 + gap * 0.3, showVal && !stacked ? valW0 * (valEff / (size * 0.9)) + gap * 1.8 : 0);
    if (lineHoriz && lineLabel) R = Math.max(R, texW(lineLabel, size) + gap * 1.6);
  }
  const [px0, px1, py0, py1] = [L, W - R, T, H - B];
  const catTop = py1 + gap * 0.8 + (vertical && showVal && lo < 0 ? valEff * 1.7 : 0);
  const [cStart, cLen] = vertical ? [px0, px1 - px0] : [py0, py1 - py0];
  const vA = vertical ? py1 : px0;
  const vB = vertical ? py0 : px1;
  const sw = cLen / n;
  const gt = group(sw);
  const k = stacked ? 1 : S;
  const bt = gt / k;
  const inner = k > 1 ? bt * 0.06 : 0;
  const V = (v) => vA + ((v - bot) / (top - bot)) * (vB - vA);
  const cross = (p, s) => [cStart + p * sw + (sw - gt) / 2 + (stacked ? 0 : s) * bt + inner / 2, bt - inner];
  const rect = (c, len, a, b) =>
    vertical
      ? { left: c, top: Math.min(a, b), width: len, height: Math.abs(a - b) }
      : { left: Math.min(a, b), top: c, width: Math.abs(a - b), height: len };
  const at = (c, v) => (vertical ? [c, v] : [v, c]);

  const numeric = edges && labels.every((l) => Number.isFinite(Number(l)));
  const catPos = (x) => {
    if (!edges) return cStart + (x + 0.5) * sw;
    if (!numeric) return cStart + x * sw;
    const e = labels.map(Number);
    let j = 0;
    while (j < n - 1 && x > e[j + 1]) j++;
    return cStart + (j + (x - e[j]) / (e[j + 1] - e[j])) * sw;
  };

  const pose = (t) => {
    let v = states[0].map((row) => row.slice());
    let slot = ranks[0].slice();
    for (let j = 0; j < changeAt.length; j++) {
      const e = easeInOutCubic(clamp((t - changeAt[j]) / glide, 0, 1));
      v = v.map((row, s) => row.map((x, i) => lerp(x, states[j + 1][s][i], e)));
      slot = slot.map((p, i) => lerp(p, ranks[j + 1][i], e));
    }
    return { v, slot };
  };

  return {
    w: W,
    h: H,
    vertical,
    n,
    S,
    edges,
    stacked,
    histogram,
    ticks,
    labelTex,
    unit,
    dec,
    valText,
    showVal,
    sizes: { cat: catEff, tick: tickSize, val: valEff, gap, line: size },
    plot: [px0, px1, py0, py1],
    sw,
    gt,
    bt,
    legendH,
    lineHoriz,
    lineOn: o.lineOn ?? "value",
    lineH,
    catTop,
    V,
    cross,
    rect,
    at,
    catPos,
    pose,
    base: V(0),
  };
};

/** A full-box absolute layer, so a Path draws in the chart's px. */
const layer = (ch) => ({ position: "absolute", left: 0, top: 0, width: ch.w, height: ch.h });

/**
 * The chart `ch` (from `bars()`). The axis and gridlines draw on from `at`;
 * bar i grows from `barsAt + rank * each`. `tones` colour the series (or the
 * bars of a single series); `line`/`lineLabel` (TeX) draw a reference line
 * from `lineAt`; `highlight` bar indices turn `highlightTone` from
 * `highlightAt` (`dim` fades the rest); `diff` [i, j] marks the gap between
 * two bars of the first series with an arrow and `diffLabel` from `diffAt`.
 */
const Bars = (
  {
    ch,
    id = "bars",
    at = 0,
    barsAt = 0.5,
    each,
    names = [],
    tones = [],
    line,
    lineLabel,
    lineAt = 0,
    lineTone,
    highlight = [],
    highlightAt = 0,
    highlightTone,
    dim = false,
    diff = [],
    diffLabel,
    diffAt = 0,
  },
  { t, theme },
) => {
  const { color, motion, radius } = theme;
  const { cat, tick, val, gap, line: size } = ch.sizes;
  const [px0, px1, py0, py1] = ch.plot;
  const { v: values, slot } = ch.pose(t);
  const first = ch.pose(0).slot;
  const stagger = each ?? motion.duration.stagger;
  const palette = [color.accent, color.muted, color.positive];
  const toneOf = (s, i) =>
    ch.S === 1 && tones.length > 1 ? tones[i % tones.length] : (tones[s] ?? palette[s % palette.length]);
  const fade = (from, dur = motion.duration.base) => easeOutCubic(clamp((t - from) / dur, 0, 1));
  const rule = size / 20;
  const keyOf = (s, i) => (s === 0 ? `${id}/b${i}` : `${id}/b${i}s${s + 1}`);
  // A box of `w` by `h` px at (left, top), its content anchored `ax` across and `ay` down.
  const box = (left, top, w, h, kid, ax = "center", ay = "center") => (
    <Box style={{ position: "absolute", left, top, width: w, height: h, alignItems: ay, justifyContent: ax }}>{kid}</Box>
  );
  const math = (key, tex, fs, ink, opacity = 1) => (
    <Math key={key} tex={tex} style={{ fontSize: fs, color: ink, opacity }} />
  );
  // Behind a value label, so a line or guide through it breaks there.
  const backed = (kid) => <Box style={{ padding: [0, size * 0.1], radius: radius.s, fill: color.bg }}>{kid}</Box>;
  const parts = [];
  const labels = [];
  const marks = [];

  ch.ticks.forEach((v, k) => {
    const shown = fade(at + k * stagger);
    const c = ch.V(v);
    if (v !== 0) {
      parts.push(
        <Box
          key={`${id}/g${fmt(v)}`}
          style={{
            position: "absolute",
            ...(ch.vertical
              ? { left: px0, top: c - rule / 2, width: px1 - px0, height: rule }
              : { left: c - rule / 2, top: py0, width: rule, height: py1 - py0 }),
            fill: color.border,
            clipInset: ch.vertical ? [0, 1 - shown, 0, 0] : [0, 0, 1 - shown, 0],
          }}
        />,
      );
    }
    const num = math(`${id}/n${fmt(v)}`, fmt(v) + ch.unit, tick, color.muted, shown);
    labels.push(
      ch.vertical
        ? box(0, c - tick, px0 - gap * 0.7, 2 * tick, num, "flex-end")
        : box(c - 3 * tick, py1 + gap * 0.5, 6 * tick, 2 * tick, num, "center", "flex-start"),
    );
  });
  const drawn = fade(at, motion.duration.slow);
  parts.push(
    <Box
      key={`${id}/base`}
      style={{
        position: "absolute",
        ...(ch.vertical
          ? { left: px0, top: ch.base - rule, width: px1 - px0, height: 2 * rule }
          : { left: ch.base - rule, top: py0, width: 2 * rule, height: py1 - py0 }),
        fill: color.muted,
        clipInset: ch.vertical ? [0, 1 - drawn, 0, 0] : [0, 0, 1 - drawn, 0],
      }}
    />,
  );

  const hl = new Set(highlight);
  const dimmed = fade(highlightAt);
  const lineC = line === undefined ? undefined : ch.lineOn === "value" ? ch.V(line) : ch.catPos(line);
  for (let i = 0; i < ch.n; i++) {
    const begin = barsAt + first[i] * stagger;
    const grow = t < begin ? 0 : spring(t - begin, motion.spring.gentle);
    const shown = clamp(grow, 0, 1);
    let [up, down] = [0, 0];
    for (let s = 0; s < ch.S; s++) {
      const x = values[s][i] * clamp(grow, 0, 1.04);
      const positive = x >= 0;
      const [c, len] = ch.cross(slot[i], s);
      let [a, b] = [0, x];
      if (ch.stacked) {
        [a, b] = positive ? [up, up + x] : [down, down + x];
        if (positive) up += x;
        else down += x;
      }
      let ink = toneOf(s, i);
      if (highlight.length) {
        const lit = hl.has(i) ? mix(ink, highlightTone, fade(highlightAt + first[i] * stagger)) : null;
        ink = lit ?? (dim ? mix(ink, color.bg, 0.55 * dimmed) : ink);
      }
      const [pa, pb] = [ch.V(a), ch.V(b)];
      const r = ch.stacked || ch.histogram ? 0 : Math.min(radius.s, len / 4);
      const style = { position: "absolute", ...ch.rect(c, len, pa, pb), fill: ink, radius: r };
      if (r > 0) {
        // The free end is rounded; the base end runs r past the baseline and is clipped off.
        const f = r / ((ch.vertical ? style.height : style.width) + r);
        if (ch.vertical) {
          if (positive) style.height += r;
          else style.top -= r;
          style.clipInset = positive ? [0, 0, f, 0] : [f, 0, 0, 0];
        } else {
          if (positive) style.left -= r;
          style.width += r;
          style.clipInset = positive ? [0, 0, 0, f] : [0, f, 0, 0];
        }
      } else if (ch.histogram) {
        style.border = { width: rule * 1.5, color: color.bg };
      }
      parts.push(<Box key={keyOf(s, i)} style={style} />);

      if (ch.showVal) {
        const text = ch.valText(values[s][i] * shown);
        const key = s === 0 ? `${id}/v${i}` : `${id}/v${i}s${s + 1}`;
        const mid = c + len / 2;
        const [tw, th] = [texW(text, val), val * 1.5];
        const [lw, lh] = [Math.max(len, tw) + gap * 2, val * 1.8];
        // The label's box (left, top) and where its text sits in it.
        let [lx, ly, ax, ay] = [0, 0, "center", "center"];
        if (ch.stacked) {
          [lx, ly] = ch.vertical ? [mid - lw / 2, (pa + pb) / 2 - lh / 2] : [(pa + pb) / 2 - lw / 2, mid - lh / 2];
        } else if (ch.vertical) {
          [lx, ly] = [mid - lw / 2, positive ? pb - gap * 0.4 - lh : pb + gap * 0.4];
          ay = positive ? "flex-end" : "flex-start";
        } else {
          [lx, ly] = [positive ? pb + gap * 0.5 : pb - gap * 0.5 - lw, mid - lh / 2];
          ax = positive ? "flex-start" : "flex-end";
        }
        const [x0, y0] = [lx + (ax === "center" ? (lw - tw) / 2 : ax === "flex-end" ? lw - tw : 0), ly + (ay === "center" ? (lh - th) / 2 : ay === "flex-end" ? lh - th : 0)];
        const crossed = lineC !== undefined && (ch.lineHoriz ? lineC > y0 - gap * 0.2 && lineC < y0 + th + gap * 0.2 : lineC > x0 - gap * 0.2 && lineC < x0 + tw + gap * 0.2);
        const kid = math(key, text, val, ch.stacked ? color.onAccent : color.text, clamp(grow * 2 - 1, 0, 1));
        const shell = crossed && !ch.stacked ? backed(kid) : kid;
        // A stacked label sits inside its segment, if it is tall enough.
        if (!ch.stacked || Math.abs(pa - pb) > lh * 1.1) marks.push(box(lx, ly, lw, lh, shell, ax, ay));
      }
    }
    if (!ch.edges) {
      const centre = (ch.vertical ? px0 : py0) + (slot[i] + 0.5) * ch.sw;
      const lines = ch.labelTex[i].split("\\\\").length;
      const kid = math(`${id}/l${i}`, ch.labelTex[i], cat, color.text, fade(at + 0.2 + first[i] * stagger));
      labels.push(
        ch.vertical
          ? box(centre - ch.sw / 2, ch.catTop, ch.sw, lines * cat * 1.6, kid, "center", "flex-start")
          : box(0, centre - ch.sw / 2, px0 - gap * 0.9, ch.sw, kid, "flex-end"),
      );
    }
  }
  if (ch.edges) {
    for (let j = 0; j <= ch.n; j++) {
      const kid = math(`${id}/e${j}`, ch.labelTex[j], cat, color.text, fade(at + 0.2 + j * stagger));
      labels.push(
        ch.vertical
          ? box(px0 + j * ch.sw - ch.sw / 2, ch.catTop, ch.sw, cat * 1.7, kid, "center", "flex-start")
          : box(0, py0 + j * ch.sw - cat, px0 - gap * 0.9, 2 * cat, kid, "flex-end"),
      );
    }
  }

  if (line !== undefined) {
    const p = easeInOutCubic(clamp((t - lineAt) / motion.duration.slow, 0, 1));
    const c = lineC;
    const ink = lineTone ?? color.text;
    const dash = size * 0.35;
    parts.push(
      <Path
        key={`${id}/line`}
        d={ch.lineHoriz ? `M${px0} ${c} L${px1} ${c}` : `M${c} ${py0} L${c} ${py1}`}
        viewBox={[0, 0, ch.w, ch.h]}
        stroke={{ width: size / 9, color: ink, dash: [dash, dash], cap: "butt" }}
        trim={[0, p]}
        style={layer(ch)}
      />,
    );
    if (lineLabel) {
      const h = lineLabel.split("\\\\").length * ch.lineH;
      const w = texW(lineLabel, size) + gap * 2;
      const kid = math(`${id}/linelabel`, lineLabel, size, ink, clamp(p * 3 - 2, 0, 1));
      marks.push(
        ch.lineHoriz
          ? box(px1 + gap * 0.7, c - h / 2, w, h, kid, "flex-start")
          : box(c - w / 2, ch.legendH + gap * 0.2, w, h, kid, "center", "flex-end"),
      );
    }
  }
  parts.push(...labels, ...marks);

  if (diff.length === 2) {
    const [i, j] = diff;
    const [tall, short] = values[0][i] >= values[0][j] ? [i, j] : [j, i];
    const [ct, cs] = [ch.cross(slot[tall], 0), ch.cross(slot[short], 0)];
    const dir = Math.sign(ct[0] - cs[0]) || 1;
    const room = ch.sw - ch.gt;
    // The arrow runs up the gap beside the shorter bar, or up its middle where bars touch.
    const mid = cs[0] + cs[1] / 2;
    const x = room > size * 0.9 ? mid + dir * (cs[1] / 2 + room / 2) : mid;
    const [yTall, yShort] = [ch.V(values[0][tall]), ch.V(values[0][short])];
    const p = easeInOutCubic(clamp((t - diffAt) / motion.duration.slow, 0, 1));
    const dash = size * 0.3;
    const [g0, g1] = [ch.at(ct[0] + ct[1] / 2, yTall), ch.at(x, yTall)];
    // Keyed anchors give the Arrow overlay its ends, so it follows the chart wherever it sits.
    const anchor = (key, [ax, ay]) => (
      <Box key={key} style={{ position: "absolute", left: ax, top: ay, width: rule, height: rule }} />
    );
    parts.push(
      <Path
        key={`${id}/diffguide`}
        d={`M${g0[0]} ${g0[1]} L${g1[0]} ${g1[1]}`}
        viewBox={[0, 0, ch.w, ch.h]}
        stroke={{ width: size / 14, color: color.muted, dash: [dash, dash] }}
        trim={[0, p]}
        style={layer(ch)}
      />,
      anchor(`${id}/diffa`, ch.at(x, yShort)),
      anchor(`${id}/diffb`, ch.at(x, yTall)),
      <Arrow
        key={`${id}/diff`}
        from={`${id}/diffa`}
        to={`${id}/diffb`}
        gap={0}
        progress={p}
        head={size * 0.7}
        stroke={{ width: size / 10, color: color.text }}
      />,
    );
    if (diffLabel) {
      const w = texW(diffLabel, size) + gap * 2;
      const kid = math(`${id}/difflabel`, diffLabel, size, color.text, clamp(p * 3 - 2, 0, 1));
      parts.push(box(x - dir * (w / 2 + gap * 0.6) - w / 2, (yTall + yShort) / 2 - size, w, 2 * size, kid));
    }
  }

  if (names.length) {
    const items = names.map((name, s) => (
      <Row key={`${id}/key${s}`} gap={gap * 0.5} align="center" style={{ opacity: fade(at + 0.3) }}>
        <Box style={{ width: size * 0.55, height: size * 0.55, radius: radius.s, fill: toneOf(s, s) }} />
        <Math key={`${id}/name${s}`} tex={name} style={{ fontSize: size * 0.9, color: color.text }} />
      </Row>
    ));
    parts.push(
      <Box
        style={{
          position: "absolute",
          left: ch.vertical ? px0 : 0,
          top: 0,
          width: ch.vertical ? ch.w - px0 : ch.w,
          height: ch.legendH,
          alignItems: "center",
          justifyContent: ch.vertical ? "flex-start" : "flex-end",
          gap: size * 1.2,
        }}
      >
        {items}
      </Box>,
    );
  }
  return <Box style={{ width: ch.w, height: ch.h }}>{parts}</Box>;
};

export default (p, { theme }) => {
  const { color, motion } = theme;
  const series = [p.values, p.values2, p.values3].filter(Boolean);
  const ch = bars({
    labels: p.labels,
    series,
    to: [p.to, p.to2, p.to3].map((st) => (st.length ? st : undefined)),
    changeAt: p.changeAt,
    glide: p.glide ?? motion.duration.slow,
    orientation: p.orientation,
    histogram: p.histogram,
    stacked: p.stacked,
    sort: p.sort,
    unit: p.unit,
    max: p.max,
    step: p.step,
    valueLabels: p.valueLabels,
    decimals: p.decimals,
    names: p.names,
    line: p.line,
    lineOn: p.lineOn,
    lineLabel: p.lineLabel,
  });
  return (
    <Stack justify="center" align="center" style={{ flexGrow: 1, fill: color.bg }}>
      <Bars
        ch={ch}
        at={p.at}
        barsAt={p.barsAt}
        each={p.each}
        names={p.names}
        tones={p.tones}
        line={p.line}
        lineLabel={p.lineLabel}
        lineAt={p.lineAt}
        lineTone={p.lineTone}
        highlight={p.highlight}
        highlightAt={p.highlightAt}
        highlightTone={p.highlightTone}
        dim={p.dim}
        diff={p.diff}
        diffLabel={p.diffLabel}
        diffAt={p.diffAt}
      />
    </Stack>
  );
};
