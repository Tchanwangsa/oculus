// Grid: r x c cells of maths, as places with fills, highlights and pointers:
// matrices, arrays, DP tables, game boards, pixel images. Cells are keyed
// `<id>/c<r>_<c>` (or, with `keys`, `<id>/k<key>`, so a cell follows its value
// and a rearranged array slides into the new order); fills `<id>/b<r>_<c>`,
// marks `<id>/m<i>`, pointers `<id>/ptr<i>`, labels `<id>/rl<r>`, `<id>/cl<c>`.
// Two Grids with one id glide in a Steps. The demo is the whole kit: a
// `Steps` over the arrangements in `steps`, and the helper below. Copy `Grid`
// (with `glide`, `span` and the fit helpers) into a scene component.
// A glide (a step, a mark move, a pointer move) takes motion.duration.slow
// (0.9 s) and must finish before the scene does: cue the last one at least
// that long before the end. Keys give a cell its identity, so each must be
// unique. A step where two cells trade places arcs them past each other (the
// demo computes that and passes it as `lift`); any other rearrangement glides
// straight, over the cells between.
export const props = {
  cells: list(list(latex())),
  // A theme colour per cell. "surface" is a plain cell, "bg" no cell at all.
  tones: list(list(token("color"))).default([]),
  // When each cell's value appears; below 0 (or absent) it arrives with the grid.
  appear: list(list(num())).default([]),
  // An id per cell, row by row: the cell follows its value across states.
  keys: list(str()).optional(),
  rowLabels: list(latex()).default([]),
  colLabels: list(latex()).default([]),
  brackets: bool().default(false),
  // Cell side in px; 0 fits the grid to the frame.
  cell: num().default(0),
  enter: oneOf(["all", "row", "col", "diagonal", "cell"]).default("diagonal"),
  at: num().default(0.3),
  each: num().default(0.07),
  // Marks, by index: kind, then row | col | [r, c] | [r, c, h, w].
  markKinds: list(oneOf(["row", "col", "cell", "block"])).default([]),
  markIndex: list(list(num())).default([]),
  markTones: list(token("color")).default([]),
  markAt: list(num()).default([]),
  // When a mark fades out (absent: never), and its later positions as flat
  // [time, ...index] runs, so a window can slide.
  markOff: list(num()).default([]),
  markMoves: list(list(num())).default([]),
  // Pointers: a label on a side, an index along that side at each time.
  pointers: list(latex()).default([]),
  pointerSides: list(oneOf(["above", "below", "left", "right"])).default([]),
  pointerIndex: list(list(num())).default([]),
  pointerAt: list(list(num())).default([]),
  pointerTones: list(token("color")).default([]),
  // Later arrangements (cells, keys) and the time each is reached.
  steps: list(list(list(latex()))).default([]),
  stepKeys: list(list(str())).default([]),
  stepAt: list(num()).default([]),
};

// The 2x2 product in Falk's layout: B above right, A below left, C where
// they meet. Each entry lights the row of A and the column of B, then the
// cell they make.
const falk = Array.from({ length: 4 }, (_, k) => {
  const [i, j] = [k >> 1, k & 1];
  const from = 0.9 + 1.25 * k;
  return { from, row: [2 + i, 0, 1, 2], col: [0, 2 + j, 2, 1], to: [2 + i, 2 + j] };
});

// Edit distance "horse" -> "ros": cell (i, j) is filled from its upper,
// left and diagonal neighbours, in reading order.
const fill = Array.from({ length: 15 }, (_, n) => {
  const [i, j] = [1 + Math.floor(n / 3), 1 + (n % 3)];
  return { i, j, from: 1.2 + 0.28 * n };
});

export const examples = {
  product: {
    cells: [
      ["", "", "5", "6"],
      ["", "", "7", "8"],
      ["1", "2", "19", "22"],
      ["3", "4", "43", "50"],
    ],
    tones: [
      ["bg", "bg", "warning", "warning"],
      ["bg", "bg", "warning", "warning"],
      ["accent", "accent", "surface", "surface"],
      ["accent", "accent", "surface", "surface"],
    ],
    appear: [
      [-1, -1, -1, -1],
      [-1, -1, -1, -1],
      [-1, -1, falk[0].from + 0.6, falk[1].from + 0.6],
      [-1, -1, falk[2].from + 0.6, falk[3].from + 0.6],
    ],
    markKinds: falk.flatMap(() => ["block", "block", "cell"]),
    markIndex: falk.flatMap((f) => [f.row, f.col, f.to]),
    markTones: falk.flatMap(() => ["accent", "warning", "positive"]),
    markAt: falk.flatMap((f) => [f.from, f.from, f.from + 0.6]),
    markOff: falk.flatMap((f) => [f.from + 1.25, f.from + 1.25, f.from + 1.25]),
  },
  partition: {
    cells: [["8", "3", "7", "1", "6", "2", "5"]],
    keys: ["8", "3", "7", "1", "6", "2", "5"],
    colLabels: ["0", "1", "2", "3", "4", "5", "6"],
    markKinds: ["cell", "cell", "cell", "cell"],
    markIndex: [[0, 0], [0, 5], [0, 2], [0, 3]],
    markTones: ["accent", "warning", "accent", "warning"],
    markAt: [1.1, 1.1, 2.9, 2.9],
    markOff: [1.9, 1.9, 3.8, 3.8],
    pointers: ["\\text{lo}", "\\text{hi}", "\\text{pivot}"],
    pointerSides: ["below", "below", "above"],
    pointerIndex: [[0, 2, 3], [5, 3, 2], [6, 3]],
    pointerAt: [[0.8, 2.3, 3.9], [0.8, 2.3, 3.9], [0.8, 4.4]],
    pointerTones: ["accent", "warning", "positive"],
    steps: [
      [["2", "3", "7", "1", "6", "8", "5"]],
      [["2", "3", "1", "7", "6", "8", "5"]],
      [["2", "3", "1", "5", "6", "8", "7"]],
    ],
    stepKeys: [
      ["2", "3", "7", "1", "6", "8", "5"],
      ["2", "3", "1", "7", "6", "8", "5"],
      ["2", "3", "1", "5", "6", "8", "7"],
    ],
    stepAt: [1.6, 3.2, 4.4],
  },
  editDistance: {
    cells: [
      ["0", "1", "2", "3"],
      ["1", "1", "2", "3"],
      ["2", "2", "1", "2"],
      ["3", "2", "2", "2"],
      ["4", "3", "3", "2"],
      ["5", "4", "4", "3"],
    ],
    rowLabels: ["\\varepsilon", "h", "o", "r", "s", "e"],
    colLabels: ["\\varepsilon", "r", "o", "s"],
    appear: [0, 1, 2, 3, 4, 5].map((i) =>
      [0, 1, 2, 3].map((j) => (i > 0 && j > 0 ? fill[(i - 1) * 3 + j - 1].from + 0.1 : -1)),
    ),
    markKinds: fill.flatMap(() => ["cell", "cell", "cell", "cell"]),
    markIndex: fill.flatMap((f) => [[f.i - 1, f.j - 1], [f.i - 1, f.j], [f.i, f.j - 1], [f.i, f.j]]),
    markTones: fill.flatMap(() => ["accent", "accent", "accent", "positive"]),
    markAt: fill.flatMap((f) => [f.from - 0.2, f.from - 0.2, f.from - 0.2, f.from - 0.2]),
    markOff: fill.flatMap((f) => [f.from + 0.08, f.from + 0.08, f.from + 0.08, f.from + 0.08]),
  },
  window: {
    cells: [
      ["0", "0", "1", "1", "0", "0"],
      ["0", "1", "1", "1", "1", "0"],
      ["1", "1", "1", "1", "1", "1"],
      ["1", "1", "1", "1", "1", "1"],
      ["0", "1", "1", "1", "1", "0"],
      ["0", "0", "1", "1", "0", "0"],
    ],
    tones: [
      ["surface", "surface", "accent", "accent", "surface", "surface"],
      ["surface", "accent", "accent", "accent", "accent", "surface"],
      ["accent", "accent", "accent", "accent", "accent", "accent"],
      ["accent", "accent", "accent", "accent", "accent", "accent"],
      ["surface", "accent", "accent", "accent", "accent", "surface"],
      ["surface", "surface", "accent", "accent", "surface", "surface"],
    ],
    markKinds: ["block"],
    markIndex: [[0, 0, 3, 3]],
    markTones: ["warning"],
    markAt: [1],
    markMoves: [[1.9, 0, 1, 3, 3, 3.0, 0, 2, 3, 3, 4.1, 0, 3, 3, 3, 5.0, 1, 3, 3, 3]],
  },
  transpose: {
    cells: [
      ["1", "2", "3"],
      ["4", "5", "6"],
    ],
    keys: ["1", "2", "3", "4", "5", "6"],
    brackets: true,
    steps: [
      [
        ["1", "4"],
        ["2", "5"],
        ["3", "6"],
      ],
    ],
    stepKeys: [["1", "4", "2", "5", "3", "6"]],
    stepAt: [2.4],
  },
};

const ARITY = { row: 1, col: 1, cell: 2, block: 4 };
const SHAPE = { row: "[row]", col: "[col]", cell: "[row, col]", block: "[row, col, height, width]" };

const fail = (msg) => {
  throw new Error(`Grid: ${msg}`);
};

const chunk = (list, n) =>
  Array.from({ length: Math.floor(list.length / n) }, (_, i) => list.slice(i * n, (i + 1) * n));

// A vector at t: `first`, then each [time, ...to] of `moves` in turn,
// gliding for `dur`.
const glide = (t, first, moves, dur) => {
  let v = first;
  for (const [at, ...to] of moves) {
    const p = easeInOutCubic(clamp((t - at) / dur, 0, 1));
    v = v.map((x, i) => lerp(x, to[i], p));
  }
  return v;
};

// A mark's target as [row, col, height, width] in cells.
const span = (kind, v, rows, cols) =>
  kind === "row" ? [v[0], 0, 1, cols] : kind === "col" ? [0, v[0], rows, 1] : kind === "cell" ? [v[0], v[1], 1, 1] : v;

// Width of a TeX cell in em (a generous 0.6 a character; commands count one,
// \text's argument its letters, a fraction its wider side) and whether it is
// tall (a fraction, a sum).
const texEm = (tex) => {
  const flat = tex
    .replace(/\\[dt]?frac\{([^{}]*)\}\{([^{}]*)\}/g, (_, a, b) => (a.length >= b.length ? a : b))
    .replace(/\\(?:text|mathrm|mathbf|operatorname)\{([^}]*)\}/g, "$1")
    .replace(/\\[a-zA-Z]+/g, "x")
    .replace(/[{}^_\s]/g, "");
  return 0.6 * flat.length;
};
const texTall = (tex) => /\\(?:frac|dfrac|binom|begin)/.test(tex);

// The rows of one arrangement as a rectangle: every row the same length.
const checkRows = (what, rows) => {
  if (!Array.isArray(rows) || rows.length === 0) fail(`${what} is empty; give at least one row of cells`);
  const cols = rows[0].length;
  if (cols === 0) fail(`${what}[0] has no cells; give at least one column ("" is an empty cell)`);
  rows.forEach((row, r) => {
    if (row.length !== cols) fail(`${what}[${r}] has ${row.length} cells but ${what}[0] has ${cols}; every row needs the same number ("" is an empty cell)`);
  });
  return cols;
};

// One key per cell, row by row, each unique: a key is a cell's identity.
const checkKeys = (what, keys, count) => {
  if (keys.length !== count) fail(`${what} has ${keys.length} keys but the grid has ${count} cells; give one key per cell, row by row`);
  const first = {};
  keys.forEach((k, i) => {
    if (first[k] !== undefined) fail(`${what} has "${k}" at ${first[k]} and ${i}; a key names one cell, so make them unique (for example "${k}1", "${k}2")`);
    first[k] = i;
  });
};

// A mark's index (and each of its moves) against the grid's size.
const checkIndex = (name, kind, v, rows, cols) => {
  if (!Array.isArray(v) || v.length !== ARITY[kind] || v.some((x) => !Number.isFinite(x))) {
    fail(`${name} = ${JSON.stringify(v)} must be ${SHAPE[kind]} for a "${kind}" mark`);
  }
  const [r, k, h, w] = span(kind, v, rows, cols);
  if (r < 0 || r + h > rows) fail(`${name} = ${JSON.stringify(v)} reaches row ${r + h - 1}, but the grid has ${rows} rows (0 to ${rows - 1})`);
  if (k < 0 || k + w > cols) fail(`${name} = ${JSON.stringify(v)} reaches column ${k + w - 1}, but the grid has ${cols} columns (0 to ${cols - 1})`);
  if (h <= 0 || w <= 0) fail(`${name} = ${JSON.stringify(v)} needs a height and width above 0`);
};

/**
 * A grid of `cells` (rows of TeX, all the same length; "" is empty) with
 * `cell`-px squares, or fitted to `width` x `height` px (default most of the
 * frame; as if it had `fitRows` x `fitCols` cells) when `cell` is 0. `keys`:
 * an id per cell, row by row, so cells follow their values; `lift`: `{ key:
 * [x, y] }`, a cell's offset in cell sides (an arc). `tones`: a theme colour
 * per cell ("surface" plain, "bg" none). `appear`: seconds each value arrives
 * (below 0: with the grid). `marks`: `[{ kind: "row" | "col" | "cell" |
 * "block", index, tone, at, off, moves }]` with index `[row]`, `[col]`,
 * `[r, c]` or `[r, c, h, w]` and `moves` runs of `[time, ...index]`.
 * `pointers`: `[{ label (TeX), side, index: [i, ...], at: [time, ...], tone
 * }]`, `index` along the side, from `at[0]` (default `at`) and gliding to
 * `index[k]` at `at[k]`; pointers close on one side part to stay readable.
 * `enter`: how cells pop in from `at`, `each` apart.
 */
const Grid = (
  {
    cells, keys, tones = [], appear = [], rowLabels = [], colLabels = [], brackets = false, cell, fitRows, fitCols,
    width, height, lift = {}, enter = "diagonal", at = 0, each = 0.07, marks = [], pointers = [], id = "grid",
  },
  { t, theme, size },
) => {
  const { color, motion } = theme;
  const cols = checkRows("cells", cells);
  const rows = cells.length;
  const [R, C] = [Math.max(fitRows ?? 0, rows), Math.max(fitCols ?? 0, cols)];
  if (keys) checkKeys("keys", keys, rows * cols);
  marks.forEach((m, i) => {
    if (!(m.kind in ARITY)) fail(`marks[${i}].kind = ${JSON.stringify(m.kind)} must be "row", "col", "cell" or "block"`);
    checkIndex(`marks[${i}].index`, m.kind, m.index, R, C);
    (m.moves ?? []).forEach(([, ...to], j) => checkIndex(`marks[${i}].moves[${j}]`, m.kind, to, R, C));
  });
  pointers.forEach((p, i) => {
    if (!["above", "below", "left", "right"].includes(p.side)) fail(`pointers[${i}].side = ${JSON.stringify(p.side)} must be "above", "below", "left" or "right"`);
    const along = p.side === "above" || p.side === "below" ? C : R;
    if (!Array.isArray(p.index) || p.index.length === 0) fail(`pointers[${i}].index is missing; give the positions along the ${p.side} side, [0, ${along - 1}] at most`);
    if (p.index.some((v) => !Number.isFinite(v) || v < 0 || v > along - 1)) {
      fail(`pointers[${i}].index = ${JSON.stringify(p.index)} must lie in 0 to ${along - 1}: a pointer on the ${p.side} side points at one of ${along} ${along === C ? "columns" : "rows"}`);
    }
    if (p.at && p.at.length !== p.index.length) fail(`pointers[${i}].at has ${p.at.length} times but index has ${p.index.length} positions; give one time per position`);
  });
  const sides = new Set(pointers.map((p) => p.side));
  const flat = cells.flat();
  const labelUnits = rowLabels.length ? Math.max(0.6, 0.25 + 0.3 * Math.max(...rowLabels.map(texEm))) : 0;
  const gapK = brackets ? 0 : 0.06;
  const unitsW = C * (1 + gapK) - gapK + labelUnits + (brackets ? 0.8 : 0) + (sides.has("left") ? 1 : 0) + (sides.has("right") ? 1 : 0);
  const unitsH = R * (1 + gapK) - gapK + (colLabels.length ? 0.65 : 0) + (sides.has("above") ? 1 : 0) + (sides.has("below") ? 1 : 0);
  const [boxW, boxH] = [width ?? size.width * 0.88, height ?? size.height * 0.8];
  const c = cell || Math.min(boxW / unitsW, boxH / unitsH, 0.2 * boxH);
  // One font size for the grid, set by its typical entry (the 80th percentile
  // of widths), so one long outlier shrinks on its own and not every cell.
  const used = flat.filter(Boolean);
  const ems = used.map(texEm).sort((a, b) => a - b);
  const typical = ems.length ? ems[Math.floor(0.8 * (ems.length - 1))] : 0;
  const tall = used.filter(texTall).length > 0.2 * used.length;
  const fs = Math.min(tall ? 0.27 * c : 0.4 * c, (0.78 * c) / Math.max(typical, 0.6));
  const fontOf = (tex) => Math.min(fs, texTall(tex) ? 0.27 * c : fs, (0.78 * c) / Math.max(texEm(tex), 0.6));
  const gap = gapK * c;
  const pitch = c + gap;
  const gw = cols * pitch - gap;
  const gh = rows * pitch - gap;
  const bw = brackets ? 0.4 * c : 0;
  const lw = labelUnits * c;
  const lh = colLabels.length ? 0.65 * c : 0;
  const gx = bw + lw + (sides.has("left") ? c : 0);
  const gy = lh + (sides.has("above") ? c : 0);
  const W = gx + gw + bw + (sides.has("right") ? c : 0);
  const H = gy + gh + (sides.has("below") ? c : 0);
  const fade = (from, dur) => easeInOutCubic(clamp((t - from) / dur, 0, 1));
  const pop = (from) => (t < from ? 0 : spring(t - from, motion.spring.snappy));
  const popped = (p) => ({ scale: p, opacity: Math.min(1, 3 * p) });
  const order = (r, k) =>
    enter === "row" ? r : enter === "col" ? k : enter === "diagonal" ? r + k : enter === "cell" ? r * cols + k : 0;
  // The whole entrance lasts at most 1.2 s, however many cells there are.
  const stagger = enter === "all" ? 0 : Math.min(each, 1.2 / Math.max(1, order(rows - 1, cols - 1)));
  const arrive = (r, k) => at + order(r, k) * stagger;
  const live = (tone) => tone !== undefined && tone !== color.surface && tone !== color.bg;

  const fills = [];
  const values = [];
  for (let r = 0; r < rows; r++) {
    for (let k = 0; k < cols; k++) {
      const tone = tones[r]?.[k];
      const [left, top] = [gx + k * pitch, gy + r * pitch];
      if (tone !== color.bg) {
        const tint = live(tone) ? mix(brackets ? color.bg : color.surface, tone, 0.32) : brackets ? undefined : color.surface;
        fills.push(
          <Box
            key={`${id}/b${r}_${k}`}
            style={{
              position: "absolute", left, top, width: c, height: c, radius: 0.14 * c,
              ...(tint ? { fill: tint } : {}),
              ...(brackets ? {} : { border: { width: 0.025 * c, color: color.border } }),
              ...popped(pop(arrive(r, k))),
            }}
          />,
        );
      }
      const tex = cells[r][k];
      const raw = keys ? keys[r * cols + k] : undefined;
      const key = raw === undefined ? `${id}/c${r}_${k}` : `${id}/k${raw}`;
      if (tex) {
        const when = (appear[r]?.[k] ?? -1) >= 0 ? appear[r][k] : arrive(r, k);
        const [dx, dy] = lift[raw] ?? [0, 0];
        values.push(
          <Box
            key={key}
            style={{
              position: "absolute", left, top, width: c, height: c, alignItems: "center", justifyContent: "center",
              translateX: dx * c, translateY: dy * c,
              ...popped(pop(when)),
            }}
          >
            <Math key={`${key}/v`} tex={tex} display={false} style={{ fontSize: fontOf(tex), color: color.text }} />
          </Box>,
        );
      }
    }
  }

  const bands = marks.map((m, i) => {
    const first = span(m.kind, m.index, rows, cols);
    const moves = (m.moves ?? []).map(([when, ...to]) => [when, ...span(m.kind, to, rows, cols)]);
    const [r, k, h, w] = glide(t, first, moves, motion.duration.slow);
    const off = m.off === undefined || m.off < 0 ? 0 : fade(m.off, motion.duration.fast);
    const on = fade(m.at ?? 0, motion.duration.fast) * (1 - off);
    const e = 0.05 * c;
    const rect = {
      position: "absolute", left: gx + k * pitch - e, top: gy + r * pitch - e,
      width: w * pitch - gap + 2 * e, height: h * pitch - gap + 2 * e, radius: 0.2 * c,
    };
    return [
      <Box key={`${id}/m${i}/f`} style={{ ...rect, fill: m.tone, opacity: 0.2 * on }} />,
      <Box key={`${id}/m${i}`} style={{ ...rect, border: { width: 0.05 * c, color: m.tone }, opacity: on }} />,
    ];
  });

  const label = (key, tex, box, tint) => (
    <Box key={key} style={{ position: "absolute", ...box, alignItems: "center", justifyContent: "center", opacity: fade(at, motion.duration.base) }}>
      <Math key={`${key}/v`} tex={tex} display={false} style={{ fontSize: 0.3 * c, color: tint }} />
    </Box>
  );
  const rowTags = rowLabels.map((tex, r) =>
    tex ? label(`${id}/rl${r}`, tex, { left: gx - bw - lw, top: gy + r * pitch, width: lw - 0.1 * c, height: c }, color.muted) : null,
  );
  const colTags = colLabels.map((tex, k) =>
    tex ? label(`${id}/cl${k}`, tex, { left: gx + k * pitch, top: gy - lh, width: c, height: lh }, color.muted) : null,
  );

  const sq = 0.045 * c;
  const serif = 0.2 * c;
  const bracket = (name, x, left) => {
    const h = gh + 0.12 * c;
    const bar = { position: "absolute", fill: color.text, radius: sq / 2 };
    return (
      <Box
        key={`${id}/${name}`}
        style={{ position: "absolute", left: x, top: gy - 0.06 * c, width: serif, height: h, clipInset: [0, 0, 1 - fade(at, motion.duration.slow), 0] }}
      >
        <Box style={{ ...bar, left: left ? 0 : serif - sq, top: 0, width: sq, height: h }} />
        <Box style={{ ...bar, left: 0, top: 0, width: serif, height: sq }} />
        <Box style={{ ...bar, left: 0, top: h - sq, width: serif, height: sq }} />
      </Box>
    );
  };

  const tip = 0.3 * c;
  const tag = 0.5 * c;
  const turn = { below: 0, above: 180, left: 90, right: -90 };
  const wheres = pointers.map((p) => {
    const when = p.at ?? [at];
    return glide(t, [p.index[0]], p.index.slice(1).map((v, j) => [when[j + 1], v]), motion.duration.slow)[0];
  });
  // Pointers within a cell and a third of each other on a side part along it,
  // in the order they sit (a tie keeps list order): labels by half a cell,
  // heads by less, so each still points into its own cell; eased in and out.
  const parted = (i) => {
    let s = 0;
    pointers.forEach((q, j) => {
      if (j === i || q.side !== pointers[i].side) return;
      const d = wheres[i] - wheres[j];
      const near = clamp((1.35 - Math.abs(d)) / 0.5, 0, 1);
      s += near * clamp((d + (i < j ? -0.4 : 0.4)) / 0.4, -1, 1);
    });
    return s;
  };
  const arrows = pointers.map((p, i) => {
    const shown = fade((p.at ?? [at])[0], motion.duration.base);
    const ink = p.tone ?? color.accent;
    const flat = p.side === "below" || p.side === "above";
    const part = parted(i);
    const shift = (k) => (flat ? { translateX: k * part * c } : { translateY: k * part * c });
    const head = (
      <Box style={{ width: tip, height: tip, ...shift(0.18) }}>
        <Path d="M0 10 L10 10 L5 0 Z" viewBox={[0, 0, 10, 10]} fill={ink} style={{ width: tip, height: tip, rotate: turn[p.side] }} />
      </Box>
    );
    const name = (w) => (
      <Box style={{ width: w, height: tag, alignItems: "center", justifyContent: "center", ...shift(0.55) }}>
        <Math key={`${id}/ptr${i}/v`} tex={p.label} display={false} style={{ fontSize: 0.34 * c, color: ink }} />
      </Box>
    );
    const key = `${id}/ptr${i}`;
    const slide = (1 - shown) * 0.25 * c;
    const along = wheres[i] * pitch;
    if (flat) {
      const down = p.side === "below";
      return (
        <Stack key={key} align="center" style={{ position: "absolute", left: gx + along, top: down ? gy + gh + 0.1 * c : gy - lh - 0.1 * c - tip - tag, width: c, opacity: shown, translateY: down ? slide : -slide }}>
          {down ? head : name(c)}
          {down ? name(c) : head}
        </Stack>
      );
    }
    const west = p.side === "left";
    const w = tip + 0.6 * c;
    return (
      <Row key={key} align="center" style={{ position: "absolute", left: west ? gx - bw - lw - 0.1 * c - w : gx + gw + bw + 0.1 * c, top: gy + along, width: w, height: c, opacity: shown, translateX: west ? -slide : slide }}>
        {west ? name(0.6 * c) : head}
        {west ? head : name(0.6 * c)}
      </Row>
    );
  });

  return (
    <Box key={id} style={{ width: W, height: H }}>
      {fills}
      {bands}
      {values}
      {rowTags}
      {colTags}
      {brackets ? bracket("lb", gx - 0.1 * c - serif, true) : null}
      {brackets ? bracket("rb", gx + gw + 0.1 * c, false) : null}
      {arrows}
    </Box>
  );
};

// Where each key sits, [row, col], in one arrangement.
const placesOf = (s) => {
  const cols = s.cells[0].length;
  return Object.fromEntries(s.keys.map((k, i) => [k, [Math.floor(i / cols), i % cols]]));
};

// The two keys that trade places between arrangements a and b, as
// [{ key, from, to }, ...]; null unless exactly two cells move, and swap.
const exchange = (a, b) => {
  if (!a.keys || !b.keys) return null;
  const [pa, pb] = [placesOf(a), placesOf(b)];
  const moved = Object.keys(pa).filter((k) => pb[k] && (pa[k][0] !== pb[k][0] || pa[k][1] !== pb[k][1]));
  if (moved.length !== 2) return null;
  const [x, y] = moved;
  return pa[x].join() === pb[y].join() && pa[y].join() === pb[x].join()
    ? moved.map((key) => ({ key, from: pa[key], to: pb[key] }))
    : null;
};

// `lift` at t for a Steps of `states` at `stepAt` gliding `dur`: the two cells
// of a swap bow to opposite sides of their path (half a cell and a bit), back
// to nothing as they land, so they pass each other rather than through.
const arcs = (states, stepAt, dur, t) => {
  const lift = {};
  stepAt.forEach((from, k) => {
    const p = (t - from) / dur;
    const swap = p > 0 && p < 1 && t < (stepAt[k + 1] ?? Infinity) ? exchange(states[k], states[k + 1]) : null;
    if (!swap) return;
    const bow = 0.55 * Math.sin(Math.PI * easeInOutCubic(p));
    for (const { key, from: a, to: b } of swap) {
      const [dr, dc] = [b[0] - a[0], b[1] - a[1]];
      const len = Math.hypot(dr, dc);
      lift[key] = [(bow * dr) / len, (-bow * dc) / len];
    }
  });
  return lift;
};

export default (p, { t, theme, size }) => {
  const { color, motion, space } = theme;
  const n = p.markKinds.length;
  const fits = (name, list, count, what) => {
    if (list.length > count) fail(`${name} has ${list.length} entries but ${what} has ${count}; ${name} runs parallel to ${what}`);
  };
  for (const name of ["markIndex", "markTones", "markAt", "markOff", "markMoves"]) fits(name, p[name], n, "markKinds");
  for (const name of ["pointerSides", "pointerIndex", "pointerAt", "pointerTones"]) fits(name, p[name], p.pointers.length, "pointers");
  if (p.stepKeys.length > p.steps.length) fail(`stepKeys has ${p.stepKeys.length} entries but steps has ${p.steps.length}; give one per step`);
  if (p.stepAt.length !== p.steps.length) fail(`stepAt has ${p.stepAt.length} times but steps has ${p.steps.length} arrangements; give the time each is reached`);
  if (!p.keys && p.stepKeys.length) fail("stepKeys is set but keys is not; give keys for the first arrangement too");
  if (p.keys) {
    p.steps.forEach((_, i) => {
      if (!p.stepKeys[i]) fail(`stepKeys[${i}] is missing; keys is set, so every step needs its keys (one per cell, row by row)`);
    });
  }
  const marks = p.markKinds.map((kind, i) => {
    if (!p.markIndex[i]) fail(`markIndex[${i}] is missing; markKinds[${i}] is "${kind}", so give ${SHAPE[kind]}`);
    const moves = p.markMoves[i] ?? [];
    if (moves.length % (ARITY[kind] + 1)) fail(`markMoves[${i}] has ${moves.length} numbers; a "${kind}" move is a time then ${SHAPE[kind]}, ${ARITY[kind] + 1} numbers each`);
    return {
      kind,
      index: p.markIndex[i],
      tone: p.markTones[i] ?? color.accent,
      at: p.markAt[i],
      off: p.markOff[i],
      moves: chunk(moves, ARITY[kind] + 1),
    };
  });
  const pointers = p.pointers.map((label, i) => ({
    label,
    side: p.pointerSides[i] ?? "below",
    index: p.pointerIndex[i],
    at: p.pointerAt[i]?.length ? p.pointerAt[i] : undefined,
    tone: p.pointerTones[i],
  }));
  const states = [{ cells: p.cells, keys: p.keys }, ...p.steps.map((cells, i) => ({ cells, keys: p.stepKeys[i] }))];
  states.forEach((s, i) => {
    const what = i ? `steps[${i - 1}]` : "cells";
    checkRows(what, s.cells);
    if (s.keys) checkKeys(i ? `stepKeys[${i - 1}]` : "keys", s.keys, s.cells.length * s.cells[0].length);
  });
  const fitRows = Math.max(...states.map((s) => s.cells.length));
  const fitCols = Math.max(...states.map((s) => s.cells[0].length));
  // The frame less a margin all round (the theme's widest space, scaled to
  // the frame), so the grid fits at any video size.
  const margin = space.xxl * Math.min(size.width / 1920, size.height / 1080);
  const lift = arcs(states, p.stepAt, motion.duration.slow, t);
  const grid = (s) => (
    <Grid
      cells={s.cells}
      keys={s.keys}
      tones={p.tones}
      appear={p.appear}
      rowLabels={p.rowLabels}
      colLabels={p.colLabels}
      brackets={p.brackets}
      cell={p.cell}
      fitRows={fitRows}
      fitCols={fitCols}
      width={size.width - 2 * margin}
      height={size.height - 2 * margin}
      lift={lift}
      enter={p.enter}
      at={p.at}
      each={p.each}
      marks={marks}
      pointers={pointers}
    />
  );
  return (
    <Stack justify="center" align="center" style={{ flexGrow: 1, fill: color.bg }}>
      {states.length > 1 ? (
        <Steps at={p.stepAt} dur={motion.duration.slow}>
          {states.map(grid)}
        </Steps>
      ) : (
        grid(states[0])
      )}
    </Stack>
  );
};
