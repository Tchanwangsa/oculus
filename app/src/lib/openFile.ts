import { invoke } from "@tauri-apps/api/core";
import { useSidePanelStore } from "@/stores/sidePanelStore";
import { humanizeSlug } from "@/lib/format";
import { isPdfBacked } from "@/lib/fileTypes";
import { getFileByRelativePath, markFileAccessed, type DbFile } from "@/lib/db";

/** What the panel header shows: real filenames stay, slugs get prettified.
 *  Takes the two columns it reads rather than a whole row, so the chat's
 *  `@` menu labels files the same way the side panel does. */
/** Categories whose rows are real files with real filenames — a download, an
 *  inline image, one of the student's own uploads — as opposed to the Canvas
 *  documents stored under a slug. */
const REAL_FILENAME = new Set(["file", "image", "upload"]);

export function fileTitle(file: Pick<DbFile, "category" | "filename">): string {
  return REAL_FILENAME.has(file.category ?? "")
    ? file.filename
    : humanizeSlug(file.filename);
}

/** Fired after a file's last_accessed_at is stamped, so open lists refresh. */
export const FILE_ACCESSED_EVENT = "oculus:file-accessed";

/** Stamps last_accessed_at and tells open lists to refresh. */
export function recordFileAccess(file: Pick<DbFile, "id">): void {
  markFileAccessed(file.id)
    .then(() => window.dispatchEvent(new CustomEvent(FILE_ACCESSED_EVENT)))
    .catch(console.error);
}

/**
 * The one way any list row opens a file: PDFs, pages, announcements, images
 * and Office documents (rendered from their converted sibling PDF) open in
 * the side panel; other binaries hand off to the system viewer since we can't
 * render them. Either way the access is recorded.
 */
export function openFileSmart(file: DbFile): void {
  recordFileAccess(file);
  // An upload can be anything the student had lying around — a zip, a
  // notebook, a recording — so it takes the same hand-off a download does.
  const binary = file.category === "file" || file.category === "upload";
  if (binary && !isPdfBacked(file.filename)) {
    invoke("open_course_file", { relativePath: file.relative_path }).catch(
      console.error,
    );
    return;
  }
  useSidePanelStore.getState().open({ kind: "file", file });
}

/**
 * The shape of a path the agent can be talking about: the library paths it is
 * given (`courses/<subject>/…`) and the ones it actually uses, which carry a
 * `../` because every thread runs from `agents/` beside `courses/`.
 *
 * Matched on shape rather than resolved, so a timeline of a hundred tool rows
 * costs no queries — the lookup happens when one is clicked.
 */
const LIBRARY_PATH = /^(?:\.\.\/)?(courses\/[^\s]+)$/;

/** The library path inside an agent's tool argument, if that is what it is. */
export function libraryPath(raw: string | null | undefined): string | null {
  const m = raw ? LIBRARY_PATH.exec(raw.trim()) : null;
  return m ? m[1] : null;
}

/**
 * Opens a file the agent named. Falls back to the system viewer for anything
 * the library knows nothing about — a path under `courses/` that has no row is
 * a file on disk that never made it through a sync, and handing it to the OS
 * is better than a click that does nothing.
 */
export function openLibraryPath(path: string): void {
  getFileByRelativePath(path)
    .then((file) => {
      if (file) openFileSmart(file);
      else invoke("open_course_file", { relativePath: path }).catch(console.error);
    })
    .catch(console.error);
}
