/**
 * Myers' O(ND) diff over interned token ids, in linear space: find the
 * middle snake, recurse on both halves. Every step of work is charged to a
 * budget; once it runs out the diff stops and reports failure, so the caller
 * can diff coarser.
 */

export const EQ = 0;
export const DEL = -1;
export const INS = 1;
export type OpType = typeof EQ | typeof DEL | typeof INS;

/** A run of the edit script, as token indices into `x` and `y`. */
export type Emit = (t: OpType, xFrom: number, xTo: number, yFrom: number, yTo: number) => void;

export interface Budget {
  left: number;
}

/** Emits the edit script in order; false when `budget` ran out part way. */
export function diffSeq(x: Int32Array, y: Int32Array, budget: Budget, emit: Emit): boolean {
  seq(x, 0, y, 0, budget, emit);
  return budget.left >= 0;
}

/** `x`/`y` are views; `xBase`/`yBase` turn their indices back into absolute ones. */
function seq(x: Int32Array, xBase: number, y: Int32Array, yBase: number, budget: Budget, emit: Emit) {
  let n = x.length;
  let m = y.length;
  let head = 0;
  while (head < n && head < m && x[head] === y[head]) head++;
  let tail = 0;
  while (tail < n - head && tail < m - head && x[n - 1 - tail] === y[m - 1 - tail]) tail++;
  budget.left -= head + tail + 1;
  if (budget.left < 0) return;
  if (head) emit(EQ, xBase, xBase + head, yBase, yBase + head);
  const xs = x.subarray(head, n - tail);
  const ys = y.subarray(head, m - tail);
  n = xs.length;
  m = ys.length;
  const xb = xBase + head;
  const yb = yBase + head;
  if (n === 0 && m > 0) emit(INS, xb, xb, yb, yb + m);
  else if (m === 0 && n > 0) emit(DEL, xb, xb + n, yb, yb);
  else if (n > 0 && m > 0) {
    const split = bisect(xs, ys, budget);
    if (budget.left < 0) return;
    if (split) {
      const [sx, sy] = split;
      seq(xs.subarray(0, sx), xb, ys.subarray(0, sy), yb, budget, emit);
      if (budget.left < 0) return;
      seq(xs.subarray(sx), xb + sx, ys.subarray(sy), yb + sy, budget, emit);
    } else {
      emit(DEL, xb, xb + n, yb, yb);
      emit(INS, xb + n, xb + n, yb, yb + m);
    }
  }
  if (tail) emit(EQ, xBase + head + n, xBase + head + n + tail, yBase + head + m, yBase + head + m + tail);
}

/** The middle snake's start, walking forward and backward at once; null when
 *  the two share nothing or the budget ran out. After diff-match-patch's `diff_bisect_`. */
function bisect(x: Int32Array, y: Int32Array, budget: Budget): [number, number] | null {
  const n = x.length;
  const m = y.length;
  const maxD = Math.ceil((n + m) / 2);
  const off = maxD;
  // Room for diagonals -maxD-1..maxD+1, so no read falls outside the array.
  const size = 2 * maxD + 2;
  const v1 = new Int32Array(size).fill(-1);
  const v2 = new Int32Array(size).fill(-1);
  v1[off + 1] = 0;
  v2[off + 1] = 0;
  const delta = n - m;
  const front = delta % 2 !== 0;
  let k1start = 0;
  let k1end = 0;
  let k2start = 0;
  let k2end = 0;
  for (let d = 0; d < maxD; d++) {
    let work = 0;
    for (let k1 = -d + k1start; k1 <= d - k1end; k1 += 2) {
      const k1o = off + k1;
      let x1 = k1 === -d || (k1 !== d && v1[k1o - 1] < v1[k1o + 1]) ? v1[k1o + 1] : v1[k1o - 1] + 1;
      let y1 = x1 - k1;
      while (x1 < n && y1 < m && x[x1] === y[y1]) {
        x1++;
        y1++;
        work++;
      }
      work++;
      v1[k1o] = x1;
      if (x1 > n) k1end += 2;
      else if (y1 > m) k1start += 2;
      else if (front) {
        const k2o = off + delta - k1;
        if (k2o >= 0 && k2o < size && v2[k2o] !== -1 && x1 >= n - v2[k2o]) return [x1, y1];
      }
    }
    for (let k2 = -d + k2start; k2 <= d - k2end; k2 += 2) {
      const k2o = off + k2;
      let x2 = k2 === -d || (k2 !== d && v2[k2o - 1] < v2[k2o + 1]) ? v2[k2o + 1] : v2[k2o - 1] + 1;
      let y2 = x2 - k2;
      while (x2 < n && y2 < m && x[n - x2 - 1] === y[m - y2 - 1]) {
        x2++;
        y2++;
        work++;
      }
      work++;
      v2[k2o] = x2;
      if (x2 > n) k2end += 2;
      else if (y2 > m) k2start += 2;
      else if (!front) {
        const k1o = off + delta - k2;
        if (k1o >= 0 && k1o < size && v1[k1o] !== -1) {
          const x1 = v1[k1o];
          if (x1 >= n - x2) return [x1, off + x1 - k1o];
        }
      }
    }
    budget.left -= work;
    if (budget.left < 0) return null;
  }
  return null;
}
