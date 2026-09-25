/**
 * pdf.js, loaded on demand and memoised (`GlobalWorkerOptions` is global).
 * `pdf_viewer.mjs` reads `globalThis.pdfjsLib` when it is *evaluated*, and
 * static imports are hoisted, so the core must be awaited and assigned before
 * the viewer is imported. Lazy loading also keeps pdf.js out of the entry chunk.
 */

type PdfjsCore = typeof import("pdfjs-dist");
type PdfjsViewer = typeof import("pdfjs-dist/web/pdf_viewer.mjs");

export type Pdfjs = PdfjsCore & PdfjsViewer;

let pending: Promise<Pdfjs> | null = null;

export function loadPdfjs(): Promise<Pdfjs> {
  return (pending ??= (async () => {
    const core = await import("pdfjs-dist");
    // Vite rewrites this to the emitted worker asset.
    core.GlobalWorkerOptions.workerSrc = new URL(
      "pdfjs-dist/build/pdf.worker.min.mjs",
      import.meta.url,
    ).toString();
    (globalThis as unknown as { pdfjsLib: PdfjsCore }).pdfjsLib = core;
    // The stylesheet rides the lazy chunk, so it lands after `index.css`; the
    // overrides there win on specificity, not order.
    const [viewer] = await Promise.all([
      import("pdfjs-dist/web/pdf_viewer.mjs"),
      import("pdfjs-dist/web/pdf_viewer.css"),
    ]);
    return { ...core, ...viewer };
  })());
}
