import type { DbFile } from "@/lib/db";
import { displayCode, humanizeSlug } from "@/lib/format/format";
import { isLessonAsset } from "./fileTypes";

/** Categories whose rows carry real filenames, not Canvas slugs. */
const REAL_FILENAME = new Set(["file", "image", "upload"]);

/** A file's display title: real filenames stay, slugs get humanised. Takes
 *  two columns so the chat's `@` menu can label a path the same way. */
export function fileTitle(file: Pick<DbFile, "category" | "filename">): string {
  // A document's filename was typed by the student: never humanise it.
  if (file.category === "whiteboard") return file.filename.replace(/\.whiteboard$/i, "");
  if (file.category === "notebook") return file.filename.replace(/\.notebook$/i, "");
  if (file.category === "document") return file.filename.replace(/\.md$/i, "");
  return REAL_FILENAME.has(file.category ?? "") || isLessonAsset(file)
    ? file.filename
    : humanizeSlug(file.filename);
}

/** A folder path's display name: a subject's own folder by its code, any other humanised. */
export function folderTitle(path: string): string {
  const parts = path.replace(/\/+$/, "").split("/");
  const name = parts[parts.length - 1] ?? "";
  return parts.length === 2 && parts[0] === "courses" ? displayCode(name) : humanizeSlug(name);
}

export function filePagePath(subjectId: number, relativePath: string): string {
  return `/subjects/${subjectId}/file?path=${encodeURIComponent(relativePath)}`;
}
