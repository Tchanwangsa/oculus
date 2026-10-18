import type { DbFile } from "@/lib/db";
import { parseCanvasDocument } from "./model";
import { createCanvasSessions } from "./sessions";
import { deleteCanvasFile, readCanvas, renameCanvasFile, saveCanvas } from "./files";

export const canvasSessions = createCanvasSessions<DbFile>({
  validate: (text) => { parseCanvasDocument(text); },
  read: readCanvas, write: saveCanvas, rename: renameCanvasFile, remove: deleteCanvasFile,
  drafts: {
    getItem: (key) => localStorage.getItem(key),
    setItem: (key, value) => localStorage.setItem(key, value),
    removeItem: (key) => localStorage.removeItem(key),
  },
});
