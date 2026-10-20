import type { DbFile } from "@/lib/db";
import { parseCanvasDocument } from "./model";
import { canvasDrafts } from "./drafts";
import { createCanvasSessions } from "./sessions";
import { canvasAuthority, deleteCanvasFile, readCanvas, renameCanvasFile, saveCanvas } from "./files";

export const canvasSessions = createCanvasSessions<DbFile>({
  validate: (text) => { parseCanvasDocument(text); },
  read: readCanvas, write: saveCanvas, rename: renameCanvasFile, remove: deleteCanvasFile, authority: canvasAuthority,
  drafts: canvasDrafts,
});
