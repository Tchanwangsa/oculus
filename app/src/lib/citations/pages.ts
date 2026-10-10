import { readCourseFile } from "@/lib/files/courseFiles";
import type { LineRange } from "./parse";
import { normalizeText } from "./text";

/** A parsed `.md` with its pages: `lines` 0-based, `start` 1-based. */
interface PagedMd {
  lines: string[];
  pages: { pageNo: number; start: number; text: string; blocks: PdfBlock[] }[];
  record: PagesJson;
}

/** One MinerU block of a page: `bbox` is `[x0, y0, x1, y1]` in fractions of
 *  the page (top-left origin, y down), `start..end` its UTF-16 span in the
 *  page's markdown. Records parsed before blocks were kept have none. */
export interface PdfBlock {
  kind: string;
  bbox: [number, number, number, number];
  start: number;
  end: number;
}

/** A parse's `.pages.json` (`ParsePage` in `app/src-tauri/src/parse/mod.rs`). */
export interface PagesJson {
  pages: { page_no: number; markdown: string; blocks?: PdfBlock[] }[];
}

/** A read older than this is redone by a `fresh` load, so a viewer opened
 *  after a re-parse shows the new record. */
const FRESH_MS = 2000;

const paged = new Map<string, { at: number; doc: Promise<PagedMd | null> }>();

/**
 * The parser writes the `.md` as the `.pages.json` pages' markdown joined
 * with "\n\n" (`document_markdown` in `app/src-tauri/src/parse/mod.rs`), blank
 * pages keeping their slot, so page k starts at line
 * 1 + Σ_{j<k} (lines(md_j) + 1). Cached per path; a failed read is retried.
 */
function loadPaged(mdPath: string, fresh = false): Promise<PagedMd | null> {
  const cached = paged.get(mdPath);
  if (cached && !(fresh && Date.now() - cached.at > FRESH_MS)) return cached.doc;
  const json = mdPath.replace(/\.md$/i, ".pages.json");
  const doc = Promise.all([readCourseFile(mdPath), readCourseFile(json)])
    .then(([md, raw]) => {
      const record = JSON.parse(raw) as PagesJson;
      let start = 1;
      const pages = record.pages.map((pg) => {
        const at = start;
        start += pg.markdown.split("\n").length + 1;
        return { pageNo: pg.page_no, start: at, text: pg.markdown, blocks: pg.blocks ?? [] };
      });
      return { lines: md.split("\n"), pages, record };
    })
    .catch(() => {
      // Not parsed yet, perhaps: the next ask reads again.
      if (paged.get(mdPath)?.doc === doc) paged.delete(mdPath);
      return null;
    });
  paged.set(mdPath, { at: Date.now(), doc });
  return doc;
}

/** A parsed `.md`'s `.pages.json`, from the same cached read as `citedPage`;
 *  null when the file is not parsed. `fresh` re-reads a stale cache. */
export async function loadPagesRecord(mdPath: string, fresh = false): Promise<PagesJson | null> {
  return (await loadPaged(mdPath, fresh))?.record ?? null;
}

const pageMaps = new WeakMap<PagedMd, Map<number, string>>();

/** A parsed `.md`'s pages, `page_no` (1-based) → that page's markdown, from
 *  the same cached read as `citedPage`; null when the file is not parsed. */
export async function loadParsedPages(mdPath: string): Promise<Map<number, string> | null> {
  const doc = await loadPaged(mdPath);
  if (!doc) return null;
  let map = pageMaps.get(doc);
  if (!map) {
    map = new Map(doc.pages.map((p) => [p.pageNo, p.text]));
    pageMaps.set(doc, map);
  }
  return map;
}

/** One markdown line as the reader sees it: no heading, list, emphasis,
 *  code, maths, table, image, link or HTML syntax. */
function plainLine(line: string): string {
  return line
    .replace(/<[^>]+>/g, " ")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, " ")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\$\$[^$]*\$\$|\$[^$\n]*\$/g, " ")
    .replace(/^\s*[|:\-\s]+$/, "")
    .replace(/^\s{0,3}(?:#{1,6}\s+|>\s?)/, "")
    .replace(/^\s*(?:[-*+•]|\d+[.)])\s+/, "")
    .replace(/[|*_`~]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

/** The cited lines' text for highlighting, at most three lines. */
export function quoteFromLines(lines: string[], range: LineRange): string {
  const to = Math.min(range.to, range.from + 2, lines.length);
  return lines
    .slice(range.from - 1, to)
    .map(plainLine)
    .filter(Boolean)
    .join(" ");
}

/** Where a line citation of a parsed `.md` lands: its PDF page, the quote
 *  to highlight, and the page's blocks the lines overlap (empty for a record
 *  without blocks, or when the page was found by searching for the quote). */
export interface CitedPage {
  page: number;
  quote: string;
  blocks: number[];
}

/** The PDF page a line of a parsed `.md` falls on, checked against the
 *  page's text and searched for when it disagrees; null if unknowable. */
function pageOfLine(doc: PagedMd, range: LineRange): CitedPage | null {
  if (range.from < 1 || range.from > doc.lines.length || !doc.pages.length) return null;
  const quote = quoteFromLines(doc.lines, range);
  let k = doc.pages.length - 1;
  while (k > 0 && doc.pages[k].start > range.from) k--;
  const pg = doc.pages[k];
  const needle = normalizeText(quote);
  if (needle && !normalizeText(pg.text).includes(needle)) {
    const found = doc.pages.find((p) => normalizeText(p.text).includes(needle));
    if (found) return { page: found.pageNo, quote, blocks: [] };
  }
  const blocks = blocksOnLines(pg.text, pg.blocks, range.from - pg.start, range.to - pg.start);
  return { page: pg.pageNo, quote, blocks };
}

/** The indices of `blocks` overlapping lines `from..to` (0-based, inclusive)
 *  of a page's markdown. */
export function blocksOnLines(text: string, blocks: readonly PdfBlock[], from: number, to: number): number[] {
  const lines = text.split("\n");
  if (!blocks.length || from < 0 || from >= lines.length) return [];
  const last = Math.min(Math.max(to, from), lines.length - 1);
  let a = 0;
  for (let i = 0; i < from; i++) a += lines[i].length + 1;
  let b = a;
  for (let i = from; i <= last; i++) b += lines[i].length + (i < last ? 1 : 0);
  if (b <= a) return [];
  const out: number[] = [];
  blocks.forEach((blk, i) => {
    if (blk.start < b && blk.end > a) out.push(i);
  });
  return out;
}

/** Page, quote and blocks for a line of a parsed `.md`. */
export async function citedPage(mdPath: string, range: LineRange): Promise<CitedPage | null> {
  const doc = await loadPaged(mdPath);
  return doc ? pageOfLine(doc, range) : null;
}
