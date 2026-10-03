import { useStoredState } from "@/hooks/useStoredState";
import { useCallback, useEffect, useRef, useState } from "react";
import type { PDFDocumentProxy } from "pdfjs-dist";
import type {
  PDFLinkService,
  PDFViewer as PdfjsViewer,
} from "pdfjs-dist/web/pdf_viewer.mjs";
import {
  BookOpen,
  CaretLeft,
  CaretRight,
  CircleNotch,
  CursorText,
  File as FileIcon,
  Hand,
  MagnifyingGlassMinus,
  MagnifyingGlassPlus,
  Rows,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { loadPdfjs, type Pdfjs } from "@/lib/pdfjs";
import { centerIn, matchSpans } from "@/lib/locateQuote";
import type { FileLocate } from "@/stores/sidePanelStore";

type LayoutMode = "scroll" | "single" | "spread";

/** What a plain drag does. `select` reads the page, `pan` moves it. */
type Tool = "select" | "pan";

const MODE_KEY = "oculus-pdf-layout";
const TOOL_KEY = "oculus-pdf-tool";

/** pdf.js's own clamps (`MIN_SCALE`/`MAX_SCALE`), mirrored only to grey out
 *  the toolbar buttons. Scale is absolute (1 = actual size), not relative to
 *  the fit, so a slide fitted to the side panel already sits near 0.37. */
const MIN_ZOOM = 0.1;
const MAX_ZOOM = 10;

/** One toolbar press, as a ratio — pdf.js's `steps: 1` rounds to a tenth,
 *  which stutters at small scales. */
const ZOOM_STEP = 1.1;

/** `updateScale`'s `drawingDelay`: pdf.js previews via `--scale-factor` at
 *  once and re-rasterises this long after the last change (>= 1000 disables
 *  the postponement). */
const DRAW_DELAY = 400;

/** Pixels per line when `deltaMode === DOM_DELTA_LINE`. */
const LINE_HEIGHT = 16;

/** WebKit sometimes drops `gestureend`, which would latch `gestureActive` and
 *  kill ⌘-scroll zoom; the flag releases itself after this much quiet. */
const GESTURE_LAPSE = 400;

/** Scale presets that are re-applied on a container resize; a number the
 *  reader chose survives it. */
const FIT_VALUES = new Set(["auto", "page-width", "page-fit", "page-actual"]);

/** `auto` is page-width capped at 125%; plain `page-width` blows a 16:9 slide
 *  past the screen in the full-page view. */
const DEFAULT_FIT = "auto";

interface Props {
  src: string;
  /** A cited spot: go to its page and highlight its quote there. */
  locate?: FileLocate;
}

/** The class a cited passage's text-layer spans carry (`index.css`). */
const HIT = "citation-hit";

type Engine = {
  pdfjs: Pdfjs;
  viewer: PdfjsViewer;
  /** Held here because `viewer.linkService` is typed as the read-only
   *  interface, not the concrete service `setDocument` needs. */
  linkService: PDFLinkService;
};

/** Maps the three layouts onto pdf.js's scroll × spread modes; both paged
 *  layouts are `ScrollMode.PAGE`, and `SpreadMode.ODD` pairs 1|2, 3|4. */
function applyLayout(viewer: PdfjsViewer, pdfjs: Pdfjs, mode: LayoutMode) {
  viewer.scrollMode =
    mode === "scroll" ? pdfjs.ScrollMode.VERTICAL : pdfjs.ScrollMode.PAGE;
  viewer.spreadMode =
    mode === "spread" ? pdfjs.SpreadMode.ODD : pdfjs.SpreadMode.NONE;
}

/**
 * PDF viewer: pdf.js's own `PDFViewer` (layout, virtualisation, zoom anchoring,
 * text layer) under this app's toolbar. What is ours: the toolbar, the three
 * layouts, the select-versus-pan tool, and pinch/⌘-wheel zoom, which pdf.js
 * does not bind itself. pdf.js loads through `@/lib/pdfjs` (see there for why).
 */
export function PDFViewer({ src, locate }: Props) {
  const [numPages, setNumPages] = useState(0);
  const [page, setPage] = useState(1);
  /** pdf.js's absolute scale (1 is actual size), as of the last change to the
   *  displayed percentage or to whether it sits at an end stop. */
  const [scale, setScale] = useState(1);
  const [mode, setMode] = useStoredState<LayoutMode>(MODE_KEY, (stored) =>
    (stored as LayoutMode) || "scroll",
  );
  const [tool, setTool] = useStoredState<Tool>(TOOL_KEY, (stored) =>
    (stored as Tool) || "select",
  );
  const [loadError, setLoadError] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);

  /** Read from `pagesinit` without re-running the mount effect. */
  const modeRef = useRef(mode);
  modeRef.current = mode;

  const containerRef = useRef<HTMLDivElement>(null);
  /** The `.pdfViewer` element pdf.js fills with pages. */
  const viewerElRef = useRef<HTMLDivElement>(null);
  const engineRef = useRef<Engine | null>(null);
  /** Bumped when the engine is live, to re-run the effects that need it. */
  const [engineReady, setEngineReady] = useState(0);
  /** Bumped on `pagesinit`, zeroed per document: pages can be scrolled to. */
  const [pagesReady, setPagesReady] = useState(0);


  // ── The viewer itself ────────────────────────────────────────────────────

  useEffect(() => {
    let cancelled = false;
    let engine: Engine | null = null;

    loadPdfjs()
      .then((pdfjs) => {
        const container = containerRef.current;
        const viewerEl = viewerElRef.current;
        if (cancelled || !container || !viewerEl) return;

        const eventBus = new pdfjs.EventBus();
        const linkService = new pdfjs.PDFLinkService({ eventBus });
        const viewer = new pdfjs.PDFViewer({
          container,
          viewer: viewerEl,
          eventBus,
          linkService,
          // The page frame is drawn in `index.css` instead.
          removePageBorders: true,
        });
        linkService.setViewer(viewer);

        // Both must happen on `pagesinit`: `setDocument` resets scroll/spread
        // mode, and a fit needs the pages' size. Layout first — the fit reads
        // the spread mode.
        eventBus.on("pagesinit", () => {
          applyLayout(viewer, pdfjs, modeRef.current);
          viewer.currentScaleValue = DEFAULT_FIT;
          setPagesReady((n) => n + 1);
        });
        eventBus.on("pagechanging", (e: { pageNumber: number }) => {
          // In scroll mode the toolbar doesn't show a current page. Avoid
          // rerendering the React host as pdf.js scrolls through the document.
          if (modeRef.current !== "scroll") setPage(e.pageNumber);
        });
        eventBus.on("scalechanging", (e: { scale: number }) => {
          // The toolbar displays whole percentages and only uses the exact
          // scale to disable the end-stop buttons. Ignore finer changes.
          setScale((current) =>
            Math.round(current * 100) === Math.round(e.scale * 100) &&
            (current <= MIN_ZOOM) === (e.scale <= MIN_ZOOM) &&
            (current >= MAX_ZOOM) === (e.scale >= MAX_ZOOM)
              ? current
              : e.scale,
          );
        });

        engine = { pdfjs, viewer, linkService };
        engineRef.current = engine;
        setEngineReady((n) => n + 1);
      })
      .catch((err: Error) => {
        // `main.tsx` paints unhandled rejections over the window.
        if (!cancelled) setLoadError(err.message);
      });

    return () => {
      cancelled = true;
      engine?.viewer.setDocument(null as never);
      if (engineRef.current === engine) engineRef.current = null;
    };
  }, []);

  // ── The document ─────────────────────────────────────────────────────────

  useEffect(() => {
    const engine = engineRef.current;
    if (!engine) return;
    const { viewer, linkService } = engine;

    setLoaded(false);
    setLoadError(null);
    setNumPages(0);
    setPage(1);
    setPagesReady(0);

    let cancelled = false;
    let doc: PDFDocumentProxy | null = null;
    const task = engine.pdfjs.getDocument(src);

    task.promise
      .then((pdf) => {
        if (cancelled) {
          pdf.destroy().catch(() => {});
          return;
        }
        doc = pdf;
        setNumPages(pdf.numPages);
        setLoaded(true);
        viewer.setDocument(pdf);
        linkService.setDocument(pdf, null);
      })
      .catch((err: Error) => {
        // A cancelled load rejects too, and that is not a failure to report.
        if (!cancelled) setLoadError(err.message);
      });

    return () => {
      cancelled = true;
      viewer.setDocument(null as never);
      linkService.setDocument(null as never, null);
      task.destroy().catch(() => {});
      doc?.destroy().catch(() => {});
    };
  }, [src, engineReady]);

  // ── Layout ───────────────────────────────────────────────────────────────

  useEffect(() => {
    const engine = engineRef.current;
    if (!engine) return;
    applyLayout(engine.viewer, engine.pdfjs, mode);
    // Scroll mode leaves `page` untouched; sync the toolbar when paging is
    // enabled so it starts at the page pdf.js is already showing.
    if (mode !== "scroll") setPage(engine.viewer.currentPageNumber);
  }, [mode, engineReady]);

  // ── Citation locate ──────────────────────────────────────────────────────
  //
  // Go to the cited page, then mark the quote's spans once that page's text
  // layer exists — now, or on `textlayerrendered`, which also fires when
  // pdf.js re-renders a page it had evicted, so the marks come back while
  // this locate is current. Scrolls to the passage only the first time.

  useEffect(() => {
    const viewer = engineRef.current?.viewer;
    const container = containerRef.current;
    if (!viewer || !container || !pagesReady || !locate?.page) return;
    for (const el of container.querySelectorAll(`.${HIT}`)) el.classList.remove(HIT);
    const pageNumber = Math.min(Math.max(1, locate.page), viewer.pagesCount);
    viewer.scrollPageIntoView({ pageNumber });
    const quote = locate.quote;
    if (!quote) return;
    let scrolled = false;
    const mark = () => {
      const layer = container.querySelector(`.page[data-page-number="${pageNumber}"] .textLayer`);
      if (!layer) return;
      const spans = Array.from(layer.querySelectorAll<HTMLElement>("span:not(.markedContent)"));
      const hits = matchSpans(spans, quote);
      for (const el of hits) el.classList.add(HIT);
      if (hits.length && !scrolled) {
        scrolled = true;
        centerIn(container, hits[0]);
      }
    };
    mark();
    const onRendered = (e: { pageNumber: number }) => {
      if (e.pageNumber === pageNumber) mark();
    };
    viewer.eventBus.on("textlayerrendered", onRendered);
    return () => viewer.eventBus.off("textlayerrendered", onRendered);
    // `locate` is read through its `seq`: a new object with the same seq is
    // the same citation (a refreshed row).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [locate?.seq, pagesReady]);

  const prev = useCallback(() => engineRef.current?.viewer.previousPage(), []);
  const next = useCallback(() => engineRef.current?.viewer.nextPage(), []);

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

  // ── Zoom ─────────────────────────────────────────────────────────────────

  /** Scale by a ratio about a point (or the view's centre). pdf.js wants
   *  `origin` in the container's offset space — it subtracts `offsetTop`/
   *  `offsetLeft` — so client coordinates are converted, not passed through. */
  const zoomBy = useCallback(
    (factor: number, clientX?: number, clientY?: number) => {
      const viewer = engineRef.current?.viewer;
      const container = containerRef.current;
      if (!viewer || !container) return;
      let origin: [number, number] | undefined;
      if (clientX != null && clientY != null) {
        const rect = container.getBoundingClientRect();
        origin = [
          clientX - rect.left + container.offsetLeft,
          clientY - rect.top + container.offsetTop,
        ];
      }
      viewer.updateScale({
        scaleFactor: factor,
        origin,
        drawingDelay: DRAW_DELAY,
      });
    },
    [],
  );

  const resetZoom = useCallback(() => {
    const viewer = engineRef.current?.viewer;
    if (viewer) viewer.currentScaleValue = DEFAULT_FIT;
  }, []);

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

  // Re-apply a fit when the container resizes; pdf.js leaves re-fitting to
  // the host viewer.
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      const viewer = engineRef.current?.viewer;
      const value = viewer?.currentScaleValue;
      if (viewer && value && FIT_VALUES.has(value)) {
        viewer.currentScaleValue = value;
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // ── Drag to pan ──────────────────────────────────────────────────────────
  //
  // Moves the scroller's own offsets (see `ui/Lightbox.tsx`). In the select
  // tool, a press on a `.textLayer span` (a word) selects and anywhere else
  // pans — not `.textLayer` itself, which covers the whole page.

  const drag = useRef<{
    x: number;
    y: number;
    left: number;
    top: number;
  } | null>(null);
  const [dragging, setDragging] = useState(false);

  const onPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    const el = containerRef.current;
    if (!el) return;
    const target = e.target as Element;
    // Links and form widgets in the annotation layer stay clickable.
    if (target.closest?.(".annotationLayer section")) return;
    // ⌥ pans from the words themselves.
    if (tool === "select" && !e.altKey && target.closest?.(".textLayer span"))
      return;
    // Otherwise WebKit drags a text selection along behind the pan.
    e.preventDefault();
    drag.current = {
      x: e.clientX,
      y: e.clientY,
      left: el.scrollLeft,
      top: el.scrollTop,
    };
    setDragging(true);
    e.currentTarget.setPointerCapture(e.pointerId);
  };

  const onPointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const el = containerRef.current;
    const d = drag.current;
    if (!el || !d) return;
    el.scrollLeft = d.left - (e.clientX - d.x);
    el.scrollTop = d.top - (e.clientY - d.y);
  };

  const endDrag = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!drag.current) return;
    drag.current = null;
    setDragging(false);
    if (e.currentTarget.hasPointerCapture(e.pointerId))
      e.currentTarget.releasePointerCapture(e.pointerId);
  };

  // ── Chrome ───────────────────────────────────────────────────────────────

  const lastPage = mode === "spread" ? Math.max(1, numPages - 1) : numPages;
  const pageLabel =
    mode === "spread" && page + 1 <= numPages ? `${page}–${page + 1}` : page;

  return (
    <div className="flex flex-col h-full min-h-0">
      {/* One slim control row: layout · page · zoom */}
      <div className="shrink-0 flex items-center gap-3 px-3 h-9 border-b border-border-subtle bg-surface">
        <ToggleGroup
          type="single"
          value={mode}
          onValueChange={(v) => v && setMode(v as LayoutMode)}
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

        <ToggleGroup
          type="single"
          value={tool}
          onValueChange={(v) => v && setTool(v as Tool)}
          variant="outline"
          size="sm"
          className="shrink-0"
        >
          <ModeItem value="select" label="Select text — drag the margins to pan">
            <CursorText size={12} />
          </ModeItem>
          <ModeItem value="pan" label="Pan — or hold ⌥ while dragging">
            <Hand size={12} />
          </ModeItem>
        </ToggleGroup>

        {mode !== "scroll" && numPages > 1 && (
          <div className="flex items-center gap-1 text-xs text-muted-foreground">
            <Button
              variant="ghost"
              size="icon-xs"
              disabled={page <= 1}
              onClick={prev}
              aria-label="Previous page"
            >
              <CaretLeft size={13} />
            </Button>
            <span className="tabular-nums">
              {pageLabel} / {numPages}
            </span>
            <Button
              variant="ghost"
              size="icon-xs"
              disabled={page >= lastPage}
              onClick={next}
              aria-label="Next page"
            >
              <CaretRight size={13} />
            </Button>
          </div>
        )}
        {mode === "scroll" && numPages > 0 && (
          <span className="text-xs text-muted-foreground tabular-nums">
            {numPages} pages
          </span>
        )}

        <div className="ml-auto flex items-center gap-1 text-xs text-muted-foreground">
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={() => zoomBy(1 / ZOOM_STEP)}
            disabled={scale <= MIN_ZOOM}
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
            {Math.round(scale * 100)}%
          </button>
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={() => zoomBy(ZOOM_STEP)}
            disabled={scale >= MAX_ZOOM}
            aria-label="Zoom in"
          >
            <MagnifyingGlassPlus size={13} />
          </Button>
        </div>
      </div>

      {/* pdf.js throws unless its container is `absolute`. */}
      <div className="relative flex-1 min-h-0">
        <div
          ref={containerRef}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={endDrag}
          onPointerCancel={endDrag}
          className={cn(
            "pdf-surface absolute inset-0 overflow-auto",
            // One cursor class at a time: Tailwind, not source order, decides
            // which of two wins.
            dragging ? "cursor-grabbing" : "cursor-grab",
            // `pdf_viewer.css` is unlayered, so its `cursor: text` on words
            // beats any utility; panning switches the text layer off instead
            // (no cursor, no selection, no `.textLayer span` match).
            (tool === "pan" || dragging) && "[&_.textLayer]:pointer-events-none",
          )}
        >
          <div ref={viewerElRef} className="pdfViewer" />
        </div>

        {loadError ? (
          <div className="absolute inset-0 flex items-center justify-center px-8 bg-card">
            <Alert variant="destructive" className="w-auto">
              <AlertDescription className="text-xs">
                Failed to load PDF: {loadError}
              </AlertDescription>
            </Alert>
          </div>
        ) : (
          !loaded && (
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
  value: LayoutMode | Tool;
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
