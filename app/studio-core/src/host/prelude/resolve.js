// Resolution: walks an element tree from the root in source order, calls each
// function component with its subtree's ctx and returns host nodes only. Top-
// down is what lets Sequence and friends shift time for their children, which
// classic JSX (children evaluated before parents) cannot.
const RESOLVED = Symbol('resolved');

// A node whose children resolve with `ctx` instead of their parent's.
const withCtx = (ctx, children) => {
  const n = { ctx, children };
  n[SCOPE] = true;
  return Object.freeze(n);
};

// Errors thrown outside the component's own code carry the TSX position of
// the element being resolved; the host falls back to it when the stack has
// no frame in the component. The innermost element wins.
const tagLoc = (e, loc) => {
  if (loc && e !== null && typeof e === 'object' && !Object.prototype.hasOwnProperty.call(e, '__studioLoc')) {
    try {
      Object.defineProperty(e, '__studioLoc', { value: loc });
    } catch (_) {}
  }
  return e;
};

const resolveHost = (node, ctx) => {
  if (node[RESOLVED]) return node;
  const copy = Object.assign({}, node);
  if (node.k === 'box') copy.children = resolveInto(node.children || [], ctx, []);
  copy[RESOLVED] = true;
  return copy;
};

const resolveInto = (v, ctx, out) => {
  if (v == null || v === false || v === true) return out;
  if (Array.isArray(v)) {
    for (const c of v) resolveInto(c, ctx, out);
    return out;
  }
  if (isHost(v)) {
    out.push(resolveHost(v, ctx));
    return out;
  }
  if (isScope(v)) return resolveInto(v.children, v.ctx, out);
  if (isComponent(v)) {
    const start = out.length;
    try {
      const r = v.fn(v.props, ctx);
      if (!(r == null || typeof r === 'boolean' || Array.isArray(r) || isElement(r))) {
        throw new TypeError(`<${componentName(v.fn)}> must return an element, an array of them or null, got ${describe(r)}`);
      }
      resolveInto(r, ctx, out);
    } catch (e) {
      throw tagLoc(e, v.loc);
    }
    if (v.key !== undefined && out.length === start + 1 && out[start].key === undefined) {
      out[start] = Object.assign({}, out[start], { key: v.key });
    }
    return out;
  }
  throw new TypeError(`expected an element, got ${describe(v)}`);
};

// Resolved host nodes for `children` under `ctx`, for built-ins that edit
// their children's roots (Enter, Exit, a hidden Sequence).
const resolveNow = (children, ctx) => resolveInto(children, ctx, []);

// Composes motion onto a resolved root without wrapping it (a wrapper Box
// would change flex layout): opacity and scale multiply, translate and blur
// add, clipInset takes the larger inset per side.
const compose = (node, fx) => {
  const s = Object.assign({}, node.style);
  if (fx.opacity !== undefined) s.opacity = (s.opacity ?? 1) * fx.opacity;
  if (fx.translateX) s.translateX = (s.translateX ?? 0) + fx.translateX;
  if (fx.translateY) s.translateY = (s.translateY ?? 0) + fx.translateY;
  if (fx.scale !== undefined && fx.scale !== 1) s.scale = (s.scale ?? 1) * fx.scale;
  if (fx.blur) s.blur = (s.blur ?? 0) + fx.blur;
  if (fx.clipInset) {
    const own = Array.isArray(s.clipInset) ? s.clipInset : [0, 0, 0, 0];
    s.clipInset = fx.clipInset.map((v, i) => Math.max(v, own[i] ?? 0));
  }
  return Object.assign({}, node, { style: s });
};
