import type { ImportOutcome } from "@/lib/files/uploads";
import type { Download } from "./types";

/**
 * A download saved in Oculus is an upload: the frontend writes its `files`
 * row and kicks the parse, as for a picked file (`recordUploads`). Rows come
 * again in every event and after a reload, so each download is recorded once:
 * an id is claimed before the first await, and a row already in the database
 * (a reload, or identical bytes Rust deduplicated onto an existing upload) is
 * left as it is, so its file is never parsed twice.
 */

export interface RecordDeps {
  /** The subject a code names, or null if it is unknown. */
  subjectOf: (code: string) => Promise<{ id: number; code: string } | null>;
  /** Whether the `files` row already exists. */
  hasRow: (relativePath: string) => Promise<boolean>;
  /** Writes the rows and kicks the parses: `recordUploads`. */
  record: (subject: { id: number; code: string }, outcomes: ImportOutcome[]) => Promise<void>;
}

export function createSavedRecorder(deps: RecordDeps) {
  const claimed = new Set<number>();

  async function recordOne(d: Download): Promise<void> {
    const saved = d.saved_to;
    if (saved?.kind !== "oculus" || !saved.outcome.file) return;
    if (await deps.hasRow(saved.outcome.file.relative_path)) return;
    const subject = await deps.subjectOf(saved.subject_code);
    if (!subject) throw new Error(`no subject ${saved.subject_code}`);
    await deps.record(subject, [saved.outcome]);
  }

  /** Records every saved Oculus download not recorded yet. A failure frees
   *  the id, so the next list tries it again. */
  return async function recordSaved(downloads: readonly Download[]): Promise<void> {
    const fresh = downloads.filter(
      (d) => d.state === "saved" && d.saved_to?.kind === "oculus" && !claimed.has(d.id),
    );
    for (const d of fresh) claimed.add(d.id);
    await Promise.all(
      fresh.map((d) =>
        recordOne(d).catch((e) => {
          claimed.delete(d.id);
          console.warn(`[downloads] could not record download ${d.id}: ${e}`);
        }),
      ),
    );
  };
}
