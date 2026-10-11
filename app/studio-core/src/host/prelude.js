// Studio prelude: the JSX factory, primitives, prop builders and motion
// helpers every component sees as globals. Pure: no clock, seeded random,
// no I/O. Evaluated as a module into a fresh context for every load and
// call, holding one component; the host calls the `__studio*` entry points.
(() => {
  const g = globalThis;
  const define = (name, value) =>
    Object.defineProperty(g, name, { value, writable: false, enumerable: false, configurable: false });

  const describe = (v) => {
    if (v === null) return 'null';
    if (Array.isArray(v)) return 'an array';
    if (typeof v === 'object') return v.k ? `a <${elementName(v.k)}> element` : 'an object';
    if (typeof v === 'function') return 'a function';
    if (typeof v === 'string') return JSON.stringify(v);
    return String(v);
  };
  const elementName = (k) => ({ box: 'Box', text: 'Text', image: 'Image' })[k] || String(k);

  // Elements. Box, Text and Image are host elements; everything else is a
  // function component called as (props, ctx).
  let currentCtx = null;
  const ALLOWED = {
    box: ['style', 'key', 'children'],
    text: ['style', 'key', 'children'],
    image: ['style', 'key', 'src'],
  };
  const IGNORED = ['__source', '__self'];
  const flatten = (children, out) => {
    for (const c of children) {
      if (c == null || c === false || c === true) continue;
      if (Array.isArray(c)) flatten(c, out);
      else out.push(c);
    }
    return out;
  };
  const isElement = (v) => v !== null && typeof v === 'object' && !Array.isArray(v) && typeof v.k === 'string';

  function h(type, props, ...children) {
    props = props || {};
    const kids = flatten(children, []);
    if (typeof type === 'function') {
      const p = Object.assign({}, props);
      if (kids.length) p.children = kids;
      return type(p, currentCtx);
    }
    const allowed = ALLOWED[type];
    if (!allowed) {
      throw new TypeError(`unknown element ${describe(type)}; use Box, Text, Image, Stack, Row or a function component`);
    }
    const name = elementName(type);
    for (const key of Object.keys(props)) {
      if (allowed.includes(key) || IGNORED.includes(key)) continue;
      const hint = type === 'image' || key === 'src' ? '' : ` (style keys go inside style={{ ${key}: ... }})`;
      throw new TypeError(`<${name}> has no prop "${key}"; it takes ${allowed.join(', ')}${hint}`);
    }
    const node = { k: type };
    if (props.style !== undefined) {
      if (props.style === null || typeof props.style !== 'object' || Array.isArray(props.style)) {
        throw new TypeError(`<${name}> style must be an object, got ${describe(props.style)}`);
      }
      node.style = props.style;
    }
    if (props.key !== undefined && props.key !== null) node.key = String(props.key);
    const src = props.__source;
    if (src && typeof src.lineNumber === 'number') node.loc = [src.lineNumber, src.columnNumber || 0];
    if (type === 'text') {
      for (const c of kids) {
        if (typeof c === 'string') continue;
        if (typeof c === 'number' && Number.isFinite(c)) continue;
        throw new TypeError(
          `<Text> children must be strings or numbers, got ${describe(c)}; to mix styles put several <Text> in a <Row>`
        );
      }
      node.text = kids.join('');
    } else if (type === 'image') {
      if (kids.length) throw new TypeError('<Image> takes no children');
      if (typeof props.src !== 'string' || !props.src) {
        throw new TypeError(`<Image> needs src="file.png" (relative to assets/), got ${describe(props.src)}`);
      }
      node.src = props.src;
    } else {
      for (const c of kids) {
        if (!isElement(c)) {
          throw new TypeError(`<Box> children must be elements, got ${describe(c)}; put text inside <Text>`);
        }
      }
      node.children = kids;
    }
    return node;
  }
  define('__studio_h', h);
  define('Box', 'box');
  define('Text', 'text');
  define('Image', 'image');
  const Fragment = (props) => (props && props.children) || [];
  define('Fragment', Fragment);
  define('__studio_Fragment', Fragment);

  // Stack (column) and Row (row): a Box with layout shorthands.
  const flex = (direction, name) =>
    function (props) {
      const { gap, align, justify, wrap, style, key, children, __source, __self, ...rest } = props;
      const extra = Object.keys(rest);
      if (extra.length) {
        throw new TypeError(`<${name}> has no prop "${extra[0]}"; it takes gap, align, justify, wrap, style, key, children`);
      }
      const s = { flexDirection: direction };
      if (gap !== undefined) s.gap = gap;
      if (align !== undefined) s.alignItems = align;
      if (justify !== undefined) s.justifyContent = justify;
      if (wrap) s.flexWrap = 'wrap';
      return h('box', { key, __source, style: Object.assign(s, style) }, children);
    };
  define('Stack', flex('column', 'Stack'));
  define('Row', flex('row', 'Row'));

  // Prop builders. Each serialises (toJSON) to what Rust's schema reads.
  const builder = (spec) => ({
    __prop: true,
    spec,
    default(v) {
      return builder(Object.assign({}, spec, { default: v }));
    },
    optional() {
      return builder(Object.assign({}, spec, { optional: true }));
    },
    toJSON() {
      return spec;
    },
  });
  define('str', () => builder({ kind: 'str', optional: false }));
  define('num', () => builder({ kind: 'num', optional: false }));
  define('bool', () => builder({ kind: 'bool', optional: false }));
  define('latex', () => builder({ kind: 'latex', optional: false }));
  define('list', (of) => {
    if (!of || !of.__prop) throw new TypeError('list(x) needs a prop builder, e.g. list(str())');
    return builder({ kind: 'list', optional: false, of: of.spec });
  });
  define('oneOf', (values) => {
    if (!Array.isArray(values)) throw new TypeError('oneOf takes an array, e.g. oneOf(["left", "right"])');
    return builder({ kind: 'oneOf', optional: false, values });
  });
  define('token', (kind) => {
    if (typeof kind !== 'string') throw new TypeError('token(kind) takes a theme section name, e.g. token("color")');
    return builder({ kind: 'token', optional: false, token: kind });
  });

  // Motion helpers: pure functions of t.
  const clamp = (x, lo, hi) => Math.min(hi, Math.max(lo, x));
  const lerp = (a, b, p) => a + (b - a) * p;
  const linear = (p) => p;
  const easeOutCubic = (p) => 1 - Math.pow(1 - p, 3);
  const easeInOutCubic = (p) => (p < 0.5 ? 4 * p * p * p : 1 - Math.pow(-2 * p + 2, 3) / 2);
  // CSS cubic-bezier(x1, y1, x2, y2): solve x(s) = p for s, return y(s).
  const cubicBezier = (x1, y1, x2, y2) => {
    const bx = (s) => 3 * x1 * s * (1 - s) * (1 - s) + 3 * x2 * s * s * (1 - s) + s * s * s;
    const by = (s) => 3 * y1 * s * (1 - s) * (1 - s) + 3 * y2 * s * s * (1 - s) + s * s * s;
    const dx = (s) => 3 * x1 * (1 - s) * (1 - s) + 6 * (x2 - x1) * s * (1 - s) + 3 * (1 - x2) * s * s;
    return (p) => {
      if (p <= 0) return 0;
      if (p >= 1) return 1;
      let s = p;
      for (let i = 0; i < 8; i++) {
        const d = dx(s);
        if (Math.abs(d) < 1e-6) break;
        const next = s - (bx(s) - p) / d;
        if (next < 0 || next > 1) break;
        s = next;
      }
      if (Math.abs(bx(s) - p) > 1e-5) {
        let lo = 0, hi = 1;
        s = p;
        for (let i = 0; i < 40; i++) {
          if (bx(s) < p) lo = s; else hi = s;
          s = (lo + hi) / 2;
        }
      }
      return by(s);
    };
  };
  const interpolate = (t, inputs, outputs, options) => {
    const o = options || {};
    const n = inputs.length;
    if (n < 2 || outputs.length !== n) {
      throw new TypeError('interpolate(t, inputs, outputs) needs two or more inputs and as many outputs');
    }
    for (let i = 1; i < n; i++) {
      if (!(inputs[i] > inputs[i - 1])) throw new TypeError('interpolate inputs must rise strictly');
    }
    let x = t;
    if (o.clamp !== false) x = clamp(x, inputs[0], inputs[n - 1]);
    let i = 0;
    while (i < n - 2 && x > inputs[i + 1]) i++;
    let p = (x - inputs[i]) / (inputs[i + 1] - inputs[i]);
    if (o.easing) p = o.easing(p);
    return outputs[i] + (outputs[i + 1] - outputs[i]) * p;
  };
  // Closed-form damped spring from 0 to 1 at rest, so any t can be sampled.
  const spring = (t, options) => {
    const o = options || {};
    const k = o.stiffness ?? 170, c = o.damping ?? 26, m = o.mass ?? 1;
    if (t <= 0) return 0;
    const w0 = Math.sqrt(k / m), z = c / (2 * Math.sqrt(k * m));
    if (z < 1) {
      const wd = w0 * Math.sqrt(1 - z * z);
      return 1 - Math.exp(-z * w0 * t) * (Math.cos(wd * t) + ((z * w0) / wd) * Math.sin(wd * t));
    }
    if (z === 1) return 1 - Math.exp(-w0 * t) * (1 + w0 * t);
    const s = Math.sqrt(z * z - 1), r1 = -w0 * (z - s), r2 = -w0 * (z + s);
    return 1 - (r2 * Math.exp(r1 * t) - r1 * Math.exp(r2 * t)) / (r2 - r1);
  };
  const parseHex = (c) => {
    const m = typeof c === 'string' && /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/.exec(c);
    if (!m) throw new TypeError(`mix: ${describe(c)} is not #rgb, #rrggbb or #rrggbbaa`);
    let h = m[1];
    if (h.length === 3) h = h.split('').map((d) => d + d).join('');
    if (h.length === 6) h += 'ff';
    return [0, 2, 4, 6].map((i) => parseInt(h.slice(i, i + 2), 16));
  };
  const mix = (a, b, p) => {
    const x = parseHex(a), y = parseHex(b);
    const ch = x.map((v, i) => clamp(Math.round(v + (y[i] - v) * p), 0, 255));
    const hex = (v) => v.toString(16).padStart(2, '0');
    return '#' + ch.slice(0, 3).map(hex).join('') + (ch[3] === 255 ? '' : hex(ch[3]));
  };
  for (const [name, fn] of Object.entries({
    clamp, lerp, linear, easeOutCubic, easeInOutCubic, cubicBezier, interpolate, spring, mix,
  })) define(name, fn);

  // Seeded Math.random (mulberry32), reseeded at the start of every call.
  let seed = 0;
  Math.random = function random() {
    seed = (seed + 0x6d2b79f5) | 0;
    let r = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    r = (r + Math.imul(r ^ (r >>> 7), 61 | r)) ^ r;
    return ((r ^ (r >>> 14)) >>> 0) / 4294967296;
  };

  // console goes to the frame's diagnostics, not stdout.
  let logs = [];
  const MAX_LOGS = 200;
  const log = (level) => (...args) => {
    if (logs.length >= MAX_LOGS) return;
    const text = args
      .map((a) => {
        if (typeof a === 'string') return a;
        try {
          return JSON.stringify(a);
        } catch (_) {
          return String(a);
        }
      })
      .join(' ');
    logs.push(level === 'log' ? text : `${level}: ${text}`);
  };
  define('console', Object.freeze({ log: log('log'), info: log('info'), warn: log('warn'), error: log('error') }));

  const deepFreeze = (v) => {
    if (v && typeof v === 'object' && !Object.isFrozen(v)) {
      Object.freeze(v);
      for (const k of Object.keys(v)) deepFreeze(v[k]);
    }
    return v;
  };

  // Host entry points. `ns` is the component module's namespace.
  let theme = deepFreeze({});
  define('__studioSetTheme', (json) => {
    theme = deepFreeze(JSON.parse(json));
  });
  define('__studioExports', (ns) => {
    if (typeof ns.default !== 'function') {
      throw new TypeError('a component needs `export default (props, ctx) => <...>`');
    }
    if (ns.props === undefined) {
      throw new TypeError('a component needs `export const props = { ... }` (use {} for none), so its props can be checked and edited');
    }
    return JSON.stringify({ props: ns.props, examples: ns.examples === undefined ? {} : ns.examples });
  });
  define('__studioCall', (ns, propsJson, t, duration, width, height, callSeed) => {
    seed = callSeed | 0;
    currentCtx = Object.freeze({ t, duration, theme, size: Object.freeze({ width, height }) });
    const out = ns.default(JSON.parse(propsJson), currentCtx);
    if (!isElement(out)) {
      throw new TypeError(`the default export must return one element (Box, Text, Image, Stack or Row), got ${describe(out)}`);
    }
    return JSON.stringify(out);
  });
  define('__studioTakeLogs', () => {
    const out = JSON.stringify(logs);
    logs = [];
    return out;
  });
})();
