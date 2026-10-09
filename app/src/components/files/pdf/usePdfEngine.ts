import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type RefObject,
  type SetStateAction,
} from "react";
import type { PDFDocumentProxy } from "pdfjs-dist";
import { loadPdfjs } from "@/lib/pdf/pdfjs";
import {
  DEFAULT_FIT,
  FIND_CLOSED,
  MAX_ZOOM,
  MIN_ZOOM,
} from "@/components/files/pdf/constants";
import { applyLayout, shownPages } from "@/components/files/pdf/layout";
import type { Engine, Find, LayoutMode, Shown } from "@/components/files/pdf/types";

interface PdfEngineArgs {
  src: string;
  mode: LayoutMode;
  containerRef: RefObject<HTMLDivElement | null>;
  /** The `.pdfViewer` element pdf.js fills with pages. */
  viewerElRef: RefObject<HTMLDivElement | null>;
  setFind: Dispatch<SetStateAction<Find>>;
  setPageDraft: (draft: string | null) => void;
}

/** pdf.js's own `PDFViewer` on the container: created once, handed each
 *  document, and kept in the layout mode. Reports the page range, the scale
 *  and the load state the toolbar shows. */
export function usePdfEngine({
  src,
  mode,
  containerRef,
  viewerElRef,
  setFind,
  setPageDraft,
}: PdfEngineArgs) {
  const [numPages, setNumPages] = useState(0);
  const [shown, setShown] = useState<Shown>({ first: 1, last: 1 });
  /** Mirrors `shown` so a scroll that leaves the range alone skips setState. */
  const shownRef = useRef(shown);
  /** pdf.js's absolute scale (1 is actual size), as of the last change to the
   *  displayed percentage or to whether it sits at an end stop. */
  const [scale, setScale] = useState(1);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);

  /** Read from `pagesinit` without re-running the mount effect. */
  const modeRef = useRef(mode);
  modeRef.current = mode;

  const engineRef = useRef<Engine | null>(null);
  /** Bumped when the engine is live, to re-run the effects that need it. */
  const [engineReady, setEngineReady] = useState(0);
  /** Bumped on `pagesinit`, zeroed per document: pages can be scrolled to. */
  const [pagesReady, setPagesReady] = useState(0);

  const syncShown = useCallback(() => {
    const viewer = engineRef.current?.viewer;
    const next = viewer && shownPages(viewer, modeRef.current);
    const current = shownRef.current;
    if (!next || (next.first === current.first && next.last === current.last))
      return;
    shownRef.current = next;
    setShown(next);
  }, []);

  useEffect(() => {
    let cancelled = false;
    let engine: Engine | null = null;

    loadPdfjs()
      .then((pdfjs) => {
        const container = containerRef.current;
        const viewerEl = viewerElRef.current;
        if (cancelled || !container || !viewerEl) return;

        const eventBus = new pdfjs.EventBus();
        const linkService = new pdfjs.PDFLinkService({ eventBus });
        // The viewer hands it each document itself.
        const findController = new pdfjs.PDFFindController({
          linkService,
          eventBus,
        });
        const viewer = new pdfjs.PDFViewer({
          container,
          viewer: viewerEl,
          eventBus,
          linkService,
          findController,
          // The page frame is drawn in `index.css` instead.
          removePageBorders: true,
        });
        linkService.setViewer(viewer);

        // Both must happen on `pagesinit`: `setDocument` resets scroll/spread
        // mode, and a fit needs the pages' size. Layout first — the fit reads
        // the spread mode.
        eventBus.on("pagesinit", () => {
          applyLayout(viewer, pdfjs, modeRef.current);
          viewer.currentScaleValue = DEFAULT_FIT;
          setPagesReady((n) => n + 1);
          syncShown();
        });
        // The paged layouts name the current page or spread, which
        // `pagechanging` reports even before the pages are laid out.
        eventBus.on("pagechanging", () => {
          if (modeRef.current !== "scroll") syncShown();
        });
        // Continuous scroll names the pages on screen; this fires per scroll
        // frame, and `syncShown` skips setState unless the range moved.
        eventBus.on("updateviewarea", syncShown);
        const count = (m: { current: number; total: number }) =>
          m.current ? `${m.current} of ${m.total}` : `${m.total} matches`;
        // While a search is pending the last status stands, rather than
        // blinking out on every keystroke.
        eventBus.on(
          "updatefindcontrolstate",
          (e: { state: number; matchesCount: { current: number; total: number } }) =>
            setFind((f) => {
              const status =
                e.state === pdfjs.FindState.PENDING
                  ? f.status
                  : e.state === pdfjs.FindState.NOT_FOUND
                    ? "No results"
                    : e.matchesCount.total
                      ? count(e.matchesCount)
                      : undefined;
              return status === f.status ? f : { ...f, status };
            }),
        );
        eventBus.on(
          "updatefindmatchescount",
          (e: { matchesCount: { current: number; total: number } }) =>
            setFind((f) => {
              if (!e.matchesCount.total) return f;
              const status = count(e.matchesCount);
              return status === f.status ? f : { ...f, status };
            }),
        );
        eventBus.on("scalechanging", (e: { scale: number }) => {
          // The toolbar displays whole percentages and only uses the exact
          // scale to disable the end-stop buttons. Ignore finer changes.
          setScale((current) =>
            Math.round(current * 100) === Math.round(e.scale * 100) &&
            (current <= MIN_ZOOM) === (e.scale <= MIN_ZOOM) &&
            (current >= MAX_ZOOM) === (e.scale >= MAX_ZOOM)
              ? current
              : e.scale,
          );
        });

        engine = { pdfjs, viewer, linkService };
        engineRef.current = engine;
        setEngineReady((n) => n + 1);
      })
      .catch((err: Error) => {
        // `main.tsx` paints unhandled rejections over the window.
        if (!cancelled) setLoadError(err.message);
      });

    return () => {
      cancelled = true;
      engine?.viewer.setDocument(null as never);
      if (engineRef.current === engine) engineRef.current = null;
    };
  }, [syncShown]);

  useEffect(() => {
    const engine = engineRef.current;
    if (!engine) return;
    const { viewer, linkService } = engine;

    setLoaded(false);
    setLoadError(null);
    setNumPages(0);
    shownRef.current = { first: 1, last: 1 };
    setShown(shownRef.current);
    setPageDraft(null);
    // `setDocument` resets the find controller; the bar goes with it.
    setFind(FIND_CLOSED);
    setPagesReady(0);

    let cancelled = false;
    let doc: PDFDocumentProxy | null = null;
    const task = engine.pdfjs.getDocument(src);

    task.promise
      .then((pdf) => {
        if (cancelled) {
          pdf.destroy().catch(() => {});
          return;
        }
        doc = pdf;
        setNumPages(pdf.numPages);
        setLoaded(true);
        viewer.setDocument(pdf);
        linkService.setDocument(pdf, null);
      })
      .catch((err: Error) => {
        // A cancelled load rejects too, and that is not a failure to report.
        if (!cancelled) setLoadError(err.message);
      });

    return () => {
      cancelled = true;
      viewer.setDocument(null as never);
      linkService.setDocument(null as never, null);
      task.destroy().catch(() => {});
      doc?.destroy().catch(() => {});
    };
  }, [src, engineReady]);

  useEffect(() => {
    const engine = engineRef.current;
    if (!engine) return;
    applyLayout(engine.viewer, engine.pdfjs, mode);
    syncShown();
  }, [mode, engineReady, syncShown]);

  return { engineRef, numPages, shown, scale, loadError, loaded, pagesReady };
}
