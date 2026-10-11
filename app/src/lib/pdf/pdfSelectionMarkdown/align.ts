/** `t[t, t+len)` equals `m[m, m+len)`. */
export interface Match {
  t: number;
  m: number;
  len: number;
}

export interface Alignment {
  /** Increasing in both strings, never overlapping. */
  runs: Match[];
  tLen: number;
  mLen: number;
  /** The share of the shorter string inside runs of at least `GRAM` chars. */
  coverage: number;
}

/** Anchor length. On the fixtures 6–12 all score within a few windows of
 *  each other; shorter grams also tried inside gaps made garbled maths match
 *  by chance, so a gap is re-aligned with grams unique inside it instead. */
const GRAM = 8;

/** Below this share of the shorter side in runs of `GRAM`+ chars, the page's
 *  mapping is guesswork (see `align`). */
export const MIN_COVERAGE = 0.3;

/** A text layer this short is matched whole or not at all. */
export const SHORT = 2 * GRAM;

/** Indices of the longest chain of anchors increasing in `m` (`t` already
 *  increases): patience sorting. */
function longestIncreasing(ms: number[]): number[] {
  const tails: number[] = [];
  const prev = new Array<number>(ms.length);
  for (let k = 0; k < ms.length; k++) {
    let lo = 0;
    let hi = tails.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (ms[tails[mid]] < ms[k]) lo = mid + 1;
      else hi = mid;
    }
    prev[k] = lo > 0 ? tails[lo - 1] : -1;
    tails[lo] = k;
  }
  const out: number[] = [];
  for (let k = tails.length ? tails[tails.length - 1] : -1; k >= 0; k = prev[k]) out.push(k);
  return out.reverse();
}

/** Per `k`-gram of `s[a, b)`, its only start, or -1 when it repeats. */
function uniqueGrams(s: string, a: number, b: number, k: number): Map<string, number> {
  const out = new Map<string, number>();
  for (let i = a; i + k <= b; i++) {
    const g = s.slice(i, i + k);
    out.set(g, out.has(g) ? -1 : i);
  }
  return out;
}

/** Aligns `t[t0, t1)` to `m[m0, m1)`, appending runs to `out` in order:
 *  patience diff, recursing into the gaps between its runs. */
function alignGap(t: string, m: string, t0: number, t1: number, m0: number, m1: number, out: Match[]): void {
  const k = GRAM;
  if (t1 - t0 < k || m1 - m0 < k) return;
  const tg = uniqueGrams(t, t0, t1, k);
  const mg = uniqueGrams(m, m0, m1, k);
  const ts: number[] = [];
  const ms: number[] = [];
  for (let i = t0; i + k <= t1; i++) {
    const g = t.slice(i, i + k);
    if (tg.get(g) !== i) continue;
    const j = mg.get(g);
    if (j === undefined || j < 0) continue;
    ts.push(i);
    ms.push(j);
  }
  if (!ts.length) return;

  // Chain → runs, merging anchors on one diagonal, trimming overlaps.
  const runs: Match[] = [];
  for (const c of longestIncreasing(ms)) {
    let rt = ts[c];
    let rm = ms[c];
    let len = k;
    const last = runs[runs.length - 1];
    if (last && rt - last.t === rm - last.m && rt <= last.t + last.len) {
      last.len = Math.max(last.len, rt + len - last.t);
      continue;
    }
    if (last) {
      const cut = Math.max(last.t + last.len - rt, last.m + last.len - rm, 0);
      rt += cut;
      rm += cut;
      len -= cut;
      if (len <= 0) continue;
    }
    runs.push({ t: rt, m: rm, len });
  }

  // Grow each run outward while the chars agree, up to its neighbours.
  for (let r = 0; r < runs.length; r++) {
    const run = runs[r];
    const lo = runs[r - 1];
    const loT = lo ? lo.t + lo.len : t0;
    const loM = lo ? lo.m + lo.len : m0;
    while (run.t > loT && run.m > loM && t[run.t - 1] === m[run.m - 1]) {
      run.t--;
      run.m--;
      run.len++;
    }
    const hi = runs[r + 1];
    const hiT = hi ? hi.t : t1;
    const hiM = hi ? hi.m : m1;
    while (run.t + run.len < hiT && run.m + run.len < hiM && t[run.t + run.len] === m[run.m + run.len]) {
      run.len++;
    }
  }

  let pt = t0;
  let pm = m0;
  for (const run of runs) {
    alignGap(t, m, pt, run.t, pm, run.m, out);
    out.push(run);
    pt = run.t + run.len;
    pm = run.m + run.len;
  }
  alignGap(t, m, pt, t1, pm, m1, out);
}

/**
 * A monotonic alignment of text-layer skeleton `t` to markdown skeleton `m`:
 * grams unique to both anchor it, the longest increasing chain of anchors
 * wins, each run is grown while the chars agree, and the gaps are aligned the
 * same way. Null when less than `minCoverage` of the shorter string is in
 * runs of `GRAM`+ chars — the page then copies as plain text.
 */
export function align(t: string, m: string, minCoverage = MIN_COVERAGE): Alignment | null {
  if (!t.length || !m.length) return null;
  if (t.length < SHORT) {
    const i = m.indexOf(t);
    if (i < 0 || m.indexOf(t, i + 1) >= 0) return null;
    return { runs: [{ t: 0, m: i, len: t.length }], tLen: t.length, mLen: m.length, coverage: 1 };
  }
  const runs: Match[] = [];
  alignGap(t, m, 0, t.length, 0, m.length, runs);
  // The shorter side: a text layer can carry text the markdown never had
  // (embedded TeX source, a diagram's labels) and the markdown OCR'd text.
  const strong = runs.reduce((n, r) => (r.len >= GRAM ? n + r.len : n), 0);
  const coverage = strong / Math.min(t.length, m.length);
  if (coverage < minCoverage) return null;
  return { runs, tLen: t.length, mLen: m.length, coverage };
}

/** A boundary in `t` (0…tLen) carried to `m`: exact inside a run,
 *  proportional across a gap. */
export function mapBoundary(al: Alignment, p: number): number {
  const { runs } = al;
  let lo = 0;
  let hi = runs.length;
  // The first run starting after `p`.
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (runs[mid].t <= p) lo = mid + 1;
    else hi = mid;
  }
  const before = runs[lo - 1];
  if (before && p <= before.t + before.len) return before.m + (p - before.t);
  const pt = before ? before.t + before.len : 0;
  const pm = before ? before.m + before.len : 0;
  const after = runs[lo];
  const nt = after ? after.t : al.tLen;
  const nm = after ? after.m : al.mLen;
  if (nt === pt) return pm;
  return pm + Math.round(((p - pt) * (nm - pm)) / (nt - pt));
}
