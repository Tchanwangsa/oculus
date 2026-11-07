import type { ArtifactFormat, ArtifactInput } from "./types";

/** What Save to Revision writes for a fence in a reply: its program, the
 *  format that says how to draw it, and its state. `row` is the reply's
 *  thread and row, kept as the artifact's source. */
export function artifactInput({
  title,
  program,
  format,
  state,
  subject,
  row,
}: {
  title: string;
  program: string;
  format: ArtifactFormat;
  state: Record<string, unknown>;
  subject: number | null;
  row: { thread: number; row: number } | null;
}): ArtifactInput {
  return {
    title: title.trim(),
    program,
    format,
    state,
    subject_id: subject,
    source_ref: row ? `${row.thread}:${row.row}` : null,
  };
}
