import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import { upsertFile } from "@/lib/db";
import { isPipelineFile } from "@/lib/files/fileTypes";
import { parseFile } from "@/lib/files/courseFiles";

// The student's own files: Rust copies them into one of their folders (or the
// subject's root), then they are ordinary library files. This adds the row
// and the parse kick.

export interface ImportedFile {
  filename: string;
  relative_path: string;
  file_type: string;
  size_bytes: number;
  /** The row's category, from the path (`layout::category_of`): a dropped
   *  `.md` is a document, a `.canvas` a canvas. */
  category: string;
}

/** `file` and `error` are both set when the bytes landed but the Office → PDF
 *  conversion failed: the file is in the library but has nothing to parse. */
export interface ImportOutcome {
  source: string;
  file: ImportedFile | null;
  error: string | null;
}

/** Fired after uploads land, so open lists refresh. */
export const UPLOADS_CHANGED_EVENT = "oculus:uploads-changed";

const announce = () =>
  window.dispatchEvent(new CustomEvent(UPLOADS_CHANGED_EVENT));

/** The native open panel; `[]` if cancelled. Deliberately unfiltered: any file
 *  can be kept, only PDF-backed ones and spreadsheets also become searchable.
 *  `folders` picks folders instead of files. */
export async function pickUploads(folders = false): Promise<string[]> {
  const picked = await open({
    multiple: true,
    directory: folders,
    title: folders ? "Add folders to this subject" : "Add files to this subject",
  });
  if (picked == null) return [];
  return Array.isArray(picked) ? picked : [picked];
}

/** Copies picked files (or folders) into the subject's uploads, or into
 *  `targetDir` (a data-dir-relative folder of the student's own), then records
 *  them. */
export async function addUploads(
  subject: { id: number; code: string },
  paths: string[],
  targetDir?: string,
): Promise<ImportOutcome[]> {
  const outcomes = await invoke<ImportOutcome[]>("import_uploads", {
    subjectCode: subject.code,
    paths,
    ...(targetDir == null ? {} : { targetDir }),
  });
  await recordUploads(subject, outcomes);
  return outcomes;
}

/** Writes the `files` row for each file that landed and kicks its parse:
 *  picked files and browser downloads saved in Oculus alike. The row must
 *  exist before the parse is kicked: `useBackendEvents` resolves a finished
 *  parse to its file by `(subject_id, relative_path)` to embed it. */
export async function recordUploads(
  subject: { id: number; code: string },
  outcomes: readonly ImportOutcome[],
): Promise<void> {
  for (const outcome of outcomes) {
    const file = outcome.file;
    if (!file) continue;
    await upsertFile(
      subject.id,
      file.filename,
      file.relative_path,
      file.file_type,
      file.size_bytes,
      file.category,
    );
    // Fire-and-forget: the parse (a spreadsheet's conversion to text) runs in
    // Rust, a retryable failure is re-kicked by `useQualitySweep` and any
    // other surfaces on the file row.
    if (!outcome.error && isPipelineFile(file.filename)) {
      parseFile(subject.id, subject.code, file.relative_path).catch((e) => console.warn(`[uploads] parse ${file.relative_path}: ${e}`));
    }
  }

  announce();
}
