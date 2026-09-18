/**
 * pdf.js, loaded on demand, in the one order that works.
 *
 * Two facts shape this module.
 *
 * **`pdf_viewer.mjs` treats the core as an external global.** It is a webpack
 * bundle whose module body starts with `const {...} = globalThis.pdfjsLib`, so
 * the global must already be set when the import is *evaluated* — not when our
 * code runs. Static imports are hoisted and evaluated before any statement in
 * the importing module, so `import * as lib` + an assignment + `import
 * "…/pdf_viewer.mjs"` in one file throws on load, however the lines are
 * ordered. The `await`s below are what make the order real rather than
 * textual.
 *
 * **It is also worth splitting out.** The core, the viewer layer and the
 * worker are ~1.7 MB of the bundle for a view that is not the one the app
 * opens on, so dynamic `import()` pays twice: it fixes the ordering *and*
 * keeps pdf.js out of the entry chunk until a PDF is actually opened.
 *
 * One promise, memoised — every viewer that mounts shares the same module
 * instances, which matters because `GlobalWorkerOptions` is global state.
 */

type PdfjsCore = typeof import("pdfjs-dist");
type PdfjsViewer = typeof import("pdfjs-dist/web/pdf_viewer.mjs");

export type Pdfjs = PdfjsCore & PdfjsViewer;

let pending: Promise<Pdfjs> | null = null;

export function loadPdfjs(): Promise<Pdfjs> {
  return (pending ??= (async () => {
    const core = await import("pdfjs-dist");
    // Vite rewrites this to the emitted worker asset; pdf.js itself never
    // resolves the specifier.
    core.GlobalWorkerOptions.workerSrc = new URL(
      "pdfjs-dist/build/pdf.worker.min.mjs",
      import.meta.url,
    ).toString();
    (globalThis as unknown as { pdfjsLib: PdfjsCore }).pdfjsLib = core;
    // Dynamic, so the viewer's stylesheet rides the lazy chunk instead of the
    // entry CSS. It is ~24 KB gzipped once its icons are inlined, and it is of
    // no use to a session that never opens a PDF. It also lands *after*
    // `index.css` in the cascade — which the overrides there are written not to
    // depend on either way, since they win on specificity.
    const [viewer] = await Promise.all([
      import("pdfjs-dist/web/pdf_viewer.mjs"),
      import("pdfjs-dist/web/pdf_viewer.css"),
    ]);
    return { ...core, ...viewer };
  })());
}
