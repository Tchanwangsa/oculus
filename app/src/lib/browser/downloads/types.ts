import type { ImportOutcome } from "@/lib/files/uploads";

/**
 * The browser's download list as Rust sends it on `browser-downloads`
 * (`app/src-tauri/src/shell/browser/downloads/mod.rs` is the source of truth).
 * Fields are snake_case, as `browser-state`'s are. The URL never crosses: it
 * can carry a signed token.
 */

export type DownloadState =
  | "receiving"
  | "ready"
  | "delivering"
  | "saved"
  | "discarded"
  | "failed"
  /** Never downloaded: a streaming playlist. `error` says so. */
  | "refused";

/** The student's answer to the Save dialog. */
export type Dest =
  /** `target_dir`: a data-dir-relative folder of the student's own in that
   *  subject; the subject's root when absent. */
  | { kind: "oculus"; subject_code: string; target_dir?: string }
  | { kind: "path"; path: string }
  | { kind: "discard" };

/** Where a saved download landed. For `oculus` the frontend writes the
 *  `files` row from `outcome.file`. */
export type SavedTo =
  | { kind: "oculus"; subject_code: string; outcome: ImportOutcome }
  | { kind: "path"; path: string };

export interface Download {
  /** Unique for the session, never reused. */
  id: number;
  /** The tab it belongs to. A tab that had shown no page when it started
   *  downloading was opened only for that, and Rust closes it: its download
   *  belongs to the tab that opened it, or with no opener to the closed tab
   *  itself. Any tab may be closed by now. */
  tab: number;
  /** A safe file name: what the dialog shows and the save panel suggests. */
  name: string;
  /** Null for a `data:` download; a `blob:` gives its page's host. */
  host: string | null;
  state: DownloadState;
  /** Bytes on disk; the final size once ready. There is no total. */
  received: number;
  /** A choice made while it was still receiving. */
  choice: Dest | null;
  saved_to: SavedTo | null;
  /** Why it failed or was refused, or why a delivery did not land (it is
   *  `ready` again). */
  error: string | null;
}

export interface DownloadsSnapshot {
  downloads: Download[];
}

/** Where the dialog last sent a download, kept as a browser pref. */
export type LastDestination = "oculus" | "mac";

/** Waiting for the student: no choice yet, so the dialog can ask. */
export function isWaiting(d: Download): boolean {
  return d.state === "ready" || (d.state === "receiving" && d.choice == null);
}

/** Done with: the list can drop it (`browser_download_clear`). */
export function isSettled(d: Download): boolean {
  return d.state === "saved" || d.state === "discarded" || isProblem(d);
}

/** Ended with nothing kept, by no choice of the student's. */
export function isProblem(d: Download): boolean {
  return d.state === "failed" || d.state === "refused";
}
