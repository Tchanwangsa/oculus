import type { DbFile } from "@/lib/db";
import type { ImportedFile } from "@/lib/files/uploads";
import { isOwnPath } from "@/lib/files/libraryLayout";

/**
 * The two drawing documents: a canvas (an infinite board, `<title>.canvas`) and a notebook
 * (pages, `<title>.notebook`), anywhere among a subject's own files. The same JSON and the same
 * editor; the kind is the file's extension, matching `DrawingKind` in
 * `app/src-tauri/src/library/files/canvases.rs`, and
 * never what the document holds: `paper` in a `.canvas` is kept and ignored (`notebookPaper`).
 */
export type DrawingKind = "canvas" | "notebook";

export const DRAWING_KINDS: Record<DrawingKind, { extension: string; category: string }> = {
  canvas: { extension: ".canvas", category: "canvas" },
  notebook: { extension: ".notebook", category: "notebook" },
};

/** The kind an own file's path names (`courses/<code>/…/<title><extension>`), or null. Dot-names
 *  are staging files and hidden folders, never drawings. */
export function drawingKindOfPath(path: string): DrawingKind | null {
  if (!isOwnPath(path) || path.split("/").some((part) => part.startsWith("."))) return null;
  const name = path.slice(path.lastIndexOf("/") + 1);
  for (const kind of Object.keys(DRAWING_KINDS) as DrawingKind[]) {
    const { extension } = DRAWING_KINDS[kind];
    if (name.endsWith(extension) && name.length > extension.length) return kind;
  }
  return null;
}

/** A files row's drawing kind, by its category; null for anything that is not a drawing. */
export function drawingKindOf(file: Pick<DbFile, "category">): DrawingKind | null {
  return file.category === "canvas" || file.category === "notebook" ? file.category : null;
}

export interface DrawingRowPlan {
  add: ImportedFile[];
  touch: { row: DbFile; size: number }[];
  remove: DbFile[];
}

/**
 * What to change so `kind`'s rows match its files on disk. Only rows of that kind's category are
 * ever touched or removed: a notebook listing never removes a canvas row, nor the reverse.
 */
export function planDrawingRows(kind: DrawingKind, onDisk: readonly ImportedFile[], rows: readonly DbFile[]): DrawingRowPlan {
  const { category } = DRAWING_KINDS[kind];
  const existing = new Map(rows.filter((f) => f.category === category).map((f) => [f.relative_path, f]));
  const plan: DrawingRowPlan = { add: [], touch: [], remove: [] };
  for (const file of onDisk) {
    if (drawingKindOfPath(file.relative_path) !== kind) continue;
    const row = existing.get(file.relative_path);
    existing.delete(file.relative_path);
    if (!row) plan.add.push(file);
    else if (row.size_bytes !== file.size_bytes) plan.touch.push({ row, size: file.size_bytes });
  }
  plan.remove = [...existing.values()];
  return plan;
}
