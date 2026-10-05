import { useEffect } from "react";
import { centerIn, matchText, scrollerOf } from "@/lib/locateQuote";
import type { FileLocate } from "@/lib/openFile";

/** The CSS Custom Highlight a cited markdown passage is painted with
 *  (`::highlight(citation-hit)` in `index.css`). */
const HIGHLIGHT = "citation-hit";

/**
 * A cited passage of markdown under `root`: highlighted and scrolled to once
 * the text has rendered (it loads async, hence the observer). A locate with a
 * page is the PDF's to show (`PDFViewer`). A new `seq` jumps again.
 */
export function useLocateHighlight(root: HTMLElement | null, locate?: FileLocate): void {
  const quote = locate && !locate.page ? locate.quote : undefined;
  const seq = locate?.seq;
  useEffect(() => {
    if (!quote || !root || !("highlights" in CSS)) return;
    const find = () => {
      const range = matchText(root, quote);
      const el = range?.startContainer.parentElement;
      if (!range || !el) return false;
      CSS.highlights.set(HIGHLIGHT, new Highlight(range));
      const scroller = scrollerOf(el);
      if (scroller) centerIn(scroller, el);
      return true;
    };
    if (find()) return () => CSS.highlights.delete(HIGHLIGHT);
    const watch = new MutationObserver(() => find() && watch.disconnect());
    watch.observe(root, { childList: true, subtree: true });
    return () => {
      watch.disconnect();
      CSS.highlights.delete(HIGHLIGHT);
    };
  }, [root, quote, seq]);
}
