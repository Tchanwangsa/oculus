import type { PDFLinkService, PDFViewer as PdfjsViewer } from "pdfjs-dist/web/pdf_viewer.mjs";
import type { Pdfjs } from "@/lib/pdf/pdfjs";

export type LayoutMode = "scroll" | "single" | "spread";

/** The pages the toolbar names: one page, a spread, or a scrolled range. */
export type Shown = { first: number; last: number };

export type Find = { open: boolean; query: string; status?: string };

export type Engine = {
  pdfjs: Pdfjs;
  viewer: PdfjsViewer;
  /** Held here because `viewer.linkService` is typed as the read-only
   *  interface, not the concrete service `setDocument` needs. */
  linkService: PDFLinkService;
};
