import { invoke } from "@tauri-apps/api/core";
import { useTabStore } from "@/stores/shell/tabStore";
import { openBeside } from "@/lib/shell/tabRouters";
import { humanizeSlug } from "@/lib/format/format";
import { isPdfBacked, isSheetFile, isVideoFile, parsedMdRelPath, parsedMdSource } from "@/lib/files/fileTypes";
import { getFileByRelativePath, getLecture, markFileAccessed, type DbFile } from "@/lib/db";
import { attachmentPath } from "@/lib/harness/attachments";
import { isWebUrl, openExternal } from "@/lib/browser";
import { libraryLinkTarget } from "@/lib/files/libraryLinks";
import { readCourseFile } from "@/lib/files/courseFiles";
import { lecturePagePath } from "@/lib/lectures";
import { citedPage, fullCitation, quoteFromLines, type Citation } from "@/lib/citations";

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

export function recordFileAccess(file: Pick<DbFile, "id"> & Partial<Pick<DbFile, "subject_id">>): void {
  markFileAccessed(file.id)
    .then(() => window.dispatchEvent(new CustomEvent(FILE_ACCESSED_EVENT, {
      detail: { subjectId: file.subject_id },
    })))
    .catch(console.error);
}

/** A spot in the file a citation points at: a PDF page (and the passage to
 *  highlight on it, or the parse's blocks on it to box), or for markdown just
 *  the passage. `seq` is fresh per click, so citing the same spot again
 *  re-jumps. Rides in the file page's router state (`{ locate }`), not its
 *  URL: quotes are long. */
export interface FileLocate {
  page?: number;
  quote?: string;
  /** Indices into the page's `.pages.json` blocks (`PdfBlock`). */
  blocks?: number[];
  seq: number;
}

/** The `locate` a file page was opened at, if its router state holds one. */
export function routeLocate(state: unknown): FileLocate | undefined {
  const l = (state as { locate?: Partial<Record<keyof FileLocate, unknown>> } | null)?.locate;
  if (!l || typeof l !== "object" || typeof l.seq !== "number") return undefined;
  return {
    page: typeof l.page === "number" ? l.page : undefined,
    quote: typeof l.quote === "string" ? l.quote : undefined,
    blocks:
      Array.isArray(l.blocks) && l.blocks.every((b) => Number.isInteger(b)) ? (l.blocks as number[]) : undefined,
    seq: l.seq,
  };
}

/** Opens a PDF's page with its parse details showing: a file row's failed
 *  parse icon. The value is fresh per click, so a second click re-opens them. */
export function openFileParseDetails(file: DbFile): void {
  recordFileAccess(file);
  openBeside(filePagePath(file.subject_id, file.relative_path), { parseDetails: Date.now() });
}

/** The `parseDetails` a file page was opened with, if any. */
export function routeParseDetails(state: unknown): number | undefined {
  const v = (state as { parseDetails?: unknown } | null)?.parseDetails;
  return typeof v === "number" ? v : undefined;
}

/** A binary the app has no viewer for: it opens in the system viewer. PDFs,
 *  Office documents (via their converted PDF), spreadsheets (as their text)
 *  and videos render in-app. */
function systemOnly(file: DbFile): boolean {
  return !isPdfBacked(file.filename) && !isSheetFile(file.filename) && !isVideoFile(file.filename);
}

/** How any list row opens a file: anything renderable as its page in the
 *  side panel (`openBeside`), at `locate` if given, other binaries in the
 *  system viewer. */
export function openFileSmart(file: DbFile, locate?: FileLocate): void {
  recordFileAccess(file);
  const binary = file.category === "file" || file.category === "upload";
  if (binary && systemOnly(file)) {
    invoke("open_course_file", { relativePath: file.relative_path }).catch(
      console.error,
    );
    return;
  }
  openBeside(
    filePagePath(file.subject_id, file.relative_path),
    locate ? { locate } : undefined,
  );
}

export function filePagePath(subjectId: number, relativePath: string): string {
  return `/subjects/${subjectId}/file?path=${encodeURIComponent(relativePath)}`;
}

/** A row's `data-tab-href` (`lib/shell/newTabClicks.ts`): its ⌘-click route, or
 *  null for a binary that only opens in the system viewer. */
export function filePageHref(file: DbFile): string | null {
  if (file.category === "file" && systemOnly(file)) return null;
  return filePagePath(file.subject_id, file.relative_path);
}

/** A full-shape library path (`citations/index.ts`) without its location: what a
 *  tool row or image needs. Shape-only, so rendering costs no queries. */
export function libraryPath(raw: string | null | undefined): string | null {
  return fullCitation(raw)?.path ?? null;
}

/** Folder under `courses/<CODE>/` to category. Must agree with
 *  `category_from_path` in `app/src-tauri/src/library/paths/categories.rs`, which sets a row's
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
 *  already `<img>`-loadable; see `app/src/lib/harness/attachments.ts`). */
export type TextPart =
  | { kind: "text"; text: string }
  | { kind: "path"; path: string; cite: Citation }
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
    const cite = picture ? null : fullCitation(m[1]);
    const i = m.index ?? 0;
    if (!picture && !cite) continue;
    if (i > at) parts.push({ kind: "text", text: text.slice(at, i) });
    parts.push(
      picture
        ? { kind: "image", path: picture, raw: m[1] }
        : { kind: "path", path: cite!.path, cite: cite! },
    );
    at = i + m[0].length;
  }
  if (at < text.length) parts.push({ kind: "text", text: text.slice(at) });
  return parts;
}

/** Opens a file the agent named; see `openCitation`. */
export function openLibraryPath(path: string, newTab = false): void {
  openCitation({ path }, newTab);
}

/** A link in a note: a library file (the same resolution `FileViewer` uses)
 *  or library path opens in the side panel, a web URL in the in-app browser. */
export function openNoteLink(href: string, files: DbFile[]): void {
  const target = libraryLinkTarget(href, files);
  if (target) {
    openFileSmart(target);
    return;
  }
  const lib = libraryPath(href);
  if (lib) {
    openLibraryPath(lib);
    return;
  }
  const url = /^www\./i.test(href) ? `https://${href}` : href;
  if (isWebUrl(url)) void openExternal(url);
}

let locateSeq = 0;

/**
 * Opens a citation. A course file opens in the side panel at the cited spot
 * (`citationLocate`); a path with no row, an `agents/` file, or a binary goes
 * to the system viewer; `lectures/<id>/…` opens the lecture. `newTab` is the
 * ⌘-click, answered here rather than through `data-tab-href` because the
 * route is a database lookup away. Pictures under `agents/` are the caller's
 * to show (the lightbox in `components/markdown/Citation.tsx`).
 */
export function openCitation(cite: Citation, newTab = false): void {
  openCitationAsync(cite, newTab).catch(console.error);
}

async function openCitationAsync(cite: Citation, newTab: boolean): Promise<void> {
  const system = () => invoke<void>("open_course_file", { relativePath: cite.path });
  const lectureId = /^lectures\/([^/]+)\//.exec(cite.path)?.[1];
  if (lectureId) {
    const lecture = await getLecture(lectureId);
    if (!lecture) return system();
    if (newTab) useTabStore.getState().addTab(lecturePagePath(lecture));
    else openBeside(lecturePagePath(lecture));
    return;
  }
  if (!cite.path.startsWith("courses/")) return system();
  const file = await resolveLibraryFile(cite.path);
  if (!file) return system();
  const locate = await citationLocate(cite, file);
  const href = newTab ? filePageHref(file) : null;
  if (href) {
    useTabStore.getState().addTab(href, locate ? { locate } : undefined);
    return;
  }
  openFileSmart(file, locate);
}

/** Where in `file` the citation points. A line of a parsed `.md` becomes its
 *  PDF page, the line's text and the parse blocks it covers; a `#page=` on
 *  anything PDF-backed is just the page; a line of a plain markdown file, or
 *  of a spreadsheet's text, is just its text. */
async function citationLocate(cite: Citation, file: DbFile): Promise<FileLocate | undefined> {
  const seq = ++locateSeq;
  const pdf = isPdfBacked(file.filename);
  if (pdf && cite.line && cite.path === parsedMdRelPath(file)) {
    const hit = await citedPage(cite.path, cite.line);
    if (hit)
      return {
        page: hit.page,
        quote: hit.quote || undefined,
        blocks: hit.blocks.length ? hit.blocks : undefined,
        seq,
      };
  }
  if (pdf && cite.page) return { page: cite.page, seq };
  const textPath = isSheetFile(file.filename) ? parsedMdRelPath(file) : file.relative_path;
  if (!pdf && cite.line && textPath && /\.md$/i.test(textPath)) {
    const text = await readCourseFile(textPath).catch(() => "");
    const quote = quoteFromLines(text.split("\n"), cite.line);
    if (quote) return { quote, seq };
  }
  return undefined;
}

/** The row behind a library path, falling back to the PDF/Office source of
 *  a parsed `.md` the agent read (`parsedMdSource`). */
async function resolveLibraryFile(path: string): Promise<DbFile | null> {
  const direct = await getFileByRelativePath(path);
  if (direct) return direct;
  const source = parsedMdSource(path);
  return source ? getFileByRelativePath(source) : null;
}
