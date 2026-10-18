import { invoke } from "@tauri-apps/api/core";
import { deleteFileRow, getFileByRelativePath, getFilesForSubject, renameFileRow, touchFileRow, upsertFile, type DbFile } from "@/lib/db";
import type { ImportedFile } from "@/lib/files/uploads";
import { createCanvasDocument, serializeCanvasDocument } from "./model";

export const CANVASES_CHANGED_EVENT = "oculus:canvases-changed";
const announce = (subjectId: number) => window.dispatchEvent(new CustomEvent(CANVASES_CHANGED_EVENT, { detail: { subjectId } }));

async function rowFor(subjectId: number, file: ImportedFile): Promise<DbFile> {
  await upsertFile(subjectId, file.filename, file.relative_path, "canvas", file.size_bytes, "canvas");
  const row = await getFileByRelativePath(file.relative_path);
  if (!row) throw new Error(`No library row for ${file.relative_path}`);
  return row;
}

export async function createCanvas(subject: { id: number; code: string }, title = "Untitled"): Promise<DbFile> {
  const imported = await invoke<ImportedFile>("create_canvas", { subjectCode: subject.code, title, content: serializeCanvasDocument(createCanvasDocument()) });
  const row = await rowFor(subject.id, imported);
  announce(subject.id);
  return row;
}

export const readCanvas = (relativePath: string) => invoke<string>("read_canvas", { relativePath });

export async function saveCanvas(file: DbFile, content: string): Promise<void> {
  const bytes = await invoke<number>("write_canvas", { relativePath: file.relative_path, content });
  await touchFileRow(file.id, bytes, true);
}

/** Sessions serialize this operation with every write and update their path. */
export async function renameCanvasFile(file: DbFile, title: string): Promise<DbFile> {
  const moved = await invoke<ImportedFile>("rename_canvas", { relativePath: file.relative_path, title });
  await renameFileRow(file.id, moved.filename, moved.relative_path);
  const updated = { ...file, filename: moved.filename, relative_path: moved.relative_path };
  announce(file.subject_id);
  return updated;
}

export async function deleteCanvasFile(file: DbFile): Promise<void> {
  await invoke("delete_canvas", { relativePath: file.relative_path });
  await deleteFileRow(file.id);
  announce(file.subject_id);
}

export async function reconcileCanvases(subject: { id: number; code: string }): Promise<void> {
  const [onDisk, rows] = await Promise.all([
    invoke<ImportedFile[]>("list_canvases", { subjectCode: subject.code }), getFilesForSubject(subject.id),
  ]);
  const existing = new Map(rows.filter((f) => f.category === "canvas").map((f) => [f.relative_path, f]));
  let changed = false;
  for (const file of onDisk) {
    const row = existing.get(file.relative_path);
    existing.delete(file.relative_path);
    if (!row) { await rowFor(subject.id, file); changed = true; }
    else if (row.size_bytes !== file.size_bytes) { await touchFileRow(row.id, file.size_bytes, false); changed = true; }
  }
  for (const row of existing.values()) { await deleteFileRow(row.id); changed = true; }
  if (changed) announce(subject.id);
}
