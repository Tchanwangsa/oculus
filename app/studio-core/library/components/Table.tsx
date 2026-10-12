// Table: cells of maths in rows, revealed row by row (or column by column)
// on their cues, rows marked by a band. Every cell is a Math keyed
// `<id>/r<i>c<j>` (headers `<id>/h<j>`), so a Math elsewhere with a cell's
// key takes its glyphs: the demo lifts a cell out of the table into a
// formula below it. Copy `Table` into a scene component to compose it.
export const props = {
  header: list(latex()).default([]),
  rows: list(list(latex())),
  // The header at `at`, then row (or column) i at at + (i + 1) * each.
  at: num().default(0.3),
  each: num().default(0.4),
  by: oneOf(["row", "col"]).default("row"),
  // Rows banded from markAt.
  mark: list(num()).default([]),
  markAt: num().default(3),
  // A cell ("r1c2") that flies out into `liftTex` (which must contain its
  // value) from liftAt.
  lift: str().optional(),
  liftTex: latex().optional(),
  liftAt: num().default(3.4),
  tone: token("color").default("accent"),
};

export const examples = {
  powers: {
    header: ["k", "2^k", "2^k \\bmod 15"],
    rows: [
      ["0", "1", "1"],
      ["1", "2", "2"],
      ["2", "4", "4"],
      ["3", "8", "8"],
      ["4", "16", "1"],
      ["5", "32", "2"],
    ],
    at: 0.3,
    each: 0.35,
    mark: [4],
    markAt: 3,
  },
  euclid: {
    header: ["a", "b", "a \\bmod b"],
    rows: [
      ["21", "12", "9"],
      ["12", "9", "3"],
      ["9", "3", "0"],
    ],
    each: 0.5,
    lift: "r1c2",
    liftTex: "\\gcd(21, 12) = 3",
    liftAt: 2.6,
  },
};

/**
 * `rows` of TeX cells under an optional `header`, each column `colWidth`
 * px. The header and its rule arrive at `at`; row (or with `by: "col"`,
 * column) i fades up at `times[i]` (default `at + (i + 1) * each` with a
 * header, `at + i * each` without); `marks` band rows: `[{ rows: [4], color, at }]`. A cell
 * named in `hide` keeps its place but draws nothing (it has moved).
 */
const Table = (
  { header = [], rows, at = 0, each = 0.4, times, by = "row", marks = [], hide = [], colWidth = 300, size = 52, id = "table" },
  { t, theme },
) => {
  const { color, motion } = theme;
  const cols = Math.max(header.length, ...rows.map((r) => r.length));
  const rowHeight = 2 * size;
  const w = size / 12;
  // The header and its rule come first, at `at`; then the rows (or columns).
  const start = header.length ? at + each : at;
  const when = (i) => (times ? times[i] : start + i * each);
  const fade = (from) => easeOutCubic(clamp((t - from) / motion.duration.base, 0, 1));
  const shown = (i) => fade(when(i));
  // The fade is on the Math itself, not its cell, so a Math elsewhere that
  // takes the cell's key morphs from the cell's own opacity.
  const cell = (key, tex, k, ink, hidden) => (
    <Box style={{ width: colWidth, height: rowHeight, alignItems: "center", justifyContent: "center" }}>
      {hidden ? null : (
        <Math key={key} tex={tex} style={{ fontSize: size, color: ink, opacity: k, translateY: (1 - k) * size * 0.4 }} />
      )}
    </Box>
  );
  const head = header.length ? (
    <Row>
      {header.map((h, j) => cell(`${id}/h${j}`, h, by === "col" ? shown(j) : fade(at), color.muted))}
    </Row>
  ) : null;
  const rule = header.length ? (
    <Box
      key={`${id}/rule`}
      style={{ width: cols * colWidth, height: w, radius: w / 2, fill: color.text, clipInset: [0, 1 - easeInOutCubic(clamp((t - at) / motion.duration.slow, 0, 1)), 0, 0] }}
    />
  ) : null;
  const body = rows.map((r, i) => {
    let band = 0;
    let ink = color.bg;
    for (const m of marks) {
      if (m.rows.includes(i)) {
        band = Math.max(band, clamp((t - m.at) / motion.duration.base, 0, 1));
        ink = m.color;
      }
    }
    return (
      <Row key={`${id}/row${i}`} style={{ radius: size * 0.3, fill: mix(color.bg, ink, 0.22 * band) }}>
        {Array.from({ length: cols }, (_, j) =>
          cell(`${id}/r${i}c${j}`, r[j] ?? "", by === "col" ? shown(j) : shown(i), color.text, hide.includes(`r${i}c${j}`)),
        )}
      </Row>
    );
  });
  return (
    <Stack align="center">
      {head}
      {rule}
      {body}
    </Stack>
  );
};

export default ({ header, rows, at, each, by, mark, markAt, lift, liftTex, liftAt, tone }, { theme }) => {
  const { color, motion } = theme;
  const table = (lifted) => (
    <Stack justify="center" align="center" gap={theme.space.xl} style={{ flexGrow: 1, fill: color.bg }}>
      <Table
        header={header}
        rows={rows}
        at={at}
        each={each}
        by={by}
        marks={[{ rows: mark, color: tone, at: markAt }]}
        hide={lifted ? [lift] : []}
      />
      {lift ? (
        // The slot is there in both states, so the table stays put.
        <Box style={{ height: 160, alignItems: "center" }}>
          {lifted ? <Math key={`table/${lift}`} tex={liftTex} style={{ fontSize: 96, color: color.text }} /> : null}
        </Box>
      ) : null}
    </Stack>
  );
  if (!lift || !liftTex) return table(false);
  return (
    <Steps at={[liftAt]} dur={motion.duration.slow * 1.4} lag={0.08}>
      {table(false)}
      {table(true)}
    </Steps>
  );
};
