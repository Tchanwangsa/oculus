import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import { deleteFileRow, upsertFile, type DbFile } from "@/lib/db";
import { isPdfBacked } from "@/lib/fileTypes";

/**
 * The student's own files: material that belongs to a subject but was never on
 * Canvas — a tutor's handout, last year's exam, notes from a friend.
 *
 * There is almost nothing here, and that is the point. Rust copies the bytes
 * into `courses/<code>/uploads/`, and from that moment the file is an ordinary
 * library file: the Office converter, the parser, the embedder, ⌘K, semantic
 * search and the chat agent all key off the path and know nothing about where
 * it came from. What this module adds is the row and the first parse kick.
 */

/** One file that landed, as Rust reports it. */
export interface ImportedFile {
  filename: string;
  relative_path: string;
  file_type: string;
  size_bytes: number;
}

/** What became of one picked file. `file` and `error` are both set when the
 *  bytes landed but the Office → PDF conversion did not: the file is in the
 *  library and opens, it just has nothing for the parser to read. */
export interface ImportOutcome {
  source: string;
  file: ImportedFile | null;
  error: string | null;
}

/** Fired after uploads land or one is removed, so open lists refresh — the
 *  contract `LECTURES_CHANGED_EVENT` already has with the lecture lists. */
export const UPLOADS_CHANGED_EVENT = "oculus:uploads-changed";

const announce = () =>
  window.dispatchEvent(new CustomEvent(UPLOADS_CHANGED_EVENT));

/**
 * The native open panel. Returns the paths picked, or `[]` if it was
 * cancelled — the picker hands back paths rather than bytes, so nothing large
 * ever crosses the IPC bridge.
 *
 * Deliberately unfiltered. Anything can be worth keeping beside a subject; the
 * formats that additionally become *searchable* are the PDF-backed ones, and
 * the page says so rather than hiding everything else from the panel.
 */
export async function pickUploads(): Promise<string[]> {
  const picked = await open({ multiple: true, title: "Add files to this subject" });
  if (picked == null) return [];
  return Array.isArray(picked) ? picked : [picked];
}

/**
 * Copy files into a subject and start the pipeline on each.
 *
 * The row is written *before* the parse is kicked, and that order is
 * load-bearing: the embed hop in `useBackendEvents` resolves a finished parse
 * back to a file by `(subject_id, relative_path)`, so a file parsed before its
 * row exists would be indexed by nothing and never embedded.
 */
export async function addUploads(
  subject: { id: number; code: string },
  paths: string[],
): Promise<ImportOutcome[]> {
  const outcomes = await invoke<ImportOutcome[]>("import_uploads", {
    subjectCode: subject.code,
    paths,
  });

  for (const outcome of outcomes) {
    const file = outcome.file;
    if (!file) continue;
    await upsertFile(
      subject.id,
      file.filename,
      file.relative_path,
      file.file_type,
      file.size_bytes,
      "upload",
    );
    // Fire-and-forget, like every other parse request: the sidecar may be
    // warming up or absent, and the quality sweep re-requests what it missed.
    if (!outcome.error && isPdfBacked(file.filename)) {
      invoke("parse_file", {
        subjectId: subject.id,
        subjectCode: subject.code,
        relativePath: file.relative_path,
      }).catch((e) => console.warn(`[uploads] parse ${file.relative_path}: ${e}`));
    }
  }

  announce();
  return outcomes;
}

/** Remove an upload: the file, its derived PDF and parse artifacts on disk,
 *  then its row — which cascades the `pages` table's embeddings with it. */
export async function removeUpload(file: DbFile): Promise<void> {
  await invoke("delete_upload", { relativePath: file.relative_path });
  await deleteFileRow(file.id);
  announce();
}
