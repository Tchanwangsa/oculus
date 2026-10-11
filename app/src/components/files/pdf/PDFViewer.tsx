import { useStoredState } from "@/hooks/ui/useStoredState";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { FindBar } from "@/components/ui/search/FindBar";
import { centerIn, matchSpans } from "@/lib/citations/locateQuote";
import type { FileLocate } from "@/lib/files/openFile";
import { selectContents, useFindTarget } from "@/lib/menu/find";
import { useDataDir } from "@/hooks/backend/useDataDir";
import { blockRect, type BlockRef } from "@/lib/pdf/pdfBlocks";
import type { PdfMdLink } from "@/components/files/pdf/pdfMdLink";
import { libraryImageSrc } from "@/lib/files/libraryLinks";
import { copyPdfAsMarkdown, dragPdfAsMarkdown } from "@/lib/pdf/pdfSelectionMarkdown";
import type { Rect } from "@/lib/pdf/pdfFind";
import {
  PAD,
  anchorAt,
  clampZoom,
  fitScale,
  layoutPages,
  nearPages,
  shownPages,
  spreadStart,
  type Anchor,
  type Layout,
  type LayoutMode,
  type Shown,
} from "./layout";
import { PdfLoadState } from "./PdfLoadState";
import { PdfPage } from "./PdfPage";
import { PdfToolbar } from "./PdfToolbar";
import { watchSelection } from "./textLayer";
import { usePdfDocument } from "./usePdfDocument";
import { usePdfFind } from "./usePdfFind";
import { usePdfPageKeys } from "./usePdfPageKeys";
import { usePdfRecord } from "./usePdfRecord";
import { usePdfViewSize } from "./usePdfViewSize";
import { usePdfZoomInput } from "./usePdfZoomInput";

const MODE_KEY = "oculus-pdf-layout";

/** Pages are redrawn this long after the last zoom change; until then the
 *  old rasters stretch. */
const DRAW_DELAY = 400;

/** The class a cited passage's text-layer spans carry (`styles/pdf.css`). */
const HIT = "citation-hit";

/** Layout pixels between the view's top and a block brought to it; the
 *  reading line a position is read at sits just under it, so a restore
 *  reports the block it restored. */
const PLACE = 12;

/** The position is reported this long after the last scroll. */
const REPORT_MS = 150;

interface Props {
  /** Library-relative, as Rust's `pdf_*` commands take it. */
  path: string;
  /** A cited spot: go to its page and highlight its quote there. */
  locate?: FileLocate;
  /** The parse's `.md` (library-relative): a selection copies as its
   *  markdown (`lib/pdf/pdfSelectionMarkdown`). */
  markdownPath?: string;
  /** Shared with the Markdown face (`pdfMdLink.ts`): the position is
   *  reported to it and restored from it on mount. */
  link?: PdfMdLink;
}

/** Where a page jump lands: the page's top, or a spot on it brought into
 *  view — centred, or with `top` just under the view's top. */
type ScrollTarget = { page: number; rect?: Rect; top?: boolean };

/**
 * PDF viewer over Rust's `pdf_*` commands (`lib/pdf/pdfView.ts`): pages are laid
 * out by `layout.ts`, drawn as rasters near the view, with a selectable text
 * layer (`textLayer.ts`), find (`usePdfFind.ts`) and links. One document per
 * mount, so a new path starts clean.
 */
export function PDFViewer(props: Props) {
  return <Viewer key={props.path} {...props} />;
}

function Viewer({ path, locate, markdownPath, link }: Props) {
  const { sizes, loadError } = usePdfDocument(path);
  const [mode, setMode] = useStoredState<LayoutMode>(MODE_KEY, (stored) =>
    stored === "single" || stored === "spread" ? stored : "scroll",
  );
  /** The paged layouts' page (a spread's first). */
  const [current, setCurrent] = useState(1);
  /** Absolute (1 is printed size); null until the first fit. */
  const [scale, setScale] = useState<number | null>(null);
  /** The scale pages are drawn at, which trails `scale` by `DRAW_DELAY`. */
  const [renderScale, setRenderScale] = useState(1);
  const [shown, setShown] = useState<Shown>({ first: 1, last: 1 });
  const [near, setNear] = useState<Shown>({ first: 0, last: 0 });
  /** Bumped to apply `targetRef` after the next layout. */
  const [scrollTick, setScrollTick] = useState(0);
  /** The page box's text while it has focus; null shows the live range. */
  const [pageDraft, setPageDraft] = useState<string | null>(null);
  const findRef = useRef<HTMLInputElement>(null);

  /** The whole viewer, toolbar included: what ⌘F engagement is judged on. */
  const rootRef = useRef<HTMLDivElement>(null);
  /** The scroller. */
  const containerRef = useRef<HTMLDivElement>(null);
  /** The pages' parent: what Select All takes. */
  const viewerElRef = useRef<HTMLDivElement>(null);
  const { blocks, recordReady, pagesRef } = usePdfRecord(markdownPath);
  /** The blocks a citation names. */
  const [cited, setCited] = useState<{ page: number; blocks: number[] } | null>(null);
  const dataDir = useDataDir();

  // Mirrors for callbacks that must stay stable.
  const modeRef = useRef(mode);
  modeRef.current = mode;
  const currentRef = useRef(current);
  currentRef.current = current;
  const sizesRef = useRef(sizes);
  sizesRef.current = sizes;
  const scaleRef = useRef<number | null>(null);
  const shownRef = useRef(shown);
  const nearRef = useRef(near);
  /** In continuous scroll, the page filling most of the view. */
  const mainRef = useRef(1);
  /** The size follows the view until the reader zooms. */
  const fitRef = useRef(true);
  const anchorRef = useRef<Anchor | null>(null);
  const targetRef = useRef<ScrollTarget | null>(null);
  const settleRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const textListeners = useRef(new Set<(page: number) => void>());
  const blocksRef = useRef(blocks);
  blocksRef.current = blocks;
  /** Set once the shared position is restored (or there was none); no
   *  position is reported before, so the opening scroll can't overwrite it. */
  const placedRef = useRef(false);
  const reportRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  /** The citation this mount jumped to. */
  const jumpSeqRef = useRef<number | null>(null);

  const { view, dpr } = usePdfViewSize(containerRef);

  const layout = useMemo<Layout | null>(
    () => (sizes && scale != null && view.width ? layoutPages(sizes, mode, scale, current, view.width) : null),
    [sizes, mode, scale, current, view.width],
  );
  const layoutRef = useRef(layout);
  layoutRef.current = layout;

  useEffect(() => watchSelection(containerRef.current!), []);

  useEffect(
    () => () => {
      if (settleRef.current != null) clearTimeout(settleRef.current);
    },
    [],
  );

  /** Re-reads the page range, the pages worth drawing and the main page from
   *  the scroll position. */
  const sync = useCallback(() => {
    const el = containerRef.current;
    const lay = layoutRef.current;
    if (!el || !lay?.boxes.length) return;
    let nextShown: Shown;
    let nextNear: Shown;
    if (modeRef.current === "scroll") {
      const r = shownPages(lay, el.scrollTop, el.clientHeight);
      if (r) mainRef.current = r.main;
      nextShown = r?.shown ?? shownRef.current;
      nextNear = nearPages(lay, el.scrollTop, el.clientHeight);
    } else {
      nextShown = { first: lay.boxes[0].page, last: lay.boxes[lay.boxes.length - 1].page };
      nextNear = nextShown;
      mainRef.current = nextShown.first;
    }
    const same = (a: Shown, b: Shown) => a.first === b.first && a.last === b.last;
    if (!same(nextShown, shownRef.current)) setShown((shownRef.current = nextShown));
    if (!same(nextNear, nearRef.current)) setNear((nearRef.current = nextNear));
  }, []);

  /** Shows `page` — its top, or `rect` on it: centred if it is out of view,
   *  or with `top` just under the view's top. */
  const goToPage = useCallback((page: number, rect?: Rect, top?: boolean) => {
    const count = sizesRef.current?.length ?? 0;
    if (!count) return;
    const p = Math.min(Math.max(1, page), count);
    const m = modeRef.current;
    if (m !== "scroll") setCurrent(m === "spread" ? spreadStart(p) : p);
    targetRef.current = { page: p, rect, top };
    setScrollTick((n) => n + 1);
  }, []);

  /** A block's box in points, if the record has it. */
  const rectOf = useCallback((page: number, block: number) => {
    const b = blocksRef.current?.get(page)?.[block];
    const size = sizesRef.current?.[page - 1];
    return b && size ? blockRect(b, size.width, size.height) : null;
  }, []);

  /** Shows a block near the view's top, or a page's top. */
  const showRef = useCallback(
    (ref: BlockRef) => {
      const rect = ref.block != null ? rectOf(ref.page, ref.block) : null;
      if (rect) goToPage(ref.page, rect, true);
      else goToPage(ref.page);
    },
    [goToPage, rectOf],
  );

  /** The block at the reading line, by layout arithmetic: on the first page
   *  in view that has one there, the highest block reaching past the line.
   *  Without blocks, the page being read. */
  const readPosition = useCallback((): BlockRef | null => {
    const el = containerRef.current;
    const lay = layoutRef.current;
    if (!el || !lay?.boxes.length) return null;
    const top = el.scrollTop;
    const bottom = top + el.clientHeight;
    const line = top + PLACE + 1;
    const all = blocksRef.current;
    if (all) {
      for (const box of lay.boxes) {
        if (box.top >= bottom) break;
        if (box.top + box.height <= line) continue;
        let best: number | null = null;
        let bestTop = Infinity;
        (all.get(box.page) ?? []).forEach((b, i) => {
          const r = blockRect(b, 1, 1);
          if (!r) return;
          const y0 = box.top + r.y * box.height;
          const y1 = y0 + r.height * box.height;
          if (y1 > line && y0 < bottom && y0 < bestTop) {
            bestTop = y0;
            best = i;
          }
        });
        if (best != null) return { page: box.page, block: best };
      }
    }
    return { page: modeRef.current === "scroll" ? mainRef.current : currentRef.current };
  }, []);

  /** Sets the scale, keeping the spot under view point (`vx`, `vy`) — the
   *  view's centre by default — where it is. */
  const zoomTo = useCallback((target: number, vx?: number, vy?: number) => {
    const next = clampZoom(target);
    const prev = scaleRef.current;
    if (prev == null || next === prev) return;
    const el = containerRef.current;
    const lay = layoutRef.current;
    // Several zooms before a render keep the first one's anchor.
    if (!anchorRef.current && el && lay) {
      const ax = vx ?? el.clientWidth / 2;
      const ay = vy ?? el.clientHeight / 2;
      anchorRef.current = anchorAt(lay, el.scrollLeft + ax, el.scrollTop + ay, ax, ay);
    }
    scaleRef.current = next;
    setScale(next);
    if (settleRef.current != null) clearTimeout(settleRef.current);
    settleRef.current = setTimeout(() => {
      settleRef.current = null;
      setRenderScale(scaleRef.current ?? next);
    }, DRAW_DELAY);
  }, []);

  /** A zoom the reader asked for, about a pointer in client coordinates. The
   *  pointer and the scroller's rect are in visual pixels, which page zoom
   *  scales; `offsetWidth` over the rect's width turns them into layout ones. */
  const zoomBy = useCallback(
    (factor: number, clientX?: number, clientY?: number) => {
      const el = containerRef.current;
      if (!el || scaleRef.current == null) return;
      fitRef.current = false;
      let vx: number | undefined;
      let vy: number | undefined;
      if (clientX != null && clientY != null) {
        const rect = el.getBoundingClientRect();
        const k = rect.width ? el.offsetWidth / rect.width : 1;
        vx = (clientX - rect.left) * k - el.clientLeft;
        vy = (clientY - rect.top) * k - el.clientTop;
      }
      zoomTo(scaleRef.current * factor, vx, vy);
    },
    [zoomTo],
  );

  /** The fit, about the top of the view so the page being read stays. */
  const fit = useCallback(() => {
    const s = sizesRef.current;
    const el = containerRef.current;
    if (!s || !el || !el.clientWidth) return;
    const page = modeRef.current === "scroll" ? mainRef.current : currentRef.current;
    const next = fitScale(s, modeRef.current, page, el.clientWidth, el.clientHeight);
    if (scaleRef.current == null) {
      scaleRef.current = next;
      setScale(next);
      setRenderScale(next);
    } else {
      zoomTo(next, el.clientWidth / 2, 0);
    }
  }, [zoomTo]);

  const resetZoom = useCallback(() => {
    fitRef.current = true;
    fit();
  }, [fit]);

  // The fit follows the view's size and the layout until the reader zooms.
  useLayoutEffect(() => {
    if (sizes && view.width && fitRef.current) fit();
  }, [sizes, view.width, view.height, mode, fit]);

  // After a layout: put a zoom's anchor back under the pointer, apply a page
  // jump, then re-read the range.
  useLayoutEffect(() => {
    const el = containerRef.current;
    if (!el || !layout) return;
    const anchor = anchorRef.current;
    anchorRef.current = null;
    const boxOf = (page: number) => layout.boxes.find((b) => b.page === page);
    if (anchor) {
      const box = boxOf(anchor.page);
      if (box) {
        el.scrollLeft = box.left + anchor.fx * box.width - anchor.vx;
        el.scrollTop = box.top + anchor.fy * box.height - anchor.vy;
      }
    }
    const target = targetRef.current;
    const box = target && boxOf(target.page);
    if (target && box) {
      targetRef.current = null;
      const size = sizes![box.page - 1];
      const k = box.width / size.width;
      if (!target.rect) {
        el.scrollTop = box.top - PAD;
      } else {
        const r = target.rect;
        const y = box.top + r.y * k;
        const h = r.height * k;
        if (target.top) el.scrollTop = y - PLACE;
        else if (y < el.scrollTop || y + h > el.scrollTop + el.clientHeight)
          el.scrollTop = y - (el.clientHeight - h) / 2;
        const x = box.left + r.x * k;
        const w = r.width * k;
        if (x < el.scrollLeft || x + w > el.scrollLeft + el.clientWidth)
          el.scrollLeft = x - (el.clientWidth - w) / 2;
      }
    }
    sync();
  }, [layout, scrollTick, sizes, sync]);

  const changeMode = (next: LayoutMode) => {
    if (next === mode) return;
    const page = shownRef.current.first;
    modeRef.current = next;
    setMode(next);
    setCurrent(next === "spread" ? spreadStart(page) : page);
    targetRef.current = { page };
    setScrollTick((n) => n + 1);
  };

  const ready = !!layout;

  // Once laid out and the record read, put back the position the other face
  // left — unless a citation not yet shown is about to jump. From then on,
  // report the block at the top after each scroll settles.
  useEffect(() => {
    if (!ready || !recordReady || placedRef.current) return;
    placedRef.current = true;
    const anchor = link?.anchor;
    if (!anchor || (locate?.page && link.locateSeq !== locate.seq)) return;
    showRef(anchor);
    // Once, on the first layout with the record.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ready, recordReady]);

  const report = useCallback(() => {
    reportRef.current = null;
    const ref = readPosition();
    if (ref && link) link.anchor = ref;
  }, [link, readPosition]);

  const reportSoon = () => {
    if (!link || !placedRef.current) return;
    if (reportRef.current != null) clearTimeout(reportRef.current);
    reportRef.current = setTimeout(report, REPORT_MS);
  };

  // A pending report lands on unmount, before the scroller leaves the
  // document, so a toggle right after a scroll keeps the position.
  useLayoutEffect(
    () => () => {
      if (reportRef.current == null) return;
      clearTimeout(reportRef.current);
      report();
    },
    [report],
  );

  /** The cited blocks' boxes, on the cited page only; other pages get none,
   *  so their memoised `PdfPage`s stay put. */
  const citedBoxes = useMemo(() => {
    if (!cited) return null;
    const rects = cited.blocks.map((b) => rectOf(cited.page, b)).filter((r): r is Rect => !!r);
    return rects.length ? { page: cited.page, rects } : null;
    // `rectOf` reads `blocks` and `sizes` through refs; they are listed so a
    // record or document arriving redraws the boxes.
  }, [cited, blocks, sizes, rectOf]);

  // Go to the cited page and mark the cited blocks' boxes when the record has
  // them; otherwise mark the quote's spans once that page's text layer exists
  // — after the jump, or when it is built, which also happens when a page
  // scrolled away comes back, so the marks return while this locate is
  // current. Scrolls only the first time a citation is shown, so a remount
  // (back from the Markdown face) keeps the reader's place.
  const onTextLayer = useCallback((page: number) => {
    for (const listener of textListeners.current) listener(page);
  }, []);

  useEffect(() => {
    const container = containerRef.current;
    const count = sizesRef.current?.length ?? 0;
    if (!ready || !recordReady || !container || !count || !locate?.page) return;
    // A citation is this mount's to jump to if no face showed it before; the
    // ref keeps that answer through StrictMode's second run.
    if (link?.locateSeq !== locate.seq) jumpSeqRef.current = locate.seq;
    if (link) link.locateSeq = locate.seq;
    const jump = jumpSeqRef.current === locate.seq;
    for (const el of container.querySelectorAll(`.${HIT}`)) el.classList.remove(HIT);
    const pageNumber = Math.min(Math.max(1, locate.page), count);
    const boxed = (locate.blocks ?? []).filter((b) => rectOf(pageNumber, b));
    if (boxed.length) {
      setCited({ page: pageNumber, blocks: boxed });
      if (jump) goToPage(pageNumber, rectOf(pageNumber, boxed[0])!);
      return () => setCited(null);
    }
    setCited(null);
    if (jump) goToPage(pageNumber);
    const quote = locate.quote;
    if (!quote) return;
    let scrolled = !jump;
    const mark = () => {
      const layer = container.querySelector(`.page[data-page-number="${pageNumber}"] .textLayer`);
      if (!layer) return;
      const spans = Array.from(layer.querySelectorAll<HTMLElement>(":scope > span"));
      const hits = matchSpans(spans, quote);
      for (const el of hits) el.classList.add(HIT);
      if (hits.length && !scrolled) {
        scrolled = true;
        centerIn(container, hits[0]);
      }
    };
    // After the jump lands, so it doesn't undo the centring.
    const frame = requestAnimationFrame(mark);
    const onBuilt = (page: number) => {
      if (page === pageNumber) mark();
    };
    textListeners.current.add(onBuilt);
    return () => {
      cancelAnimationFrame(frame);
      textListeners.current.delete(onBuilt);
    };
    // `locate` is read through its `seq`: a new object with the same seq is
    // the same citation (a refreshed row).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [locate?.seq, ready, recordReady]);

  /** A figure's link as the Markdown view's copy writes it, so both faces of
   *  a file copy the same text. */
  const resolveImage = (src: string) =>
    markdownPath ? libraryImageSrc(src, markdownPath, dataDir) : src;

  const numPages = sizes?.length ?? 0;
  const step = mode === "spread" ? 2 : 1;
  const prev = useCallback(() => {
    const from = mode === "scroll" ? shownRef.current.first : current;
    if (from - step >= 1) goToPage(mode === "spread" ? spreadStart(from) - 2 : from - 1);
  }, [mode, current, step, goToPage]);
  const next = useCallback(() => {
    const from = mode === "scroll" ? shownRef.current.first : current;
    const to = mode === "spread" ? spreadStart(from) + 2 : from + 1;
    if (to <= numPages) goToPage(to);
  }, [mode, current, numPages, goToPage]);

  /** The page box's Enter: the page's top, or in a spread the spread holding
   *  it. Anything but a whole number just reverts. */
  const jumpTo = (text: string) => {
    const trimmed = text.trim();
    if (numPages && /^\d+$/.test(trimmed)) goToPage(Number(trimmed));
  };

  usePdfPageKeys(mode, prev, next);

  const find = usePdfFind(
    path,
    numPages,
    () => (modeRef.current === "scroll" ? shownRef.current.first : currentRef.current),
    goToPage,
  );

  const closeFind = () => {
    find.setOpen(false);
    find.setQuery("");
  };

  /** Seeds the query from a text selection in the pages, then selects the
   *  field, so a repeat ⌘F replaces what is there. */
  const openFind = () => {
    const container = containerRef.current;
    const selection = window.getSelection();
    const picked =
      container &&
      selection &&
      !selection.isCollapsed &&
      selection.rangeCount &&
      container.contains(selection.getRangeAt(0).commonAncestorContainer)
        ? selection.toString().replace(/\s+/g, " ").trim()
        : "";
    if (picked && picked !== find.query) find.setQuery(picked);
    find.setOpen(true);
    // After mount when the bar was closed.
    requestAnimationFrame(() => findRef.current?.select());
  };

  const stepFind = (backwards: boolean) => {
    if (!find.open) openFind();
    else if (find.query) find.step(backwards);
  };

  // A PDF can be open in several tabs and side panels at once;
  // `lib/menu/find.ts` picks the one ⌘F reaches. Select All takes the pages, not
  // the toolbar.
  useFindTarget(rootRef, {
    open: openFind,
    step: stepFind,
    selectAll: () => {
      if (viewerElRef.current) selectContents(viewerElRef.current);
    },
  });

  usePdfZoomInput(containerRef, zoomBy);

  // Idle, the box names every page on screen; focused, it holds just the first
  // page to edit. The total is fixed text beside it.
  const pageLabel =
    shown.first === shown.last ? String(shown.first) : `${shown.first}–${shown.last}`;
  const pageText = pageDraft ?? pageLabel;

  return (
    <div ref={rootRef} className="flex flex-col h-full min-h-0">
      <PdfToolbar
        mode={mode}
        onMode={changeMode}
        numPages={numPages}
        shown={shown}
        pageText={pageText}
        pageLabel={pageLabel}
        onPageDraft={setPageDraft}
        onJump={jumpTo}
        onPrev={prev}
        onNext={next}
        scale={scale ?? 1}
        onZoom={zoomBy}
        onResetZoom={resetZoom}
      />

      {find.open && (
        <FindBar
          inputRef={findRef}
          query={find.query}
          onQueryChange={find.setQuery}
          onStep={(backwards) => find.query && find.step(backwards)}
          onClose={closeFind}
          status={find.status}
          placeholder="Find in PDF"
          className="shrink-0 border-b border-border-subtle bg-surface"
        />
      )}

      <div className="relative flex-1 min-h-0">
        <div
          ref={containerRef}
          onScroll={() => {
            sync();
            reportSoon();
          }}
          // Capture, so the markdown is on the clipboard before anything
          // inside the pages could write its own.
          onCopyCapture={(e) => pagesRef.current && copyPdfAsMarkdown(e, e.currentTarget, pagesRef.current, resolveImage)}
          onDragStart={(e) => pagesRef.current && dragPdfAsMarkdown(e, e.currentTarget, pagesRef.current, resolveImage)}
          className="pdf-surface absolute inset-0"
        >
          <div
            ref={viewerElRef}
            data-selectable
            className="pdf-pages"
            style={layout ? { width: layout.width, height: layout.height } : undefined}
          >
            {layout &&
              sizes &&
              layout.boxes.map((box) => (
                <PdfPage
                  key={box.page}
                  path={path}
                  box={box}
                  size={sizes[box.page - 1]}
                  renderScale={renderScale}
                  dpr={dpr}
                  near={box.page >= near.first && box.page <= near.last}
                  highlights={find.highlights.get(box.page)}
                  blockBoxes={citedBoxes?.page === box.page ? citedBoxes.rects : undefined}
                  onTextLayer={onTextLayer}
                  onGoToPage={goToPage}
                />
              ))}
          </div>
        </div>

        <PdfLoadState error={loadError} loading={!layout} />
      </div>
    </div>
  );
}
