import { useState, useCallback } from "react";
import { Document, Page, pdfjs } from "react-pdf";
import {
  ChevronLeftIcon,
  ChevronRightIcon,
  ArrowPathIcon,
} from "@heroicons/react/20/solid";
import "react-pdf/dist/Page/AnnotationLayer.css";
import "react-pdf/dist/Page/TextLayer.css";

pdfjs.GlobalWorkerOptions.workerSrc = new URL(
  "pdfjs-dist/build/pdf.worker.min.mjs",
  import.meta.url,
).toString();

interface Props {
  src: string;
}

export function PDFViewer({ src }: Props) {
  const [numPages, setNumPages] = useState<number>(0);
  const [page, setPage] = useState(1);
  const [loadError, setLoadError] = useState<string | null>(null);

  const onLoadSuccess = useCallback(({ numPages }: { numPages: number }) => {
    setNumPages(numPages);
    setPage(1);
    setLoadError(null);
  }, []);

  const onLoadError = useCallback((err: Error) => {
    setLoadError(err.message);
  }, []);

  if (loadError) {
    return (
      <div className="h-full flex items-center justify-center px-8">
        <p className="text-xs text-destructive">Failed to load PDF: {loadError}</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Page nav */}
      {numPages > 1 && (
        <div className="shrink-0 flex items-center justify-center gap-3 py-2 border-b border-border bg-surface text-xs text-muted-foreground">
          <button
            disabled={page <= 1}
            onClick={() => setPage((p) => Math.max(1, p - 1))}
            className="p-0.5 rounded hover:bg-surface-raised disabled:opacity-30"
          >
            <ChevronLeftIcon className="size-[14px]" />
          </button>
          <span>
            {page} / {numPages}
          </span>
          <button
            disabled={page >= numPages}
            onClick={() => setPage((p) => Math.min(numPages, p + 1))}
            className="p-0.5 rounded hover:bg-surface-raised disabled:opacity-30"
          >
            <ChevronRightIcon className="size-[14px]" />
          </button>
        </div>
      )}

      {/* PDF canvas */}
      <div className="flex-1 overflow-auto flex justify-center py-4 px-2">
        <Document
          file={src}
          onLoadSuccess={onLoadSuccess}
          onLoadError={onLoadError}
          loading={
            <div className="flex items-center gap-2 text-muted-foreground mt-8">
              <ArrowPathIcon className="size-[16px] animate-spin" />
              <span className="text-sm">Loading PDF…</span>
            </div>
          }
        >
          <Page
            pageNumber={page}
            width={620}
            renderTextLayer
            renderAnnotationLayer
          />
        </Document>
      </div>
    </div>
  );
}
