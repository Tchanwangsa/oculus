import type { MdSkeleton, Pair, Skeleton } from "./skeleton";

/** What may stand between a line's start and its first selected char without
 *  being content: a heading, list or quote marker, a printed bullet. */
const BLOCK_PREFIX = /^[ \t]*(?:>[ \t]*)*(?:#{1,6}[ \t]+|[-*+•][ \t]+|\d{1,9}[.)][ \t]+)?$/;

/**
 * `md[S, E)` snapped outward: atoms whole, a split delimiter whole, a line's
 * block prefix kept; an emphasis pair cut by an end is reopened or closed at
 * it. Image targets go through `resolveImage`.
 */
export function sliceMarkdown(
  md: string,
  sk: MdSkeleton,
  S: number,
  E: number,
  resolveImage: (src: string) => string = (src) => src,
): string {
  if (S >= E) return "";

  // Atoms and pairs both move S left and E right; repeat until neither does.
  for (let changed = true; changed; ) {
    changed = false;
    for (const atom of sk.atoms) {
      if (atom.start >= E || atom.end <= S) continue;
      if (atom.start < S) {
        S = atom.start;
        changed = true;
      }
      if (atom.end > E) {
        E = atom.end;
        changed = true;
      }
    }
    for (const p of sk.pairs) {
      if (S >= p.close.start || E <= p.open.end) continue;
      if (S > p.open.start && S <= p.open.end) {
        S = p.open.start;
        changed = true;
      }
      if (E < p.close.end && E >= p.close.start) {
        E = p.close.end;
        changed = true;
      }
    }
  }

  const lineStart = md.lastIndexOf("\n", S - 1) + 1;
  if (BLOCK_PREFIX.test(md.slice(lineStart, S))) S = lineStart;

  // Pairs the slice cuts through, reopened before it and closed after it.
  const reopen: Pair[] = [];
  const reclose: Pair[] = [];
  for (const p of sk.pairs) {
    if (S >= p.close.start || E <= p.open.end) continue;
    if (S > p.open.start) reopen.push(p);
    if (E < p.close.end) reclose.push(p);
  }
  reopen.sort((x, y) => x.open.start - y.open.start);
  reclose.sort((x, y) => x.close.start - y.close.start);

  let body = "";
  let cursor = S;
  for (const atom of sk.atoms) {
    if (!atom.url || atom.start < S || atom.end > E || atom.url.start < cursor) continue;
    body += md.slice(cursor, atom.url.start) + resolveImage(md.slice(atom.url.start, atom.url.end));
    cursor = atom.url.end;
  }
  body = (body + md.slice(cursor, E)).trim();
  if (!body) return "";
  const prefix = reopen.map((p) => md.slice(p.open.start, p.open.end)).join("");
  const suffix = reclose.map((p) => md.slice(p.close.start, p.close.end)).join("");
  return prefix + body + suffix;
}

/** How many skeleton chars start before source offset `p`. */
export function charsBefore(sk: Skeleton, p: number): number {
  let lo = 0;
  let hi = sk.from.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (sk.from[mid] < p) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}
