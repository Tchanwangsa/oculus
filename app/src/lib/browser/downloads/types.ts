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
  | "failed";

/** The student's answer to the Save dialog. */
export type Dest =
  | { kind: "oculus"; subject_code: string }
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
  /** The tab it belongs to; for a blank popup Rust closed, the tab that
   *  opened it. May be closed by now. */
  tab: number;
  /** A safe file name: what the dialog shows and the save panel suggests. */
  name: string;
  /** Null for `blob:` and `data:` downloads. */
  host: string | null;
  state: DownloadState;
  /** Bytes on disk; the final size once ready. There is no total. */
  received: number;
  /** A choice made while it was still receiving. */
  choice: Dest | null;
  saved_to: SavedTo | null;
  /** Why it failed, or why a delivery did not land (it is `ready` again). */
  error: string | null;
  can_retry: boolean;
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
  return d.state === "saved" || d.state === "discarded" || d.state === "failed";
}
