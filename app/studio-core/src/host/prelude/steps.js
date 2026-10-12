// Steps: N children are N states of one place, a derivation rewritten in
// place. Before at[0] the first shows; transition k (state k to k + 1) runs
// from at[k] for `dur`, cut short by the next cue so it always finishes, and
// each boundary is continuous. It is a Morph between the pair the time
// picks, so shared keys glide, `lag` staggers and `fade` fades as there.
function Steps(props, ctx) {
  const { at, dur, ease, lag, fade, children, key, __source, __self, ...rest } = props;
  onlyProps('Steps', rest, 'at, dur, ease, lag, fade, key, children');
  const states = renderChildren(expandFragments(children || [], []), ctx);
  if (states.length < 2 || !states.every(isElement)) {
    throw new TypeError(`<Steps> takes two or more states (elements or (ctx) => element), got ${states.length === 1 ? describe(states[0]) : states.length}`);
  }
  if (!Array.isArray(at) || at.length !== states.length - 1) {
    throw new TypeError(`<Steps> at must list one start time per transition (${states.length - 1} for ${states.length} states), got ${describe(at)}`);
  }
  at.forEach((v, i) => {
    finiteArgs('<Steps>', { [`at[${i}]`]: v });
    if (i > 0 && !(v > at[i - 1])) throw new RangeError(`<Steps> at must rise strictly, got at[${i - 1}] = ${at[i - 1]} then at[${i}] = ${v}`);
  });
  const base = ctx.theme.motion && ctx.theme.motion.duration && ctx.theme.motion.duration.base;
  const d = dur === undefined ? (base ?? 0.45) : nonNegative('Steps', 'dur', dur);
  let k = -1;
  while (k + 1 < at.length && ctx.t >= at[k + 1]) k++;
  let pair = [0, 1];
  let p = 0;
  if (k >= 0) {
    const len = Math.min(d, (k + 1 < at.length ? at[k + 1] : Infinity) - at[k]);
    pair = [k, k + 1];
    p = len > 0 ? clamp((ctx.t - at[k]) / len, 0, 1) : 1;
  }
  return h('morph', { p, lag, fade, ease: ease ?? easeInOutCubic, __source }, states[pair[0]], states[pair[1]]);
}
define('Steps', Steps);
