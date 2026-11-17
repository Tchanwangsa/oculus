import { invoke } from "@tauri-apps/api/core";

/** `"own"` is the student's: they may add, rename, move and delete there.
 *  `"synced"` is what Oculus syncs, read-only to them. */
export type DriveZone = "synced" | "own";

export interface FolderEntry {
  relativePath: string;
  name: string;
  zone: DriveZone;
  /** Files and folders may be made inside it. */
  writable: boolean;
  /** The folder itself may be renamed, moved or deleted. */
  editable: boolean;
  /** Entries a listing of it shows. */
  childCount: number;
  /** Unix milliseconds. */
  modified: number | null;
}

export interface FileEntry {
  relativePath: string;
  name: string;
  zone: DriveZone;
  editable: boolean;
  size: number;
  modified: number | null;
}

/** A lecture whose recording folder sits in the listed folder; it opens the
 *  lecture page, not the folder. */
export interface RecordingEntry {
  lectureId: string;
  subjectId: number;
  relativePath: string;
  title: string;
  date: string;
}

/** One folder from disk (`list_folder` in `app/src-tauri/src/library/drive/`).
 *  Rows from `files` are joined on by `relativePath`, in the frontend. */
export interface FolderListing {
  /** `""` is the Drive root: its folders are the subjects. */
  rel: string;
  zone: DriveZone;
  /** New files and folders may be made here (the New menu, drops). */
  writable: boolean;
  folders: FolderEntry[];
  files: FileEntry[];
  recordings: RecordingEntry[];
}

/** A folder's contents, or with `recursive` every folder and file below it,
 *  flat. Dot-names, app state and parse artifacts are already left out. */
export const listFolder = (rel: string, recursive = false): Promise<FolderListing> =>
  invoke<FolderListing>("list_folder", { rel, recursive });

/** A notebook's first page picture (PNG bytes), or null before the editor
 *  has written one. */
export async function notebookCover(fileId: number): Promise<Blob | null> {
  const bytes = await invoke<ArrayBuffer>("drive_notebook_cover", { fileId });
  return bytes.byteLength > 0 ? new Blob([bytes], { type: "image/png" }) : null;
}

/** Fired after Drive adds, renames, moves or deletes entries, so listings,
 *  trees and file lists of those subjects reload. */
export const DRIVE_CHANGED_EVENT = "oculus:drive-changed";

export function announceDriveChange(subjectId?: number): void {
  window.dispatchEvent(new CustomEvent(DRIVE_CHANGED_EVENT, { detail: { subjectId } }));
}
