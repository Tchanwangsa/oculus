/**
 * The word-level diff behind a note's History: which words of a saved
 * version changed against its base, for marking on the page and stepping
 * through. Myers over word/space/newline/punctuation tokens, cleaned up for
 * reading (`cleanup.ts`). Two unrelated versions blow the work budget; the
 * middle is then diffed by lines and each changed block refined by words
 * while budget lasts, so a big diff can't freeze the UI.
 */

import { cleanup, merge, type Op } from "./cleanup";
import { DEL, diffSeq, EQ, INS, type Budget, type OpType } from "./myers";
import { lineBounds, splitsUnit, tokenBounds, wordAt, wordBefore } from "./tokenize";

export { diffHunks, diffStats } from "./hunks";

/** One edit: `a[fromA,toA)` became `b[fromB,toB)`. UTF-16 offsets. */
export interface DiffChange {
  fromA: number;
  toA: number;
  fromB: number;
  toB: number;
}

/** Changes on the same or adjacent lines of `b`, for navigation. */
export interface DiffHunk {
  fromB: number;
  toB: number;
  changes: DiffChange[];
}

/** Work units (token steps) for the word diff of the whole middle, the line
 *  diff that replaces it, and all the word refinements of its blocks. */
const WORD_BUDGET = 2_000_000;
const LINE_BUDGET = 4_000_000;
const REFINE_BUDGET = 6_000_000;

/** Word-level, cleaned for reading; sorted, non-overlapping, non-adjacent. */
export function diffText(a: string, b: string): DiffChange[] {
  if (a === b) return [];
  const [head, tail] = commonEnds(a, b);
  const ops: Op[] = [{ t: EQ, text: a.slice(0, head) }];
  ops.push(...diffMiddle(a, head, a.length - tail, b, head, b.length - tail));
  ops.push({ t: EQ, text: a.slice(a.length - tail) });
  return toChanges(cleanup(ops));
}

/** Shared head and tail lengths, pulled back so neither cuts a word, a
 *  surrogate pair, a CRLF or an emoji. */
function commonEnds(a: string, b: string): [number, number] {
  const max = Math.min(a.length, b.length);
  let head = 0;
  while (head < max && a.charCodeAt(head) === b.charCodeAt(head)) head++;
  // `a` and `b` agree before `head`, and from their ends back to the tail.
  const cutsHead = (i: number) =>
    splitsUnit(a, i) || splitsUnit(b, i) || (wordBefore(a, i) && (wordAt(a, i) || wordAt(b, i)));
  while (head > 0 && cutsHead(head)) head--;
  let tail = 0;
  while (tail < max - head && a.charCodeAt(a.length - 1 - tail) === b.charCodeAt(b.length - 1 - tail)) tail++;
  const cutsTail = (ea: number, eb: number) =>
    splitsUnit(a, ea) || splitsUnit(b, eb) || (wordAt(a, ea) && (wordBefore(a, ea) || wordBefore(b, eb)));
  while (tail > 0 && cutsTail(a.length - tail, b.length - tail)) tail--;
  return [head, tail];
}

function diffMiddle(a: string, aFrom: number, aTo: number, b: string, bFrom: number, bTo: number): Op[] {
  if (aFrom === aTo || bFrom === bTo) return replace(a, aFrom, aTo, b, bFrom, bTo);
  const words = tryDiff(a, tokenBounds(a, aFrom, aTo), b, tokenBounds(b, bFrom, bTo), { left: WORD_BUDGET });
  if (words) return words;
  const lines = tryDiff(a, lineBounds(a, aFrom, aTo), b, lineBounds(b, bFrom, bTo), { left: LINE_BUDGET });
  if (!lines) return replace(a, aFrom, aTo, b, bFrom, bTo);
  return refine(merge(lines), a, aFrom, b, bFrom);
}

/** Re-diffs by words each block where lines were both deleted and inserted. */
function refine(ops: Op[], a: string, aFrom: number, b: string, bFrom: number): Op[] {
  const budget: Budget = { left: REFINE_BUDGET };
  const out: Op[] = [];
  let pa = aFrom;
  let pb = bFrom;
  for (let i = 0; i < ops.length; i++) {
    const op = ops[i];
    const next = ops[i + 1];
    if (op.t === DEL && next?.t === INS) {
      const aTo = pa + op.text.length;
      const bTo = pb + next.text.length;
      const words =
        budget.left > 0 ? tryDiff(a, tokenBounds(a, pa, aTo), b, tokenBounds(b, pb, bTo), budget) : null;
      out.push(...(words ?? [op, next]));
      pa = aTo;
      pb = bTo;
      i++;
      continue;
    }
    out.push(op);
    if (op.t !== INS) pa += op.text.length;
    if (op.t !== DEL) pb += op.text.length;
  }
  return out;
}

function replace(a: string, aFrom: number, aTo: number, b: string, bFrom: number, bTo: number): Op[] {
  return [
    { t: DEL, text: a.slice(aFrom, aTo) },
    { t: INS, text: b.slice(bFrom, bTo) },
  ];
}

/** Diffs two token sequences given by their boundaries; null when the work
 *  runs past `budget` (which is left spent). */
function tryDiff(a: string, ab: number[], b: string, bb: number[], budget: Budget): Op[] | null {
  const ids = new Map<string, number>();
  const intern = (s: string, bounds: number[]) => {
    const out = new Int32Array(bounds.length - 1);
    for (let i = 0; i < out.length; i++) {
      const tok = s.slice(bounds[i], bounds[i + 1]);
      let id = ids.get(tok);
      if (id === undefined) ids.set(tok, (id = ids.size));
      out[i] = id;
    }
    return out;
  };
  const x = intern(a, ab);
  const y = intern(b, bb);
  const ops: Op[] = [];
  const push = (t: OpType, text: string) => {
    const last = ops[ops.length - 1];
    if (last?.t === t) last.text += text;
    else ops.push({ t, text });
  };
  const done = diffSeq(x, y, budget, (t, xFrom, xTo, yFrom, yTo) => {
    if (t === INS) push(t, b.slice(bb[yFrom], bb[yTo]));
    else push(t, a.slice(ab[xFrom], ab[xTo]));
  });
  return done ? ops : null;
}

function toChanges(ops: Op[]): DiffChange[] {
  const out: DiffChange[] = [];
  let pa = 0;
  let pb = 0;
  let open: DiffChange | null = null;
  for (const op of ops) {
    if (op.t === EQ) {
      open = null;
      pa += op.text.length;
      pb += op.text.length;
      continue;
    }
    if (!open) out.push((open = { fromA: pa, toA: pa, fromB: pb, toB: pb }));
    if (op.t === DEL) open.toA = pa += op.text.length;
    else open.toB = pb += op.text.length;
  }
  return out;
}
