/**
 * A parsed PDF's blocks (`PdfBlock` in `.pages.json`) as the two faces of a
 * file use them: the Markdown view renders the record as one document whose
 * top-level elements carry `data-page` / `data-block`, and the PDF view turns
 * a block's `bbox` into a box in points. A `BlockRef` names a spot both faces
 * can show — a block, or a page when the record has no blocks.
 */
import type { PagesJson, PdfBlock } from "@/lib/citations";
import type { Rect } from "@/lib/pdf/pdfFind";
import { normalizeMath } from "@/lib/markdown/math";

export interface BlockRef {
  page: number;
  /** An index into the page's blocks; absent for text outside every block. */
  block?: number;
}

/** The record as one markdown document, and which page and block each part
 *  of it came from. */
export interface BlockDoc {
  text: string;
  /** Ascending offsets into `text` where each non-empty chunk starts. */
  starts: number[];
  /** The page and block each chunk came from, by index. */
  refs: BlockRef[];
}

/** The blocks of a page that are usable: a sane span inside the markdown
 *  and a box, in markdown order, keeping their record indices; a block
 *  overlapping the one before it is dropped. */
function usableBlocks(markdown: string, blocks: readonly PdfBlock[] | undefined) {
  const out: { index: number; start: number; end: number }[] = [];
  (blocks ?? []).forEach((b, index) => {
    if (!Number.isInteger(b?.start) || !Number.isInteger(b?.end)) return;
    if (b.start < 0 || b.end <= b.start || b.end > markdown.length) return;
    out.push({ index, start: b.start, end: b.end });
  });
  out.sort((a, b) => a.start - b.start);
  let pos = 0;
  return out.filter((b) => {
    if (b.start < pos) return false;
    pos = b.end;
    return true;
  });
}

/**
 * Pages joined by "\n\n" as the `.md` is, each cut at its blocks' spans so
 * text between or around blocks stays with its page. `normalizeMath` runs per
 * chunk, which moves offsets, so each chunk's start is taken after it.
 */
export function buildBlockDoc(record: PagesJson): BlockDoc {
  let text = "";
  const starts: number[] = [];
  const refs: BlockRef[] = [];
  const push = (chunk: string, ref: BlockRef) => {
    if (!chunk) return;
    starts.push(text.length);
    refs.push(ref);
    text += normalizeMath(chunk);
  };
  record.pages.forEach((pg, i) => {
    if (i > 0) text += "\n\n";
    const md = pg.markdown ?? "";
    const page = pg.page_no;
    const cuts = usableBlocks(md, pg.blocks);
    let pos = 0;
    for (const c of cuts) {
      push(md.slice(pos, c.start), { page });
      push(md.slice(c.start, c.end), { page, block: c.index });
      pos = c.end;
    }
    push(md.slice(pos), { page });
  });
  return { text, starts, refs };
}

/** The chunk holding `offset` of the document. */
export function refAt(doc: BlockDoc, offset: number): BlockRef | null {
  let lo = 0;
  let hi = doc.starts.length - 1;
  if (hi < 0 || offset < doc.starts[0]) return null;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (doc.starts[mid] <= offset) lo = mid;
    else hi = mid - 1;
  }
  return doc.refs[lo];
}

/** A block's box in points on a page `width` × `height` points. */
export function blockRect(block: PdfBlock, width: number, height: number): Rect | null {
  const b = block.bbox;
  if (!Array.isArray(b) || b.length !== 4 || !b.every(Number.isFinite)) return null;
  const clamp = (v: number) => Math.min(1, Math.max(0, v));
  const [x0, y0, x1, y1] = b.map(clamp);
  if (x1 <= x0 || y1 <= y0) return null;
  return { x: x0 * width, y: y0 * height, width: (x1 - x0) * width, height: (y1 - y0) * height };
}
