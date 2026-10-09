import type { PDFViewer as PdfjsViewer } from "pdfjs-dist/web/pdf_viewer.mjs";
import type { Pdfjs } from "@/lib/pdf/pdfjs";
import { PAGE_SHARE, VIEWPORT_SHARE } from "@/components/files/pdf/constants";
import type { LayoutMode, Shown } from "@/components/files/pdf/types";

/** One entry of `PDFViewer._getVisiblePages()`, which pdf.js types as `Object`.
 *  `visibleArea` is null when the page is wholly in view; its numbers are
 *  offsets within the page, so page zoom does not skew them. */
type VisiblePage = {
  id: number;
  visibleArea: { minY: number; maxY: number } | null;
  view: { div: HTMLElement };
};

export function shownPages(viewer: PdfjsViewer, mode: LayoutMode): Shown | null {
  const count = viewer.pagesCount;
  if (!count) return null;
  if (mode !== "scroll") {
    // `SpreadMode.ODD` pairs 1|2, 3|4, so a spread starts on an odd page.
    const page = viewer.currentPageNumber;
    const first = mode === "spread" && page % 2 === 0 ? page - 1 : page;
    return { first, last: mode === "spread" ? Math.min(first + 1, count) : first };
  }
  const { views } = (
    viewer as unknown as { _getVisiblePages(): { views: VisiblePage[] } }
  )._getVisiblePages();
  if (!views.length) return null;
  const viewport = viewer.container.clientHeight;
  let first = Infinity;
  let last = -Infinity;
  for (const { id, visibleArea, view } of views) {
    const height = view.div.clientHeight;
    const seen = visibleArea ? visibleArea.maxY - visibleArea.minY : height;
    if (seen >= viewport * VIEWPORT_SHARE || seen >= height * PAGE_SHARE) {
      first = Math.min(first, id);
      last = Math.max(last, id);
    }
  }
  // Sorted most-visible first, so this is the page that fills the most.
  if (first === Infinity) first = last = views[0].id;
  return { first, last };
}

/** Maps the three layouts onto pdf.js's scroll × spread modes; both paged
 *  layouts are `ScrollMode.PAGE`, and `SpreadMode.ODD` pairs 1|2, 3|4. */
export function applyLayout(viewer: PdfjsViewer, pdfjs: Pdfjs, mode: LayoutMode) {
  viewer.scrollMode =
    mode === "scroll" ? pdfjs.ScrollMode.VERTICAL : pdfjs.ScrollMode.PAGE;
  viewer.spreadMode =
    mode === "spread" ? pdfjs.SpreadMode.ODD : pdfjs.SpreadMode.NONE;
}
