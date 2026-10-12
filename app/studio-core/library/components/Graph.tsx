// Graph: nodes and edges laid out on a circle, as a tree, in columns or by
// hand. `layout()` maps a graph to px positions; `Graph` draws the nodes
// (popping in), the edges (drawn on, directed ones with a head on the
// shaft's true end tangent, a pair of opposite edges bowed apart, a
// self-loop as a loop) and their weights. Time drives the story: `visit`
// recolours nodes in order and lights the edge each was reached by, `path`
// walks a marker along a node sequence. Keys: nodes `<id>/n<i>` (label
// `<id>/l<i>`), edges `<id>/e<a>-<b>` (weight `<id>/w<a>-<b>`), the
// walk `<id>/p<j>` and its marker `<id>/here`, so two Graphs with one id
// and the same nodes glide between layouts in a Steps. Edges between one
// pair fan out; a plain-text label too long for its node is set beside it
// (same key); a bad index or a path off the edges throws a readable error.
// Copy `layout` and `Graph` into a scene component to compose them.
export const props = {
  // A TeX label per node ("" for none); two or more plain letters are set as text.
  nodes: list(latex()),
  // Pairs [from, to] of node indices; [a, a] is a self-loop.
  edges: list(list(num())),
  // A TeX weight per edge, in edge order.
  weights: list(latex()).default([]),
  directed: bool().default(false),
  layout: oneOf(["circle", "tree", "layers", "manual"]).default("circle"),
  // tree: the root node. layers: nodes per column (else columns by longest path).
  root: num().default(0),
  columns: list(num()).default([]),
  // manual: each node's x and y in 0..1.
  xs: list(num()).default([]),
  ys: list(num()).default([]),
  // A theme colour per node that tints it at rest; "faint" leaves a node plain.
  tones: list(token("color")).default([]),
  // Nodes pop in from `at`, edges draw on from edgesAt (default after the nodes).
  at: num().default(0.2),
  edgesAt: num().optional(),
  // Nodes in the order they are reached, at visitAt[k] (else visitStart + k * each).
  visit: list(num()).default([]),
  visitAt: list(num()).default([]),
  visitStart: num().default(2),
  each: num().default(0.6),
  // The node each visited node was reached from (-1 for none); default: the earliest visited neighbour.
  via: list(num()).default([]),
  // Which edges light as nodes are reached: the one each came by, all between visited nodes, or none.
  trace: oneOf(["tree", "all", "none"]).default("tree"),
  // A node sequence a marker walks from walkAt, a step every walkEach.
  path: list(num()).default([]),
  walkAt: num().default(2),
  walkEach: num().default(0.8),
  // Largest node radius in px; it shrinks when nodes crowd. The demo scales it with the frame.
  radius: num().default(60),
  tone: token("color").default("accent"),
  pathTone: token("color").default("positive"),
};

export const examples = {
  bfs: {
    nodes: ["A", "B", "C", "D", "E", "F", "G", "H"],
    edges: [[0, 1], [1, 2], [2, 3], [3, 4], [4, 5], [5, 6], [6, 7], [7, 0], [0, 3], [4, 7]],
    layout: "circle",
    visit: [0, 1, 7, 3, 2, 6, 4, 5],
    visitStart: 2.2,
    each: 0.5,
  },
  search: {
    nodes: ["8", "4", "12", "2", "6", "10", "14", "1", "3", "5", "7", "9", "11", "13", "15"],
    edges: [
      [0, 1], [0, 2], [1, 3], [1, 4], [2, 5], [2, 6],
      [3, 7], [3, 8], [4, 9], [4, 10], [5, 11], [5, 12], [6, 13], [6, 14],
    ],
    layout: "tree",
    path: [0, 1, 4, 9],
    walkAt: 2.4,
    walkEach: 0.9,
  },
  weather: {
    nodes: ["Sun", "Cloud", "Rain"],
    edges: [[0, 0], [0, 1], [0, 2], [1, 0], [1, 1], [1, 2], [2, 0], [2, 1], [2, 2]],
    weights: ["0.6", "0.3", "0.1", "0.3", "0.4", "0.3", "0.2", "0.4", "0.4"],
    directed: true,
    layout: "manual",
    xs: [0.16, 0.84, 0.5],
    ys: [0.12, 0.12, 0.92],
    path: [0, 0, 1, 2, 2, 0],
    walkAt: 2.2,
    walkEach: 0.65,
    radius: 84,
  },
  network: {
    nodes: ["x_1", "x_2", "x_3", "", "", "", "", "", "", "", "", "y_1", "y_2"],
    edges: [
      [0, 3], [0, 4], [0, 5], [0, 6], [1, 3], [1, 4], [1, 5], [1, 6], [2, 3], [2, 4], [2, 5], [2, 6],
      [3, 7], [3, 8], [3, 9], [3, 10], [4, 7], [4, 8], [4, 9], [4, 10],
      [5, 7], [5, 8], [5, 9], [5, 10], [6, 7], [6, 8], [6, 9], [6, 10],
      [7, 11], [7, 12], [8, 11], [8, 12], [9, 11], [9, 12], [10, 11], [10, 12],
    ],
    layout: "layers",
    columns: [3, 4, 4, 2],
    visit: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
    visitAt: [2.2, 2.2, 2.2, 3, 3, 3, 3, 3.8, 3.8, 3.8, 3.8, 4.6, 4.6],
    trace: "all",
    radius: 38,
  },
  schedule: {
    nodes: ["A", "B", "C", "D", "E", "F", "G"],
    edges: [[0, 1], [0, 2], [1, 3], [2, 3], [2, 4], [3, 5], [4, 5], [5, 6]],
    directed: true,
    layout: "layers",
    path: [0, 2, 3, 5, 6],
    walkAt: 2.4,
    walkEach: 0.8,
  },
};

/**
 * Centres of `nodes` as [x, y] px in a `width` by `height` box, `pad` px
 * in from each side (one number, or [left, top, right, bottom]). `kind`:
 * "circle" (node 0 at the top, clockwise); "tree" (top-down from `root`:
 * leaves take consecutive slots, a parent sits over its children, any
 * fan-out; nodes `root` cannot reach start further trees); "layers" (left
 * to right: `columns` counts nodes per column in index order, else a node's
 * column is its longest path from a source, ordered to cut crossings);
 * "manual" (`xs`, `ys` in 0..1).
 */
const layout = (kind, nodes, edges, { width, height, root = 0, columns = [], xs = [], ys = [], pad = 0 }) => {
  const n = nodes.length;
  const [pl, pt, pr, pb] = Array.isArray(pad) ? pad : [pad, pad, pad, pad];
  const [w, h] = [width - pl - pr, height - pt - pb];
  const idx = nodes.map((_, i) => i);
  if (kind === "manual") return idx.map((i) => [pl + (xs[i] ?? 0.5) * w, pt + (ys[i] ?? 0.5) * h]);
  if (kind === "circle") {
    const R = n > 1 ? Math.min(w, h) / 2 : 0;
    return idx.map((i) => {
      const a = -Math.PI / 2 + (2 * Math.PI * i) / n;
      return [pl + w / 2 + R * Math.cos(a), pt + h / 2 + R * Math.sin(a)];
    });
  }
  if (kind === "tree") {
    const adj = idx.map(() => []);
    for (const [a, b] of edges) {
      if (a !== b) {
        adj[a].push(b);
        adj[b].push(a);
      }
    }
    const kids = idx.map(() => []);
    const depth = idx.map(() => -1);
    const roots = [];
    for (const r of [root, ...idx]) {
      if (depth[r] >= 0) continue;
      roots.push(r);
      depth[r] = 0;
      const queue = [r];
      while (queue.length) {
        const v = queue.shift();
        for (const u of adj[v]) {
          if (depth[u] < 0) {
            depth[u] = depth[v] + 1;
            kids[v].push(u);
            queue.push(u);
          }
        }
      }
    }
    const slot = idx.map(() => 0);
    let leaves = 0;
    const place = (v) => {
      if (!kids[v].length) {
        slot[v] = leaves++;
        return;
      }
      kids[v].forEach(place);
      slot[v] = (slot[kids[v][0]] + slot[kids[v][kids[v].length - 1]]) / 2;
    };
    roots.forEach(place);
    const deepest = Math.max(...depth);
    return idx.map((i) => [
      pl + (leaves > 1 ? slot[i] / (leaves - 1) : 0.5) * w,
      pt + (deepest > 0 ? depth[i] / deepest : 0.5) * h,
    ]);
  }
  const layer = idx.map(() => 0);
  if (columns.length) {
    let k = 0;
    columns.forEach((count, j) => {
      for (let m = 0; m < count && k < n; m++) layer[k++] = j;
    });
    while (k < n) layer[k++] = columns.length - 1;
  } else {
    for (let pass = 0; pass < n; pass++) {
      for (const [a, b] of edges) {
        if (a !== b && layer[b] <= layer[a] && layer[a] < n - 1) layer[b] = layer[a] + 1;
      }
    }
  }
  const count = Math.max(...layer) + 1;
  const cols = Array.from({ length: count }, (_, l) => idx.filter((i) => layer[i] === l));
  if (!columns.length) {
    // Order each column by the mean row of its neighbours in the column before, then after.
    const row = idx.map(() => 0);
    cols.forEach((c) => c.forEach((v, j) => (row[v] = j)));
    const sweep = (l, side) => {
      const mean = (v) => {
        const near = [];
        for (const [a, b] of edges) {
          if (a === v && layer[b] === l + side) near.push(row[b]);
          if (b === v && layer[a] === l + side) near.push(row[a]);
        }
        return near.length ? near.reduce((s, x) => s + x, 0) / near.length : row[v];
      };
      cols[l].sort((p, q) => mean(p) - mean(q) || row[p] - row[q]);
      cols[l].forEach((v, j) => (row[v] = j));
    };
    for (let l = 1; l < count; l++) sweep(l, -1);
    for (let l = count - 2; l >= 0; l--) sweep(l, 1);
  }
  const tallest = Math.max(...cols.map((c) => c.length));
  const out = idx.map(() => [0, 0]);
  cols.forEach((c, l) => {
    c.forEach((v, j) => {
      const step = tallest > 1 ? h / (tallest - 1) : 0;
      out[v] = [pl + (count > 1 ? l / (count - 1) : 0.5) * w, pt + h / 2 + (j - (c.length - 1) / 2) * step];
    });
  });
  return out;
};

/** Plain words (hyphens allowed inside a word): set as text, and moved outside a node that is too small to hold them. */
const isPlain = (s) => /^[A-Za-z]{2,}(-[A-Za-z]{2,})*( [A-Za-z0-9]+(-[A-Za-z0-9]+)*)*$/.test(s);

/** TeX for a label: plain words and single capitals are text, anything else is maths. */
const labelTex = (s) => (isPlain(s) || /^[A-Z]$/.test(s) ? `\\text{${s}}` : s);

/** Rough px width of `s` set as text at font size `size`. */
const textW = (s, size) => 0.6 * size * s.length;

/** `s` split at spaces into at most three lines, as narrow as that allows (from 14 letters wide). */
const wrapWords = (s) => {
  const words = s.split(" ");
  for (let chars = 14; ; chars += 2) {
    const lines = [];
    for (const word of words) {
      const last = lines.length - 1;
      if (last >= 0 && lines[last].length + 1 + word.length <= chars) lines[last] += ` ${word}`;
      else lines.push(word);
    }
    if (lines.length <= 3) return lines;
  }
};

/** The label size that fits `label` across a node of radius `r`. */
const fitIn = (label, r) => (2.28 * r) / glyphs(label);

/** Rough count of glyphs a TeX label draws, to size it (a relation or operator spaced out counts twice). */
const glyphs = (s) => Math.max(1, s.replace(/\\[a-zA-Z]+|[{}^_$\\]/g, "").replace(/[=+<>-]/g, "$&$&").length);

/**
 * A self-loop on a node of radius `r` centred at the origin, opening
 * towards unit vector `u`: a circle of radius 0.7 r that meets the node 36
 * degrees either side of `u`. Returns the shaft's path data, the head's
 * triangle (null without a head; a head of `head` px sits at the arc's end
 * and the shaft stops at its base) and how far the loop reaches from the
 * centre.
 */
const loopShape = (r, u, head) => {
  const alpha = 0.2 * Math.PI;
  const lr = 0.7 * r;
  const h = r * Math.sin(alpha);
  const dc = r * Math.cos(alpha) + Math.sqrt(lr * lr - h * h);
  const span = Math.PI - Math.asin(h / lr);
  const spin = ([x, y]) => [x * u[0] - y * u[1], x * u[1] + y * u[0]];
  const on = (phi) => spin([dc + lr * Math.cos(phi), lr * Math.sin(phi)]);
  const back = head > 0 ? (0.6 * head) / lr : 0;
  const [a, b] = [on(-span), on(span - back)];
  const d = `M${a[0]} ${a[1]} A${lr} ${lr} 0 1 1 ${b[0]} ${b[1]}`;
  if (head <= 0) return [d, null, dc + lr];
  const tip = on(span);
  const dir = spin([-Math.sin(span), Math.cos(span)]);
  const base = [tip[0] - dir[0] * head, tip[1] - dir[1] * head];
  const side = [-dir[1] * head * 0.5, dir[0] * head * 0.5];
  return [d, [tip, [base[0] + side[0], base[1] + side[1]], [base[0] - side[0], base[1] - side[1]]], dc + lr];
};

/** Throws a readable error for an input the graph cannot be drawn from. */
const check = ({ nodes, edges, weights, directed, kind, root, columns, xs, ys, visit, via, path }) => {
  const n = nodes.length;
  const fail = (msg) => {
    throw new Error(`Graph: ${msg}`);
  };
  const range = `there are ${n} node${n === 1 ? "" : "s"} (indices 0 to ${n - 1})`;
  const node = (v, what) => {
    if (!Number.isInteger(v) || v < 0 || v >= n) fail(`${what} names node ${JSON.stringify(v)}, but ${range}`);
  };
  const joins = (a, b) => edges.some(([x, y]) => (x === a && y === b) || (!directed && x === b && y === a));
  if (!["circle", "tree", "layers", "manual"].includes(kind)) fail(`layout ${JSON.stringify(kind)} is not "circle", "tree", "layers" or "manual"`);
  if (n === 0) fail("nodes is empty; give at least one label");
  edges.forEach((e, i) => {
    if (!Array.isArray(e) || e.length !== 2) fail(`edges[${i}] = ${JSON.stringify(e)} is not a [from, to] pair of node indices`);
    node(e[0], `edges[${i}] = ${JSON.stringify(e)}`);
    node(e[1], `edges[${i}] = ${JSON.stringify(e)}`);
  });
  if (weights.length > edges.length) fail(`weights has ${weights.length} entries for ${edges.length} edges; give one per edge, or fewer to leave the last edges unweighted`);
  visit.forEach((v, k) => node(v, `visit[${k}]`));
  if (via.length > visit.length) fail(`via has ${via.length} entries for ${visit.length} visited nodes; give one per visit, -1 for none`);
  via.forEach((v, k) => {
    if (v === -1) return;
    node(v, `via[${k}]`);
    if (!joins(v, visit[k])) fail(`via[${k}] = ${v}, but no edge ${directed ? "runs from" : "joins"} node ${v} ${directed ? "to" : "and"} node ${visit[k]} (visit[${k}])`);
  });
  path.forEach((v, j) => node(v, `path[${j}]`));
  for (let j = 1; j < path.length; j++) {
    const [a, b] = [path[j - 1], path[j]];
    if (!joins(a, b)) fail(`path[${j - 1}], path[${j}] = ${a}, ${b}, but no edge ${a === b ? `[${a}, ${a}] loops` : directed ? `runs from node ${a} to node ${b}` : `joins nodes ${a} and ${b}`}; a path must follow edges`);
  }
  if (kind === "tree") node(root, "root");
  if (kind === "layers" && columns.length) {
    const total = columns.reduce((sum, c) => sum + c, 0);
    if (columns.some((c) => !Number.isInteger(c) || c < 0) || total !== n) fail(`columns ${JSON.stringify(columns)} must be counts that add up to the ${n} nodes, but add up to ${total}`);
  }
  if (kind === "manual") {
    if (xs.length < n || ys.length < n) fail(`manual layout needs an x and a y (0 to 1) per node: ${n} nodes, but xs has ${xs.length} and ys has ${ys.length}`);
  }
};

/**
 * The graph in a `width` by `height` box, everything drawn inside it. The
 * node radius is at most `radius` and shrinks to a third of the closest
 * pair's distance; the box's edges leave room for loops and weights.
 * Nodes pop in from `at`, a beat apart; edges draw on from `edgesAt`
 * (default just after the nodes). See the props for `visit`, `via`,
 * `trace` and `path`.
 */
const Graph = (
  {
    nodes, edges, weights = [], directed = false, kind = "circle", width = 1600, height = 880,
    root = 0, columns = [], xs = [], ys = [], tones = [], at = 0, edgesAt,
    visit = [], visitAt = [], visitStart = 0, each = 0.6, via = [], trace = "tree",
    path = [], walkAt = 0, walkEach = 0.8, radius = 60, tone, pathTone, id = "graph",
  },
  { t, theme },
) => {
  check({ nodes, edges, weights, directed, kind, root, columns, xs, ys, visit, via, path });
  const { color, motion } = theme;
  const ink = tone ?? color.accent;
  const walkInk = pathTone ?? color.positive;
  const n = nodes.length;
  const m = edges.length;
  const tagged = (e) => weights[e] !== undefined && weights[e] !== "";

  // Positions, edge shapes and label places for node radius r and box padding pad.
  const arrange = (r, pad) => {
    const w = clamp(r * 0.07, 2, 5) * (m > 24 ? 0.55 : 1);
    const head = directed ? Math.max(14, 5 * w) : 0;
    const hug = r + (directed ? w : 0.5 * w);
    const wfs = clamp(r * 0.55, 26, 40);
    const ring = clamp(r * 0.06, 2, 5);
    const pos = layout(kind, nodes, edges, { width, height, root, columns, xs, ys, pad });
    const mid = [0, 1].map((k) => pos.reduce((s, p) => s + p[k], 0) / Math.max(1, n));
    const outward = (v) => {
      const d = [pos[v][0] - mid[0], pos[v][1] - mid[1]];
      const len = Math.hypot(d[0], d[1]);
      return len < 1e-6 ? [0, -1] : [d[0] / len, d[1] / len];
    };
    // The edge a to b between the circles' rims, its ends turned `shift` rad
    // off the centre line to the side it bows, drawn as a quadratic whose
    // control point sits off the chord's middle (positive `bend` degrees bows left).
    const shaft = (a, b, bend, shift = 0) => {
      const len0 = Math.hypot(pos[b][0] - pos[a][0], pos[b][1] - pos[a][1]) || 1;
      const u = [(pos[b][0] - pos[a][0]) / len0, (pos[b][1] - pos[a][1]) / len0];
      const normal = [-u[1], u[0]];
      const [fwd, side] = [Math.cos(shift) * hug, Math.sin(shift) * hug];
      const p0 = [pos[a][0] + u[0] * fwd - normal[0] * side, pos[a][1] + u[1] * fwd - normal[1] * side];
      const p1 = [pos[b][0] - u[0] * fwd - normal[0] * side, pos[b][1] - u[1] * fwd - normal[1] * side];
      const len = Math.hypot(p1[0] - p0[0], p1[1] - p0[1]);
      const lift = (len / 2) * Math.tan((bend * Math.PI) / 360);
      const ctrl = [(p0[0] + p1[0]) / 2 - normal[0] * lift, (p0[1] + p1[1]) / 2 - normal[1] * lift];
      const along = (s) => [
        (1 - s) ** 2 * p0[0] + 2 * (1 - s) * s * ctrl[0] + s * s * p1[0],
        (1 - s) ** 2 * p0[1] + 2 * (1 - s) * s * ctrl[1] + s * s * p1[1],
      ];
      return { a, b, u, normal, bend, len, p0, p1, along, apex: along(0.5) };
    };
    // A straight edge that would graze another node bows round it.
    const bendFor = (a, b) => {
      const [dx, dy] = [pos[b][0] - pos[a][0], pos[b][1] - pos[a][1]];
      const len = Math.hypot(dx, dy) || 1;
      const room = 1.35 * r;
      let bend = 0;
      let worst = room;
      for (let k = 0; k < n; k++) {
        if (k === a || k === b) continue;
        const [px, py] = [pos[k][0] - pos[a][0], pos[k][1] - pos[a][1]];
        const s = (px * dx + py * dy) / (len * len);
        const off = (-px * dy + py * dx) / len;
        if (s <= 0 || s >= 1 || Math.abs(off) >= worst) continue;
        worst = Math.abs(off);
        const need = (room - Math.abs(off)) / (4 * s * (1 - s));
        const deg = clamp((360 * Math.atan((4 * need) / len)) / Math.PI, 14, 80);
        bend = off >= 0 ? deg : -deg;
      }
      return bend;
    };
    // Edges between one pair of nodes (either way round) fan out side by side.
    const pairKey = (a, b) => (a < b ? `${a},${b}` : `${b},${a}`);
    const lanes = {};
    edges.forEach(([a, b], e) => {
      if (a !== b) lanes[pairKey(a, b)] = [...(lanes[pairKey(a, b)] ?? []), e];
    });
    const shapes = edges.map(([a, b], e) => {
      if (a === b) return null;
      const group = lanes[pairKey(a, b)];
      if (group.length === 1) return shaft(a, b, bendFor(a, b));
      if (directed && group.length === 2 && edges[group[0]][0] !== edges[group[1]][0]) return shaft(a, b, 16, 0.4);
      return shaft(a, b, (a < b ? 1 : -1) * (group.indexOf(e) - (group.length - 1) / 2) * clamp(120 / group.length, 24, 48));
    });
    const extent = (text, size, dir) => 0.5 * (Math.abs(dir[0]) * glyphs(text) * 0.75 * size + Math.abs(dir[1]) * 1.45 * size);
    // Each edge's loop (self-loops) and weight label: [x, y] of its centre.
    const loops = edges.map(([a, b]) => (a === b ? loopShape(r, outward(a), head) : null));
    const tags = edges.map(([a, b], e) => {
      if (!tagged(e)) return null;
      const text = weights[e];
      if (a === b) {
        const u = outward(a);
        const away = loops[e][2] + extent(text, wfs, u) + 14;
        return [pos[a][0] + u[0] * away, pos[a][1] + u[1] * away];
      }
      // On the outer side of a bowed edge, else the upper one.
      const s = shapes[e];
      let side = s.bend === 0 ? s.normal : s.bend > 0 ? [-s.normal[0], -s.normal[1]] : s.normal;
      if (s.bend === 0 && (side[1] > 0.01 || (Math.abs(side[1]) <= 0.01 && side[0] < 0))) side = [-side[0], -side[1]];
      const away = extent(text, wfs, side) + 0.6 * w + 8;
      return [s.apex[0] + side[0] * away, s.apex[1] + side[1] * away];
    });
    // The loop at node a as a disk, and each weight as a rect, for labels to avoid.
    const loopDisk = (a, e) => {
      const u = outward(a);
      const reach = loops[e][2] - 0.7 * r;
      return [pos[a][0] + u[0] * reach, pos[a][1] + u[1] * reach, 0.7 * r + w];
    };
    const tagRects = tags.flatMap((c, e) => {
      if (!c) return [];
      const half = [0.5 * glyphs(weights[e]) * 0.75 * wfs, 0.5 * 1.45 * wfs];
      return [[c[0] - half[0], c[1] - half[1], c[0] + half[0], c[1] + half[1]]];
    });
    // A plain-text label too long to read inside its node goes outside, at the side
    // (below first; outward on a circle) that crosses the fewest edges, nodes and weights.
    const dots = shapes.flatMap((s) => (s ? Array.from({ length: 17 }, (_, k) => s.along(k / 16)) : []));
    const disks = edges.flatMap(([a, b], e) => (a === b ? [loopDisk(a, e)] : []));
    const outside = [];
    const set = nodes.map((label, i) => {
      if (!isPlain(label) || fitIn(label, r) >= 0.37 * radius) return null;
      const size = clamp(0.7 * r, 0.4 * radius, 0.7 * radius);
      const lines = wrapWords(label);
      const [lw, lh] = [Math.max(...lines.map((l) => textW(l, size))) + 0.3 * size, 1.3 * size * lines.length];
      const gap = 6 + 2 * ring;
      const down = [[0, 1], [0, -1], [1, 0], [-1, 0], [0.7071, 0.7071], [-0.7071, 0.7071], [0.7071, -0.7071], [-0.7071, -0.7071]];
      const dirs = kind === "circle" ? [outward(i), ...down] : down;
      let best = null;
      dirs.forEach((d, rank) => {
        const k = Math.max(Math.abs(d[0]), Math.abs(d[1]));
        const c = [pos[i][0] + d[0] * (r + gap) + (d[0] / k) * (lw / 2), pos[i][1] + d[1] * (r + gap) + (d[1] / k) * (lh / 2)];
        const rc = [c[0] - lw / 2, c[1] - lh / 2, c[0] + lw / 2, c[1] + lh / 2];
        const near = ([x, y], rad) => Math.hypot(x - clamp(x, rc[0], rc[2]), y - clamp(y, rc[1], rc[3])) < rad;
        const over = (q) => rc[0] < q[2] && q[0] < rc[2] && rc[1] < q[3] && q[1] < rc[3];
        const cost =
          dots.filter(([x, y]) => x > rc[0] - 6 && x < rc[2] + 6 && y > rc[1] - 6 && y < rc[3] + 6).length +
          20 * pos.filter((q, k) => k !== i && near(q, r + 4)).length +
          20 * disks.filter(([x, y, rad]) => near([x, y], rad)).length +
          10 * tagRects.filter(over).length +
          20 * outside.filter(over).length +
          0.5 * rank;
        if (!best || cost < best.cost) best = { cost, c, rc };
      });
      outside.push(best.rc);
      return { c: best.c, lines, size, w: lw, h: lh };
    });
    // The box everything occupies.
    const box = [Infinity, Infinity, -Infinity, -Infinity];
    const grow = (x0, y0, x1, y1, m = 0) => {
      box[0] = Math.min(box[0], x0 - m);
      box[1] = Math.min(box[1], y0 - m);
      box[2] = Math.max(box[2], x1 + m);
      box[3] = Math.max(box[3], y1 + m);
    };
    // Text keeps a margin off the frame; its box is wider than the glyphs.
    const air = 0.5 * wfs;
    pos.forEach(([x, y]) => grow(x - r, y - r, x + r, y + r));
    edges.forEach(([a], e) => {
      if (loops[e]) {
        const [x, y, rad] = loopDisk(a, e);
        grow(x - rad, y - rad, x + rad, y + rad);
      }
      if (tags[e]) {
        const half = [0.5 * glyphs(weights[e]) * 0.75 * wfs, 0.5 * 1.45 * wfs];
        grow(tags[e][0] - half[0], tags[e][1] - half[1], tags[e][0] + half[0], tags[e][1] + half[1], air);
      }
    });
    outside.forEach((rc) => grow(...rc, air));
    return { pos, shapes, loops, tags, set, box, outward, shaft, w, head, hug, wfs, ring };
  };

  // Settle the radius and padding: shrink nodes that crowd, and pad each side by what pokes out of it.
  const closest = (pos) => {
    let best = Infinity;
    for (let i = 0; i < n; i++) for (let j = i + 1; j < n; j++) best = Math.min(best, Math.hypot(pos[i][0] - pos[j][0], pos[i][1] - pos[j][1]));
    return best;
  };
  let r = radius;
  let pad = [1.2 * r, 1.2 * r, 1.2 * r, 1.2 * r];
  let layoutNow = arrange(r, pad);
  for (let pass = 0; pass < 4; pass++) {
    r = clamp(0.32 * closest(layoutNow.pos), 12, radius);
    layoutNow = arrange(r, pad);
    const [x0, y0, x1, y1] = layoutNow.box;
    pad = [pad[0] - x0 + 4, pad[1] - y0 + 4, pad[2] - (width - x1) + 4, pad[3] - (height - y1) + 4].map((p) => Math.max(p, 0));
  }
  layoutNow = arrange(r, pad);
  const { pos, shapes, loops, tags, set, outward, shaft, w, head, hug, wfs, ring } = layoutNow;
  // One label size for the graph (not below half the largest); a longer label shrinks alone to fit.
  const text = Math.max(0.5 * r * 0.85, Math.min(r * 0.85, ...nodes.map((label, i) => (set[i] ? Infinity : fitIn(label, r)))));

  // Visits: when each node is reached, and when the edge it was reached by lights.
  const reached = nodes.map(() => Infinity);
  visit.forEach((v, k) => (reached[v] = visitAt.length > k ? visitAt[k] : visitStart + k * each));
  const lit = edges.map(() => Infinity);
  const joins = (e, a, b) => (edges[e][0] === a && edges[e][1] === b) || (!directed && edges[e][0] === b && edges[e][1] === a);
  if (trace === "all") {
    edges.forEach(([a, b], e) => {
      if (a !== b) lit[e] = Math.max(reached[a], reached[b]);
    });
  } else if (trace === "tree") {
    visit.forEach((v, k) => {
      let from = via[k] ?? -1;
      for (let j = 0; from < 0 && j < k; j++) if (edges.some((_, e) => joins(e, visit[j], v))) from = visit[j];
      const e = edges.findIndex((_, i) => from >= 0 && joins(i, from, v));
      if (e >= 0) lit[e] = reached[v];
    });
  }
  // The walk: step j goes path[j-1] to path[j], travelling for 0.8 of walkEach.
  const stepAt = (j) => walkAt + (j - 1) * walkEach + 0.1 * walkEach;
  const travel = 0.8 * walkEach;
  const there = nodes.map(() => Infinity);
  path.forEach((v, j) => (there[v] = Math.min(there[v], j === 0 ? walkAt : stepAt(j) + travel)));

  const soft = motion.duration.base;
  const strength = (since) => easeInOutCubic(clamp((t - since) / soft, 0, 1));
  const stagger = Math.min(motion.duration.stagger, 1.1 / Math.max(1, n));
  const drawFrom = edgesAt ?? at + n * stagger + 0.1;
  const drawEach = Math.min(motion.duration.stagger, 1 / Math.max(1, m));
  const drawn = (e) => easeInOutCubic(clamp((t - drawFrom - e * drawEach) / soft, 0, 1));

  // A label of maths centred on (x, y), in a box wide enough for it.
  const tag = (key, tex, [x, y], size, ink2, shown) => (
    <Box style={{ position: "absolute", left: x - 3 * size, top: y - size, width: 6 * size, height: 2 * size, alignItems: "center", justifyContent: "center", opacity: shown }}>
      <Math key={key} tex={tex} style={{ fontSize: size, color: ink2 }} />
    </Box>
  );
  // A point the edge's keyed Arrow attaches to, so it follows the layout.
  const anchor = (key, [x, y]) => <Box key={key} style={{ position: "absolute", left: x - 0.5, top: y - 0.5, width: 1, height: 1 }} />;

  const seen = {};
  const parts = [];
  const labels = [];
  edges.forEach(([a, b], e) => {
    const base = `${id}/e${a}-${b}`;
    const dup = seen[base] ? `#${seen[base]}` : "";
    const key = base + dup;
    seen[base] = (seen[base] ?? 0) + 1;
    const q = drawn(e);
    const k = strength(lit[e]);
    if (a !== b) {
      const s = shapes[e];
      const stroke = { width: w * (1 + 0.5 * k), color: mix(color.faint, ink, k) };
      parts.push(anchor(`${key}/s`, s.p0), anchor(`${key}/t`, s.p1));
      parts.push(<Arrow key={key} from={`${key}/s`} to={`${key}/t`} bend={s.bend} gap={0} head={head} progress={q} stroke={stroke} />);
    }
    if (tags[e]) labels.push(tag(`${id}/w${a}-${b}${dup}`, labelTex(weights[e]), tags[e], wfs, mix(color.muted, ink, k), q));
  });

  // A self-loop at node `i` as paths in the node's own box (centre-relative).
  const loopDrawn = (i, key, stroke, q, e) => {
    const [d, tri] = loops[e];
    const span = 2.4 * r;
    const place = { position: "absolute", left: r - span, top: r - span, width: 2 * span, height: 2 * span };
    const grow = clamp((q - 0.9) / 0.1, 0, 1);
    return [
      <Path key={key} d={d} viewBox={[-span, -span, 2 * span, 2 * span]} stroke={{ ...stroke, cap: "round", join: "round" }} trim={[0, q]} style={place} />,
      tri && grow > 0 ? (
        <Path
          key={`${key}/head`}
          d={`M${tri[0][0]} ${tri[0][1]} L${tri[1][0]} ${tri[1][1]} L${tri[2][0]} ${tri[2][1]} Z`}
          viewBox={[-span, -span, 2 * span, 2 * span]}
          fill={stroke.color}
          style={{ ...place, opacity: grow }}
        />
      ) : null,
    ];
  };

  // The walk: each step's edge stroked on in turn, and the marker riding it.
  const steps = [];
  const loopSteps = [];
  const stepShape = (a, b) => {
    const e = edges.findIndex((x) => x[0] === a && x[1] === b);
    if (e >= 0 && shapes[e]) return shapes[e];
    const back = edges.findIndex((x) => x[0] === b && x[1] === a);
    if (back >= 0 && !directed && shapes[back]) return shaft(a, b, -shapes[back].bend, 0);
    return shaft(a, b, 0);
  };
  for (let j = 1; j < path.length; j++) {
    const [a, b] = [path[j - 1], path[j]];
    const q = easeInOutCubic(clamp((t - stepAt(j)) / travel, 0, 1));
    const stroke = { width: 1.7 * w, color: walkInk };
    if (a === b) {
      const e = edges.findIndex((x) => x[0] === a && x[1] === a);
      if (q > 0 && e >= 0) loopSteps.push({ a, j, q, stroke, e });
      continue;
    }
    const s = stepShape(a, b);
    steps.push(
      anchor(`${id}/p${j}/s`, s.p0),
      anchor(`${id}/p${j}/t`, s.p1),
      <Arrow key={`${id}/p${j}`} from={`${id}/p${j}/s`} to={`${id}/p${j}/t`} bend={s.bend} gap={0} head={head} progress={q} stroke={stroke} />,
    );
  }
  let here = null;
  if (path.length > 1 && t >= walkAt) {
    const j = clamp(Math.floor((t - stepAt(1)) / walkEach) + 1, 1, path.length - 1);
    const q = easeInOutCubic(clamp((t - stepAt(j)) / travel, 0, 1));
    const [a, b] = [path[j - 1], path[j]];
    let at2 = pos[a];
    if (a !== b) {
      // Centre to rim, along the shaft, rim to centre; the nodes hide both ends.
      const s = stepShape(a, b);
      const run = q * (2 * hug + s.len);
      if (run < hug) at2 = [pos[a][0] + s.u[0] * run, pos[a][1] + s.u[1] * run];
      else if (run > hug + s.len) at2 = [s.p1[0] + s.u[0] * (run - hug - s.len), s.p1[1] + s.u[1] * (run - hug - s.len)];
      else at2 = s.along((run - hug) / s.len);
    }
    const dot = clamp(0.28 * r, 9, 18);
    here = (
      <Box
        key={`${id}/here`}
        style={{ position: "absolute", left: at2[0] - dot, top: at2[1] - dot, width: 2 * dot, height: 2 * dot, radius: dot, fill: walkInk, opacity: a === b ? 0 : clamp((t - walkAt) / soft, 0, 1) }}
      />
    );
  }

  const popOf = (i) => (t < at + i * stagger ? 0 : spring(t - (at + i * stagger), motion.spring.snappy));
  nodes.forEach((_, i) => {
    const o = set[i];
    if (!o) return;
    const pop = popOf(i);
    labels.push(
      <Stack
        key={`${id}/l${i}`}
        align="center"
        justify="center"
        style={{ position: "absolute", left: o.c[0] - o.w / 2, top: o.c[1] - o.h / 2, width: o.w, height: o.h, scale: pop, opacity: Math.min(1, 3 * pop) }}
      >
        {o.lines.map((line) => (
          <Box style={{ width: o.w, height: 1.3 * o.size, alignItems: "center", justifyContent: "center" }}>
            <Math tex={`\\text{${line}}`} style={{ fontSize: o.size, color: color.text }} />
          </Box>
        ))}
      </Stack>,
    );
  });

  const circles = nodes.map((label, i) => {
    const pop = popOf(i);
    const seenAt = strength(reached[i]);
    const next = Math.min(Infinity, ...reached.filter((x) => x > reached[i] + 1e-6));
    const current = seenAt * (1 - strength(next));
    const walked = strength(there[i]);
    const rest = tones[i] === color.faint ? undefined : tones[i];
    let fill = rest ? mix(color.surface, rest, 0.25) : color.surface;
    let rim = rest ?? color.faint;
    fill = mix(fill, mix(color.surface, ink, 0.22), seenAt);
    rim = mix(rim, ink, seenAt);
    fill = mix(fill, ink, current);
    fill = mix(fill, mix(color.surface, walkInk, 0.3), walked);
    rim = mix(rim, walkInk, walked);
    const own = [];
    edges.forEach(([a, b], e) => {
      if (a !== i || b !== i) return;
      const k = strength(lit[e]);
      own.push(...loopDrawn(i, `${id}/e${i}-${i}`, { width: w * (1 + 0.5 * k), color: mix(color.faint, ink, k) }, drawn(e), e));
    });
    for (const s of loopSteps) if (s.a === i) own.push(...loopDrawn(i, `${id}/p${s.j}`, s.stroke, s.q, s.e));
    return (
      <Box
        key={`${id}/n${i}`}
        style={{
          position: "absolute",
          left: pos[i][0] - r,
          top: pos[i][1] - r,
          width: 2 * r,
          height: 2 * r,
          radius: r,
          fill,
          border: { width: ring, color: rim },
          alignItems: "center",
          justifyContent: "center",
          scale: pop,
          opacity: Math.min(1, 3 * pop),
        }}
      >
        {own}
        {label === "" || set[i] ? null : (
          <Math key={`${id}/l${i}`} tex={labelTex(label)} style={{ fontSize: Math.min(text, fitIn(label, r)), color: mix(color.text, color.onAccent, current * (1 - walked)) }} />
        )}
      </Box>
    );
  });

  return (
    <Box style={{ width, height }}>
      {parts}
      {steps}
      {here}
      {circles}
      {labels}
    </Box>
  );
};

export default (props, { theme, size }) => {
  const { layout: kind, radius, ...rest } = props;
  // The 1600 x 880 box of a 1920 x 1080 frame, scaled to the frame and kept off its edges.
  const width = Math.min((5 / 6) * size.width, size.width - 2 * theme.space.xxl);
  const height = Math.min((22 / 27) * size.height, size.height - 2 * theme.space.xl);
  const scale = Math.min(width / 1600, height / 880);
  return (
    <Stack justify="center" align="center" style={{ flexGrow: 1, fill: theme.color.bg }}>
      <Graph {...rest} kind={kind} width={width} height={height} radius={(radius ?? 60) * scale} />
    </Stack>
  );
};
