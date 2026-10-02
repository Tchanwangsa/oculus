import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import { base64, IMAGE_EXTENSIONS } from "@/lib/attachments";

import {
  deleteFileRow,
  getFileByRelativePath,
  getFilesForSubject,
  getSetting,
  renameFileRow,
  setSetting,
  touchFileRow,
  upsertFile,
  type DbFile,
} from "@/lib/db";
import type { ImportedFile } from "@/lib/uploads";

/**
 * The student's own markdown notes, per subject, at
 * `courses/<code>/documents/<title>.md` with `category = 'document'` — an
 * ordinary library file otherwise. **The title is the filename**; there is no
 * title column. The folder can change behind the app's back, hence
 * {@link reconcileDocuments}.
 */

/** Fired when a document is created, renamed, deleted or found on disk — not
 *  on save, which changes nothing a list shows. */
export const DOCUMENTS_CHANGED_EVENT = "oculus:documents-changed";

const announce = () =>
  window.dispatchEvent(new CustomEvent(DOCUMENTS_CHANGED_EVENT));

/** Upsert the row for a file Rust just reported and read it back. */
async function rowFor(
  subjectId: number,
  file: ImportedFile,
): Promise<DbFile> {
  await upsertFile(
    subjectId,
    file.filename,
    file.relative_path,
    "md",
    file.size_bytes,
    "document",
  );
  const row = await getFileByRelativePath(file.relative_path);
  if (!row) throw new Error(`no row for ${file.relative_path}`);
  return row;
}

/** A new, empty document; a taken title steps aside (`notes-2.md`). */
export async function createDocument(
  subject: { id: number; code: string },
  title = "Untitled",
): Promise<DbFile> {
  const file = await invoke<ImportedFile>("create_document", {
    subjectCode: subject.code,
    title,
  });
  const row = await rowFor(subject.id, file);
  announce();
  return row;
}

/** Write the text and touch the row as *seen* — the student's own edit is not
 *  news for the unseen dot. */
export async function saveDocument(file: DbFile, content: string): Promise<void> {
  const bytes = await invoke<number>("write_document", {
    relativePath: file.relative_path,
    content,
  });
  await touchFileRow(file.id, bytes, true);
}

/** Rename (move) a document, returning its path now — which may be a stepped-
 *  aside name if the title was taken. The row keeps its id. */
export async function renameDocument(file: DbFile, title: string): Promise<string> {
  const moved = await invoke<ImportedFile>("rename_document", {
    relativePath: file.relative_path,
    title,
  });
  if (moved.relative_path !== file.relative_path) {
    await renameFileRow(file.id, moved.filename, moved.relative_path);
    announce();
  }
  return moved.relative_path;
}

/**
 * Write a pasted picture beside the note, returning `assets/<stamp>.png`
 * relative to it. Beside the note, not in `agents/attachments/`, so the
 * relative link resolves for anything handed the folder. Written on arrival,
 * since a link cannot point at a file not yet written.
 */
export async function attachDocumentImage(file: DbFile, picture: Blob): Promise<string> {
  return invoke<string>("attach_document_image", {
    relativePath: file.relative_path,
    data: await base64(picture),
  });
}

/** The same for a dropped path; the bytes are read in Rust. */
export async function attachDocumentFile(file: DbFile, path: string): Promise<string> {
  return invoke<string>("attach_document_file", {
    relativePath: file.relative_path,
    path,
  });
}

/** The native open panel for pictures to put in a note; `[]` if cancelled. */
export async function pickDocumentImages(): Promise<string[]> {
  const picked = await open({
    multiple: true,
    title: "Add pictures to this note",
    filters: [{ name: "Images", extensions: IMAGE_EXTENSIONS }],
  });
  if (picked == null) return [];
  return Array.isArray(picked) ? picked : [picked];
}

// ── Inline suggestions ───────────────────────────────────────────────────────

/** Text to insert at the caret of the note at `path`, from the
 *  `documentSuggestions` job's model; `""` for none. A higher `requestId`
 *  supersedes a lower one, which then resolves `""`. */
export function suggestDocument(req: {
  requestId: number;
  path: string;
  before: string;
  after: string;
}): Promise<string> {
  return invoke<string>("document_suggest", req);
}

/** Drop the suggestion in flight, if any. */
export function cancelDocumentSuggestion(): Promise<void> {
  return invoke("document_suggest_cancel");
}

const SUGGESTIONS_KEY = "document_suggestions_enabled";

/** Off unless the student turned it on: every suggestion is a model turn. */
export async function getDocumentSuggestions(): Promise<boolean> {
  return (await getSetting(SUGGESTIONS_KEY)) === "1";
}

export async function setDocumentSuggestions(on: boolean): Promise<void> {
  await setSetting(SUGGESTIONS_KEY, on ? "1" : "0");
}

/** Delete the file, then its row. No trash, so the page confirms first. */
export async function deleteDocument(file: DbFile): Promise<void> {
  await invoke("delete_document", { relativePath: file.relative_path });
  await deleteFileRow(file.id);
  announce();
}

/**
 * Sync the subject's `document` rows with its `documents/` folder, which other
 * editors can change: new files get rows, missing files lose them, and a size
 * change is touched *unseen*. Returns whether anything changed (already
 * announced).
 */
export async function reconcileDocuments(
  subject: { id: number; code: string },
): Promise<boolean> {
  const [onDisk, rows] = await Promise.all([
    invoke<ImportedFile[]>("list_documents", { subjectCode: subject.code }),
    getFilesForSubject(subject.id),
  ]);
  const byPath = new Map(
    rows.filter((r) => r.category === "document").map((r) => [r.relative_path, r]),
  );
  const seen = new Set<string>();
  let changed = false;

  for (const file of onDisk) {
    seen.add(file.relative_path);
    const row = byPath.get(file.relative_path);
    if (!row) {
      await rowFor(subject.id, file);
      changed = true;
    } else if (row.size_bytes !== file.size_bytes) {
      await touchFileRow(row.id, file.size_bytes, false);
      changed = true;
    }
  }
  for (const row of byPath.values()) {
    if (seen.has(row.relative_path)) continue;
    await deleteFileRow(row.id);
    changed = true;
  }

  if (changed) announce();
  return changed;
}
