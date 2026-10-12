/**
 * A line diff for comparing a file with Ed's copy of it: the longest common
 * subsequence of lines, after trimming the shared head and tail. `removed`
 * lines are only in `a`, `added` only in `b`; within a change, removals come
 * first.
 */

export type LineDiffKind = "same" | "removed" | "added";

export interface DiffLine {
  kind: LineDiffKind;
  text: string;
}

export type DiffRow = DiffLine | { kind: "fold"; count: number };

/** Past this many LCS cells (a 2000 × 2000 line middle) the changed middle is
 *  shown as all removed then all added rather than filling the table. */
const MAX_CELLS = 4_000_000;

export const splitLines = (text: string): string[] => (text === "" ? [] : text.split(/\r?\n/));

export function lineDiff(a: string, b: string): DiffLine[] {
  const x = splitLines(a);
  const y = splitLines(b);
  let head = 0;
  while (head < x.length && head < y.length && x[head] === y[head]) head++;
  let tail = 0;
  while (tail < x.length - head && tail < y.length - head && x[x.length - 1 - tail] === y[y.length - 1 - tail]) tail++;

  const out: DiffLine[] = x.slice(0, head).map((text) => ({ kind: "same", text }));
  out.push(...middle(x.slice(head, x.length - tail), y.slice(head, y.length - tail)));
  out.push(...x.slice(x.length - tail).map((text): DiffLine => ({ kind: "same", text })));
  return out;
}

function middle(x: string[], y: string[]): DiffLine[] {
  const n = x.length;
  const m = y.length;
  const removed = (text: string): DiffLine => ({ kind: "removed", text });
  const added = (text: string): DiffLine => ({ kind: "added", text });
  if (n === 0 || m === 0 || (n + 1) * (m + 1) > MAX_CELLS) return [...x.map(removed), ...y.map(added)];

  // lcs[i][j]: the LCS length of x[i..] and y[j..], row-major.
  const w = m + 1;
  const lcs = new Uint32Array((n + 1) * w);
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      lcs[i * w + j] =
        x[i] === y[j] ? lcs[(i + 1) * w + j + 1] + 1 : Math.max(lcs[(i + 1) * w + j], lcs[i * w + j + 1]);
    }
  }
  const out: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (x[i] === y[j]) {
      out.push({ kind: "same", text: x[i] });
      i++;
      j++;
    } else if (lcs[(i + 1) * w + j] >= lcs[i * w + j + 1]) {
      out.push(removed(x[i++]));
    } else {
      out.push(added(y[j++]));
    }
  }
  while (i < n) out.push(removed(x[i++]));
  while (j < m) out.push(added(y[j++]));
  return out;
}

/** Folds runs of unchanged lines, keeping `context` lines beside each change.
 *  A run that would fold a single line stays. */
export function foldUnchanged(lines: DiffLine[], context = 3): DiffRow[] {
  const out: DiffRow[] = [];
  let i = 0;
  while (i < lines.length) {
    if (lines[i].kind !== "same") {
      out.push(lines[i++]);
      continue;
    }
    let end = i;
    while (end < lines.length && lines[end].kind === "same") end++;
    const keepHead = i === 0 ? 0 : context;
    const keepTail = end === lines.length ? 0 : context;
    const folded = end - i - keepHead - keepTail;
    if (folded > 1) {
      out.push(...lines.slice(i, i + keepHead));
      out.push({ kind: "fold", count: folded });
      out.push(...lines.slice(end - keepTail, end));
    } else {
      out.push(...lines.slice(i, end));
    }
    i = end;
  }
  return out;
}
