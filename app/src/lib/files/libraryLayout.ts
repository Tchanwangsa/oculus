/** The library's path shapes, for the frontend: the one TypeScript file that
 *  spells its folder names. Mirrors `app/src-tauri/src/library/layout/`, which
 *  writes the paths; `app/tests/fixtures/library/categories.json` holds both
 *  to the same answers. */

/** The data-dir folder holding one folder per subject. */
export const COURSES_DIR = "courses";

/** The data-dir folder holding one folder per lecture recording. */
export const RECORDINGS_DIR = "lectures";

/** A subject's folder each own kind is created in. */
export const OWN_DIRS = {
  upload: "uploads",
  document: "documents",
  canvas: "canvases",
  notebook: "notebooks",
} as const;

/** The synced folder holding Ed lessons. */
export const LESSONS_DIR = "lessons";

/** The synced folder holding Canvas files. */
export const FILES_DIR = "files";

/** The synced folder holding Canvas pages. */
export const PAGES_DIR = "pages";

/** Subject-relative folder to `files.category` (`category_of` in Rust). */
const CATEGORY_FOLDERS = new Map<string, string>(Object.entries({
  [PAGES_DIR]: "page",
  assignments: "assignment",
  quizzes: "quiz",
  announcements: "announcement",
  ed: "ed",
  [LESSONS_DIR]: "lessons",
  [FILES_DIR]: "file",
  modules: "module",
  images: "image",
  [OWN_DIRS.document]: "document",
  [OWN_DIRS.canvas]: "canvas",
  [OWN_DIRS.notebook]: "notebook",
  [OWN_DIRS.upload]: "upload",
}));

/** The synced single files at a subject's root. */
const SYNCED_FILES = new Map([
  ["home.md", "home"],
  ["syllabus.md", "syllabus"],
]);

const COURSE_PREFIX = new RegExp(`^${COURSES_DIR}/[^/]+/`);

/** True for a path inside some subject's folder. */
export const isCoursePath = (path: string): boolean => path.startsWith(`${COURSES_DIR}/`);

/** The path inside its subject (`courses/X/files/a.pdf` → `files/a.pdf`);
 *  anything else as it is. */
export const courseRelOf = (path: string): string => path.replace(COURSE_PREFIX, "");

/** `files.category` of a subject-relative path. */
export function categoryOf(inner: string): string {
  const file = SYNCED_FILES.get(inner);
  if (file) return file;
  const slash = inner.indexOf("/");
  if (slash < 0) return "other";
  return CATEGORY_FOLDERS.get(inner.slice(0, slash)) ?? "other";
}

/** True when a subject-relative path sits in `folder` (one of `OWN_DIRS`,
 *  `LESSONS_DIR`…). */
export const inFolder = (path: string, folder: string): boolean => courseRelOf(path).startsWith(`${folder}/`);

/** `lectures/<id>`, a recording's data-dir-relative folder. */
export const recordingRel = (id: string): string => `${RECORDINGS_DIR}/${id}`;

/** `^lectures/` as a regex source, for matchers that need the id's own
 *  stopping rule. */
export const RECORDINGS_PREFIX = `^${RECORDINGS_DIR}\\/`;

/** A PDF's parse record (`<stem>.pages.json`), from its parsed markdown
 *  (`<stem>.md`); `layout::derived` in Rust. */
export const pagesRecordOf = (mdPath: string): string => mdPath.replace(/\.md$/i, ".pages.json");

/** A PDF's parsed markdown (`<stem>.md`), from the PDF's path. */
export const parsedMdOf = (pdfPath: string): string => pdfPath.replace(/\.pdf$/i, ".md");
