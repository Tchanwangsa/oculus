import { readCourseFile } from "@/lib/files/courseFiles";
import type { LineRange } from "./parse";
import { normalizeText } from "./text";

/** A parsed `.md` with its pages: `lines` 0-based, `start` 1-based. */
interface PagedMd {
  lines: string[];
  pages: { pageNo: number; start: number; text: string }[];
}

interface PagesJson {
  pages: { page_no: number; markdown: string }[];
}

const paged = new Map<string, Promise<PagedMd | null>>();

/**
 * The parser writes the `.md` as the `.pages.json` pages' markdown joined
 * with "\n\n" (`document_markdown` in `app/src-tauri/src/parse/mod.rs`), blank
 * pages keeping their slot, so page k starts at line
 * 1 + Σ_{j<k} (lines(md_j) + 1). Cached per path; a failed read is retried.
 */
function loadPaged(mdPath: string): Promise<PagedMd | null> {
  let p = paged.get(mdPath);
  if (!p) {
    const json = mdPath.replace(/\.md$/i, ".pages.json");
    p = Promise.all([readCourseFile(mdPath), readCourseFile(json)])
      .then(([md, raw]) => {
        const pagesJson = JSON.parse(raw) as PagesJson;
        let start = 1;
        const pages = pagesJson.pages.map((pg) => {
          const at = start;
          start += pg.markdown.split("\n").length + 1;
          return { pageNo: pg.page_no, start: at, text: pg.markdown };
        });
        return { lines: md.split("\n"), pages };
      })
      .catch(() => {
        // Not parsed yet, perhaps: the next ask reads again.
        paged.delete(mdPath);
        return null;
      });
    paged.set(mdPath, p);
  }
  return p;
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

/** The PDF page a line of a parsed `.md` falls on, checked against the
 *  page's text and searched for when it disagrees; null if unknowable. */
function pageOfLine(doc: PagedMd, range: LineRange): { page: number; quote: string } | null {
  if (range.from < 1 || range.from > doc.lines.length || !doc.pages.length) return null;
  const quote = quoteFromLines(doc.lines, range);
  let k = doc.pages.length - 1;
  while (k > 0 && doc.pages[k].start > range.from) k--;
  const needle = normalizeText(quote);
  if (needle && !normalizeText(doc.pages[k].text).includes(needle)) {
    const found = doc.pages.find((p) => normalizeText(p.text).includes(needle));
    if (found) return { page: found.pageNo, quote };
  }
  return { page: doc.pages[k].pageNo, quote };
}

/** Page and quote for a line of a parsed `.md`. */
export async function citedPage(
  mdPath: string,
  range: LineRange,
): Promise<{ page: number; quote: string } | null> {
  const doc = await loadPaged(mdPath);
  return doc ? pageOfLine(doc, range) : null;
}
