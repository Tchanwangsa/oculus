// Overlays bound to keys: Brace, Surround, Underline, Indicate and Arrow
// with `from`/`to`. Each is an `overlay` host node Rust draws after layout
// and morphing (`src/overlay/`), so it follows its target as it moves. It is
// absolutely placed and takes no space; its children are a Brace's label.
// `progress` (0..1) draws it on, or `at` and `dur` (with `ease`) give it
// from the time; without either it is fully drawn.
const overlayProgress = (what, props, ctx, fallback) => {
  if (props.progress !== undefined) {
    finiteArgs(`<${what}>`, { progress: props.progress });
    return props.progress;
  }
  if (props.at === undefined) return fallback;
  const motion = ctx.theme.motion || {};
  const base = (motion.duration || {})[what === 'Indicate' ? 'slow' : 'base'];
  const dur = props.dur === undefined ? (base ?? 0.6) : nonNegative(what, 'dur', props.dur);
  finiteArgs(`<${what}>`, { at: props.at });
  const linearly = what === 'Indicate';
  const ease = toEasing(props.ease, ctx.theme, linearly ? linear : easeInOutCubic, `<${what}>`);
  return progress(ctx.t, props.at, dur, ease);
};

const overlayNode = (what, kind, fields, props, ctx, label) => {
  const node = Object.assign({ k: 'overlay', kind }, fields);
  for (const [k, v] of Object.entries(node)) if (v === undefined) delete node[k];
  node.progress = overlayProgress(what, props, ctx, 1);
  node.style = Object.assign({ position: 'absolute', left: 0, top: 0 }, props.style);
  if (props.key !== undefined && props.key !== null) node.key = String(props.key);
  const loc = locOf(props);
  if (loc) node.loc = loc;
  let kids = [];
  if (label !== undefined && label !== null && label !== false) {
    const el = typeof label === 'string' || typeof label === 'number'
      ? h('text', { style: Object.assign({}, ctx.theme.type && ctx.theme.type.body, { color: ctx.theme.color && ctx.theme.color.text }) }, String(label))
      : label;
    if (!isElement(el)) throw new TypeError(`<${what}> label must be a string or an element, got ${describe(label)}`);
    kids = resolveNow([el], ctx);
  }
  node.children = kids;
  return node;
};

const needTarget = (what, target) => {
  if (typeof target !== 'string' || !target) {
    throw new TypeError(`<${what}> needs target="key" (a key, or a key prefix such as "eq/lhs"), got ${describe(target)}`);
  }
  return target;
};

const TIMING = 'at, dur, ease, progress';
function Brace(props, ctx) {
  const { target, side = 'below', label, gap, size, color, at, dur, ease, progress: _p, style, key, children, __source, __self, ...rest } = props;
  onlyProps('Brace', rest, `target, side, label, gap, size, color, ${TIMING}, style, key`);
  if (!['below', 'above', 'left', 'right'].includes(side)) {
    throw new TypeError(`<Brace> side must be "below", "above", "left" or "right", got ${describe(side)}`);
  }
  return overlayNode('Brace', 'brace', { target: needTarget('Brace', target), side, gap, size, color }, props, ctx, label);
}

function Surround(props, ctx) {
  const { target, pad, radius, width, color, at, dur, ease, progress: _p, style, key, children, __source, __self, ...rest } = props;
  onlyProps('Surround', rest, `target, pad, radius, width, color, ${TIMING}, style, key`);
  return overlayNode('Surround', 'surround', { target: needTarget('Surround', target), pad, radius, width, color }, props, ctx);
}

function Underline(props, ctx) {
  const { target, gap, width, color, at, dur, ease, progress: _p, style, key, children, __source, __self, ...rest } = props;
  onlyProps('Underline', rest, `target, gap, width, color, ${TIMING}, style, key`);
  return overlayNode('Underline', 'underline', { target: needTarget('Underline', target), gap, width, color }, props, ctx);
}

function Indicate(props, ctx) {
  const { target, scale, color, at, dur, progress: _p, key, children, __source, __self, ...rest } = props;
  onlyProps('Indicate', rest, 'target, scale, color, at, dur, progress, key');
  if (at === undefined && _p === undefined) throw new TypeError('<Indicate> needs at={seconds} (or progress) for its pulse');
  return overlayNode('Indicate', 'indicate', { target: needTarget('Indicate', target), scale, color }, props, ctx);
}

const arrowEnd = (name, v) => {
  if (typeof v === 'string' && v) return v;
  if (Array.isArray(v) && v.length === 2) {
    finiteArgs('<Arrow>', { [`${name}[0]`]: v[0], [`${name}[1]`]: v[1] });
    return v;
  }
  throw new TypeError(`<Arrow> ${name} must be a key or [x, y], got ${describe(v)}`);
};

function keyedArrow(props, ctx) {
  const { from, to, bend = 0, head, gap, stroke = {}, at, dur, ease, progress: _p, style, key, children, __source, __self, ...rest } = props;
  onlyProps('Arrow', rest, `from, to, bend, head, gap, stroke, ${TIMING}, style, key (points is for an Arrow without from and to)`);
  if (children && children.length) throw new TypeError('<Arrow> takes no children');
  finiteArgs('<Arrow>', { bend });
  return overlayNode('Arrow', 'arrow', {
    from: arrowEnd('from', from), to: arrowEnd('to', to), bend, head, gap,
    width: stroke.width, color: stroke.color ?? (ctx.theme.color && ctx.theme.color.text),
  }, props, ctx);
}

define('Brace', Brace);
define('Surround', Surround);
define('Underline', Underline);
define('Indicate', Indicate);
