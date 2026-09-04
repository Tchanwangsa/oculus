import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { Document, Page, pdfjs } from "react-pdf";
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
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import "react-pdf/dist/Page/AnnotationLayer.css";
import "react-pdf/dist/Page/TextLayer.css";

pdfjs.GlobalWorkerOptions.workerSrc = new URL(
  "pdfjs-dist/build/pdf.worker.min.mjs",
  import.meta.url,
).toString();

type LayoutMode = "scroll" | "single" | "spread";

const MODE_KEY = "oculus-pdf-layout";
const MIN_ZOOM = 0.4;
const MAX_ZOOM = 4;

const clampZoom = (z: number) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z));

interface Props {
  src: string;
}

/**
 * PDF viewer with three layouts — continuous vertical scroll, single page,
 * and two-page spread — plus arrow-key paging (←/→ and ↑/↓), pinch-to-zoom
 * (trackpad pinch or ⌘/ctrl-scroll), and two-finger panning while zoomed.
 */
export function PDFViewer({ src }: Props) {
  const [numPages, setNumPages] = useState(0);
  const [page, setPage] = useState(1);
  const [mode, setMode] = useState<LayoutMode>(
    () => (localStorage.getItem(MODE_KEY) as LayoutMode) || "scroll",
  );
  const [zoom, setZoom] = useState(1);
  const [containerWidth, setContainerWidth] = useState(0);
  const [loadError, setLoadError] = useState<string | null>(null);

  const scrollRef = useRef<HTMLDivElement>(null);
  const zoomRef = useRef(zoom);
  zoomRef.current = zoom;
  const pinchBase = useRef(1);

  useEffect(() => {
    localStorage.setItem(MODE_KEY, mode);
  }, [mode]);

  // Reset per document.
  useEffect(() => {
    setPage(1);
    setZoom(1);
    setLoadError(null);
  }, [src]);

  const onLoadSuccess = useCallback(({ numPages }: { numPages: number }) => {
    setNumPages(numPages);
    setLoadError(null);
  }, []);

  const onLoadError = useCallback((err: Error) => setLoadError(err.message), []);

  // ── Page stepping ────────────────────────────────────────────────────────

  const step = mode === "spread" ? 2 : 1;
  const maxPage = Math.max(1, mode === "spread" ? numPages - 1 : numPages);
  const prev = useCallback(
    () => setPage((p) => Math.max(1, p - step)),
    [step],
  );
  const next = useCallback(
    () => setPage((p) => Math.min(maxPage, p + step)),
    [step, maxPage],
  );

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

  // ── Sizing ───────────────────────────────────────────────────────────────

  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setContainerWidth(el.clientWidth));
    ro.observe(el);
    setContainerWidth(el.clientWidth);
    return () => ro.disconnect();
  }, []);

  // ── Zoom: pinch (WebKit gesture events), ⌘/ctrl + scroll, buttons ────────

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;

    const onWheel = (e: WheelEvent) => {
      // Chrome/WebKit report trackpad pinch as ctrlKey wheel.
      if (!e.ctrlKey && !e.metaKey) return;
      e.preventDefault();
      setZoom(clampZoom(zoomRef.current * (1 - e.deltaY * 0.01)));
    };
    // WKWebView (Safari engine) fires real gesture events for pinch.
    const onGestureStart = (e: Event) => {
      e.preventDefault();
      pinchBase.current = zoomRef.current;
    };
    const onGestureChange = (e: Event) => {
      e.preventDefault();
      const scale = (e as unknown as { scale: number }).scale;
      if (scale) setZoom(clampZoom(pinchBase.current * scale));
    };

    el.addEventListener("wheel", onWheel, { passive: false });
    el.addEventListener("gesturestart", onGestureStart);
    el.addEventListener("gesturechange", onGestureChange);
    return () => {
      el.removeEventListener("wheel", onWheel);
      el.removeEventListener("gesturestart", onGestureStart);
      el.removeEventListener("gesturechange", onGestureChange);
    };
  }, []);

  // ── Layout maths ─────────────────────────────────────────────────────────

  const gutter = 32;
  const columns = mode === "spread" ? 2 : 1;
  const basePageWidth =
    containerWidth > 0
      ? Math.min((containerWidth - gutter * 2) / columns, 900)
      : 620;
  const pageWidth = Math.max(120, Math.round(basePageWidth * zoom));

  if (loadError) {
    return (
      <div className="h-full flex items-center justify-center px-8">
        <Alert variant="destructive" className="w-auto">
          <AlertDescription className="text-xs">
            Failed to load PDF: {loadError}
          </AlertDescription>
        </Alert>
      </div>
    );
  }

  const loading = (
    <div className="flex items-center gap-2 text-muted-foreground mt-8">
      <CircleNotch size={16} className="animate-spin" />
      <span className="text-sm">Loading PDF…</span>
    </div>
  );

  const spreadPages =
    mode === "spread"
      ? [page, page + 1].filter((p) => p <= numPages)
      : [page];

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
              {mode === "spread" && page + 1 <= numPages
                ? `${page}–${page + 1}`
                : page}{" "}
              / {numPages}
            </span>
            <Button
              variant="ghost"
              size="icon-xs"
              disabled={page >= maxPage}
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
            onClick={() => setZoom((z) => clampZoom(z / 1.2))}
            disabled={zoom <= MIN_ZOOM}
            aria-label="Zoom out"
          >
            <MagnifyingGlassMinus size={13} />
          </Button>
          <button
            onClick={() => setZoom(1)}
            className="tabular-nums w-11 text-center hover:text-foreground transition-colors"
            aria-label="Reset zoom"
            title="Reset zoom"
          >
            {Math.round(zoom * 100)}%
          </button>
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={() => setZoom((z) => clampZoom(z * 1.2))}
            disabled={zoom >= MAX_ZOOM}
            aria-label="Zoom in"
          >
            <MagnifyingGlassPlus size={13} />
          </Button>
        </div>
      </div>

      {/* Pages. overflow-auto gives two-finger panning for free when zoomed. */}
      <div ref={scrollRef} className="flex-1 min-h-0 overflow-auto">
        <Document
          file={src}
          onLoadSuccess={onLoadSuccess}
          onLoadError={onLoadError}
          loading={<div className="flex justify-center">{loading}</div>}
          className="min-h-full"
        >
          {mode === "scroll" ? (
            <div className="flex flex-col items-center gap-4 py-4 px-4 w-max min-w-full">
              {Array.from({ length: numPages }, (_, i) => (
                <Page
                  key={i + 1}
                  pageNumber={i + 1}
                  width={pageWidth}
                  renderTextLayer
                  renderAnnotationLayer
                  loading={
                    <div
                      style={{ width: pageWidth, height: pageWidth * 1.29 }}
                      className="bg-surface rounded"
                    />
                  }
                  className="shadow-sm"
                />
              ))}
            </div>
          ) : (
            <div
              className={cn(
                "flex items-start justify-center gap-3 py-4 px-4 w-max min-w-full",
              )}
            >
              {spreadPages.map((p) => (
                <Page
                  key={p}
                  pageNumber={p}
                  width={pageWidth}
                  renderTextLayer
                  renderAnnotationLayer
                  loading={
                    <div
                      style={{ width: pageWidth, height: pageWidth * 1.29 }}
                      className="bg-surface rounded"
                    />
                  }
                  className="shadow-sm"
                />
              ))}
            </div>
          )}
        </Document>
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
