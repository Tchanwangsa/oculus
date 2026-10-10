import { normalizeMath } from "@/lib/markdown/math";
import { align, mapBoundary, SHORT, type Alignment } from "./align";
import { charsBefore, sliceMarkdown } from "./slice";
import { markdownSkeleton, textSkeleton, type MdSkeleton, type Skeleton, type Span } from "./skeleton";

/** A page's slot in the document: source and skeleton spans. */
export interface DocPage {
  start: number;
  end: number;
  skStart: number;
  skEnd: number;
  parsed: boolean;
}

/** The parsed pages as one markdown string (joined like the `.md` file), so a
 *  selection over several pages is one contiguous slice. */
export interface ParsedDoc {
  md: string;
  sk: MdSkeleton;
  pages: Map<number, DocPage>;
}

const docs = new WeakMap<ReadonlyMap<number, string>, ParsedDoc>();

/** `pages` (page_no → markdown) as a document, cached per map. Each page is
 *  scanned on its own, so a stray delimiter cannot pair across pages. */
export function parsedDoc(pages: ReadonlyMap<number, string>): ParsedDoc {
  const hit = docs.get(pages);
  if (hit) return hit;
  const doc: ParsedDoc = { md: "", sk: { text: "", from: [], to: [], atoms: [], pairs: [] }, pages: new Map() };
  const last = Math.max(0, ...pages.keys());
  const shift = (sp: Span, by: number): Span => ({ start: sp.start + by, end: sp.end + by });
  for (let n = 1; n <= last; n++) {
    if (n > 1) doc.md += "\n\n";
    const md = pages.get(n);
    const base = doc.md.length;
    const skStart = doc.sk.text.length;
    if (md !== undefined) {
      const sk = markdownSkeleton(md);
      doc.md += md;
      doc.sk.text += sk.text;
      for (const f of sk.from) doc.sk.from.push(f + base);
      for (const t of sk.to) doc.sk.to.push(t + base);
      for (const a of sk.atoms) doc.sk.atoms.push({ ...shift(a, base), url: a.url && shift(a.url, base) });
      for (const p of sk.pairs) doc.sk.pairs.push({ open: shift(p.open, base), close: shift(p.close, base) });
    }
    doc.pages.set(n, { start: base, end: doc.md.length, skStart, skEnd: doc.sk.text.length, parsed: md !== undefined });
  }
  docs.set(pages, doc);
  return doc;
}

/** Text carried onto the next PDF page: MinerU files a paragraph under the
 *  page it starts on. A page's unmatched lead (trail) is looked for at the end
 *  (start) of the previous (next) page's markdown, within this many skeleton
 *  chars of it (a paper's page number and margin line numbers come to ~150)… */
const SPILL_SLACK = 200;

/** …with at least this share of it matched. */
const SPILL_COVERAGE = 0.5;

/** An alignment of `t[offset, …)` to the document's skeleton from `base`. */
interface Region {
  al: Alignment;
  offset: number;
  base: number;
}

/** A page's text layer mapped onto the document. */
interface PageMap {
  main: Region;
  lead: Region | null;
  trail: Region | null;
}

function runsEnd(al: Alignment): number {
  const last = al.runs[al.runs.length - 1];
  return last ? last.t + last.len : 0;
}

/** Like `mapBoundary`, but held to the matched span: outside it is text the
 *  region does not have (a running header above a carried paragraph). */
function mapClamped(al: Alignment, p: number): number {
  const first = al.runs[0];
  const last = al.runs[al.runs.length - 1];
  if (p <= first.t) return first.m;
  if (p >= last.t + last.len) return last.m + last.len;
  return mapBoundary(al, p);
}

function pageMap(doc: ParsedDoc, n: number, t: Skeleton): PageMap | null {
  const page = doc.pages.get(n);
  if (!page?.parsed || !t.text.length) return null;
  const text = doc.sk.text;
  const window = (a: DocPage, b: DocPage) => text.slice(a.skStart, b.skEnd);
  const prev = doc.pages.get(n - 1);
  const next = doc.pages.get(n + 1);

  const own = align(t.text, window(page, page));
  if (!own) {
    // A page made mostly of carried text: align against its neighbours too.
    const lo = prev ?? page;
    const wide = align(t.text, window(lo, next ?? page));
    return wide && { main: { al: wide, offset: 0, base: lo.skStart }, lead: null, trail: null };
  }
  const main: Region = { al: own, offset: 0, base: page.skStart };
  let lead: Region | null = null;
  let trail: Region | null = null;
  const firstT = own.runs.length ? own.runs[0].t : t.text.length;
  if (prev?.parsed && firstT >= SHORT) {
    const m = window(prev, prev);
    const al = align(t.text.slice(0, firstT), m, SPILL_COVERAGE);
    if (al?.runs.length && m.length - (al.runs[al.runs.length - 1].m + al.runs[al.runs.length - 1].len) <= SPILL_SLACK)
      lead = { al, offset: 0, base: prev.skStart };
  }
  const lastT = runsEnd(own);
  if (next?.parsed && t.text.length - lastT >= SHORT) {
    const al = align(t.text.slice(lastT), window(next, next), SPILL_COVERAGE);
    if (al?.runs.length && al.runs[0].m <= SPILL_SLACK) trail = { al, offset: lastT, base: next.skStart };
  }
  return { main, lead, trail };
}

/** A text-layer boundary carried to a document skeleton boundary. */
function toDoc(pm: PageMap, p: number): number {
  const { main, lead, trail } = pm;
  if (lead && p < main.al.runs[0].t) return lead.base + mapClamped(lead.al, p - lead.offset);
  if (trail && p > runsEnd(main.al)) return trail.base + mapClamped(trail.al, p - trail.offset);
  return main.base + mapBoundary(main.al, p - main.offset);
}

/** Source offset of skeleton boundary `b` read as a start: its next char. */
function startAt(doc: ParsedDoc, b: number): number {
  return b >= doc.sk.text.length ? doc.md.length : doc.sk.from[b];
}

/** …read as an end: just past its previous char. */
function endAt(doc: ParsedDoc, b: number): number {
  return b <= 0 ? 0 : doc.sk.to[b - 1];
}

/** Where the page's own text begins in the document: before any of its
 *  markdown, or where its carried-over lead begins. */
function pageStart(doc: ParsedDoc, page: DocPage, pm: PageMap | null): number {
  return pm?.lead ? startAt(doc, pm.lead.base + pm.lead.al.runs[0].m) : page.start;
}

/** Where it ends: after all of its markdown (a trailing figure included), or
 *  where its carried-on trail ends. */
function pageEnd(doc: ParsedDoc, page: DocPage, pm: PageMap | null): number {
  return pm?.trail ? endAt(doc, pm.trail.base + runsEnd(pm.trail.al)) : page.end;
}

/** One page's part in a selection. */
export interface PageSelection {
  page: number;
  /** The page's text-layer text, or null when it is not rendered. */
  layer: string | null;
  /** Offsets into `layer`; null for the page's own start / end. */
  start: number | null;
  end: number | null;
}

/** A page copied as its text-layer text, among markdown pages: cleaned up
 *  (no NULs or controls, ligatures split), and `$` escaped so it cannot pair
 *  into maths. */
export function plainText(text: string): string {
  return text
    .replace(/[\x00-\x08\x0b\x0c\x0e-\x1f]/g, "")
    .replace(/[\ufb00-\ufb06]/g, (c) => c.normalize("NFKC"))
    .replace(/\$/g, "\\$")
    .trim();
}

/**
 * A selection over `sel` (its pages in order; only the first and last may be
 * partial) as markdown, or "" when no part of it could be read from `doc`.
 * The ends are carried to the document and everything between is one slice;
 * an end on a page that is unparsed or cannot be aligned copies that page's
 * selected text instead.
 */
export function markdownOfSelection(
  doc: ParsedDoc,
  sel: PageSelection[],
  resolveImage: (src: string) => string = (src) => src,
): string {
  const parts: string[] = [];
  let parsed = false;
  let cursor: number | null = null;
  const take = (end: number) => {
    if (cursor !== null && end > cursor) {
      const md = sliceMarkdown(doc.md, doc.sk, cursor, end, resolveImage);
      if (md) {
        parts.push(md);
        parsed = true;
      }
    }
    cursor = null;
  };
  const plain = (p: PageSelection) => {
    if (p.layer) parts.push(plainText(p.layer.slice(p.start ?? 0, p.end ?? p.layer.length)));
  };

  sel.forEach((p, i) => {
    const page = doc.pages.get(p.page);
    if (!page?.parsed) {
      take(page ? page.start : doc.md.length);
      plain(p);
      return;
    }
    const first = i === 0;
    const last = i === sel.length - 1;
    const t = p.layer === null ? null : textSkeleton(p.layer);
    const pm = (first || last) && t ? pageMap(doc, p.page, t) : null;
    // An end before the first char or after the last is the page's own edge.
    const tLen = t ? t.text.length : 0;
    const ts = t && p.start !== null ? charsBefore(t, p.start) : 0;
    const te = t && p.end !== null ? charsBefore(t, p.end) : tLen;

    if (first) {
      if (ts === 0) cursor = pageStart(doc, page, pm);
      else if (ts >= tLen) cursor = pageEnd(doc, page, pm);
      else if (pm) cursor = startAt(doc, toDoc(pm, ts));
      else {
        plain(p);
        return;
      }
    } else if (cursor === null) {
      cursor = page.start;
    }
    if (!last) return;
    const end =
      te >= tLen ? pageEnd(doc, page, pm) : te === 0 ? pageStart(doc, page, pm) : pm ? endAt(doc, toDoc(pm, te)) : null;
    // Unaligned, or carried before the start (text MinerU put elsewhere).
    if (end === null || (cursor !== null && end <= cursor)) {
      if (!first) take(page.start);
      cursor = null;
      plain(p);
      return;
    }
    take(end);
  });
  if (!parsed) return "";
  return normalizeMath(parts.filter(Boolean).join("\n\n"))
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}
