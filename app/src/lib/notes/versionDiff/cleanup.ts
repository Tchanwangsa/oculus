/**
 * Makes a minimal edit script readable, after diff-match-patch's semantic
 * cleanup: slide lone edits onto word and line boundaries, and fold short
 * equalities sandwiched between larger edits into one replacement, so a
 * rewrite reads "quick brown → slow red", not alternating fragments.
 */

import { DEL, EQ, INS, type OpType } from "./myers";
import { countWords, splitsUnit, wordAt, wordBefore } from "./tokenize";

export interface Op {
  t: OpType;
  text: string;
}

/** How far a lone edit may slide either way looking for a better boundary. */
const SLIDE = 256;

export function cleanup(ops: Op[]): Op[] {
  let out = merge(ops);
  out = merge(slide(out));
  out = merge(eliminate(out));
  return merge(slide(out));
}

/** Joins equalities, puts each run of edits as one deletion then one
 *  insertion, and drops empty ops. */
export function merge(ops: Op[]): Op[] {
  const out: Op[] = [];
  let del = "";
  let ins = "";
  const flush = () => {
    if (del) out.push({ t: DEL, text: del });
    if (ins) out.push({ t: INS, text: ins });
    del = ins = "";
  };
  for (const op of ops) {
    if (!op.text) continue;
    if (op.t === DEL) del += op.text;
    else if (op.t === INS) ins += op.text;
    else {
      flush();
      const last = out[out.length - 1];
      if (last?.t === EQ) last.text += op.text;
      else out.push({ t: EQ, text: op.text });
    }
  }
  flush();
  return out;
}

/** Slides each edit that sits alone between two equalities to the position
 *  whose two edges score best (rightmost on a tie). */
function slide(ops: Op[]): Op[] {
  for (let i = 1; i < ops.length - 1; i++) {
    const before = ops[i - 1];
    const edit = ops[i];
    const after = ops[i + 1];
    if (before.t !== EQ || after.t !== EQ || edit.t === EQ) continue;
    const s = before.text + edit.text + after.text;
    const len = edit.text.length;
    const p0 = before.text.length;
    let p = p0;
    while (p > 0 && p0 - p < SLIDE && s.charCodeAt(p - 1) === s.charCodeAt(p + len - 1)) p--;
    let best = p0;
    let bestScore = boundaryScore(s, p0) + boundaryScore(s, p0 + len);
    for (; ; p++) {
      const score = boundaryScore(s, p) + boundaryScore(s, p + len);
      if (score >= bestScore && (score > bestScore || p > best)) {
        best = p;
        bestScore = score;
      }
      if (p + len >= s.length || p - p0 >= SLIDE || s.charCodeAt(p) !== s.charCodeAt(p + len)) break;
    }
    if (best !== p0) {
      before.text = s.slice(0, best);
      edit.text = s.slice(best, best + len);
      after.text = s.slice(best + len);
    }
  }
  return ops;
}

const isSpace = (c: number) => c === 32 || c === 9 || c === 10 || c === 13 || c === 12 || c === 11 || c === 0xa0;
const isBreak = (c: number) => c === 10 || c === 13;

/** How good a cut at `i` is: document edge 6, blank line 5, line break 4,
 *  sentence end 3, space 2, punctuation 1, mid-word 0; never inside a
 *  surrogate pair or CRLF. */
function boundaryScore(s: string, i: number): number {
  if (i <= 0 || i >= s.length) return 6;
  if (splitsUnit(s, i)) return -100;
  const c1 = s.charCodeAt(i - 1);
  const c2 = s.charCodeAt(i);
  const word1 = wordBefore(s, i);
  const word2 = wordAt(s, i);
  const space1 = !word1 && isSpace(c1);
  const space2 = !word2 && isSpace(c2);
  const blank1 = c1 === 10 && /\n\r?\n$/.test(s.slice(Math.max(0, i - 3), i));
  const blank2 = isBreak(c2) && /^\r?\n\r?\n/.test(s.slice(i, i + 4));
  if (blank1 || blank2) return 5;
  if (isBreak(c1) || isBreak(c2)) return 4;
  if (!word1 && !space1 && space2) return 3;
  if (space1 || space2) return 2;
  if (!word1 || !word2) return 1;
  return 0;
}

/** Turns each short equality — at most one word, no newline — that is no
 *  longer than the edits on both sides of it into a deletion plus an
 *  insertion. Changes on different lines stay separate. */
function eliminate(input: Op[]): Op[] {
  const ops = input.slice();
  const equalities: number[] = [];
  let last: string | null = null;
  let ins1 = 0;
  let del1 = 0;
  let ins2 = 0;
  let del2 = 0;
  for (let i = 0; i < ops.length; i++) {
    const op = ops[i];
    if (op.t === EQ) {
      equalities.push(i);
      ins1 = ins2;
      del1 = del2;
      ins2 = del2 = 0;
      last = op.text.includes("\n") || countWords(op.text) > 1 ? null : op.text;
      continue;
    }
    if (op.t === INS) ins2 += op.text.length;
    else del2 += op.text.length;
    if (last !== null && last.length <= Math.max(ins1, del1) && last.length <= Math.max(ins2, del2)) {
      const at = equalities[equalities.length - 1];
      ops.splice(at, 1, { t: DEL, text: last }, { t: INS, text: last });
      // The previous equality may now be eliminable too: rescan from the
      // one before it.
      equalities.pop();
      equalities.pop();
      i = equalities.length ? equalities[equalities.length - 1] : -1;
      ins1 = del1 = ins2 = del2 = 0;
      last = null;
    }
  }
  return ops;
}
