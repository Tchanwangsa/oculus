import { useEffect, useRef, useState } from "react";
import { openPdf } from "@/lib/pdf/pdfView";
import { renderPage } from "@/lib/pdf/pdfRender";

/** Device pixels across a card's first page. */
const WIDTH = 360;

/**
 * A PDF's first page, rasterised by the viewer's own renderer (`pdf_open`,
 * `pdf_render` through `openPdf` and `renderPage`'s queue). `path` is the
 * PDF itself, or an Office file's converted copy. Calls `onFail` when the
 * PDF can't be opened, so the card falls back to its glyph.
 */
export function PdfThumb({ path, onFail }: { path: string; onFail: () => void }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [drawn, setDrawn] = useState(false);
  const fail = useRef(onFail);
  fail.current = onFail;

  useEffect(() => {
    let live = true;
    const doc = openPdf(path);
    doc.pages
      .then(async (pages) => {
        const first = pages[0];
        if (!first) throw new Error("no pages");
        const width = WIDTH;
        const height = Math.max(1, Math.round((WIDTH * first.height) / first.width));
        const image = await renderPage(path, 0, width, height, () => live);
        const el = canvas.current;
        if (!image || !el || !live) return;
        el.width = width;
        el.height = height;
        el.getContext("2d")?.putImageData(image, 0, 0);
        setDrawn(true);
      })
      .catch(() => { if (live) fail.current(); })
      .finally(() => doc.release());
    return () => { live = false; };
  }, [path]);

  return (
    <canvas
      ref={canvas}
      aria-hidden
      className={drawn ? "h-full w-full bg-white object-cover object-top" : "hidden"}
    />
  );
}
