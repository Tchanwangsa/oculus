import { invoke } from "@tauri-apps/api/core";

import { base64 } from "@/lib/attachments";

import {
  deleteFileRow,
  getFileByRelativePath,
  getFilesForSubject,
  renameFileRow,
  touchFileRow,
  upsertFile,
  type DbFile,
} from "@/lib/db";
import type { ImportedFile } from "@/lib/uploads";

/**
 * The student's own notes: markdown they write in the app, per subject.
 *
 * A document is the same shape as an upload with the bytes coming from a
 * textarea instead of Finder. Rust keeps it at `courses/<code>/documents/
 * <title>.md`, the row is `category = 'document'`, and from there it is an
 * ordinary library file — ⌘K finds it, the side panel renders it, the chat
 * agent reads it under `courses/`. **The title is the filename** without its
 * `.md`; there is no separate title column to drift from the name on disk.
 *
 * What this module adds over `uploads.ts` is a write path and a reconcile: a
 * note is edited in place rather than copied in once, and the folder is plain
 * markdown on disk that a text editor, or an agent handed the folder, can
 * change behind the app's back.
 */

/** Fired after a document is created, renamed, deleted or found on disk, so
 *  open lists refresh — `useSubjectFiles` listens for it directly. Not fired
 *  on a save: the row's size and stamps change, but nothing a list is looking
 *  at, and a note autosaves every pause. */
export const DOCUMENTS_CHANGED_EVENT = "oculus:documents-changed";

const announce = () =>
  window.dispatchEvent(new CustomEvent(DOCUMENTS_CHANGED_EVENT));

/** A row for a file Rust just reported, read back so the caller has the full
 *  `DbFile` a route or an editor wants. */
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

/**
 * A new, empty document. A taken title steps aside on disk (`notes.md` →
 * `notes-2.md`) rather than overwriting, the same rule an upload follows, and
 * the row is written before anything can point at it.
 */
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

/** Write the document's text and bring the row up to date with it. The row
 *  is touched as *seen*: this is the student's own edit, not a change for the
 *  unseen dot to flag. */
export async function saveDocument(file: DbFile, content: string): Promise<void> {
  const bytes = await invoke<number>("write_document", {
    relativePath: file.relative_path,
    content,
  });
  await touchFileRow(file.id, bytes, true);
}

/**
 * Rename a document, which is to say move its file. Returns the path it now
 * has — the one it had when the name was unchanged, or when the new name was
 * taken and Rust stepped it aside to something close. The row keeps its id,
 * so a page holding it can follow the move without losing its place.
 */
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
 * Write a pasted picture beside the note and answer the path to link it by —
 * `assets/<stamp>.png`, relative to the note itself.
 *
 * Beside the note rather than in `agents/attachments/`, where a composer's
 * picture goes (`app/src/lib/attachments.ts`), because a note is a library
 * file: a relative link resolves for the preview, for the file viewer and for
 * anything else handed the folder, and a picture under `agents/` would
 * resolve for none of them. Rust names the file and sniffs the bytes, so
 * nothing of the clipboard's reaches the filesystem
 * (`app/src-tauri/src/files.rs`).
 *
 * The write happens on arrival, not on a later save: there is no send to
 * defer to here, and a link cannot point at a file that will be written
 * afterwards.
 */
export async function attachDocumentImage(file: DbFile, picture: Blob): Promise<string> {
  return invoke<string>("attach_document_image", {
    relativePath: file.relative_path,
    data: await base64(picture),
  });
}

/** The same for a picture dropped from Finder, which arrives as a path: the
 *  bytes are read in Rust and never cross the IPC. */
export async function attachDocumentFile(file: DbFile, path: string): Promise<string> {
  return invoke<string>("attach_document_file", {
    relativePath: file.relative_path,
    path,
  });
}

/** Delete a document: the file, then its row. There is no trash — the note is
 *  gone — which is why the page asks first. */
export async function deleteDocument(file: DbFile): Promise<void> {
  await invoke("delete_document", { relativePath: file.relative_path });
  await deleteFileRow(file.id);
  announce();
}

/**
 * Bring the subject's `document` rows into line with its `documents/` folder.
 *
 * The folder is not this module's alone: the student can write a note into it
 * from any text editor, or drag one out in Finder. Neither touches the
 * database, so the list would not show the first and would keep showing the
 * second. A file with no row gets one; a row with no file loses it; a file
 * whose size no longer matches its row was rewritten by someone else, and its
 * row is touched *unseen* so the recency dot says so. Returns whether
 * anything changed, and has already announced it if so.
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
