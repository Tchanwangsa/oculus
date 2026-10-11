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
const ONE_ELEMENT = 'the default export must return one element (Box, Text, Image, Path, Stack or Row)';
define('__studioCall', (ns, propsJson, t, duration, width, height, callSeed) => {
  seed = callSeed | 0;
  const ctx = Object.freeze({ t, duration, theme, size: Object.freeze({ width, height }) });
  const out = ns.default(JSON.parse(propsJson), ctx);
  if (!isElement(out)) throw new TypeError(`${ONE_ELEMENT}, got ${describe(out)}`);
  const roots = resolveInto(out, ctx, []);
  if (roots.length !== 1) {
    const got = roots.length ? `${roots.length} elements; wrap them in a Box` : 'nothing';
    throw tagLoc(new TypeError(`${ONE_ELEMENT}, got ${got}`), out.loc);
  }
  return JSON.stringify(roots[0]);
});
define('__studioTakeLogs', () => {
  const out = JSON.stringify(logs);
  logs = [];
  return out;
});
