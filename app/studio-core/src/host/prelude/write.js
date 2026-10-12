// Write: every <Math> in its children is written on, each piece's outline
// stroked then filled, in reading order. `at` is when: a number writes the
// whole formula from then, or { group: seconds } cues each \k group on its
// own (Rust assigns the pieces). `dur` (a number or { group: seconds }) is
// each group's length; `stroke` is the outline width in em.
const writeGroups = (at, dur, base, t) => {
  const len = (g) => {
    const d = dur !== null && typeof dur === 'object' ? dur[g] : dur;
    return d === undefined ? base : nonNegative('Write', g === null ? 'dur' : `dur.${g}`, d);
  };
  const p = (start, d) => (d > 0 ? (t - start) / d : t >= start ? 1 : 0);
  if (typeof at === 'number') {
    finiteArgs('<Write>', { at });
    return [[null, p(at, len(null))]];
  }
  if (at === null || typeof at !== 'object' || Array.isArray(at) || !Object.keys(at).length) {
    throw new TypeError(`<Write> at must be seconds or { group: seconds } naming \\k groups, got ${describe(at)}`);
  }
  return Object.keys(at).map((g) => {
    finiteArgs('<Write>', { [`at.${g}`]: at[g] });
    return [g, p(at[g], len(g))];
  });
};

const withWrite = (node, spec, found) => {
  if (node.k === 'math') {
    found.n++;
    return Object.assign({}, node, { write: spec });
  }
  if (!node.children || node.k === 'text') return node;
  return Object.assign({}, node, { children: node.children.map((c) => withWrite(c, spec, found)) });
};

function Write(props, ctx) {
  const { at = 0, dur, lag, stroke = 0.025, children, key, __source, __self, ...rest } = props;
  onlyProps('Write', rest, 'at, dur, lag, stroke, key, children');
  if (lag !== undefined && (typeof lag !== 'number' || !(lag >= 0 && lag <= 1))) {
    throw new TypeError(`<Write> lag must be a number from 0 to 1, got ${describe(lag)}`);
  }
  finiteArgs('<Write>', { stroke });
  const base = ctx.theme.motion && ctx.theme.motion.duration && ctx.theme.motion.duration.slow;
  const spec = { groups: writeGroups(at, dur, base ?? 0.9, ctx.t), stroke };
  if (lag !== undefined) spec.lag = lag;
  const found = { n: 0 };
  const roots = resolveNow(renderChildren(children, ctx), ctx).map((r) => withWrite(r, spec, found));
  if (!found.n) {
    throw new TypeError('<Write> found no <Math> in its children; it writes Math only (for words, use <Math tex="\\text{…}">)');
  }
  return roots;
}
define('Write', Write);
