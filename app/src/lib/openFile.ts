import { invoke } from "@tauri-apps/api/core";
import { useSidePanelStore } from "@/stores/sidePanelStore";
import { useTabStore } from "@/stores/tabStore";
import { humanizeSlug } from "@/lib/format";
import { isPdfBacked, parsedMdSource } from "@/lib/fileTypes";
import { getFileByRelativePath, markFileAccessed, type DbFile } from "@/lib/db";
import { attachmentPath } from "@/lib/attachments";

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

/** The full-page route for a file — the side panel's peek, tab-sized. */
export function filePagePath(subjectId: number, relativePath: string): string {
  return `/subjects/${subjectId}/file?path=${encodeURIComponent(relativePath)}`;
}

/**
 * Where a row that opens `file` leads when it is ⌘-clicked, or null for a file
 * that has no page to lead to — the binaries `openFileSmart` hands to the
 * system viewer, which have no route and no tab.
 *
 * This is what a list row puts in `data-tab-href` (`lib/newTabClicks.ts`). A
 * row cannot use an `href` for it: the plain click opens the side panel beside
 * the page you are on, and only the ⌘-click is a navigation at all.
 */
export function filePageHref(file: DbFile): string | null {
  if (file.category === "file" && !isPdfBacked(file.filename)) return null;
  return filePagePath(file.subject_id, file.relative_path);
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

/**
 * The same path written from the filesystem root, which is the form a CLI
 * agent's own tools hand it and therefore the form it writes into a markdown
 * link: `/…/com.tchan.oculus/courses/<subject>/…`. Left to the anchor, such a
 * link is resolved against the dev server's origin and 404s.
 *
 * Anchored on a leading `/` so that a command which merely *contains* a
 * library path (`cat ../courses/…`) stays a command, and lazy up to the first
 * `courses/` so the capture is the library path and not a suffix of it. The
 * root itself is not checked: an absolute path with `courses/` inside it, in a
 * thread that can only reach the library, is that library's.
 */
const ABSOLUTE_PATH = /^\/.*?\/(courses\/.+)$/;

/** A `:97` or `:97-120` tail — how an agent cites the line it read. Nothing
 *  downstream has a line anchor to honour, so it is trimmed rather than left
 *  to make the path unmatchable. */
const LINE_SUFFIX = /:\d+(?:-\d+)?$/;

/** micromark percent-encodes a link destination on its way to `href`, so the
 *  data directory's "Application Support" arrives as "Application%20Support".
 *  `decodeURI` and not `decodeURIComponent`: a literal `/` that was written
 *  `%2F` is not a separator. */
function decodePath(raw: string): string {
  if (!raw.includes("%")) return raw;
  try {
    return decodeURI(raw);
  } catch {
    return raw;
  }
}

/** The library path inside an agent's tool argument or link, if that is what
 *  it is. */
export function libraryPath(raw: string | null | undefined): string | null {
  if (!raw) return null;
  const path = decodePath(raw.trim()).replace(LINE_SUFFIX, "");
  const m = ABSOLUTE_PATH.exec(path) ?? LIBRARY_PATH.exec(path);
  return m ? m[1] : null;
}

/** Folder under `courses/<CODE>/` to category. Mirrors `category_from_path`
 *  in `app/src-tauri/src/paths.rs`, which is where a row's `category` column
 *  comes from in the first place — the two tables have to agree, or a file
 *  drawn from a path wears a different glyph from the same file drawn from
 *  its row. */
const CATEGORY_FOLDERS: Record<string, string> = {
  "pages/": "page",
  "assignments/": "assignment",
  "quizzes/": "quiz",
  "announcements/": "announcement",
  "ed/": "ed",
  "files/": "file",
  "modules/": "module",
  "images/": "image",
};

/**
 * What a library path says about itself: its last segment is the filename,
 * and the folder it sits in is the category — which is everything
 * `categoryIconFor` and `fileTitle` ask for.
 *
 * Derived rather than looked up, for the same reason `libraryPath` matches on
 * shape: a message with a mention in it, or a timeline of a hundred of them,
 * costs no queries. The row is only read when one is clicked.
 */
export function pathFile(path: string): Pick<DbFile, "category" | "filename"> {
  const rel = path.replace(/^courses\/[^/]+\//, "");
  return { category: categoryFromPath(rel), filename: rel.slice(rel.lastIndexOf("/") + 1) };
}

/** The category of a path already relative to its subject folder, which is
 *  the form the Rust table above matches on. */
function categoryFromPath(rel: string): string {
  if (rel === "home.md" || rel === "syllabus.md") return rel.slice(0, -3);
  const folder = Object.keys(CATEGORY_FOLDERS).find((f) => rel.startsWith(f));
  return folder ? CATEGORY_FOLDERS[folder] : "other";
}

/** A run of prose, a library path that was fenced inside it, or a picture the
 *  student attached — `agents/attachments/…`, already in the form an `<img>`
 *  loads it by (`app/src/lib/attachments.ts`). */
export type TextPart =
  | { kind: "text"; text: string }
  | { kind: "path"; path: string }
  | { kind: "image"; path: string; raw: string };

/** A backtick-fenced run: how the composer writes a mention, and how an agent
 *  writes a path when it quotes one back. */
const FENCED = /`([^`\n]+)`/g;

/**
 * Text split into its prose and the library paths fenced in it — the one
 * matcher every reader of a message uses, so the chip a composer wrote and
 * the chip a bubble draws are decided by the same rule.
 *
 * Only a fence whose *whole* content is a library path counts. A backticked
 * command, flag or snippet is prose that happens to be fenced, and it comes
 * back as the text it was, backticks and all.
 */
export function splitLibraryPaths(text: string): TextPart[] {
  const parts: TextPart[] = [];
  let at = 0;
  for (const m of text.matchAll(FENCED)) {
    // An attachment is checked first: it is a path in this library too, but
    // not one under `courses/`, and it is drawn rather than chipped.
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

/**
 * Opens a file the agent named. Falls back to the system viewer for anything
 * the library knows nothing about — a path under `courses/` that has no row is
 * a file on disk that never made it through a sync, and handing it to the OS
 * is better than a click that does nothing.
 *
 * The one ⌘-click that cannot go through `data-tab-href`
 * (`lib/newTabClicks.ts`): a chip in a thread carries a library path, not a
 * route, and which route it stands for is a database lookup away. So the
 * modifier is passed in and answered here, once the row is in hand — and a
 * path that resolves to no page, or to no row at all, opens the way it always
 * did rather than in an empty tab.
 */
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

/** The row behind a library path, looking through the parser's markdown to the
 *  file it came from when the path has no row of its own — an agent cites the
 *  `.md` it actually read, and the library only knows the PDF or Office
 *  document under it ([`parsedMdSource`]). */
async function resolveLibraryFile(path: string): Promise<DbFile | null> {
  const direct = await getFileByRelativePath(path);
  if (direct) return direct;
  const source = parsedMdSource(path);
  return source ? getFileByRelativePath(source) : null;
}
