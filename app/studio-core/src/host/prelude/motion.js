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
// An easing option: a function, cubic-bezier points, or a theme easing name.
const toEasing = (e, theme, fallback, what) => {
  if (e === undefined || e === null) return fallback;
  if (typeof e === 'function') return e;
  if (Array.isArray(e) && e.length === 4 && e.every(Number.isFinite)) return cubicBezier(...e);
  const named = typeof e === 'string' && theme && theme.motion && theme.motion.easing && theme.motion.easing[e];
  if (named) return cubicBezier(...named);
  throw new TypeError(`${what}: easing must be a function, [x1, y1, x2, y2] or a theme.motion.easing name, got ${describe(e)}`);
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
// Progress of [at, at + dur] at t, clamped to 0..1 then eased; dur 0 jumps.
const progress = (t, at, dur, ease) => ease(dur > 0 ? clamp((t - at) / dur, 0, 1) : t >= at ? 1 : 0);
const finiteArgs = (what, values) => {
  for (const [name, v] of Object.entries(values)) {
    if (typeof v !== 'number' || !Number.isFinite(v)) {
      throw new TypeError(`${what}: ${name} must be a finite number, got ${describe(v)}`);
    }
  }
};
// For Text's `reveal.progress`: linear unless an easing is given (each unit
// eases on its own in the renderer).
const revealProgress = (t, at, dur, easing) => {
  finiteArgs('revealProgress(t, at, dur)', { t, at, dur });
  return progress(t, at, dur, toEasing(easing, null, linear, 'revealProgress'));
};
// A number counting from `from` to `to` over [at, at + dur], as a string
// with fixed decimals; pair it with fontVariantNumeric: 'tabular-nums'.
const counter = (t, from, to, options) => {
  const o = options || {};
  const at = o.at ?? 0, dur = o.dur ?? 1, decimals = o.decimals ?? 0;
  finiteArgs('counter(t, from, to)', { t, from, to, at, dur });
  if (!(Number.isInteger(decimals) && decimals >= 0 && decimals <= 20)) {
    throw new TypeError(`counter: decimals must be an integer from 0 to 20, got ${describe(decimals)}`);
  }
  const p = progress(t, at, dur, toEasing(o.easing, null, easeOutCubic, 'counter'));
  const v = lerp(from, to, p);
  const s = v.toFixed(decimals);
  return Number(s) === 0 ? (0).toFixed(decimals) : s;
};
for (const [name, fn] of Object.entries({
  clamp, lerp, linear, easeOutCubic, easeInOutCubic, cubicBezier, interpolate, spring, mix, revealProgress, counter,
})) define(name, fn);
