// ModClock: the residues 0 … n−1 round a circle, 0 at the top, and a walk
// from residue to residue drawn as arcs through the inside, each arc stroked
// on with a head, the residue reached lighting up: 2^k mod 15 walks 1, 2, 4,
// 8 and back to 1, so the walk closes into a loop of length 4. Residues are
// keyed `<id>/r<k>` (the dot) and `<id>/n<k>` (the number), the marker on
// the walk `<id>/here`. Copy `ModClock` into a scene component to compose it.
export const props = {
  n: num().default(15),
  // The walk: base^k mod n for k = 0 … steps−1.
  base: num().default(2),
  steps: num().default(9),
  // The circle draws from `at`; the walk starts at walkAt, a step every `each`.
  at: num().default(0.2),
  walkAt: num().default(1.4),
  each: num().default(0.5),
  // base^k ≡ r (mod n) under the circle, rewritten on each step.
  centre: bool().default(true),
  tone: token("color").default("accent"),
};

export const examples = {
  powers: { n: 15, base: 2, steps: 9 },
  seven: { n: 7, base: 3, steps: 7, each: 0.5 },
};

/**
 * Residues 0 … n−1 on a circle of `radius` px, drawn from `at`. `walk` is
 * the residues visited in order; step s (an arc from walk[s−1] to walk[s])
 * draws from `times[s]` (default `walkAt + s * each`) over `each`. `centre`
 * is a TeX label per step, shown under the circle from its step on (keyed
 * `<id>/centre`).
 */
const ModClock = (
  { n, walk = [], at = 0, walkAt = 0, each = 0.6, times, centre, radius = 300, id = "clock", tone, size = 44 },
  { t, theme },
) => {
  const { color, motion } = theme;
  const pad = 2.4 * size;
  const c = radius + pad;
  const box = 2 * c;
  const w = size / 9;
  const pos = (k, r = radius) => {
    const a = -Math.PI / 2 + (2 * Math.PI * k) / n;
    return [c + r * Math.cos(a), c + r * Math.sin(a)];
  };
  const ring = easeInOutCubic(clamp((t - at) / (2 * motion.duration.slow), 0, 1));
  const when = (s) => (times ? times[s] : walkAt + s * each);
  // The step under way (or last finished) and how far its arc is drawn.
  let s = -1;
  while (s + 1 < walk.length && t >= when(s + 1)) s++;
  // Step i's arc: residue to residue, bowed towards the centre by a fifth
  // of its chord, as a quadratic (start, control, end).
  const arc = (i) => {
    const [x0, y0] = pos(walk[i - 1]);
    const [x1, y1] = pos(walk[i]);
    const len = Math.hypot(x1 - x0, y1 - y0) || 1;
    const [mx, my] = [(x0 + x1) / 2, (y0 + y1) / 2];
    const towards = Math.hypot(c - mx, c - my) || 1;
    const bow = 0.2 * len;
    return { x0, y0, x1, y1, len, cx: mx + ((c - mx) / towards) * bow, cy: my + ((c - my) / towards) * bow };
  };
  const along = (a, u) => [
    (1 - u) ** 2 * a.x0 + 2 * (1 - u) * u * a.cx + u ** 2 * a.x1,
    (1 - u) ** 2 * a.y0 + 2 * (1 - u) * u * a.cy + u ** 2 * a.y1,
  ];
  const drawn = (i) => easeInOutCubic(clamp((t - when(i)) / (0.8 * each), 0, 1));
  const arcs = [];
  for (let i = 1; i <= s && i < walk.length; i++) {
    if (walk[i - 1] === walk[i]) continue;
    const a = arc(i);
    const q = drawn(i);
    // Stop short of the dots: the stroke runs between their edges.
    const gap = Math.min(0.3, (0.7 * size) / a.len);
    const ink = mix(color.bg, tone, i < s ? 0.45 : 1);
    arcs.push(
      <Path
        d={`M${a.x0} ${a.y0} Q${a.cx} ${a.cy} ${a.x1} ${a.y1}`}
        viewBox={[0, 0, box, box]}
        stroke={{ width: w, color: ink, cap: "round" }}
        trim={[gap, gap + q * (1 - 2 * gap)]}
        style={{ position: "absolute", left: 0, top: 0, width: box, height: box }}
      />,
    );
    if (q > 0.85) {
      // A head on the end tangent, its tip at the trimmed end.
      const [tx, ty] = along(a, 1 - gap);
      const [dx, dy] = [a.x1 - a.cx, a.y1 - a.cy];
      const l = Math.hypot(dx, dy) || 1;
      const [ux, uy] = [dx / l, dy / l];
      const h = 3.5 * w;
      const [bx, by] = [tx - ux * h, ty - uy * h];
      arcs.push(
        <Path
          d={`M${tx} ${ty} L${bx - uy * h * 0.6} ${by + ux * h * 0.6} L${bx + uy * h * 0.6} ${by - ux * h * 0.6} Z`}
          viewBox={[0, 0, box, box]}
          fill={ink}
          style={{ position: "absolute", left: 0, top: 0, width: box, height: box, opacity: (q - 0.85) / 0.15 }}
        />,
      );
    }
  }
  const visited = new Set(walk.slice(0, s + 1));
  const residues = Array.from({ length: n }, (_, k) => {
    const shown = clamp((ring - k / n) * n * 0.5 + 0.5, 0, 1);
    const [x, y] = pos(k);
    const [lx, ly] = pos(k, radius + pad * 0.5);
    const d = size * 0.45;
    const on = visited.has(k);
    return [
      <Box
        key={`${id}/r${k}`}
        style={{
          position: "absolute",
          left: x - d / 2,
          top: y - d / 2,
          width: d,
          height: d,
          radius: d / 2,
          fill: on ? tone : color.text,
          opacity: shown,
        }}
      />,
      <Box style={{ position: "absolute", left: lx - pad / 2, top: ly - pad / 2, width: pad, height: pad, alignItems: "center", justifyContent: "center", opacity: shown }}>
        <Math key={`${id}/n${k}`} tex={String(k)} style={{ fontSize: size, color: on ? tone : color.text }} />
      </Box>,
    ];
  });
  // The marker rides the arc being drawn, from the residue it left.
  let here = null;
  if (s >= 0) {
    const moving = s > 0 && walk[s - 1] !== walk[s];
    const [hx, hy] = moving ? along(arc(s), drawn(s)) : pos(walk[s]);
    const r = size * 0.8;
    here = (
      <Box
        key={`${id}/here`}
        style={{
          position: "absolute",
          left: hx - r,
          top: hy - r,
          width: 2 * r,
          height: 2 * r,
          radius: r,
          border: { width: w, color: tone },
          opacity: s === 0 ? clamp((t - when(0)) / motion.duration.base, 0, 1) : 1,
        }}
      />
    );
  }
  const label = centre && s >= 0 ? centre[Math.min(s, centre.length - 1)] : null;
  return (
    <Stack align="center" gap={size * 0.6}>
    <Box style={{ width: box, height: box }}>
      <Path
        key={`${id}/ring`}
        d={`M${c} ${c - radius} A${radius} ${radius} 0 1 1 ${c - 0.01} ${c - radius}`}
        viewBox={[0, 0, box, box]}
        stroke={{ width: w / 2, color: color.border }}
        trim={[0, ring]}
        style={{ position: "absolute", left: 0, top: 0, width: box, height: box }}
      />
      {arcs}
      {residues}
      {here}
    </Box>
    <Box style={{ height: 2 * size, alignItems: "center" }}>
      {label ? <Math key={`${id}/centre`} tex={label} style={{ fontSize: 1.2 * size, color: color.text }} /> : null}
    </Box>
    </Stack>
  );
};

export default ({ n, base, steps, at, walkAt, each, centre, tone }, { theme }) => {
  const walk = [];
  let r = 1 % n;
  for (let k = 0; k < steps; k++) {
    walk.push(r);
    r = (r * base) % n;
  }
  const labels = walk.map((v, k) => `${base}^{${k}} \\equiv ${v} \\pmod{${n}}`);
  return (
    <Stack justify="center" align="center" style={{ flexGrow: 1, fill: theme.color.bg }}>
      <ModClock n={n} walk={walk} at={at} walkAt={walkAt} each={each} centre={centre ? labels : undefined} tone={tone} />
    </Stack>
  );
};
