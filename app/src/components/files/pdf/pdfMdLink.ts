import type { BlockRef } from "@/lib/pdf/pdfBlocks";

/**
 * What the two faces of a parsed PDF share, one per open file (`usePdfMd`):
 * the reading position the face last showing reported, which the next one
 * restores on mount. Plain mutable state, so a scroll re-renders neither the
 * page nor the markdown.
 */
export interface PdfMdLink {
  /** The block (or page) at the top of the face last scrolled. */
  anchor: BlockRef | null;
  /** The citation `seq` the PDF last jumped to: a remount doesn't jump again. */
  locateSeq: number | null;
}

export function createPdfMdLink(): PdfMdLink {
  return { anchor: null, locateSeq: null };
}
