import { invoke, isTauri } from "@tauri-apps/api/core";
import { blobToBase64 } from "@/lib/encoding/base64";
import { searchLibraryFiles } from "@/lib/db/search";
import { openCanvasLibraryPdf } from "../importPdf";
import { browserCanvasPlatform, type CanvasPlatform } from ".";
import { watchNativeFileDrop } from '@/lib/ui/nativeFileDrop';
import { assertCanvasImportSize, MAX_CANVAS_IMPORT_IMAGES, readCanvasImages } from '../importMedia';

/** The desktop app's canvas host: a native Save dialog for exports and native PDF
 * page import. Outside a Tauri webview (the dev preview harness) exports download. */
export const tauriCanvasPlatform: CanvasPlatform = {
  watchFileDrop: (target, onHover, onDrop) => isTauri() ? watchNativeFileDrop(target, onHover, onDrop) : () => {},
  async readDroppedImages(paths, signal) {
    if (paths.length > MAX_CANVAS_IMPORT_IMAGES) throw new Error('Add up to 100 images at a time');
    const images = [];
    for (const path of paths) {
      if (signal?.aborted) throw new DOMException('Image import cancelled', 'AbortError');
      const dataUrl = await invoke<string>('canvas_read_dropped_image', { path });
      const blob = await (await fetch(dataUrl)).blob();
      const name = path.split(/[\\/]/).pop() || 'Dropped image';
      images.push(...await readCanvasImages([new File([blob], name, { type: blob.type })], signal));
      assertCanvasImportSize(images);
    }
    return images;
  },
  async saveExport(blob, format) {
    if (!isTauri()) return browserCanvasPlatform.saveExport(blob, format);
    await invoke<string | null>("export_canvas_image", { format, data: await blobToBase64(blob) });
  },
  async searchPdfs(query) {
    if (!isTauri()) return browserCanvasPlatform.searchPdfs(query);
    const files = await searchLibraryFiles(query, 30, { fileType: "pdf" });
    return files.map(file => ({ id: file.id, filename: file.filename, relativePath: file.relative_path, subjectCode: file.subject_code }));
  },
  libraryPdfs: isTauri(),
  openPdf: openCanvasLibraryPdf,
  async exportPdf(title, pages) {
    const sent = await Promise.all(pages.map(async ({ width, height, png }) => ({ width, height, png: await blobToBase64(png) })));
    await invoke<string | null>("canvas_export_pdf", { title, pages: sent });
  },
};
