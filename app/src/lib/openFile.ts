import { invoke } from "@tauri-apps/api/core";
import { usePeekStore } from "@/stores/peekStore";
import { humanizeSlug } from "@/lib/format";
import { isPdfBacked } from "@/lib/fileTypes";
import { markFileAccessed, type DbFile } from "@/lib/db";

/** What the peek header shows: real filenames stay, slugs get prettified.
 *  Takes the two columns it reads rather than a whole row, so the chat's
 *  `@` menu labels files the same way the peek panel does. */
export function fileTitle(file: Pick<DbFile, "category" | "filename">): string {
  return file.category === "file" || file.category === "image"
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
 * and Office documents (rendered from their converted sibling PDF) peek
 * in-app; other binaries hand off to the system viewer since we can't render
 * them. Either way the access is recorded.
 */
export function openFileSmart(file: DbFile): void {
  recordFileAccess(file);
  if (file.category === "file" && !isPdfBacked(file.filename)) {
    invoke("open_course_file", { relativePath: file.relative_path }).catch(
      console.error,
    );
    return;
  }
  usePeekStore.getState().openFile(file);
}
