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

type LayoutMode = "scroll" | "single" | "spread";

/** What a plain drag does. `select` reads the page, `pan` moves it. */
type Tool = "select" | "pan";

const MODE_KEY = "oculus-pdf-layout";
const TOOL_KEY = "oculus-pdf-tool";

/** pdf.js's own clamps (`MIN_SCALE`/`MAX_SCALE` in `ui_utils`), which it
 *  applies inside `updateScale` whatever we ask for. Mirrored here only so the
 *  toolbar can grey its buttons out at the ends. Note the floor is genuinely
 *  low and has to be: **scale is absolute, not relative to the fit** — 1 means
 *  actual size — so a 960pt lecture slide fitted to the 360px side panel is
 *  already sitting at about 0.37. */
const MIN_ZOOM = 0.1;
const MAX_ZOOM = 10;

/** One toolbar press, as a ratio. pdf.js's own `steps: 1` rounds to a tenth,
 *  which walks 0.37 to 0.4 and reads as a stutter at the small end. */
const ZOOM_STEP = 1.1;

/** Handed to `updateScale` as `drawingDelay`: pdf.js writes the new scale to
 *  the `--scale-factor` CSS variable immediately — a compositor pass over the
 *  canvases and the text layers already on screen — and re-rasterises once,
 *  this long after the last change. It is the preview/commit split, owned by
 *  the library rather than by us, and it is the reason a pinch is smooth.
 *  Anything >= 1000 disables the postponement entirely. */
const DRAW_DELAY = 400;

/** `deltaMode === DOM_DELTA_LINE` reports notches, not pixels; a line is
 *  roughly this many pixels on this platform. */
const LINE_HEIGHT = 16;

/** A `gesturestart` with no matching `gestureend` would latch `gestureActive`
 *  and kill the ⌘-scroll path for the rest of the session. WebKit does drop
 *  the end event (a pinch that ends over a scrollbar, a window that loses the
 *  fingers), so the flag also releases itself once the pinch has been quiet
 *  this long — comfortably longer than the gap between two `gesturechange`
 *  events, which is one frame. */
const GESTURE_LAPSE = 400;

/** The zoom presets pdf.js recomputes on a resize. A number the reader chose
 *  is theirs and survives; a fit is a promise about the container and has to
 *  be re-kept when the container changes. */
const FIT_VALUES = new Set(["auto", "page-width", "page-fit", "page-actual"]);

/** What the viewer opens at. `page-width` in the full-page view makes a
 *  16:9 slide bigger than the screen; `auto` is page-width capped at 125%,
 *  which is the old 900px ceiling expressed the way pdf.js expresses it. */
const DEFAULT_FIT = "auto";

interface Props {
  src: string;
}

/** Everything pdf.js hands back at mount, kept together because nothing here
 *  is useful without the rest. */
type Engine = {
  pdfjs: Pdfjs;
  viewer: PdfjsViewer;
  /** Held rather than read back off `viewer.linkService`, which is typed as
   *  the read-only interface the viewer needs and not as the concrete service
   *  the document has to be handed to. */
  linkService: PDFLinkService;
};

/** Three buttons over pdf.js's two orthogonal modes. Continuous scroll is the
 *  vertical scroll mode; both paged layouts are `ScrollMode.PAGE`, which shows
 *  one unit at a time, and the spread is what makes that unit two pages. `ODD`
 *  pairs 1|2, 3|4 — the same pairing the old hand-rolled `[page, page + 1]`
 *  produced. */
function applyLayout(viewer: PdfjsViewer, pdfjs: Pdfjs, mode: LayoutMode) {
  viewer.scrollMode =
    mode === "scroll" ? pdfjs.ScrollMode.VERTICAL : pdfjs.ScrollMode.PAGE;
  viewer.spreadMode =
    mode === "spread" ? pdfjs.SpreadMode.ODD : pdfjs.SpreadMode.NONE;
}

/**
 * PDF viewer: pdf.js's own viewer component under this app's toolbar.
 *
 * **The layout, the windowing, the zoom anchoring and the text layer are
 * Mozilla's**, from `pdfjs-dist/web/pdf_viewer.mjs` — the same `PDFViewer`
 * class Firefox's built-in reader is built on, minus its chrome. This file
 * used to hand-roll all of it on top of `react-pdf`: a page window driven by a
 * guessed aspect ratio, a `transform: scale()` preview folded into a real
 * raster on an idle timer, and an anchor record that measured rects and
 * centring slack across two frames to keep the pinched point still. Every one
 * of those has a counterpart in the library that is better tested than ours
 * could be — `updateScale({ origin })` for the anchor, `--scale-factor` plus
 * `drawingDelay` for the preview, its own virtualisation for the window — and
 * the hand-rolled versions each carried a visible bug: the commit yanked the
 * document toward the middle, and a pinch jittered because WebKit reports it
 * twice.
 *
 * So what is left here is the parts that are this app's rather than the PDF's:
 * the pill toolbar, the three layouts mapped onto pdf.js's scroll/spread
 * modes, the select-versus-pan tool, and the two gesture paths WebKit needs
 * (see the dedupe below, which is still ours because the library does not bind
 * desktop pinch — Firefox's `app.js` does, and we are not shipping that).
 *
 * pdf.js is loaded through `@/lib/pdfjs` rather than imported directly, for
 * ordering reasons set out there; that it also keeps ~1.7 MB out of the entry
 * chunk is the second reason.
 */
export function PDFViewer({ src }: Props) {
  const [numPages, setNumPages] = useState(0);
  const [page, setPage] = useState(1);
  /** Absolute scale, straight off pdf.js. 1 is actual size. */
  const [scale, setScale] = useState(1);
  const [mode, setMode] = useState<LayoutMode>(
    () => (localStorage.getItem(MODE_KEY) as LayoutMode) || "scroll",
  );
  /** Sticky like the layout: a reader who reaches for the hand generally
   *  wants it for a document, not for one drag. */
  const [tool, setTool] = useState<Tool>(
    () => (localStorage.getItem(TOOL_KEY) as Tool) || "select",
  );
  const [loadError, setLoadError] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);

  /** The layout, readable from `pagesinit` — which fires for every document
   *  and must not re-run the mount effect to see a change. */
  const modeRef = useRef(mode);
  modeRef.current = mode;

  /** The scroller. pdf.js requires it to be absolutely positioned and reads
   *  its size for every fit, so it is the container and nothing else. */
  const containerRef = useRef<HTMLDivElement>(null);
  /** The `.pdfViewer` element pdf.js fills with pages. Ours only in the sense
   *  that we create the div. */
  const viewerElRef = useRef<HTMLDivElement>(null);
  const engineRef = useRef<Engine | null>(null);
  /** Bumped when the engine is live, to start the effects that need it.
   *  A ref alone cannot do this — nothing would re-run. */
  const [engineReady, setEngineReady] = useState(0);

  useEffect(() => {
    localStorage.setItem(MODE_KEY, mode);
  }, [mode]);

  useEffect(() => {
    localStorage.setItem(TOOL_KEY, tool);
  }, [tool]);

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
          // pdf.js's default page border is a 9px transparent frame carrying a
          // shadow image, drawn for its own grey viewer ground. Ours is a
          // hairline and a soft shadow in `index.css`, on the app's tokens.
          removePageBorders: true,
        });
        linkService.setViewer(viewer);

        // **Both of these have to happen here, not at mount.**
        // `setDocument` calls `_resetView`, which puts `_scrollMode` back to
        // VERTICAL and `_spreadMode` to NONE — so a layout chosen before the
        // document arrived (which is every layout, since the toggle is sticky
        // across files) is silently thrown away. And a fit is a statement
        // about the pages' size, so it cannot be taken before they exist.
        //
        // Order matters between the two: a spread halves the width each page
        // gets, and `#pageWidthScaleFactor` reads the spread mode when it
        // works the fit out.
        eventBus.on("pagesinit", () => {
          applyLayout(viewer, pdfjs, modeRef.current);
          viewer.currentScaleValue = DEFAULT_FIT;
        });
        eventBus.on("pagechanging", (e: { pageNumber: number }) =>
          setPage(e.pageNumber),
        );
        eventBus.on("scalechanging", (e: { scale: number }) =>
          setScale(e.scale),
        );

        engine = { pdfjs, viewer, linkService };
        engineRef.current = engine;
        setEngineReady((n) => n + 1);
      })
      .catch((err: Error) => {
        // `main.tsx` paints an unhandled rejection over the whole window, so
        // nothing in this file may leave one behind.
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
      // `destroy()` rejects the loading task if it is still in flight, which
      // is the path above's `catch`, and rejects nothing once settled.
      task.destroy().catch(() => {});
      doc?.destroy().catch(() => {});
    };
  }, [src, engineReady]);

  // ── Layout ───────────────────────────────────────────────────────────────

  useEffect(() => {
    const engine = engineRef.current;
    if (!engine) return;
    applyLayout(engine.viewer, engine.pdfjs, mode);
  }, [mode, engineReady]);

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

  /** Scale by a ratio about a point, or about the view's centre with no point.
   *
   *  `origin` is in the container's **offset** space, not client space:
   *  `#setScaleUpdatePages` subtracts `containerTopLeft` — `offsetTop`/
   *  `offsetLeft` against the nearest positioned ancestor — from it. In
   *  Firefox's viewer the container is the page, so `clientX`/`clientY` go
   *  straight in; here the scroller is inset in a card in a panel, so the
   *  pointer has to be converted. The rect and the offsets cancel to "where
   *  the pointer is inside the scroller", which is what the arithmetic wants
   *  either way. */
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

    // WebKit reports one trackpad pinch twice — as `gesture*` events *and* as
    // synthesized ctrlKey wheel events. Whichever path is live has to be the
    // only one, or the two overwrite each other's result every frame. This is
    // the one piece of the old gesture machinery worth keeping: pdf.js binds
    // neither path itself.
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
      // Exponential and clamped. A linear `1 - deltaY * 0.01` hits zero at
      // deltaY 100 and goes negative past it, and one ⌘+wheel notch on macOS
      // is ±120 — a single notch slammed the zoom to the floor.
      const raw = e.deltaY * (e.deltaMode === 1 ? LINE_HEIGHT : 1);
      const d = Math.max(-50, Math.min(50, raw));
      zoomBy(Math.exp(-d * 0.01), e.clientX, e.clientY);
    };

    // WKWebView fires real gesture events for pinch, and `scale` is cumulative
    // from the start of the gesture — hence the base, and hence the ratio
    // against the previous frame rather than the raw scale.
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

  // A fit is a promise about the container, so it has to be re-kept when the
  // container changes size — the side panel is resizable and a split tab
  // halves it outright. pdf.js observes the container itself, but only to
  // update its own cached geometry; re-fitting is the host viewer's job.
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
  // Two-finger scrolling already pans; this is the drag. It moves the
  // scroller's own offsets rather than a transform, for the reason
  // `DiagramLightbox` sets out at length — momentum, scrollbars and keyboard
  // scrolling stay the browser's job. Pointer events, not HTML5 drag: a
  // `dragstart` that sets no data is cancelled outright by WebKit (root
  // `CLAUDE.md`), and nothing here is being dragged *to* anything.
  //
  // **What makes a drag a selection is the glyph under it, not the tool.**
  // pdf.js's text layer is `position: absolute; inset: 0`, so it covers the
  // whole page — a `closest('.textLayer')` test would answer "text" over every
  // margin and leave nothing to grab. The spans inside it are the actual
  // words, and they are also the only thing pdf.js puts an I-beam on, so
  // testing for the span makes the cursor and the gesture agree for free:
  // I-beam and a selection over words, grab and a pan everywhere else.

  const drag = useRef<{
    x: number;
    y: number;
    left: number;
    top: number;
  } | null>(null);
  const [dragging, setDragging] = useState(false);

  const onPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    // Leave the right button, and anything with a modifier that means
    // something else, to be what they are.
    if (e.button !== 0) return;
    const el = containerRef.current;
    if (!el) return;
    const target = e.target as Element;
    // A link or a form widget is neither gesture — in either tool. These are
    // the only parts of the annotation layer that take pointer events at all
    // (`.annotationLayer` itself is `pointer-events: none`), so a match here
    // is always something meant to be clicked.
    if (target.closest?.(".annotationLayer section")) return;
    // ⌥ is the override, for a page dense enough that there is no blank left
    // to grab: it pans from the words themselves.
    if (tool === "select" && !e.altKey && target.closest?.(".textLayer span"))
      return;
    // Without this WebKit starts a selection from the first `pointermove` and
    // drags a highlight along behind the pan.
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

      {/* The scroller has to be `absolute` — pdf.js throws in its constructor
          otherwise, because every fit and every visible-page calculation reads
          this element's own box. Hence the relative shell around it. */}
      <div className="relative flex-1 min-h-0">
        <div
          ref={containerRef}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={endDrag}
          onPointerCancel={endDrag}
          className={cn(
            "pdf-surface absolute inset-0 overflow-auto",
            // One cursor class at a time. Tailwind orders utilities of the
            // same group by its own rules rather than by the order they are
            // written in, so two of them in one string is a coin toss.
            dragging ? "cursor-grabbing" : "cursor-grab",
            // The grab above is the ground; pdf.js's own `cursor: text` on the
            // spans cuts through it over the words, which is the whole
            // affordance. It cuts through because `pdf_viewer.css` is imported
            // unlayered and unlayered CSS outranks every `@layer` — the trap
            // root `CLAUDE.md` records for `index.css`, met from the other
            // side. It is why the hand tool and the drag both switch the layer
            // off at the pointer rather than trying to out-specify it: a dead
            // layer has no cursor, cannot be selected into, and stops matching
            // the `.textLayer span` test above, which is all three answers at
            // once.
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
