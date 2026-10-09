import { useEffect, type RefObject } from "react";
import { centerIn, matchSpans } from "@/lib/citations/locateQuote";
import type { FileLocate } from "@/lib/files/openFile";
import { HIT } from "@/components/files/pdf/constants";
import type { Engine } from "@/components/files/pdf/types";

/**
 * Go to the cited page, then mark the quote's spans once that page's text
 * layer exists — now, or on `textlayerrendered`, which also fires when
 * pdf.js re-renders a page it had evicted, so the marks come back while
 * this locate is current. Scrolls to the passage only the first time.
 */
export function usePdfCitationLocate(
  engineRef: RefObject<Engine | null>,
  containerRef: RefObject<HTMLDivElement | null>,
  locate: FileLocate | undefined,
  pagesReady: number,
) {
  useEffect(() => {
    const viewer = engineRef.current?.viewer;
    const container = containerRef.current;
    if (!viewer || !container || !pagesReady || !locate?.page) return;
    for (const el of container.querySelectorAll(`.${HIT}`)) el.classList.remove(HIT);
    const pageNumber = Math.min(Math.max(1, locate.page), viewer.pagesCount);
    viewer.scrollPageIntoView({ pageNumber });
    const quote = locate.quote;
    if (!quote) return;
    let scrolled = false;
    const mark = () => {
      const layer = container.querySelector(`.page[data-page-number="${pageNumber}"] .textLayer`);
      if (!layer) return;
      const spans = Array.from(layer.querySelectorAll<HTMLElement>("span:not(.markedContent)"));
      const hits = matchSpans(spans, quote);
      for (const el of hits) el.classList.add(HIT);
      if (hits.length && !scrolled) {
        scrolled = true;
        centerIn(container, hits[0]);
      }
    };
    mark();
    const onRendered = (e: { pageNumber: number }) => {
      if (e.pageNumber === pageNumber) mark();
    };
    viewer.eventBus.on("textlayerrendered", onRendered);
    return () => viewer.eventBus.off("textlayerrendered", onRendered);
    // `locate` is read through its `seq`: a new object with the same seq is
    // the same citation (a refreshed row).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [locate?.seq, pagesReady]);
}
