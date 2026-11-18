import { createContext, useContext } from "react";
import type { WhiteboardImportedImage, WhiteboardImportPdf } from "../importMedia";
import type { Point } from '../model';
import type { DrawingKind } from '../drawingKinds';
import { kindTitle } from '../kindWords';

/**
 * What the drawing surface needs from the host beyond the page itself. The
 * default is a plain browser (the device-sharing companion); the desktop app
 * provides `tauriWhiteboardPlatform` (`./tauri.ts`) at its root, so the surface's
 * own import graph never reaches Tauri.
 */
export interface WhiteboardLibraryPdf {
  id: number;
  filename: string;
  relativePath: string;
  subjectCode: string;
}

export interface WhiteboardPlatform {
  watchFileDrop?(target: HTMLElement, onHover: (paths: string[], at: Point | null) => void, onDrop: (paths: string[], at: Point) => void): () => void;
  readDroppedImages?(paths: string[], signal?: AbortSignal): Promise<WhiteboardImportedImage[]>;
  /** Saves an exported PNG or SVG of a drawing of `kind`, offered as `Whiteboard.png` or `Notebook.png`. */
  saveExport(blob: Blob, format: "png" | "svg", kind: DrawingKind): Promise<void>;
  /** Whether `searchPdfs` and `openPdf` reach a library; absent where they only say to open the desktop app. */
  libraryPdfs?: boolean;
  /** Searches library PDFs by filename and subject; an empty query lists recent files. */
  searchPdfs(query: string): Promise<WhiteboardLibraryPdf[]>;
  /** Opens a library result for picking pages. */
  openPdf(relativePath: string): Promise<WhiteboardImportPdf>;
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

export const browserWhiteboardPlatform: WhiteboardPlatform = {
  async saveExport(blob, format, kind) { downloadBlob(blob, `${kindTitle(kind)}.${format}`); },
  async searchPdfs() { throw new Error(PDF_NEEDS_DESKTOP); },
  async openPdf() { throw new Error(PDF_NEEDS_DESKTOP); },
};

export const WhiteboardPlatformContext = createContext<WhiteboardPlatform>(browserWhiteboardPlatform);
export const useWhiteboardPlatform = () => useContext(WhiteboardPlatformContext);
