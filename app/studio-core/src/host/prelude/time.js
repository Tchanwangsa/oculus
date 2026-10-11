// Sequencing: built-in components that shift time for their subtree. A
// child's ctx is derived from its parent's, so any frame samples directly.
const ctxAt = (ctx, t, duration) => Object.freeze(Object.assign({}, ctx, { t, duration }));
const nonNegative = (name, prop, v) => {
  if (typeof v !== 'number' || !Number.isFinite(v) || v < 0) {
    throw new RangeError(`<${name}> ${prop} must be a finite number of seconds, 0 or more, got ${describe(v)}`);
  }
  return v;
};

// Render props, (ctx) => element, read the shifted time without a component.
const renderChildren = (children, ctx) => (children || []).map((c) => (typeof c === 'function' ? c(ctx) : c));

// The window [from, from + dur) of the parent's time, as child t 0..dur.
// Outside it the subtree still resolves (t clamped) and keeps its layout
// slot, hidden; `hold` keeps it shown at t = dur after the window.
const windowed = (ctx, from, dur, hold, children) => {
  const local = ctx.t - from;
  const child = ctxAt(ctx, clamp(local, 0, dur), dur);
  const kids = renderChildren(children, child);
  if (local >= 0 && (local < dur || hold)) return withCtx(child, kids);
  return resolveNow(kids, child).map((n) => compose(n, { opacity: 0 }));
};

function Sequence(props, ctx) {
  const { from = 0, dur, hold = false, children, key, __source, __self, ...rest } = props;
  onlyProps('Sequence', rest, 'from, dur, hold, key, children');
  nonNegative('Sequence', 'from', from);
  const d = dur === undefined ? Math.max(0, ctx.duration - from) : nonNegative('Sequence', 'dur', dur);
  return windowed(ctx, from, d, !!hold, children);
}

// Fragments among Stagger's children count as their children.
const expandFragments = (list, out) => {
  for (const c of list) {
    if (isComponent(c) && c.fn === Fragment) expandFragments(c.props.children || [], out);
    else out.push(c);
  }
  return out;
};

function Stagger(props, ctx) {
  const { each, at = 0, children, key, __source, __self, ...rest } = props;
  onlyProps('Stagger', rest, 'each, at, key, children');
  nonNegative('Stagger', 'each', each);
  nonNegative('Stagger', 'at', at);
  return expandFragments(children || [], []).map((c, i) => {
    const from = at + i * each;
    return windowed(ctx, from, Math.max(0, ctx.duration - from), false, [c]);
  });
}

const durations = (what, durs) => {
  if (!Array.isArray(durs)) throw new TypeError(`${what} takes an array of durations, got ${describe(durs)}`);
  durs.forEach((d, i) => finiteArgs(what, { [`durations[${i}]`]: d }));
  return durs;
};
// Start times for clips played one after another, `gap` apart.
const series = (durs, options) => {
  const o = options || {};
  const gap = o.gap ?? 0;
  let t = o.at ?? 0;
  finiteArgs('series', { gap, at: t });
  return durations('series', durs).map((d) => {
    const start = t;
    t += d + gap;
    return start;
  });
};
// Start times for clips that all start together.
const parallel = (durs, options) => {
  const at = (options || {}).at ?? 0;
  finiteArgs('parallel', { at });
  return durations('parallel', durs).map(() => at);
};

define('Sequence', Sequence);
define('Stagger', Stagger);
define('series', series);
define('parallel', parallel);
