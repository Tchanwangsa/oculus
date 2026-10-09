import { useRef, useState } from "react";
import { CircleNotch } from "@phosphor-icons/react";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { useStoredState } from "@/hooks/ui/useStoredState";
import type { FileLocate } from "@/lib/files/openFile";
import { copyPdfAsMarkdown, dragPdfAsMarkdown } from "@/lib/pdf/pdfSelectionMarkdown";
import { FIND_CLOSED, MODE_KEY } from "@/components/files/pdf/constants";
import { PdfChrome } from "@/components/files/pdf/PdfChrome";
import { PdfFindBar } from "@/components/files/pdf/PdfFindBar";
import type { Find, LayoutMode } from "@/components/files/pdf/types";
import { usePdfCitationLocate } from "@/components/files/pdf/usePdfCitationLocate";
import { usePdfEngine } from "@/components/files/pdf/usePdfEngine";
import { usePdfFind } from "@/components/files/pdf/usePdfFind";
import { usePdfMarkdown } from "@/components/files/pdf/usePdfMarkdown";
import { usePdfPaging } from "@/components/files/pdf/usePdfPaging";
import { usePdfZoom } from "@/components/files/pdf/usePdfZoom";

interface Props {
  src: string;
  /** A cited spot: go to its page and highlight its quote there. */
  locate?: FileLocate;
  /** The parse's `.md` (library-relative): a selection copies as its
   *  markdown (`lib/pdf/pdfSelectionMarkdown/`). */
  markdownPath?: string;
}

/**
 * PDF viewer: pdf.js's own `PDFViewer` (layout, virtualisation, zoom anchoring,
 * text layer) under this app's toolbar. What is ours: the toolbar, the three
 * layouts, and pinch/⌘-wheel zoom, which pdf.js
 * does not bind itself. pdf.js loads through `@/lib/pdf/pdfjs` (see there for why).
 */
export function PDFViewer({ src, locate, markdownPath }: Props) {
  /** The page box's text while it has focus; null shows the live range. */
  const [pageDraft, setPageDraft] = useState<string | null>(null);
  const [find, setFind] = useState<Find>(FIND_CLOSED);
  const [mode, setMode] = useStoredState<LayoutMode>(MODE_KEY, (stored) =>
    (stored as LayoutMode) || "scroll",
  );

  /** The whole viewer, toolbar included: what ⌘F engagement is judged on. */
  const rootRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  /** The `.pdfViewer` element pdf.js fills with pages. */
  const viewerElRef = useRef<HTMLDivElement>(null);

  const { engineRef, numPages, shown, scale, loadError, loaded, pagesReady } = usePdfEngine({
    src,
    mode,
    containerRef,
    viewerElRef,
    setFind,
    setPageDraft,
  });
  usePdfCitationLocate(engineRef, containerRef, locate, pagesReady);
  const { pagesRef, resolveImage } = usePdfMarkdown(markdownPath);
  const { findRef, runFind, closeFind } = usePdfFind({
    find,
    setFind,
    engineRef,
    rootRef,
    containerRef,
    viewerElRef,
  });
  const { prev, next, jumpTo } = usePdfPaging(engineRef, mode);
  const { zoomBy, resetZoom } = usePdfZoom(engineRef, containerRef);

  return (
    <div ref={rootRef} className="flex flex-col h-full min-h-0">
      <PdfChrome
        mode={mode}
        setMode={setMode}
        numPages={numPages}
        shown={shown}
        pageDraft={pageDraft}
        setPageDraft={setPageDraft}
        prev={prev}
        next={next}
        jumpTo={jumpTo}
        scale={scale}
        zoomBy={zoomBy}
        resetZoom={resetZoom}
      />

      {find.open && (
        <PdfFindBar
          find={find}
          setFind={setFind}
          inputRef={findRef}
          runFind={runFind}
          onClose={closeFind}
        />
      )}

      {/* pdf.js throws unless its container is `absolute`. */}
      <div className="relative flex-1 min-h-0">
        <div
          ref={containerRef}
          // Capture: pdf.js's text layer writes its own copy and stops it.
          onCopyCapture={(e) => pagesRef.current && copyPdfAsMarkdown(e, e.currentTarget, pagesRef.current, resolveImage)}
          onDragStart={(e) => pagesRef.current && dragPdfAsMarkdown(e, e.currentTarget, pagesRef.current, resolveImage)}
          className="pdf-surface absolute inset-0 overflow-auto"
        >
          <div ref={viewerElRef} data-selectable className="pdfViewer" />
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
