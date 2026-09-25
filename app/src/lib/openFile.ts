import { invoke } from "@tauri-apps/api/core";
import { useSidePanelStore } from "@/stores/sidePanelStore";
import { useTabStore } from "@/stores/tabStore";
import { humanizeSlug } from "@/lib/format";
import { isPdfBacked, parsedMdSource } from "@/lib/fileTypes";
import { getFileByRelativePath, markFileAccessed, type DbFile } from "@/lib/db";
import { attachmentPath } from "@/lib/attachments";

/** Categories whose rows carry real filenames, not Canvas slugs. */
const REAL_FILENAME = new Set(["file", "image", "upload"]);

/** A file's display title: real filenames stay, slugs get humanised. Takes
 *  two columns so the chat's `@` menu can label a path the same way. */
export function fileTitle(file: Pick<DbFile, "category" | "filename">): string {
  // A document's filename was typed by the student: never humanise it.
  if (file.category === "document") return file.filename.replace(/\.md$/i, "");
  return REAL_FILENAME.has(file.category ?? "")
    ? file.filename
    : humanizeSlug(file.filename);
}

/** Fired after a file's last_accessed_at is stamped, so open lists refresh. */
export const FILE_ACCESSED_EVENT = "oculus:file-accessed";

export function recordFileAccess(file: Pick<DbFile, "id">): void {
  markFileAccessed(file.id)
    .then(() => window.dispatchEvent(new CustomEvent(FILE_ACCESSED_EVENT)))
    .catch(console.error);
}

/** How any list row opens a file: anything renderable (including Office
 *  documents, via their converted PDF) in the side panel, other binaries in
 *  the system viewer. */
export function openFileSmart(file: DbFile): void {
  recordFileAccess(file);
  const binary = file.category === "file" || file.category === "upload";
  if (binary && !isPdfBacked(file.filename)) {
    invoke("open_course_file", { relativePath: file.relative_path }).catch(
      console.error,
    );
    return;
  }
  useSidePanelStore.getState().open({ kind: "file", file });
}

export function filePagePath(subjectId: number, relativePath: string): string {
  return `/subjects/${subjectId}/file?path=${encodeURIComponent(relativePath)}`;
}

/** A row's `data-tab-href` (`lib/newTabClicks.ts`): its ⌘-click route, or
 *  null for a binary that only opens in the system viewer. */
export function filePageHref(file: DbFile): string | null {
  if (file.category === "file" && !isPdfBacked(file.filename)) return null;
  return filePagePath(file.subject_id, file.relative_path);
}

/** A library path as an agent writes it: `courses/…`, or `../courses/…`
 *  since threads run from `agents/`. Matched on shape so rendering costs no
 *  queries; the lookup happens on click. */
const LIBRARY_PATH = /^(?:\.\.\/)?(courses\/[^\s]+)$/;

/** The same path from the filesystem root, as agents write it into links
 *  (left to the anchor it 404s against the dev server). Anchored on `/` so a
 *  command containing a path stays a command; lazy so the capture starts at
 *  the first `courses/`. */
const ABSOLUTE_PATH = /^\/.*?\/(courses\/.+)$/;

/** A `:97` or `:97-120` line citation, trimmed — nothing honours it. */
const LINE_SUFFIX = /:\d+(?:-\d+)?$/;

/** micromark percent-encodes link destinations ("Application%20Support").
 *  `decodeURI`, not `decodeURIComponent`: an encoded `%2F` is not a separator. */
function decodePath(raw: string): string {
  if (!raw.includes("%")) return raw;
  try {
    return decodeURI(raw);
  } catch {
    return raw;
  }
}

export function libraryPath(raw: string | null | undefined): string | null {
  if (!raw) return null;
  const path = decodePath(raw.trim()).replace(LINE_SUFFIX, "");
  const m = ABSOLUTE_PATH.exec(path) ?? LIBRARY_PATH.exec(path);
  return m ? m[1] : null;
}

/** Folder under `courses/<CODE>/` to category. Must agree with
 *  `category_from_path` in `app/src-tauri/src/paths.rs`, which sets a row's
 *  `category`. */
const CATEGORY_FOLDERS: Record<string, string> = {
  "pages/": "page",
  "assignments/": "assignment",
  "quizzes/": "quiz",
  "announcements/": "announcement",
  "ed/": "ed",
  "files/": "file",
  "modules/": "module",
  "images/": "image",
  "documents/": "document",
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

/** Prose, a fenced library path, or an attached picture (`image.path` is
 *  already `<img>`-loadable; see `app/src/lib/attachments.ts`). */
export type TextPart =
  | { kind: "text"; text: string }
  | { kind: "path"; path: string }
  | { kind: "image"; path: string; raw: string };

const FENCED = /`([^`\n]+)`/g;

/** Splits text into prose and backtick-fenced library paths — the one rule
 *  composer and bubble share. Only a fence that is *wholly* a path counts;
 *  any other fence stays prose, backticks and all. */
export function splitLibraryPaths(text: string): TextPart[] {
  const parts: TextPart[] = [];
  let at = 0;
  for (const m of text.matchAll(FENCED)) {
    const picture = attachmentPath(m[1]);
    const path = picture ? null : libraryPath(m[1]);
    const i = m.index ?? 0;
    if (!picture && !path) continue;
    if (i > at) parts.push({ kind: "text", text: text.slice(at, i) });
    parts.push(
      picture ? { kind: "image", path: picture, raw: m[1] } : { kind: "path", path: path! },
    );
    at = i + m[0].length;
  }
  if (at < text.length) parts.push({ kind: "text", text: text.slice(at) });
  return parts;
}

/** Opens a file the agent named; a path with no row goes to the system
 *  viewer. `newTab` is the ⌘-click, answered here rather than through
 *  `data-tab-href` because the route is a database lookup away. */
export function openLibraryPath(path: string, newTab = false): void {
  resolveLibraryFile(path)
    .then((file) => {
      if (!file) {
        invoke("open_course_file", { relativePath: path }).catch(console.error);
        return;
      }
      const href = newTab ? filePageHref(file) : null;
      if (href) useTabStore.getState().addTab(href);
      else openFileSmart(file);
    })
    .catch(console.error);
}

/** The row behind a library path, falling back to the PDF/Office source of
 *  a parsed `.md` the agent read (`parsedMdSource`). */
async function resolveLibraryFile(path: string): Promise<DbFile | null> {
  const direct = await getFileByRelativePath(path);
  if (direct) return direct;
  const source = parsedMdSource(path);
  return source ? getFileByRelativePath(source) : null;
}
