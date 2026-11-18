import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { deleteFileRow, getFileByRelativePath, getFilesForSubject, getSubjects, renameFileRow, touchFileRow, upsertFile, type DbFile } from "@/lib/db";
import type { ImportedFile } from "@/lib/files/uploads";
import { authorityCode, toAuthorityError, type CanvasAuthorityAck, type CanvasAuthorityEvent, type CanvasAuthorityIO, type CanvasAuthoritySnapshot } from "./authority";
import type { CanvasTransaction } from "./collaboration";
import { DRAWING_KINDS, drawingKindOf, planDrawingRows, type DrawingKind } from "./drawingKinds";
import { createCanvasDocument, serializeCanvasDocument } from "./model";
import { forgetPageSnapshots } from "./snapshots";

/** Announced when a canvas or a notebook row is added, renamed, resized or removed. */
export const CANVASES_CHANGED_EVENT = "oculus:canvases-changed";
const announce = (subjectId: number) => window.dispatchEvent(new CustomEvent(CANVASES_CHANGED_EVENT, { detail: { subjectId } }));

async function rowFor(subjectId: number, file: ImportedFile, kind: DrawingKind): Promise<DbFile> {
  const { category } = DRAWING_KINDS[kind];
  await upsertFile(subjectId, file.filename, file.relative_path, category, file.size_bytes, category);
  const row = await getFileByRelativePath(file.relative_path);
  if (!row) throw new Error(`No library row for ${file.relative_path}`);
  return row;
}

/** A new drawing of `kind` in the subject's own folder for it, or in
 *  `targetDir` (a data-dir-relative folder of the student's own). */
export async function createDrawing(kind: DrawingKind, subject: { id: number; code: string }, title: string, content: string, targetDir?: string): Promise<DbFile> {
  const imported = await invoke<ImportedFile>("create_canvas", { subjectCode: subject.code, title, content, kind, ...(targetDir == null ? {} : { targetDir }) });
  const row = await rowFor(subject.id, imported, kind);
  announce(subject.id);
  return row;
}

export const createCanvas = (subject: { id: number; code: string }, title = "Untitled", content = serializeCanvasDocument(createCanvasDocument()), targetDir?: string): Promise<DbFile> =>
  createDrawing("canvas", subject, title, content, targetDir);

/** `content` must have `paper`: the native side refuses a notebook without pages. */
export const createNotebook = (subject: { id: number; code: string }, title: string, content: string, targetDir?: string): Promise<DbFile> =>
  createDrawing("notebook", subject, title, content, targetDir);

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

/** Keeps a draft the shared drawing refused as a new file of the same kind beside the original. */
export async function saveCanvasCopy(file: DbFile, title: string, content: string): Promise<DbFile> {
  const subject = (await getSubjects()).find((s) => s.id === file.subject_id);
  if (!subject) throw new Error("This canvas's subject is no longer in the library.");
  return createDrawing(drawingKindOf(file) ?? "canvas", subject, title, content);
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

/** Brings `kind`'s rows in line with its folder on disk; it never touches the other kind's rows. */
export async function reconcileDrawings(kind: DrawingKind, subject: { id: number; code: string }): Promise<void> {
  const [onDisk, rows] = await Promise.all([
    invoke<ImportedFile[]>("list_canvases", { subjectCode: subject.code, kind }), getFilesForSubject(subject.id),
  ]);
  const plan = planDrawingRows(kind, onDisk, rows);
  for (const file of plan.add) await rowFor(subject.id, file, kind);
  for (const { row, size } of plan.touch) await touchFileRow(row.id, size, false);
  for (const row of plan.remove) { await deleteFileRow(row.id); forgetPageSnapshots(row.id); }
  if (plan.add.length || plan.touch.length || plan.remove.length) announce(subject.id);
}

export const reconcileCanvases = (subject: { id: number; code: string }) => reconcileDrawings("canvas", subject);
export const reconcileNotebooks = (subject: { id: number; code: string }) => reconcileDrawings("notebook", subject);
