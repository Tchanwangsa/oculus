import { useEffect, useRef } from "react";
import { useDataDir } from "@/hooks/backend/useDataDir";
import { loadParsedPages } from "@/lib/citations";
import { libraryImageSrc } from "@/lib/files/libraryLinks";

/** The parse's pages, for copying a selection as markdown
 *  (`lib/pdf/pdfSelectionMarkdown/`). */
export function usePdfMarkdown(markdownPath: string | undefined) {
  /** The parsed pages, once read; the copy handlers need them synchronously. */
  const pagesRef = useRef<ReadonlyMap<number, string> | null>(null);
  const dataDir = useDataDir();

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

  return { pagesRef, resolveImage };
}
