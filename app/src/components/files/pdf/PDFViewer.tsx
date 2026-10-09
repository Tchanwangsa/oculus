import { useStoredState } from "@/hooks/useStoredState";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  BookOpen,
  CaretLeft,
  CaretRight,
  CircleNotch,
  File as FileIcon,
  MagnifyingGlassMinus,
  MagnifyingGlassPlus,
  Rows,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { FindBar } from "@/components/ui/FindBar";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { centerIn, matchSpans } from "@/lib/locateQuote";
import type { FileLocate } from "@/lib/openFile";
import { selectContents, useFindTarget } from "@/lib/find";
import { useDataDir } from "@/hooks/useDataDir";
import { loadParsedPages } from "@/lib/citations";
import { libraryImageSrc } from "@/lib/libraryLinks";
import { copyPdfAsMarkdown, dragPdfAsMarkdown } from "@/lib/pdfSelectionMarkdown";
import { openPdf, type PdfPageSize } from "@/lib/pdfView";
import type { Rect } from "@/lib/pdfFind";
import {
  MAX_ZOOM,
  MIN_ZOOM,
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
import { PdfPage } from "./PdfPage";
import { watchSelection } from "./textLayer";
import { usePdfFind } from "./usePdfFind";

const MODE_KEY = "oculus-pdf-layout";

/** One toolbar press, as a ratio. */
const ZOOM_STEP = 1.1;

/** Pages are redrawn this long after the last zoom change; until then the
 *  old rasters stretch. */
const DRAW_DELAY = 400;

/** Pixels per line when `deltaMode === DOM_DELTA_LINE`. */
const LINE_HEIGHT = 16;

/** WebKit sometimes drops `gestureend`, which would latch `gestureActive` and
 *  kill ⌘-scroll zoom; the flag releases itself after this much quiet. */
const GESTURE_LAPSE = 400;

/** The class a cited passage's text-layer spans carry (`index.css`). */
const HIT = "citation-hit";

interface Props {
  /** Library-relative, as Rust's `pdf_*` commands take it. */
  path: string;
  /** A cited spot: go to its page and highlight its quote there. */
  locate?: FileLocate;
  /** The parse's `.md` (library-relative): a selection copies as its
   *  markdown (`pdfSelectionMarkdown.ts`). */
  markdownPath?: string;
}

/** Where a page jump lands: the page's top, or a spot on it brought into view. */
type ScrollTarget = { page: number; rect?: Rect };

const ERRORS: Record<string, string> = {
  encrypted: "This PDF is password-protected, so it can't be shown here.",
};

/**
 * PDF viewer over Rust's `pdf_*` commands (`lib/pdfView.ts`): pages are laid
 * out by `layout.ts`, drawn as rasters near the view, with a selectable text
 * layer (`textLayer.ts`), find (`usePdfFind.ts`) and links. One document per
 * mount, so a new path starts clean.
 */
export function PDFViewer(props: Props) {
  return <Viewer key={props.path} {...props} />;
}

function Viewer({ path, locate, markdownPath }: Props) {
  const [sizes, setSizes] = useState<PdfPageSize[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [mode, setMode] = useStoredState<LayoutMode>(MODE_KEY, (stored) =>
    stored === "single" || stored === "spread" ? stored : "scroll",
  );
  /** The paged layouts' page (a spread's first). */
  const [current, setCurrent] = useState(1);
  /** Absolute (1 is printed size); null until the first fit. */
  const [scale, setScale] = useState<number | null>(null);
  /** The scale pages are drawn at, which trails `scale` by `DRAW_DELAY`. */
  const [renderScale, setRenderScale] = useState(1);
  const [view, setView] = useState({ width: 0, height: 0 });
  const [dpr, setDpr] = useState(() => window.devicePixelRatio || 1);
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
  /** The parsed pages, once read; the copy handlers need them synchronously. */
  const pagesRef = useRef<ReadonlyMap<number, string> | null>(null);
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

  const layout = useMemo<Layout | null>(
    () => (sizes && scale != null && view.width ? layoutPages(sizes, mode, scale, current, view.width) : null),
    [sizes, mode, scale, current, view.width],
  );
  const layoutRef = useRef(layout);
  layoutRef.current = layout;

  // ── The document ─────────────────────────────────────────────────────────

  useEffect(() => {
    const doc = openPdf(path);
    let live = true;
    doc.pages.then(
      (pages) => {
        if (!live) return;
        if (pages.length) setSizes(pages);
        else setLoadError("invalid");
      },
      (err) => live && setLoadError(String(err)),
    );
    return () => {
      live = false;
      doc.release();
    };
  }, [path]);

  useLayoutEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const measure = () => {
      setView((v) =>
        v.width === el.clientWidth && v.height === el.clientHeight
          ? v
          : { width: el.clientWidth, height: el.clientHeight },
      );
      // A page-zoom change moves the ratio and resizes the view together.
      setDpr(window.devicePixelRatio || 1);
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => watchSelection(containerRef.current!), []);

  useEffect(
    () => () => {
      if (settleRef.current != null) clearTimeout(settleRef.current);
    },
    [],
  );

  // ── Position ─────────────────────────────────────────────────────────────

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

  /** Shows `page` — its top, or `rect` on it centred if it is out of view. */
  const goToPage = useCallback((page: number, rect?: Rect) => {
    const count = sizesRef.current?.length ?? 0;
    if (!count) return;
    const p = Math.min(Math.max(1, page), count);
    const m = modeRef.current;
    if (m !== "scroll") setCurrent(m === "spread" ? spreadStart(p) : p);
    targetRef.current = { page: p, rect };
    setScrollTick((n) => n + 1);
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
        if (y < el.scrollTop || y + h > el.scrollTop + el.clientHeight)
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

  // ── Citation locate ──────────────────────────────────────────────────────
  //
  // Go to the cited page, then mark the quote's spans once that page's text
  // layer exists — after the jump, or when it is built, which also happens
  // when a page scrolled away comes back, so the marks return while this
  // locate is current. Scrolls to the passage only the first time.

  const ready = !!layout;
  const onTextLayer = useCallback((page: number) => {
    for (const listener of textListeners.current) listener(page);
  }, []);

  useEffect(() => {
    const container = containerRef.current;
    const count = sizesRef.current?.length ?? 0;
    if (!ready || !container || !count || !locate?.page) return;
    for (const el of container.querySelectorAll(`.${HIT}`)) el.classList.remove(HIT);
    const pageNumber = Math.min(Math.max(1, locate.page), count);
    goToPage(pageNumber);
    const quote = locate.quote;
    if (!quote) return;
    let scrolled = false;
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
  }, [locate?.seq, ready]);

  // ── Copy as markdown ─────────────────────────────────────────────────────

  useEffect(() => {
    pagesRef.current = null;
    if (!markdownPath) return;
    let live = true;
    loadParsedPages(markdownPath).then((pages) => {
      if (live) pagesRef.current = pages;
    });
    return () => {
      live = false;
    };
  }, [markdownPath]);

  /** A figure's link as the Markdown view's copy writes it, so both faces of
   *  a file copy the same text. */
  const resolveImage = (src: string) =>
    markdownPath ? libraryImageSrc(src, markdownPath, dataDir) : src;

  // ── Paging ───────────────────────────────────────────────────────────────

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

  // Arrow keys page in paged layouts; in scroll layout the list scrolls
  // natively, so the keys are left alone.
  useEffect(() => {
    if (mode === "scroll") return;
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (e.key === "ArrowLeft" || e.key === "ArrowUp") {
        e.preventDefault();
        prev();
      } else if (e.key === "ArrowRight" || e.key === "ArrowDown") {
        e.preventDefault();
        next();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [mode, prev, next]);

  // ── Find ─────────────────────────────────────────────────────────────────

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
  // `lib/find.ts` picks the one ⌘F reaches. Select All takes the pages, not
  // the toolbar.
  useFindTarget(rootRef, {
    open: openFind,
    step: stepFind,
    selectAll: () => {
      if (viewerElRef.current) selectContents(viewerElRef.current);
    },
  });

  // ── Zoom input ───────────────────────────────────────────────────────────

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    // WebKit reports one pinch twice — as `gesture*` events and as ctrlKey
    // wheel events — so the wheel path stands down while a gesture is live.
    const gestureActive = { current: false };
    let pinchBase = 1;
    let lapse: ReturnType<typeof setTimeout> | null = null;
    const armRelease = () => {
      if (lapse != null) clearTimeout(lapse);
      lapse = setTimeout(() => {
        lapse = null;
        gestureActive.current = false;
      }, GESTURE_LAPSE);
    };
    const disarm = () => {
      if (lapse != null) clearTimeout(lapse);
      lapse = null;
    };

    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey && !e.metaKey) return;
      e.preventDefault();
      if (gestureActive.current) return;
      // Exponential and clamped: a linear factor goes negative on one ±120
      // ⌘+wheel notch.
      const raw = e.deltaY * (e.deltaMode === 1 ? LINE_HEIGHT : 1);
      const d = Math.max(-50, Math.min(50, raw));
      zoomBy(Math.exp(-d * 0.01), e.clientX, e.clientY);
    };

    // Gesture `scale` is cumulative from gesturestart, so zoom by the ratio
    // to the previous frame.
    const onGestureStart = (e: Event) => {
      e.preventDefault();
      gestureActive.current = true;
      pinchBase = 1;
      armRelease();
    };
    const onGestureChange = (e: Event) => {
      e.preventDefault();
      armRelease();
      const g = e as unknown as {
        scale: number;
        clientX?: number;
        clientY?: number;
      };
      if (!g.scale) return;
      const factor = g.scale / pinchBase;
      pinchBase = g.scale;
      zoomBy(factor, g.clientX, g.clientY);
    };
    const onGestureEnd = (e: Event) => {
      e.preventDefault();
      disarm();
      gestureActive.current = false;
    };

    el.addEventListener("wheel", onWheel, { passive: false });
    el.addEventListener("gesturestart", onGestureStart);
    el.addEventListener("gesturechange", onGestureChange);
    el.addEventListener("gestureend", onGestureEnd);
    return () => {
      el.removeEventListener("wheel", onWheel);
      el.removeEventListener("gesturestart", onGestureStart);
      el.removeEventListener("gesturechange", onGestureChange);
      el.removeEventListener("gestureend", onGestureEnd);
      disarm();
    };
  }, [zoomBy]);

  // ── Chrome ───────────────────────────────────────────────────────────────

  // Idle, the box names every page on screen; focused, it holds just the first
  // page to edit. The total is fixed text beside it.
  const pageLabel =
    shown.first === shown.last ? String(shown.first) : `${shown.first}–${shown.last}`;
  const pageText = pageDraft ?? pageLabel;
  const paged = mode !== "scroll" && numPages > 1;
  const shownScale = scale ?? 1;

  return (
    <div ref={rootRef} className="flex flex-col h-full min-h-0">
      {/* One slim control row: layout · page · zoom */}
      <div className="shrink-0 flex items-center gap-3 px-3 h-9 border-b border-border-subtle bg-surface">
        <ToggleGroup
          type="single"
          value={mode}
          onValueChange={(v) => v && changeMode(v as LayoutMode)}
          variant="outline"
          size="sm"
          className="shrink-0"
        >
          <ModeItem value="scroll" label="Continuous scroll">
            <Rows size={12} />
          </ModeItem>
          <ModeItem value="single" label="Single page">
            <FileIcon size={12} />
          </ModeItem>
          <ModeItem value="spread" label="Two-page spread">
            <BookOpen size={12} />
          </ModeItem>
        </ToggleGroup>

        {numPages > 0 && (
          <div className="flex items-center gap-1 text-xs text-muted-foreground">
            {paged && (
              <Button
                variant="ghost"
                size="icon-xs"
                disabled={shown.first <= 1}
                onClick={prev}
                aria-label="Previous page"
              >
                <CaretLeft size={13} />
              </Button>
            )}
            <label className="flex h-6 cursor-text items-center gap-1 rounded-md border border-input px-2 text-xs tabular-nums text-muted-foreground shadow-none transition-[color,box-shadow] focus-within:border-ring focus-within:text-foreground focus-within:ring-[3px] focus-within:ring-ring/25 dark:bg-input/30">
              <input
                value={pageText}
                aria-label="Page"
                inputMode="numeric"
                spellCheck={false}
                autoComplete="off"
                // Focus by hand so the click doesn't drop a caret into the
                // selection `onFocus` makes.
                onMouseDown={(e) => {
                  if (document.activeElement === e.currentTarget) return;
                  e.preventDefault();
                  e.currentTarget.focus();
                }}
                onFocus={(e) => {
                  setPageDraft(String(shown.first));
                  e.currentTarget.select();
                }}
                onChange={(e) => setPageDraft(e.target.value.replace(/\D/g, ""))}
                onBlur={() => setPageDraft(null)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    jumpTo(e.currentTarget.value);
                    e.currentTarget.blur();
                  } else if (e.key === "Escape") {
                    e.stopPropagation();
                    e.currentTarget.blur();
                  }
                }}
                // Sized to the idle label so focusing doesn't resize it.
                style={{
                  width: `${Math.max(pageText.length, pageLabel.length)}ch`,
                }}
                className="bg-transparent p-0 text-center outline-none"
              />
              <span aria-hidden>/ {numPages}</span>
            </label>
            {paged && (
              <Button
                variant="ghost"
                size="icon-xs"
                disabled={shown.last >= numPages}
                onClick={next}
                aria-label="Next page"
              >
                <CaretRight size={13} />
              </Button>
            )}
          </div>
        )}

        <div className="ml-auto flex items-center gap-1 text-xs text-muted-foreground">
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={() => zoomBy(1 / ZOOM_STEP)}
            disabled={shownScale <= MIN_ZOOM}
            aria-label="Zoom out"
          >
            <MagnifyingGlassMinus size={13} />
          </Button>
          <button
            onClick={resetZoom}
            className="tabular-nums w-11 text-center hover:text-foreground transition-colors"
            aria-label="Fit page"
            title="Fit page"
          >
            {Math.round(shownScale * 100)}%
          </button>
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={() => zoomBy(ZOOM_STEP)}
            disabled={shownScale >= MAX_ZOOM}
            aria-label="Zoom in"
          >
            <MagnifyingGlassPlus size={13} />
          </Button>
        </div>
      </div>

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
          onScroll={sync}
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
                  onTextLayer={onTextLayer}
                  onGoToPage={goToPage}
                />
              ))}
          </div>
        </div>

        {loadError ? (
          <div className="absolute inset-0 flex items-center justify-center px-8 bg-card">
            {ERRORS[loadError] ? (
              <p className="text-sm text-muted-foreground text-center">{ERRORS[loadError]}</p>
            ) : (
              <Alert variant="destructive" className="w-auto">
                <AlertDescription className="text-xs">Failed to load PDF: {loadError}</AlertDescription>
              </Alert>
            )}
          </div>
        ) : (
          !layout && (
            <div className="absolute inset-0 flex items-start justify-center pt-8 pointer-events-none">
              <div className="flex items-center gap-2 text-muted-foreground">
                <CircleNotch size={16} className="animate-spin" />
                <span className="text-sm">Loading PDF…</span>
              </div>
            </div>
          )
        )}
      </div>
    </div>
  );
}

function ModeItem({
  value, label, children,
}: {
  value: LayoutMode;
  label: string;
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <ToggleGroupItem
          value={value}
          aria-label={label}
          className="h-6 px-2 data-[state=on]:bg-primary data-[state=on]:text-primary-foreground"
        >
          {children}
        </ToggleGroupItem>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}
