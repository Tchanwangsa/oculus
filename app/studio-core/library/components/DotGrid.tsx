// DotGrid: n dots in rows of `cols`, every dot keyed `<id>/d<i>`, so two
// DotGrids of the same n and id in a Steps glide each dot to its new place:
// 12 as 3 × 4, then as 2 × 6. Copy `DotGrid` into a scene component to
// compose it; the default export is a demo that steps through `cols`.
export const props = {
  n: num().default(12),
  // One arrangement per state; `at` starts each change (one fewer).
  cols: list(num()).default([4, 6]),
  at: list(num()).default([2]),
  // The dots pop in one after another from here.
  enter: num().default(0.3),
  // Dots past the last full row take the negative colour: no rectangle.
  leftover: bool().default(false),
  marks: list(num()).default([]),
  markAt: num().default(3),
  caption: bool().default(true),
  tone: token("color").default("accent"),
};

export const examples = {
  twelve: { n: 12, cols: [4, 6], at: [2.2] },
  seven: { n: 7, cols: [7, 2, 3], at: [1.8, 3.4], leftover: true },
  multiples: { n: 12, cols: [6], at: [], marks: [2, 5, 8, 11], markAt: 1.8 },
};

/**
 * n dots, `cols` to a row. `enter` and `each` pop them in (each 0 shows them
 * all); `marks` recolour subsets: `[{ dots: [0, 3], color, at }]`. With
 * `leftover` the dots of an unfinished last row take `leftover` (a colour).
 * `caption` writes rows × cols (+ the rest) under it, keyed `<id>/label`.
 */
const DotGrid = (
  { n, cols, id = "dots", size = 112, gap = 60, enter = 0, each, tone, marks = [], leftover, caption },
  { t, theme },
) => {
  const { color, motion } = theme;
  const c = Math.max(1, Math.min(cols, n));
  const rows = Math.ceil(n / c);
  const full = Math.floor(n / c) * c;
  const step = each ?? motion.duration.stagger;
  const width = c * size + (c - 1) * gap;
  const height = rows * size + (rows - 1) * gap;
  const dots = Array.from({ length: n }, (_, i) => {
    const start = enter + i * step;
    const pop = t < start ? 0 : spring(t - start, motion.spring.snappy);
    let fill = leftover && i >= full ? leftover : tone;
    for (const m of marks) {
      if (m.dots.includes(i)) fill = mix(fill, m.color, clamp((t - m.at) / motion.duration.base, 0, 1));
    }
    return (
      <Box
        key={`${id}/d${i}`}
        style={{
          position: "absolute",
          left: (i % c) * (size + gap),
          top: Math.floor(i / c) * (size + gap),
          width: size,
          height: size,
          radius: theme.radius.pill,
          fill,
          scale: pop,
          opacity: Math.min(1, 3 * pop),
        }}
      />
    );
  });
  const rest = n - full;
  const label = rows > 1 && rest > 0 ? `${rows - 1} \\times ${c} + ${rest}` : `${rows} \\times ${c}`;
  return (
    <Stack align="center" gap={size}>
      <Box style={{ width, height }}>{dots}</Box>
      {caption ? <Math key={`${id}/label`} tex={label} style={{ fontSize: size * 0.7, color: color.text }} /> : null}
    </Stack>
  );
};

export default ({ n, cols, at, enter, leftover, marks, markAt, caption, tone }, { theme }) => {
  const { color, motion } = theme;
  const grid = (c) => (
    <DotGrid
      n={n}
      cols={c}
      enter={enter}
      tone={tone}
      caption={caption}
      leftover={leftover ? color.negative : undefined}
      marks={[{ dots: marks, color: color.warning, at: markAt }]}
    />
  );
  return (
    <Stack justify="center" align="center" style={{ flexGrow: 1, fill: color.bg }}>
      {cols.length > 1 ? (
        <Steps at={at} dur={motion.duration.slow} lag={0.06}>
          {cols.map(grid)}
        </Steps>
      ) : (
        grid(cols[0])
      )}
    </Stack>
  );
};
