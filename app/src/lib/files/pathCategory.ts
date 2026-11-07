import type { DbFile } from "@/lib/db";

/** Folder under `courses/<CODE>/` to category. Must agree with
 *  `category_from_path` in `app/src-tauri/src/library/paths/categories.rs`, which sets a row's
 *  `category`. */
const CATEGORY_FOLDERS: Record<string, string> = {
  "pages/": "page",
  "assignments/": "assignment",
  "quizzes/": "quiz",
  "announcements/": "announcement",
  "ed/": "ed",
  "lessons/": "lessons",
  "files/": "file",
  "modules/": "module",
  "images/": "image",
  "documents/": "document",
  "canvases/": "canvas",
  "notebooks/": "notebook",
  "uploads/": "upload",
};

/** Category and filename derived from a library path — enough for
 *  `categoryIconFor` and `fileTitle` without a query. */
export function pathFile(path: string): Pick<DbFile, "category" | "filename"> {
  const rel = path.replace(/^courses\/[^/]+\//, "");
  return { category: categoryFromPath(rel), filename: rel.slice(rel.lastIndexOf("/") + 1) };
}

function categoryFromPath(rel: string): string {
  if (rel === "home.md" || rel === "syllabus.md") return rel.slice(0, -3);
  const folder = Object.keys(CATEGORY_FOLDERS).find((f) => rel.startsWith(f));
  return folder ? CATEGORY_FOLDERS[folder] : "other";
}
