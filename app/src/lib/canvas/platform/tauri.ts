import { invoke, isTauri } from "@tauri-apps/api/core";
import { blobToBase64 } from "@/lib/encoding/base64";
import { searchLibraryFiles } from "@/lib/db/search";
import { openCanvasLibraryPdf } from "../importPdf";
import { browserCanvasPlatform, type CanvasPlatform } from ".";

/** The desktop app's canvas host: a native Save dialog for exports and native PDF
 * page import. Outside a Tauri webview (the dev preview harness) exports download. */
export const tauriCanvasPlatform: CanvasPlatform = {
  async saveExport(blob, format) {
    if (!isTauri()) return browserCanvasPlatform.saveExport(blob, format);
    await invoke<string | null>("export_canvas_image", { format, data: await blobToBase64(blob) });
  },
  async searchPdfs(query) {
    if (!isTauri()) return browserCanvasPlatform.searchPdfs(query);
    const files = await searchLibraryFiles(query, 30, { fileType: "pdf" });
    return files.map(file => ({ id: file.id, filename: file.filename, relativePath: file.relative_path, subjectCode: file.subject_code }));
  },
  openPdf: openCanvasLibraryPdf,
  async exportPdf(title, pages) {
    const sent = await Promise.all(pages.map(async ({ width, height, png }) => ({ width, height, png: await blobToBase64(png) })));
    await invoke<string | null>("canvas_export_pdf", { title, pages: sent });
  },
};
