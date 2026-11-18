import { invoke, isTauri } from "@tauri-apps/api/core";
import { asDataUrl, canvasPdfRasterSize, type CanvasImportPdf } from "./importMedia";

/** Native PDF page import (`canvases/imports.rs`). Desktop only: the browser companion
 * never imports this module, it reaches the canvas through `platform/tauri.ts`. */
interface OpenedPdf { id: string; name: string; pages: { width: number; height: number }[] }
function importPdf(opened: OpenedPdf): CanvasImportPdf {
  let closed = false;
  return {
    name: opened.name,
    pages: opened.pages,
    async renderPage(page, thumbnail = false) {
      if (closed) throw new DOMException("PDF import closed", "AbortError");
      if (!Number.isInteger(page) || page < 1 || page > opened.pages.length) throw new Error("Choose an existing PDF page");
      const size = canvasPdfRasterSize(opened.pages[page - 1].width, opened.pages[page - 1].height, thumbnail);
      const bytes = await invoke<ArrayBuffer>("canvas_render_pdf_page", { id: opened.id, page, ...size });
      if (closed) throw new DOMException("PDF import closed", "AbortError");
      const dataUrl = await asDataUrl(new Blob([bytes], { type: "image/png" }));
      if (closed) throw new DOMException("PDF import closed", "AbortError");
      return { dataUrl, ...size, name: `${opened.name} — page ${page}` };
    },
    async close() {
      if (closed) return;
      closed = true;
      await invoke("canvas_close_pdf", { id: opened.id });
    },
  };
}
export async function openCanvasLibraryPdf(relativePath: string): Promise<CanvasImportPdf> {
  if (!isTauri()) throw new Error("Open Oculus desktop to import PDF pages");
  return importPdf(await invoke<OpenedPdf>("canvas_open_library_pdf", { relativePath }));
}
