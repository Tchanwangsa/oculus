import { useEffect, useRef, useState } from "react";
import { loadPagesRecord, loadParsedPages, type PdfBlock } from "@/lib/citations";

/** The parse's record for the PDF at `markdownPath` (none for a PDF without
 *  one): each page's blocks, whether the read has finished, and the pages'
 *  markdown, which the copy handlers need synchronously. */
export function usePdfRecord(markdownPath: string | undefined) {
  /** The parsed pages, once read. */
  const pagesRef = useRef<ReadonlyMap<number, string> | null>(null);
  /** Each page's parse blocks, once the record is read; null when it has none. */
  const [blocks, setBlocks] = useState<ReadonlyMap<number, PdfBlock[]> | null>(null);
  const [recordReady, setRecordReady] = useState(!markdownPath);

  useEffect(() => {
    pagesRef.current = null;
    setBlocks(null);
    if (!markdownPath) {
      setRecordReady(true);
      return;
    }
    let live = true;
    // The fresh read first, so the pages below share it.
    loadPagesRecord(markdownPath, true).then((record) => {
      if (!live) return;
      const map = new Map((record?.pages ?? []).map((p) => [p.page_no, p.blocks ?? []]));
      setBlocks([...map.values()].some((b) => b.length) ? map : null);
      setRecordReady(true);
    });
    loadParsedPages(markdownPath).then((pages) => {
      if (live) pagesRef.current = pages;
    });
    return () => {
      live = false;
    };
  }, [markdownPath]);

  return { blocks, recordReady, pagesRef };
}
