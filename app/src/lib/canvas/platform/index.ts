import { createContext, useContext } from "react";
import type { CanvasImportPdf } from "../importMedia";

/**
 * What the drawing surface needs from the host beyond the page itself. The
 * default is a plain browser (the device-sharing companion); the desktop app
 * provides `tauriCanvasPlatform` (`./tauri.ts`) at its root, so the surface's
 * own import graph never reaches Tauri.
 */
export interface CanvasLibraryPdf {
  id: number;
  filename: string;
  relativePath: string;
  subjectCode: string;
}

export interface CanvasPlatform {
  /** Saves an exported PNG or SVG. */
  saveExport(blob: Blob, format: "png" | "svg"): Promise<void>;
  /** Searches library PDFs by filename and subject; an empty query lists recent files. */
  searchPdfs(query: string): Promise<CanvasLibraryPdf[]>;
  /** Opens a library result for picking pages. */
  openPdf(relativePath: string): Promise<CanvasImportPdf>;
  /** Saves a notebook as a PDF, one page per picture, each as large in points as its page. Absent where only the desktop can. */
  exportPdf?(title: string, pages: { width: number; height: number; png: Blob }[]): Promise<void>;
}

export const PDF_NEEDS_DESKTOP = "Open Oculus desktop to import PDF pages. You can add image files here.";

/** Downloads through an anchor, as any page can. */
export function downloadBlob(blob: Blob, name: string): void {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = name;
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

export const browserCanvasPlatform: CanvasPlatform = {
  async saveExport(blob, format) { downloadBlob(blob, `Canvas.${format}`); },
  async searchPdfs() { throw new Error(PDF_NEEDS_DESKTOP); },
  async openPdf() { throw new Error(PDF_NEEDS_DESKTOP); },
};

export const CanvasPlatformContext = createContext<CanvasPlatform>(browserCanvasPlatform);
export const useCanvasPlatform = () => useContext(CanvasPlatformContext);
