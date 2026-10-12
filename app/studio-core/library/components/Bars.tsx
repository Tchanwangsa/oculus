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
// A bad input throws an error naming the prop. A two-line TeX label
// (`labels`, `names`, `lineLabel`, `diffLabel`) may use a bare `\\`.
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
  // The value axis reaches at least `max`, ticks every `step` (thinned to
  // fit the chart); else both from the data. The data always fits.
  max: num().optional(),
  step: num().optional(),
  // Bar labels count up; `decimals` fixes their places (else from the data).
  valueLabels: bool().default(true),
  decimals: num().optional(),
  // The axis draws on from `at`, the bars grow from barsAt, each `each` after
  // the last (else the stagger, squeezed so every bar has grown by barsAt + 1.2 s).
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

/** A key for a number: no float noise. */
const fmt = (v) => String(Number(v.toPrecision(10)));

/** Decimal places `v` needs, at most `cap`. */
const places = (v, cap) => {
  const s = String(Number(v.toFixed(cap)));
  const i = s.indexOf(".");
  return i < 0 ? 0 : s.length - i - 1;
};

/** Digits grouped in threes from 100000 up, so a long number reads at a glance. */
const grouped = (s, v) => (Math.abs(v) >= 1e5 ? s.replace(/\B(?=(\d{3})+(?!\d))/g, "{,}") : s);

/** `v` as TeX: no float noise, grouped digits, a power of ten when tiny or huge. */
const numTex = (v) => {
  if (v !== 0 && (Math.abs(v) < 1e-5 || Math.abs(v) >= 1e15)) {
    const [m, e] = Number(v.toPrecision(10)).toExponential().split("e");
    return `${m}\\times 10^{${Number(e)}}`;
  }
  const [whole, frac] = fmt(v).split(".");
  return grouped(whole, v) + (frac ? `.${frac}` : "");
};

/** `v` to `dec` places as TeX. */
const fixedTex = (v, dec) => {
  const [whole, frac] = v.toFixed(dec).split(".");
  return grouped(whole, v) + (frac ? `.${frac}` : "");
};

/** TeX with a bare `\\` (a second line) is wrapped so KaTeX accepts it. */
const multiline = (tex) => (tex.includes("\\\\") && !tex.includes("\\begin") ? `\\begin{gathered}${tex}\\end{gathered}` : tex);

const lineCount = (tex) => tex.split("\\\\").length;

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

/** The smallest 1, 2 or 5 times a power of ten that puts `span` in at most `most` steps. */
const niceStep = (span, most = 6) => {
  const p = 10 ** Math.floor(Math.log10(span / most));
  for (const m of [1, 2, 5, 10]) if (span / (m * p) <= most) return m * p;
  return 10 * p;
};

const need = (ok, msg) => {
  if (!ok) throw new Error(`Bars: ${msg}`);
};

const SERIES = ["values", "values2", "values3"];
const STATES = ["to", "to2", "to3"];

/** Throws a readable error for data `bars()` cannot draw. */
const checkData = ({ labels, series, to, changeAt, histogram }) => {
  const n = series[0].length;
  need(n > 0, "values is empty; give at least one bar");
  series.forEach((row, s) => {
    const name = SERIES[s] ?? `series ${s + 1}`;
    need(row.length === n, `${name} has ${row.length} values but values has ${n}`);
    row.forEach((x, i) => need(Number.isFinite(x), `${name}[${i}] = ${x} is not a finite number`));
  });
  need(
    labels.length === n || (histogram && labels.length === n + 1),
    `labels has ${labels.length} entries but values has ${n}${histogram ? " (a histogram's labels are its bin edges: one more than values)" : ""}`,
  );
  to.forEach((states, s) => need(!states || s < series.length, `${STATES[s]} is given but ${SERIES[s]} is not`));
  to.forEach((states, s) => {
    const name = STATES[s] ?? `to[${s}]`;
    if (!states) return;
    need(states.length <= changeAt.length, `${name} has ${states.length} states but changeAt has ${changeAt.length} times`);
    states.forEach((row, k) => {
      need(row.length === n, `${name}[${k}] has ${row.length} values but values has ${n}`);
      row.forEach((x, i) => need(Number.isFinite(x), `${name}[${k}][${i}] = ${x} is not a finite number`));
    });
  });
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
 * The value axis reaches `max` if given and always covers the data; its
 * ticks are thinned until their numbers clear each other (and number at
 * most 12). `labelOn(i)` says whether label i is shown: a crowded axis
 * keeps every k-th label and the last.
 */
const bars = (o) => {
  const { labels, series, to = [], changeAt = [], histogram = false, stacked = false, unit = "", size = 40 } = o;
  const { width: W = 1700, height: H = 920, glide = 0.9, names = [], lineLabel } = o;
  checkData({ labels, series, to, changeAt, histogram });
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
  // The axis for ticks every `step`: `max` is a floor, never a ceiling over the data.
  const axisFor = (step) => {
    const bot = Math.floor(lo / step + 1e-9) * step;
    let top = Math.max(Math.ceil(hi / step - 1e-9) * step, o.max ?? -Infinity);
    if (top <= bot) top = bot + step;
    const count = (top - bot) / step;
    // Too fine to list: only the ends, for sizing; a coarser step replaces it.
    const ticks = count > 500 ? [bot, top] : [];
    for (let k = Math.round(bot / step); count <= 500 && k * step <= top + 1e-9; k++) ticks.push(Number((k * step).toPrecision(12)));
    return { step, top, bot, ticks, count };
  };
  const span = hi - lo || 1;
  let axis = axisFor(o.step ?? niceStep(span));

  const catSize = size;
  const tickSize = size * 0.9;
  const gap = size * 0.6;
  const lineH = size * 1.7;
  const tickW = Math.max(...axis.ticks.map((v) => texW(numTex(v) + unit, tickSize)));
  // Places the data needs, at most two unless the axis steps finer than that.
  const dec = o.decimals ?? Math.min(Math.max(0, ...states.flat(2).map((v) => places(v, 6))), Math.max(2, places(axis.step, 6)));
  const big = Math.max(hi, -lo);
  const valText = (v) => (big < 1e-4 || big >= 1e15 ? numTex(Number(v.toPrecision(3))) : fixedTex(v, dec)) + unit;
  const valW0 = Math.max(texW(valText(hi), size * 0.9), texW(valText(lo), size * 0.9));
  const lineHoriz = o.line !== undefined && vertical === (o.lineOn !== "category");
  const lineLines = lineLabel ? lineCount(lineLabel) : 1;
  const nameLines = Math.max(1, ...names.map(lineCount));
  const legendH = names.length ? size * (0.5 + 1.2 * nameLines) : 0;
  const lineRow = o.line !== undefined && lineLabel && !lineHoriz ? lineLines * lineH + gap * 0.5 : 0;

  // Margins: L, R, T, B px round the plot. Vertical bars: the numbers sit
  // left, the labels below; horizontal: the labels left, the numbers below.
  let [L, R, T, B] = [0, 0, 0, 0];
  let labelTex = labels;
  let catEff = catSize;
  let valEff = size * 0.9;
  let showVal = o.valueLabels !== false;
  let every = 1;
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
    const fitted = widest * (catEff / catSize);
    if (fitted > limit * 1.001) every = Math.ceil((fitted * 1.25) / sw);
    if (!stacked) valEff = Math.min(valEff, (valEff * bt * 1.15) / valW0);
    if (valEff < size * 0.55 && !stacked) showVal = false;
    const lines = Math.max(...labelTex.map(lineCount));
    T = legendH + lineRow + (showVal ? valEff * 1.9 : gap * 1.2);
    B = gap * 0.9 + lines * catEff * 1.6 + (showVal && !stacked && lo < 0 ? valEff * 1.7 : 0);
  } else {
    T = legendH + lineRow + gap * 1.2;
    B = tickSize * 1.7 + gap;
    const wideLimit = W * 0.3;
    labelTex = wrap(labels, wideLimit, catSize);
    const widest = Math.max(...labelTex.map((l) => texW(l, catSize)));
    const sh = (H - T - B) / n;
    const lines = Math.max(...labelTex.map(lineCount));
    catEff = Math.max(catSize * 0.62, Math.min(catSize, (catSize * wideLimit) / widest, (sh * 0.92) / (lines * 1.5)));
    if (lines * catEff * 1.5 > sh * 0.92 * 1.001) every = Math.ceil((lines * catEff * 1.5 * 1.25) / sh);
    L = Math.min(wideLimit, Math.max(...labelTex.map((l) => texW(l, catEff)))) + gap * 1.4;
    const bt = group(sh) / (stacked ? 1 : S);
    if (!stacked) valEff = Math.min(valEff, bt / 1.45);
    if (valEff < size * 0.55 && !stacked) showVal = false;
    R = Math.max(gap, tickW / 2 + gap * 0.3, showVal && !stacked ? valW0 * (valEff / (size * 0.9)) + gap * 1.8 : 0);
    if (lineHoriz && lineLabel) R = Math.max(R, texW(lineLabel, size) + gap * 1.6);
  }
  const [px0, px1, py0, py1] = [L, W - R, T, H - B];

  // Ticks go no closer than 1.6 label heights (a label width and a bit,
  // horizontally) and no more than 12: a bigger step takes over when they do.
  const room = vertical ? py1 - py0 : px1 - px0;
  const most = Math.max(1, Math.min(11, Math.floor(room / (vertical ? tickSize * 1.6 : tickW * 1.25 + tickSize * 0.8))));
  if (axis.count > most + 1e-9) {
    if (o.step !== undefined) {
      // The nearest 1, 2, 5 x 10^k that is a whole multiple of `step`, else a whole multiple of it.
      const p0 = Math.floor(Math.log10(o.step));
      let found = null;
      for (let p = p0; p < p0 + 12 && !found; p++) {
        for (const m of [1, 2, 5]) {
          const c = m * 10 ** p;
          const f = c / o.step;
          if (!found && f > 1.5 && Math.abs(f - Math.round(f)) < 1e-9 && axisFor(c).count <= most + 1e-9) found = c;
        }
      }
      axis = axisFor(found ?? o.step * Math.ceil(axis.count / most));
    } else {
      for (let m = Math.min(6, most); m >= 1; m--) {
        axis = axisFor(niceStep(span, m));
        if (axis.count <= most + 1e-9) break;
      }
    }
  }
  const { top, bot, ticks } = axis;

  const catTop = py1 + gap * 0.8 + (vertical && showVal && !stacked && lo < 0 ? valEff * 1.7 : 0);
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
  // Where a `line` may sit along the bars: a bar's index, or an edge value.
  const catRange = !edges ? [-0.5, n - 0.5] : numeric ? [Number(labels[0]), Number(labels[n])] : [0, n];
  const labelCount = edges ? n + 1 : n;
  const labelOn = (i) => i === 0 || i === labelCount - 1 || (i % every === 0 && labelCount - 1 - i >= every * 0.8);

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
    labelOn,
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
    valueRange: [bot, top],
    catRange,
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
 * bar i grows from `barsAt + rank * each` (`each`: else the theme's stagger,
 * squeezed so the last bar starts within 0.6 s). `tones` colour the series
 * (or the bars of a single series); `line`/`lineLabel` (TeX) draw a
 * reference line from `lineAt` and must lie within the chart; `highlight`
 * bar indices turn `highlightTone` (default `accentStrong`) from
 * `highlightAt` (`dim` fades the rest); `diff` [i, j] marks the gap between
 * two bars of the first series from `diffAt`: a dashed guide from the taller
 * bar's end, an arrow up the shorter bar's column to that level, and
 * `diffLabel` beside the arrow where nothing else is.
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
    dim = true,
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
  const spread = 0.6;
  const growEach = each ?? Math.min(stagger, spread / Math.max(1, ch.n - 1));
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
    <Math key={key} tex={multiline(tex)} style={{ fontSize: fs, color: ink, opacity }} />
  );
  // Behind a value label, so a line or guide through it breaks there.
  const backed = (kid) => <Box style={{ padding: [0, size * 0.1], radius: radius.s, fill: color.bg }}>{kid}</Box>;
  const parts = [];
  const labels = [];
  const marks = [];

  const [vMin, vMax] = ch.valueRange;
  if (line !== undefined) {
    const [lo, hi] = ch.lineOn === "value" ? [vMin, vMax] : ch.catRange;
    need(
      line >= lo - 1e-9 && line <= hi + 1e-9,
      ch.lineOn === "value"
        ? `line = ${line} is outside the value axis (${lo} to ${hi}); raise max, or move the line`
        : `line = ${line} is outside the bars (${lo} to ${hi} along them, ${ch.edges ? "edge values" : "bar indices"})`,
    );
  }
  const lineC = line === undefined ? undefined : ch.lineOn === "value" ? ch.V(line) : ch.catPos(line);

  // The difference marker, in (cross, value) px: the taller bar's level
  // `vT`, the shorter bar's column `ms`, and where the arrow starts.
  let dg = null;
  if (diff.length) {
    need(
      diff.length === 2 && diff.every((k) => Number.isInteger(k) && k >= 0 && k < ch.n),
      `diff = [${diff}] must be two bar indices, each from 0 to ${ch.n - 1}`,
    );
    const [i, j] = diff;
    const [tall, short] = values[0][i] >= values[0][j] ? [i, j] : [j, i];
    const [ct, cs] = [ch.cross(slot[tall], 0), ch.cross(slot[short], 0)];
    const [mt, ms] = [ct[0] + ct[1] / 2, cs[0] + cs[1] / 2];
    const dir = Math.sign(mt - ms) || 1;
    const [vT, vS] = [ch.V(values[0][tall]), ch.V(values[0][short])];
    const up = Math.sign(vT - vS) || 1;
    // The shorter bar's own value label sits on the free end: start beyond it.
    const labelDir = (values[0][short] >= 0 ? 1 : -1) * (ch.vertical ? -1 : 1);
    const tw = texW(ch.valText(values[0][short]), val);
    const labelPad = ch.stacked || !ch.showVal ? gap * 0.4 : gap * 0.7 + (ch.vertical ? val * 1.5 : tw);
    let vA = vS + up * (labelDir === up ? labelPad : gap * 0.4);
    if ((vT - vA) * up < size * 2) vA = vS + up * gap * 0.4;
    dg = { tall, short, ms, mt, dir, vT, vS, vA, up, c0: Math.min(mt - dir * (ct[1] / 2), ms), c1: Math.max(mt - dir * (ct[1] / 2), ms), edge: mt - dir * (ct[1] / 2) };
  }

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
    const num = math(`${id}/n${fmt(v)}`, numTex(v) + ch.unit, tick, color.muted, shown);
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
  const hlTone = highlightTone ?? color.accentStrong;
  for (let i = 0; i < ch.n; i++) {
    const begin = barsAt + first[i] * growEach;
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
        const lit = hl.has(i) ? mix(ink, hlTone, fade(highlightAt + first[i] * growEach)) : null;
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
        // The diff guide runs along the value axis' level `dg.vT` from the taller bar to the arrow.
        const [along0, along1, across0, across1] = ch.vertical ? [y0, y0 + th, x0, x0 + tw] : [x0, x0 + tw, y0, y0 + th];
        const onGuide = dg && dg.vT > along0 - gap * 0.2 && dg.vT < along1 + gap * 0.2 && across1 > dg.c0 && across0 < dg.c1;
        const kid = math(key, text, val, ch.stacked ? color.onAccent : color.text, clamp(grow * 2 - 1, 0, 1));
        const shell = (crossed || onGuide) && !ch.stacked ? backed(kid) : kid;
        // A stacked label sits inside its segment, if it is tall enough.
        if (!ch.stacked || Math.abs(pa - pb) > lh * 1.1) marks.push(box(lx, ly, lw, lh, shell, ax, ay));
      }
    }
    if (!ch.edges && ch.labelOn(i)) {
      const centre = (ch.vertical ? px0 : py0) + (slot[i] + 0.5) * ch.sw;
      const lines = lineCount(ch.labelTex[i]);
      const kid = math(`${id}/l${i}`, ch.labelTex[i], cat, color.text, fade(at + 0.2 + first[i] * growEach));
      labels.push(
        ch.vertical
          ? box(centre - ch.sw / 2, ch.catTop, ch.sw, lines * cat * 1.6, kid, "center", "flex-start")
          : box(0, centre - ch.sw / 2, px0 - gap * 0.9, ch.sw, kid, "flex-end"),
      );
    }
  }
  if (ch.edges) {
    for (let j = 0; j <= ch.n; j++) {
      if (!ch.labelOn(j)) continue;
      const kid = math(`${id}/e${j}`, ch.labelTex[j], cat, color.text, fade(at + 0.2 + j * growEach));
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
      const h = lineCount(lineLabel) * ch.lineH;
      const w = texW(lineLabel, size) + gap * 2;
      const kid = math(`${id}/linelabel`, lineLabel, size, ink, clamp(p * 3 - 2, 0, 1));
      marks.push(
        ch.lineHoriz
          ? box(px1 + gap * 0.7, c - h / 2, w, h, kid, "flex-start")
          : box(c - w / 2, ch.legendH + gap * 0.2, w, h, kid, "center", "flex-end"),
      );
    }
  }
  // The guide runs under the labels, so a value label it crosses stays legible.
  const diffP = easeInOutCubic(clamp((t - diffAt) / motion.duration.slow, 0, 1));
  if (dg) {
    const [g0, g1] = [ch.at(dg.edge, dg.vT), ch.at(dg.ms, dg.vT)];
    parts.push(
      <Path
        key={`${id}/diffguide`}
        d={`M${g0[0]} ${g0[1]} L${g1[0]} ${g1[1]}`}
        viewBox={[0, 0, ch.w, ch.h]}
        stroke={{ width: size / 14, color: color.text, dash: [size * 0.3, size * 0.3] }}
        trim={[0, diffP]}
        style={layer(ch)}
      />,
    );
  }
  parts.push(...labels, ...marks);

  if (dg) {
    const { ms, mt, dir, vT, vS, vA, up } = dg;
    const p = diffP;
    // Keyed anchors give the Arrow overlay its ends, so it follows the chart wherever it sits.
    const anchor = (key, [ax, ay]) => (
      <Box key={key} style={{ position: "absolute", left: ax, top: ay, width: rule, height: rule }} />
    );
    const length = (vT - vA) * up;
    if (length > size * 0.5) {
      parts.push(
        anchor(`${id}/diffa`, ch.at(ms, vA)),
        anchor(`${id}/diffb`, ch.at(ms, vT)),
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
    }
    if (diffLabel) {
      const [lw, lh] = [texW(diffLabel, size) + gap * 2, lineCount(diffLabel) * size * 1.4 + size * 0.4];
      const [crossExt, valExt] = ch.vertical ? [lw, lh] : [lh, lw];
      // Occupied px of every bar (and its value label) along the value axis, per cross range.
      const room = (i) => {
        const col = values.map((row) => row[i]);
        let [hiEnd, loEnd] = [Math.max(0, ...col), Math.min(0, ...col)];
        if (ch.stacked) [hiEnd, loEnd] = [col.reduce((a, x) => a + Math.max(x, 0), 0), col.reduce((a, x) => a + Math.min(x, 0), 0)];
        const labelPx = ch.stacked || !ch.showVal ? gap * 0.5 : gap * 0.9 + (ch.vertical ? val * 1.5 : texW(ch.valText(hiEnd), val));
        let [a, b] = [ch.V(hiEnd), ch.V(loEnd)];
        const hiSide = ch.vertical ? -1 : 1;
        if (hiEnd > 0) (hiSide < 0 ? (a -= labelPx) : (a += labelPx));
        if (loEnd < 0) (hiSide < 0 ? (b += labelPx) : (b -= labelPx));
        const [c0] = ch.cross(slot[i], 0);
        const [cl, ll] = ch.cross(slot[i], ch.S - 1);
        return { c0, c1: cl + ll, v0: Math.min(a, b), v1: Math.max(a, b) };
      };
      const cols = Array.from({ length: ch.n }, (_, i) => room(i));
      const [cLo, cHi] = ch.vertical ? [px0, px1] : [py0, py1];
      const [vLo, vHi] = ch.vertical ? [py0, py1] : [px0, px1];
      const free = (cc, vc) => {
        const [a0, a1, b0, b1] = [cc - crossExt / 2, cc + crossExt / 2, vc - valExt / 2, vc + valExt / 2];
        if (a0 < cLo || a1 > cHi || b0 < vLo || b1 > vHi) return false;
        return cols.every((k) => a1 <= k.c0 || a0 >= k.c1 || b1 <= k.v0 || b0 >= k.v1);
      };
      let spot = null;
      if (length <= size * 0.5) {
        // Bars (nearly) level: no arrow fits, so the label rides above both value labels.
        const out = ch.vertical ? -1 : 1;
        const end = ch.vertical ? Math.min(vT, vS) : Math.max(vT, vS);
        const pad = ch.stacked || !ch.showVal ? gap : gap * 0.7 + (ch.vertical ? val * 1.5 : texW(ch.valText(values[0][dg.tall]), val));
        spot = [(ms + mt) / 2, end + out * (pad + valExt / 2 - gap * 0.4)];
      }
      for (const f of spot ? [] : [0.5, 0.35, 0.65, 0.2, 0.8]) {
        const vc = vA + (vT - vA) * f;
        for (const side of [-dir, dir]) {
          const cc = ms + side * (gap * 0.9 + crossExt / 2);
          if (!spot && free(cc, vc)) spot = [cc, vc];
        }
      }
      const [cc, vc] = spot ?? [ms, (vA + vT) / 2];
      const kid = math(`${id}/difflabel`, diffLabel, size, color.text, clamp(p * 3 - 2, 0, 1));
      const [bx, by] = ch.vertical ? [cc - lw / 2, vc - lh / 2] : [vc - lw / 2, cc - lh / 2];
      parts.push(box(bx, by, lw, lh, spot ? kid : backed(kid)));
    }
  }

  if (names.length) {
    const items = names.map((name, s) => (
      <Row key={`${id}/key${s}`} gap={gap * 0.5} align="center" style={{ opacity: fade(at + 0.3) }}>
        <Box style={{ width: size * 0.55, height: size * 0.55, radius: radius.s, fill: toneOf(s, s) }} />
        <Math key={`${id}/name${s}`} tex={multiline(name)} style={{ fontSize: size * 0.9, color: color.text }} />
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

export default (p, { theme, size }) => {
  const { color, motion, space } = theme;
  const series = [p.values, p.values2, p.values3].filter(Boolean);
  // Sized to the frame: the type scales with it, the chart fills what the margins leave.
  const k = Math.min(size.width / 1920, size.height / 1080);
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
    size: 40 * k,
    width: size.width - 2 * space.xxl,
    height: size.height - 2 * (space.xl + space.s),
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
