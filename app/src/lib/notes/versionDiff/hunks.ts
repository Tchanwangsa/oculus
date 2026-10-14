import type { DiffChange, DiffHunk } from "./index";
import { countWords } from "./tokenize";

/** Offsets where each line of `s` starts. */
function lineStarts(s: string): number[] {
  const out = [0];
  for (let i = s.indexOf("\n"); i !== -1; i = s.indexOf("\n", i + 1)) out.push(i + 1);
  return out;
}

/** The line holding offset `pos`. */
function lineOf(starts: number[], pos: number): number {
  let lo = 0;
  let hi = starts.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (starts[mid] <= pos) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

export function diffHunks(changes: DiffChange[], b: string): DiffHunk[] {
  const starts = lineStarts(b);
  const out: DiffHunk[] = [];
  let lastLine = -2;
  for (const c of changes) {
    const first = lineOf(starts, c.fromB);
    const end = c.toB > c.fromB ? lineOf(starts, c.toB - 1) : first;
    const hunk = out[out.length - 1];
    if (hunk && first <= lastLine + 1) {
      hunk.toB = Math.max(hunk.toB, c.toB);
      hunk.changes.push(c);
    } else {
      out.push({ fromB: c.fromB, toB: c.toB, changes: [c] });
    }
    lastLine = Math.max(lastLine, end);
  }
  return out;
}

export function diffStats(a: string, b: string, changes: DiffChange[]): { inserted: number; deleted: number } {
  let inserted = 0;
  let deleted = 0;
  for (const c of changes) {
    if (c.toB > c.fromB) inserted += countWords(b.slice(c.fromB, c.toB));
    if (c.toA > c.fromA) deleted += countWords(a.slice(c.fromA, c.toA));
  }
  return { inserted, deleted };
}
