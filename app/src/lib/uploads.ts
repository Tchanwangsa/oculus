import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import { deleteFileRow, upsertFile, type DbFile } from "@/lib/db";
import { isPdfBacked } from "@/lib/fileTypes";
import { parseFile } from "@/lib/courseFiles";

// The student's own files: Rust copies them into `courses/<code>/uploads/`,
// then they are ordinary library files. This adds the row and the parse kick.

export interface ImportedFile {
  filename: string;
  relative_path: string;
  file_type: string;
  size_bytes: number;
}

/** `file` and `error` are both set when the bytes landed but the Office → PDF
 *  conversion failed: the file is in the library but has nothing to parse. */
export interface ImportOutcome {
  source: string;
  file: ImportedFile | null;
  error: string | null;
}

/** Fired after uploads land or one is removed, so open lists refresh. */
export const UPLOADS_CHANGED_EVENT = "oculus:uploads-changed";

const announce = () =>
  window.dispatchEvent(new CustomEvent(UPLOADS_CHANGED_EVENT));

/** The native open panel; `[]` if cancelled. Deliberately unfiltered: any file
 *  can be kept, only PDF-backed ones also become searchable. */
export async function pickUploads(): Promise<string[]> {
  const picked = await open({ multiple: true, title: "Add files to this subject" });
  if (picked == null) return [];
  return Array.isArray(picked) ? picked : [picked];
}

/** The row must exist before the parse is kicked: `useBackendEvents` resolves a
 *  finished parse to its file by `(subject_id, relative_path)` to embed it. */
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
    // Fire-and-forget: the parse runs in Rust, a retryable failure is re-kicked
    // by `useQualitySweep` and any other surfaces on the file row.
    if (!outcome.error && isPdfBacked(file.filename)) {
      parseFile(subject.id, subject.code, file.relative_path).catch((e) => console.warn(`[uploads] parse ${file.relative_path}: ${e}`));
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
