import type { DbFile } from "@/lib/db";
import type { ImportedFile } from "@/lib/files/uploads";
import { COURSES_DIR, OWN_DIRS } from "@/lib/files/libraryLayout";

/**
 * The two drawing documents: a canvas (an infinite board, `<title>.canvas`) and a notebook
 * (pages, `<title>.notebook`), each in its kind's folder (`OWN_DIRS`). The same JSON and the same
 * editor; the kind is the file's folder and extension, matching `DrawingKind` in
 * `app/src-tauri/src/library/files/canvases.rs`, and
 * never what the document holds: `paper` in a `.canvas` is kept and ignored (`notebookPaper`).
 */
export type DrawingKind = "canvas" | "notebook";

export const DRAWING_KINDS: Record<DrawingKind, { folder: string; extension: string; category: string }> = {
  canvas: { folder: OWN_DIRS.canvas, extension: ".canvas", category: "canvas" },
  notebook: { folder: OWN_DIRS.notebook, extension: ".notebook", category: "notebook" },
};

/** The kind a `courses/<code>/<folder>/<title><extension>` path names, or null. */
export function drawingKindOfPath(path: string): DrawingKind | null {
  const parts = path.split("/");
  if (parts.length !== 4 || parts[0] !== COURSES_DIR) return null;
  for (const kind of Object.keys(DRAWING_KINDS) as DrawingKind[]) {
    const { folder, extension } = DRAWING_KINDS[kind];
    if (parts[2] === folder && parts[3].endsWith(extension) && parts[3].length > extension.length) return kind;
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
