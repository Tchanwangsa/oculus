/** The library's path shapes, for the frontend: the one TypeScript file that
 *  spells its folder names. Mirrors `app/src-tauri/src/library/layout/`, which
 *  writes the paths; `app/tests/fixtures/library/categories.json` holds both
 *  to the same answers.
 *
 *  A subject is `courses/<code>/`: `Course/` holds everything the sync writes
 *  (recordings included), the student's own files sit beside it at any depth
 *  and take their kind from their extension, and `.oculus/` is app state. */

/** The data-dir folder holding one folder per subject. */
export const COURSES_DIR = "courses";

/** The folder at a subject's root holding everything synced. */
export const COURSE_DIR = "Course";

/** The hidden folder at a subject's root holding app state. */
export const HIDDEN_DIR = ".oculus";

/** Where a PDF's parse record and page vectors go, under the subject root,
 *  mirroring the PDF's own folder. */
const DERIVED_DIR = `${HIDDEN_DIR}/derived`;

/** Subject-relative synced folders. */
export const FILES_DIR = `${COURSE_DIR}/Files`;
export const LESSONS_DIR = `${COURSE_DIR}/Lessons`;
export const PAGES_DIR = `${COURSE_DIR}/Pages`;
export const MODULES_DIR = `${COURSE_DIR}/Modules`;
export const RECORDINGS_DIR = `${COURSE_DIR}/Recordings`;

/** Synced folder (under `Course/`) to `files.category` (`category_of` in Rust). */
const SYNCED_CATEGORIES = new Map<string, string>(Object.entries({
  Files: "file",
  Media: "file",
  Lessons: "lessons",
  Discussion: "ed",
  Pages: "page",
  Assignments: "assignment",
  Quizzes: "quiz",
  Announcements: "announcement",
  Modules: "module",
  Images: "image",
}));

/** The folder names directly under `Course/`, synced by kind. */
export const SYNCED_DIR_NAMES = [...SYNCED_CATEGORIES.keys()];

/** The synced single files directly under `Course/`. */
const SYNCED_FILES = new Map([
  ["home.md", "home"],
  ["syllabus.md", "syllabus"],
]);

/** The student's own kinds, by extension; anything else is an upload. */
const OWN_EXTENSIONS: [extension: string, category: string][] = [
  [".md", "document"],
  [".canvas", "canvas"],
  [".notebook", "notebook"],
];

const COURSE_PREFIX = new RegExp(`^${COURSES_DIR}/[^/]+/`);

const sameName = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

/** True for a path inside some subject's folder. */
export const isCoursePath = (path: string): boolean => path.startsWith(`${COURSES_DIR}/`);

/** The path inside its subject (`courses/X/Course/Files/a.pdf` →
 *  `Course/Files/a.pdf`); anything else as it is. */
export const courseRelOf = (path: string): string => path.replace(COURSE_PREFIX, "");

/** `files.category` of a subject-relative path: inside `Course/` by its
 *  folder, beside it by its extension. */
export function categoryOf(inner: string): string {
  const slash = inner.indexOf("/");
  const first = slash < 0 ? inner : inner.slice(0, slash);
  if (sameName(first, COURSE_DIR)) {
    if (slash < 0) return "other";
    const rest = inner.slice(slash + 1);
    const file = SYNCED_FILES.get(rest);
    if (file) return file;
    const cut = rest.indexOf("/");
    return cut < 0 ? "other" : SYNCED_CATEGORIES.get(rest.slice(0, cut)) ?? "other";
  }
  if (sameName(first, HIDDEN_DIR)) return "other";
  return OWN_EXTENSIONS.find(([ext]) => inner.endsWith(ext))?.[1] ?? "upload";
}

/** True when a path inside a subject sits in `folder` (a subject-relative
 *  synced folder: `LESSONS_DIR`, `FILES_DIR`…). */
export const inFolder = (path: string, folder: string): boolean => courseRelOf(path).startsWith(`${folder}/`);

/** True for a data-dir-relative path among a subject's own files: inside a
 *  subject, outside `Course/` and `.oculus/`. */
export function isOwnPath(path: string): boolean {
  if (!COURSE_PREFIX.test(path)) return false;
  const first = courseRelOf(path).split("/")[0];
  return !sameName(first, COURSE_DIR) && !sameName(first, HIDDEN_DIR);
}

/** The recording folder (`courses/<code>/Course/Recordings/<date>-<slug>`) a
 *  path is in or is, as a lecture row's `folder_rel` stores it; null for any
 *  other path. */
export function recordingOf(path: string): string | null {
  const m = new RegExp(`^(${COURSES_DIR}/[^/]+/${RECORDINGS_DIR}/[^/#?]+)`, "i").exec(path);
  return m && !m[1].split("/").pop()!.startsWith(".") ? m[1] : null;
}

/** A PDF's parse record (`<stem>.pages.json` under the subject's
 *  `.oculus/derived/`), from its parsed markdown (`<stem>.md`, beside the
 *  PDF); `layout::derived` in Rust. Outside a subject it sits beside. */
export function pagesRecordOf(mdPath: string): string {
  const record = mdPath.replace(/\.md$/i, ".pages.json");
  const m = new RegExp(`^(${COURSES_DIR}/[^/]+)/(.+)$`).exec(record);
  return m ? `${m[1]}/${DERIVED_DIR}/${m[2]}` : record;
}

/** A PDF's parsed markdown (`<stem>.md`), from the PDF's path. */
export const parsedMdOf = (pdfPath: string): string => pdfPath.replace(/\.pdf$/i, ".md");
