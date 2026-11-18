import { invoke, isTauri } from "@tauri-apps/api/core";
import { asDataUrl, whiteboardPdfRasterSize, type WhiteboardImportPdf } from "./importMedia";

/** Native PDF page import (`whiteboards/imports.rs`). Desktop only: the browser companion
 * never imports this module, it reaches the whiteboard through `platform/tauri.ts`. */
interface OpenedPdf { id: string; name: string; pages: { width: number; height: number }[] }
function importPdf(opened: OpenedPdf): WhiteboardImportPdf {
  let closed = false;
  return {
    name: opened.name,
    pages: opened.pages,
    async renderPage(page, thumbnail = false) {
      if (closed) throw new DOMException("PDF import closed", "AbortError");
      if (!Number.isInteger(page) || page < 1 || page > opened.pages.length) throw new Error("Choose an existing PDF page");
      const size = whiteboardPdfRasterSize(opened.pages[page - 1].width, opened.pages[page - 1].height, thumbnail);
      const bytes = await invoke<ArrayBuffer>("whiteboard_render_pdf_page", { id: opened.id, page, ...size });
      if (closed) throw new DOMException("PDF import closed", "AbortError");
      const dataUrl = await asDataUrl(new Blob([bytes], { type: "image/png" }));
      if (closed) throw new DOMException("PDF import closed", "AbortError");
      return { dataUrl, ...size, name: `${opened.name} — page ${page}` };
    },
    async close() {
      if (closed) return;
      closed = true;
      await invoke("whiteboard_close_pdf", { id: opened.id });
    },
  };
}
export async function openWhiteboardLibraryPdf(relativePath: string): Promise<WhiteboardImportPdf> {
  if (!isTauri()) throw new Error("Open Oculus desktop to import PDF pages");
  return importPdf(await invoke<OpenedPdf>("whiteboard_open_library_pdf", { relativePath }));
}
