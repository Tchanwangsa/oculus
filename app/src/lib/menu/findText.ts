/**
 * The DOM-free half of find in rendered text (`hooks/find/useDomFind.ts`): text
 * nodes in document order become one corpus, a query becomes a pattern, and
 * a match's corpus offsets map back to (node, offset) pairs. Runs of text in
 * different blocks are joined by a NUL, which no pattern matches, so a match
 * never spans a block boundary.
 */

/** One text node's contents; `breakBefore` when a block boundary or `<br>`
 *  separates it from the previous one. */
export interface TextSegment {
  text: string;
  breakBefore: boolean;
}

export interface Corpus {
  text: string;
  /** Each segment's offset in `text`. */
  starts: number[];
}

export interface TextMatch {
  start: number;
  end: number;
}

/** A corpus offset as a segment index and an offset inside it. */
export interface TextPoint {
  segment: number;
  offset: number;
}

const BREAK = "\u0000";

export function buildCorpus(segments: readonly TextSegment[]): Corpus {
  const parts: string[] = [];
  const starts: number[] = [];
  let at = 0;
  segments.forEach((s, i) => {
    if (i > 0 && s.breakBefore) {
      parts.push(BREAK);
      at += 1;
    }
    starts.push(at);
    parts.push(s.text);
    at += s.text.length;
  });
  return { text: parts.join(""), starts };
}

/** Case-insensitive plain text; a whitespace run in the query matches any
 *  whitespace run, since rendering collapses them. Null for a blank query. */
export function findPattern(query: string): RegExp | null {
  if (!query.trim()) return null;
  const source = query
    .split(/\s+/)
    .map((part) => part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
    .join("\\s+");
  return new RegExp(source, "giu");
}

export function findMatches(
  corpus: Corpus,
  query: string,
  limit: number,
): { matches: TextMatch[]; capped: boolean } {
  const pattern = findPattern(query);
  const matches: TextMatch[] = [];
  if (!pattern) return { matches, capped: false };
  for (const m of corpus.text.matchAll(pattern)) {
    if (!m[0].length) continue;
    if (matches.length === limit) return { matches, capped: true };
    const start = m.index ?? 0;
    matches.push({ start, end: start + m[0].length });
  }
  return { matches, capped: false };
}

/** Maps a corpus offset into its segment. An `end` offset sitting on a
 *  boundary belongs to the segment it closes, a start to the one it opens. */
export function locate(corpus: Corpus, offset: number, end: boolean): TextPoint {
  const { starts } = corpus;
  let lo = 0;
  let hi = starts.length - 1;
  // The last segment starting at or before (`end`: strictly before) offset.
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (end ? starts[mid] < offset : starts[mid] <= offset) lo = mid;
    else hi = mid - 1;
  }
  return { segment: lo, offset: offset - starts[lo] };
}
