// Tiles: Euclid's gcd as a picture. An a × b rectangle is tiled by the
// largest squares that fit, then the strip left over by its largest, and so
// on: 21 × 12 takes one 12-square, the 9 × 12 left takes one 9-square, the
// 9 × 3 left takes three 3-squares, so gcd(21, 12) = 3, the last square's
// side. Each square is keyed `<id>/s<i>` (its side `<id>/s<i>/side`) and
// draws on in turn, its outline stroked, then its fill. Copy `squares` and
// `Tiles` into a scene component to compose them.
export const props = {
  a: num().default(21),
  b: num().default(12),
  // The rectangle draws from here; then a square every `each` seconds.
  at: num().default(0.2),
  each: num().default(0.7),
  // Squares shown (fractional draws the last one part way); overrides the times.
  step: num().optional(),
  grid: bool().default(true),
  // gcd(a, b) = side, under the rectangle once the last square is down.
  result: bool().default(true),
};

export const examples = {
  euclid: { a: 21, b: 12 },
  coprime: { a: 8, b: 5, each: 0.5 },
};

/** The squares tiling a × b, largest first: `{ x, y, side, round }` in units, `round` counting the strips. */
const squares = (a, b) => {
  const out = [];
  let [x, y, w, h, round] = [0, 0, a, b, 0];
  while (w > 0 && h > 0) {
    const s = Math.min(w, h);
    const q = Math.floor(Math.max(w, h) / s);
    for (let k = 0; k < q; k++) {
      out.push(w >= h ? { x: x + k * s, y, side: s, round } : { x, y: y + k * s, side: s, round });
    }
    if (w >= h) {
      x += q * s;
      w -= q * s;
    } else {
      y += q * s;
      h -= q * s;
    }
    round++;
  }
  return out;
};

/**
 * The a × b rectangle in `width` by `height` px (one unit square at most
 * fits both), its sides labelled, drawn from `at`; square i draws at
 * `times[i]` (default `at + 1 + i * each`) over `dur`, or `step` squares
 * are shown outright. `grid` rules the unit squares faintly.
 */
const Tiles = (
  { a, b, width = 1300, height = 600, at = 0, each = 0.7, times, step, dur, grid = true, result, id = "tiles", size = 48 },
  { t, theme },
) => {
  const { color, motion } = theme;
  const unit = Math.min(width / a, height / b);
  const [W, H] = [a * unit, b * unit];
  const pad = 2 * size;
  const w = size / 10;
  const tones = [color.accent, color.positive, color.warning, color.negative];
  const list = squares(a, b);
  const run = dur ?? motion.duration.slow;
  const shown = (i) =>
    step !== undefined ? clamp(step - i, 0, 1) : clamp((t - (times ? times[i] : at + 1 + i * each)) / run, 0, 1);
  const frame = easeInOutCubic(clamp((t - at) / motion.duration.slow, 0, 1));
  const lines = [];
  if (grid) {
    for (let i = 1; i < a; i++) lines.push(`M${(i * unit).toFixed(2)} 0 V${H.toFixed(2)}`);
    for (let j = 1; j < b; j++) lines.push(`M0 ${(j * unit).toFixed(2)} H${W.toFixed(2)}`);
  }
  const outline = (s) => `M0 0 H${s} V${s} H0 Z`;
  const last = list[list.length - 1];
  const done = shown(list.length - 1);
  const label = (key, tex, left, top, bw, bh, opacity) => (
    <Box style={{ position: "absolute", left, top, width: bw, height: bh, alignItems: "center", justifyContent: "center", opacity }}>
      <Math key={key} tex={tex} style={{ fontSize: size, color: color.text }} />
    </Box>
  );
  return (
    <Stack align="center" gap={size}>
      <Box style={{ width: W + 2 * pad, height: H + 2 * pad }}>
        {label(`${id}/a`, String(a), pad, pad + H, W, pad, frame)}
        {label(`${id}/b`, String(b), 0, pad, pad, H, frame)}
        <Box style={{ position: "absolute", left: pad, top: pad, width: W, height: H }}>
          {grid ? (
            <Path
              d={lines.join(" ") || "M0 0"}
              viewBox={[0, 0, W, H]}
              stroke={{ width: w / 2, color: color.border }}
              style={{ position: "absolute", left: 0, top: 0, width: W, height: H, opacity: frame }}
            />
          ) : null}
          {list.map((sq, i) => {
            const k = shown(i);
            const tone = tones[sq.round % tones.length];
            const px = sq.side * unit;
            return (
              <Box
                key={`${id}/s${i}`}
                style={{
                  position: "absolute",
                  left: sq.x * unit,
                  top: sq.y * unit,
                  width: px,
                  height: px,
                  fill: mix(color.bg, tone, 0.28 * clamp(k * 2 - 1, 0, 1)),
                  opacity: k > 0 ? 1 : 0,
                  alignItems: "center",
                  justifyContent: "center",
                }}
              >
                <Path
                  d={outline(px)}
                  viewBox={[0, 0, px, px]}
                  stroke={{ width: w, color: tone, join: "round", cap: "round" }}
                  trim={[0, clamp(k * 2, 0, 1)]}
                  style={{ position: "absolute", left: 0, top: 0, width: px, height: px }}
                />
                {px > 2.2 * size ? (
                  <Box style={{ opacity: clamp(k * 2 - 1, 0, 1) }}>
                    <Math key={`${id}/s${i}/side`} tex={String(sq.side)} style={{ fontSize: Math.min(px / 3, 1.6 * size), color: tone }} />
                  </Box>
                ) : null}
              </Box>
            );
          })}
          <Path
            key={`${id}/frame`}
            d={`M0 0 H${W} V${H} H0 Z`}
            viewBox={[0, 0, W, H]}
            stroke={{ width: w, color: color.text, join: "round" }}
            trim={[0, frame]}
            style={{ position: "absolute", left: 0, top: 0, width: W, height: H }}
          />
        </Box>
      </Box>
      {result ? (
        <Box style={{ opacity: clamp(done * 2 - 1, 0, 1) }}>
          <Math key={`${id}/gcd`} tex={`\\gcd(${a}, ${b}) = ${last.side}`} style={{ fontSize: 1.4 * size, color: color.text }} />
        </Box>
      ) : null}
    </Stack>
  );
};

export default ({ a, b, at, each, step, grid, result }, { theme }) => (
  <Stack justify="center" align="center" style={{ flexGrow: 1, fill: theme.color.bg }}>
    <Tiles a={a} b={b} at={at} each={each} step={step} grid={grid} result={result} />
  </Stack>
);
