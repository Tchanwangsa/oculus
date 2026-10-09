import { findPattern } from "@/lib/findText";
import type { PdfLine } from "@/lib/pdfView";

/**
 * Find in a PDF, DOM-free: a page's lines joined by spaces become one corpus,
 * folded (NFKD, marks dropped, lower case) so a search ignores case and
 * diacritics and splits ligatures, and a match's folded offsets map back to
 * ranges of characters on the lines it covers, from which the viewer draws
 * boxes with each line's `chars` stops.
 */

export interface Folded {
  text: string;
  /** Per folded UTF-16 unit, the source offset of the character it came from. */
  map: number[];
}

export function fold(source: string): Folded {
  let text = "";
  const map: number[] = [];
  for (let i = 0; i < source.length; ) {
    const ch = String.fromCodePoint(source.codePointAt(i)!);
    const f = ch.normalize("NFKD").replace(/\p{M}/gu, "").toLowerCase();
    for (let k = 0; k < f.length; k++) map.push(i);
    text += f;
    i += ch.length;
  }
  return { text, map };
}

export interface PageCorpus {
  folded: Folded;
  /** The joined text, unfolded. */
  source: string;
  /** Each line's offset in `source`. */
  starts: number[];
  lengths: number[];
}

export function pageCorpus(lines: readonly PdfLine[]): PageCorpus {
  const starts: number[] = [];
  const lengths: number[] = [];
  let at = 0;
  for (const l of lines) {
    starts.push(at);
    lengths.push(l.text.length);
    at += l.text.length + 1;
  }
  const source = lines.map((l) => l.text).join(" ");
  return { folded: fold(source), source, starts, lengths };
}

/** Characters `start..end` of one line. */
export interface LinePart {
  line: number;
  start: number;
  end: number;
}

export interface PdfMatch {
  page: number;
  parts: LinePart[];
}

/** The query as a pattern over folded text, or null for a blank one. */
export function foldedPattern(query: string): RegExp | null {
  return findPattern(fold(query).text);
}

/** Every match of `pattern` on one page, as the line ranges it covers; the
 *  space joining two lines belongs to neither. */
export function matchPage(page: number, corpus: PageCorpus, pattern: RegExp, limit: number): PdfMatch[] {
  const out: PdfMatch[] = [];
  const { folded, source, starts, lengths } = corpus;
  for (const m of folded.text.matchAll(pattern)) {
    if (!m[0].length) continue;
    if (out.length >= limit) break;
    const a = m.index ?? 0;
    const b = a + m[0].length - 1;
    const from = folded.map[a];
    const last = folded.map[b];
    const to = last + (source.codePointAt(last)! > 0xffff ? 2 : 1);
    const parts: LinePart[] = [];
    starts.forEach((s, line) => {
      const start = Math.max(from, s) - s;
      const end = Math.min(to, s + lengths[line]) - s;
      if (end > start) parts.push({ line, start, end });
    });
    if (parts.length) out.push({ page, parts });
  }
  return out;
}

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** The box of chars `start..end` of `line`, in points. Stops that don't fit
 *  the text fall back to an even split of the line. */
export function partRect(line: PdfLine, start: number, end: number): Rect {
  const n = line.text.length;
  const along = line.vertical ? line.height : line.width;
  const origin = line.vertical ? line.y : line.x;
  const stop = (i: number) =>
    line.chars.length === n + 1 ? line.chars[i] : origin + (n ? (along * i) / n : 0);
  const a = stop(start);
  const b = stop(end);
  return line.vertical
    ? { x: line.x, y: a, width: line.width, height: b - a }
    : { x: a, y: line.y, width: b - a, height: line.height };
}
