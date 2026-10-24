import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { deleteFileRow, getFileByRelativePath, getFilesForSubject, getSubjects, renameFileRow, touchFileRow, upsertFile, type DbFile } from "@/lib/db";
import type { ImportedFile } from "@/lib/files/uploads";
import { authorityCode, toAuthorityError, type CanvasAuthorityAck, type CanvasAuthorityEvent, type CanvasAuthorityIO, type CanvasAuthoritySnapshot } from "./authority";
import type { CanvasTransaction } from "./collaboration";
import { createCanvasDocument, serializeCanvasDocument } from "./model";
import { forgetPageSnapshots } from "./snapshots";

export const CANVASES_CHANGED_EVENT = "oculus:canvases-changed";
const announce = (subjectId: number) => window.dispatchEvent(new CustomEvent(CANVASES_CHANGED_EVENT, { detail: { subjectId } }));

async function rowFor(subjectId: number, file: ImportedFile): Promise<DbFile> {
  await upsertFile(subjectId, file.filename, file.relative_path, "canvas", file.size_bytes, "canvas");
  const row = await getFileByRelativePath(file.relative_path);
  if (!row) throw new Error(`No library row for ${file.relative_path}`);
  return row;
}

export async function createCanvas(subject: { id: number; code: string }, title = "Untitled", content = serializeCanvasDocument(createCanvasDocument())): Promise<DbFile> {
  const imported = await invoke<ImportedFile>("create_canvas", { subjectCode: subject.code, title, content });
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
  forgetPageSnapshots(file.id);
  announce(file.subject_id);
}

/** Keeps a draft the shared canvas refused as a new canvas beside the original. */
export async function saveCanvasCopy(file: DbFile, title: string, content: string): Promise<DbFile> {
  const subject = (await getSubjects()).find((s) => s.id === file.subject_id);
  if (!subject) throw new Error("This canvas's subject is no longer in the library.");
  return createCanvas(subject, title, content);
}

const CHANGE_EVENT = "canvas-authority-change";
const STATE_EVENT = "canvas-authority-state";
type ChangePayload = { fileId: number; epoch: string; revision: number; clientId: string | null; transaction: CanvasTransaction };
type StatePayload = { fileId: number; epoch: string; phase: "live" | "paused" | "ended" };
const authorityListeners = new Map<number, Set<(event: CanvasAuthorityEvent) => void>>();
let authorityEvents: Promise<void> | undefined;

/** One app-wide pair of listeners, registered before the first open so no event is missed. */
function listenAuthority(): Promise<void> {
  const dispatch = (event: CanvasAuthorityEvent) => { for (const listener of authorityListeners.get(event.fileId) ?? []) listener(event); };
  authorityEvents ??= Promise.all([
    listen<ChangePayload>(CHANGE_EVENT, ({ payload }) => dispatch({ kind: "change", ...payload })),
    listen<StatePayload>(STATE_EVENT, ({ payload }) => dispatch({ kind: "state", ...payload })),
  ]).then(() => {}, (error) => { authorityEvents = undefined; throw error; });
  return authorityEvents;
}

const authorityCall = <T>(command: string, args: Record<string, unknown>) =>
  invoke<T>(command, args).catch((error: unknown) => { throw toAuthorityError(error); });

/** The canvas authority's commands. It updates the `files` row itself on a background
 * thread; rename and delete also update it here so Drive's reload sees the change. */
export const canvasAuthority: CanvasAuthorityIO<DbFile> = {
  async existing(file) {
    await listenAuthority();
    try { return await authorityCall<CanvasAuthoritySnapshot>("canvas_authority_snapshot", { fileId: file.id }); }
    catch (error) { if (authorityCode(error) === "session-ended") return null; throw error; }
  },
  async open(file, text) {
    await listenAuthority();
    return authorityCall<CanvasAuthoritySnapshot>("canvas_authority_open", { fileId: file.id, relativePath: file.relative_path, text });
  },
  commit: (file, op) => authorityCall<CanvasAuthorityAck>("canvas_authority_commit", {
    fileId: file.id, clientId: op.clientId, seq: op.seq, opId: op.opId, transaction: op.transaction, viewport: op.viewport ?? null, epoch: op.epoch ?? null,
  }),
  snapshot: (file) => authorityCall<CanvasAuthoritySnapshot>("canvas_authority_snapshot", { fileId: file.id }),
  resolve: (file, choice) => authorityCall<CanvasAuthoritySnapshot>("canvas_authority_resolve_external", { fileId: file.id, choice }),
  async rename(file, title) {
    const moved = await authorityCall<ImportedFile>("canvas_authority_rename", { fileId: file.id, title });
    await renameFileRow(file.id, moved.filename, moved.relative_path);
    announce(file.subject_id);
    return { ...file, filename: moved.filename, relative_path: moved.relative_path };
  },
  async remove(file) {
    await authorityCall<void>("canvas_authority_delete", { fileId: file.id });
    await deleteFileRow(file.id);
    forgetPageSnapshots(file.id);
    announce(file.subject_id);
  },
  close: (file) => authorityCall<void>("canvas_authority_close", { fileId: file.id }),
  onRemote(fileId, listener) {
    let set = authorityListeners.get(fileId);
    if (!set) authorityListeners.set(fileId, set = new Set());
    set.add(listener);
    return () => { set.delete(listener); if (!set.size) authorityListeners.delete(fileId); };
  },
};

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
  for (const row of existing.values()) { await deleteFileRow(row.id); forgetPageSnapshots(row.id); changed = true; }
  if (changed) announce(subject.id);
}
