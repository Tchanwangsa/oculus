// Enter/Exit presets and Camera. Presets compose onto their children's
// resolved roots (see `compose`) and leave time alone: wrap them in a
// Sequence to shift it.

// Each preset maps presence v (0 = gone, 1 = in place) to motion; `dir` is
// 1 entering and -1 leaving, so an exit carries on in the entry's direction.
const PRESETS = {
  fade: (v) => ({ opacity: v }),
  rise: (v, dir, size) => ({ opacity: v, translateY: dir * (1 - v) * 0.03 * size.height }),
  'slide-left': (v, dir, size) => ({ opacity: v, translateX: dir * (1 - v) * 0.04 * size.width }),
  'slide-right': (v, dir, size) => ({ opacity: v, translateX: -dir * (1 - v) * 0.04 * size.width }),
  scale: (v) => ({ opacity: v, scale: lerp(0.9, 1, v) }),
  blur: (v) => ({ opacity: v, blur: (1 - v) * 16 }),
  // A wipe left to right: in from the left edge, out towards the right.
  mask: (v, dir) => (v >= 1 ? {} : { clipInset: dir > 0 ? [0, 1 - v, 0, 0] : [0, 0, 0, 1 - v] }),
};
const PRESET_NAMES = Object.keys(PRESETS).join(', ');

const motionDefaults = (theme) => {
  const m = (theme && theme.motion) || {};
  return { dur: (m.duration && m.duration.base) ?? 0.45, easing: m.easing || {} };
};

const presetComponent = (name, dir) =>
  named(name, function (props, ctx) {
    const { preset = 'fade', at, dur, easing, children, key, __source, __self, ...rest } = props;
    onlyProps(name, rest, 'preset, at, dur, easing, key, children');
    if (typeof preset !== 'string' || !Object.prototype.hasOwnProperty.call(PRESETS, preset)) {
      throw new TypeError(`<${name}> has no preset ${describe(preset)}; presets: ${PRESET_NAMES}`);
    }
    const fx = PRESETS[preset];
    const defaults = motionDefaults(ctx.theme);
    const d = dur === undefined ? defaults.dur : nonNegative(name, 'dur', dur);
    const start = at === undefined ? (dir > 0 ? 0 : ctx.duration - d) : at;
    finiteArgs(`<${name}>`, { at: start });
    const themed = defaults.easing[dir > 0 ? 'enter' : 'exit'];
    const ease = toEasing(easing, ctx.theme, themed ? cubicBezier(...themed) : easeOutCubic, `<${name}>`);
    const p = progress(ctx.t, start, d, ease);
    const motion = fx(dir > 0 ? p : 1 - p, dir, ctx.size);
    return resolveNow(children, ctx).map((n) => compose(n, motion));
  });

// A full-frame Box whose transform puts frame point (x, y) at the frame
// centre, zoomed and rotated about it. Children lay out as in the root.
function Camera(props, ctx) {
  const { x, y, zoom = 1, rotate = 0, style, children, key, __source, __self, ...rest } = props;
  onlyProps('Camera', rest, 'x, y, zoom, rotate, style, key, children');
  const { width: w, height: hgt } = ctx.size;
  const cx = x ?? w / 2, cy = y ?? hgt / 2;
  finiteArgs('<Camera>', { x: cx, y: cy, zoom, rotate });
  const s = Object.assign({ width: '100%', height: '100%', flexShrink: 0 }, style, {
    transformOrigin: [cx / w, cy / hgt],
    translateX: w / 2 - cx,
    translateY: hgt / 2 - cy,
    scale: zoom,
    rotate,
  });
  return h('box', { key, __source, style: s }, children);
}

define('Enter', presetComponent('Enter', 1));
define('Exit', presetComponent('Exit', -1));
define('Camera', Camera);
