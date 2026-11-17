import { invoke } from "@tauri-apps/api/core";

/**
 * Drive's folder commands (`create_folder`, `rename_entry`, `move_entries`,
 * `delete_entries`, `delete_preview`, `copy_to_own`). Rust refuses anything
 * outside the student's own files; every refusal arrives as a JSON-serialised
 * {@link DriveError}, which {@link driveErrorText} words for the screen.
 * Paths are data-dir-relative (`courses/<code>/…`).
 */

export type DriveErrorKind = "read_only" | "exists" | "not_found" | "invalid_name" | "io";

const KINDS: ReadonlySet<string> = new Set<DriveErrorKind>(["read_only", "exists", "not_found", "invalid_name", "io"]);

export interface DriveError {
  /** Null when the error was not one of the commands' typed errors. */
  kind: DriveErrorKind | null;
  message: string;
}

/** The typed error a command rejected with, or the raw text as `message`. */
export function parseDriveError(error: unknown): DriveError {
  const raw = error instanceof Error ? error.message : typeof error === "string" ? error : String(error);
  if (error && typeof error === "object" && !(error instanceof Error)) {
    const typed = typedError(error);
    if (typed) return typed;
  }
  try {
    const typed = typedError(JSON.parse(raw));
    if (typed) return typed;
  } catch { /* Not JSON: the raw text is the message. */ }
  return { kind: null, message: raw };
}

function typedError(value: unknown): DriveError | null {
  if (!value || typeof value !== "object") return null;
  const { kind, message } = value as { kind?: unknown; message?: unknown };
  if (typeof kind !== "string" || !KINDS.has(kind)) return null;
  return { kind: kind as DriveErrorKind, message: typeof message === "string" ? message : "" };
}

export const READ_ONLY_TEXT = "Course files are read-only";

/** What the screen says for a failed command. */
export function driveErrorText(error: unknown): string {
  const { kind, message } = parseDriveError(error);
  if (kind === "read_only") return READ_ONLY_TEXT;
  return message || "Something went wrong";
}

/** A new, empty folder at `rel`; returns its path (a taken name is an
 *  `exists` error, never renamed). */
export const createFolder = (rel: string) => invoke<string>("create_folder", { rel });

/** Renames a file or folder in place; returns its new path. */
export const renameEntry = (rel: string, newName: string) =>
  invoke<string>("rename_entry", { rel, newName });

/** Moves files and folders into `targetDir`; returns their new paths in the
 *  same order. Rows keep their ids, so parses and versions stay. */
export const moveEntries = (rels: string[], targetDir: string) =>
  invoke<string[]>("move_entries", { rels, targetDir });

/** Deletes files and folders, with their rows and parse artifacts. */
export const deleteEntries = (rels: string[]) => invoke<void>("delete_entries", { rels });

/** Every file a delete of `rels` would remove, for the confirmation. */
export const deletePreview = (rels: string[]) => invoke<string[]>("delete_preview", { rels });

/** Copies synced files or folders into the student's `targetDir`; returns the
 *  copies' paths in the same order. Each copy has its own row. */
export const copyToOwn = (rels: string[], targetDir: string) =>
  invoke<string[]>("copy_to_own", { rels, targetDir });
