import { invoke } from "@tauri-apps/api/core";

/**
 * The PDF viewer's side of Rust's `pdf_*` commands (`docs/viewers.md`): a
 * document is opened by its library-relative path, pages are rasterised to
 * RGBA at the device size the viewer asks for, and text and links come back
 * as geometry in PDF points of the rendered page (origin top-left, y down).
 * Documents are reference-counted so two viewers of one file share it, and
 * each page's text and links are fetched once per open document.
 */

export interface PdfPageSize {
  width: number;
  height: number;
}

/** A line of text in reading order. `chars` holds `text.length + 1` stops in
 *  points — x for a horizontal line, y for a vertical one — so char i spans
 *  `chars[i]..chars[i + 1]`. */
export interface PdfLine {
  text: string;
  x: number;
  y: number;
  width: number;
  height: number;
  vertical: boolean;
  chars: number[];
}

/** A link's box; `page` (1-based) is an internal destination. */
export interface PdfLink {
  x: number;
  y: number;
  width: number;
  height: number;
  uri?: string;
  page?: number;
}

interface Doc {
  refs: number;
  pages: Promise<PdfPageSize[]>;
  text: Map<number, Promise<PdfLine[]>>;
  links: Map<number, Promise<PdfLink[]>>;
  /** A pending `pdf_close`, cancelled if the document is opened again first
   *  (a remount, a tab moving panes). */
  closing: ReturnType<typeof setTimeout> | null;
}

const docs = new Map<string, Doc>();

/** How long a released document stays open in case it is reopened. */
const CLOSE_DELAY = 2000;

/** Opens `path` (or joins the viewer already holding it). Call `release`
 *  once when done. Rejects with Rust's error string: "not-found",
 *  "encrypted", "invalid", "outside-library". */
export function openPdf(path: string): { pages: Promise<PdfPageSize[]>; release: () => void } {
  let doc = docs.get(path);
  if (doc?.closing != null) {
    clearTimeout(doc.closing);
    doc.closing = null;
  }
  if (!doc) {
    doc = {
      refs: 0,
      pages: invoke<{ pages: PdfPageSize[] }>("pdf_open", { path }).then((r) => r.pages),
      text: new Map(),
      links: new Map(),
      closing: null,
    };
    docs.set(path, doc);
  }
  doc.refs += 1;
  const held = doc;
  let released = false;
  return {
    pages: held.pages,
    release: () => {
      if (released) return;
      released = true;
      held.refs -= 1;
      if (held.refs > 0) return;
      held.closing = setTimeout(() => {
        if (docs.get(path) !== held || held.refs > 0) return;
        docs.delete(path);
        invoke("pdf_close", { path }).catch(() => {});
      }, CLOSE_DELAY);
    },
  };
}

function cached<T>(path: string, pick: (d: Doc) => Map<number, Promise<T>>, page: number, load: () => Promise<T>): Promise<T> {
  const doc = docs.get(path);
  if (!doc) return load();
  const map = pick(doc);
  let hit = map.get(page);
  if (!hit) {
    hit = load();
    map.set(page, hit);
    // A failure is not remembered: the next ask tries again.
    hit.catch(() => map.delete(page));
  }
  return hit;
}

export function pageText(path: string, page: number): Promise<PdfLine[]> {
  return cached(path, (d) => d.text, page, () =>
    invoke<{ lines: PdfLine[] }>("pdf_text", { path, page }).then((r) => r.lines),
  );
}

export function pageLinks(path: string, page: number): Promise<PdfLink[]> {
  return cached(path, (d) => d.links, page, () => invoke<PdfLink[]>("pdf_links", { path, page }));
}

// ── Rendering ────────────────────────────────────────────────────────────────

/** Rust refuses a raster past these (`pdf_render`'s "too-large"). */
const MAX_SIDE = 8192;
const MAX_PIXELS = 40_000_000;

/** A raster size for a page of `width`×`height` device pixels, scaled down to
 *  the limits above. */
export function rasterSize(width: number, height: number): { width: number; height: number } {
  const w = Math.max(1, width);
  const h = Math.max(1, height);
  const k = Math.min(1, MAX_SIDE / w, MAX_SIDE / h, Math.sqrt(MAX_PIXELS / (w * h)));
  return { width: Math.max(1, Math.floor(w * k)), height: Math.max(1, Math.floor(h * k)) };
}

interface Job {
  run: () => Promise<void>;
  live: () => boolean;
}

/** Renders in flight at once; the rest wait, newest first, so the pages a
 *  fast scroll lands on beat the ones it passed. */
const RENDER_SLOTS = 2;
const queue: Job[] = [];
let running = 0;

function pump() {
  while (running < RENDER_SLOTS && queue.length) {
    const job = queue.pop()!;
    if (!job.live()) continue;
    running += 1;
    job.run().finally(() => {
      running -= 1;
      pump();
    });
  }
}

/** One page as pixels, or null when `live()` turned false before its turn. */
export function renderPage(
  path: string,
  page: number,
  width: number,
  height: number,
  live: () => boolean,
): Promise<ImageData | null> {
  return new Promise((resolve, reject) => {
    queue.push({
      live: () => {
        const on = live();
        if (!on) resolve(null);
        return on;
      },
      run: () =>
        invoke<ArrayBuffer>("pdf_render", { path, page, width, height }).then(
          (buf) => resolve(live() ? new ImageData(new Uint8ClampedArray(buf), width, height) : null),
          reject,
        ),
    });
    pump();
  });
}
