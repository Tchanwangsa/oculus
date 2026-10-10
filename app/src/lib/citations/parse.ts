import { parsedMdSource } from "@/lib/files/fileTypes";

/** 1-based, inclusive. */
export interface LineRange {
  from: number;
  to: number;
}

/** Where in the file a citation points. A page is 1-based, in the PDF. */
export interface CiteLocation {
  line?: LineRange;
  page?: number;
}

/** A citation whose file is known: `path` is data-dir-relative, under
 *  `courses/`, `agents/` or `lectures/`. */
export interface Citation extends CiteLocation {
  path: string;
}

/** A parsed spelling: a full `path`, or a `tail` (course-relative path or
 *  bare filename) that `resolveCitation` must look up. */
export type CitationShape =
  | ({ kind: "path"; path: string } & CiteLocation)
  | ({ kind: "tail"; tail: string } & CiteLocation);

/** `:97`, `:97-120`, `:L97`, `:L97-L120`, `#L97`, `#L97-L120`, `#L97-120`,
 *  `#page=12`. */
const SUFFIX = /(?::L?(\d+)(?:-L?(\d+))?|#L(\d+)(?:-L?(\d+))?|#page=(\d+))$/;

export const ROOTS = "(?:courses|agents|lectures)";

/** From the filesystem root, cut at the data dir (`IDENTIFIER` in
 *  `app/src-tauri/src/library/paths/mod.rs`); any `/…/courses/…` also counts, but
 *  `agents/` and `lectures/` are common names elsewhere. May hold spaces
 *  ("Application Support"), so `COMMAND` keeps a command a command. */
const ABSOLUTE = new RegExp(`^\\/.*?\\/com\\.tchan\\.oculus\\/(${ROOTS}\\/.+)$`);
const ABSOLUTE_COURSES = /^\/.*?\/(courses\/.+)$/;
const COMMAND = /\s[-/]|[|&;<>$`*]/;

/** Data-dir-relative, or `../` from `agents/`, where every thread runs. */
const RELATIVE = new RegExp(`^(?:\\.\\.\\/)?(${ROOTS}\\/\\S+)$`);

/** Relative to the agent's cwd (`agents/`). */
const AGENT_CWD = /^(?:\.\/(\S+)|((?:attachments|outputs)\/\S+))$/;

/** A course-relative path: one of `CATEGORY_FOLDERS` (`openFile.ts`) or the
 *  student's `uploads/`, ending in an extension. */
const COURSE_RELATIVE =
  /^(?:pages|assignments|quizzes|announcements|ed|files|modules|images|documents|uploads)\/\S+\.[A-Za-z0-9]+$/;

/** A bare filename with a document or picture extension. */
const BARE_NAME =
  /^[^\s/\\:]+\.(?:pdf|md|docx?|pptx?|xls[xm]?|ods|txt|csv|png|jpe?g|gif|webp|svg)$/i;

const SCHEME = /^[a-z][a-z0-9+.-]*:/i;

/** A page named in a link's text, for an href that carries none. */
const TEXT_PAGE = /\b(?:pdf\s+)?(?:page|p\.)\s*(\d+)\b/i;

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

function location(m: RegExpExecArray | null): CiteLocation {
  if (!m) return {};
  if (m[5]) return { page: Number(m[5]) };
  const from = Number(m[1] ?? m[3]);
  const to = Number(m[2] ?? m[4] ?? from);
  return { line: { from, to: Math.max(from, to) } };
}

/**
 * The citation a string spells, or null. `linkText` is a link's visible text,
 * read for a page only when the href has no location. `tails` off keeps to
 * full paths (for callers that cannot wait on a lookup).
 */
export function parseCitation(
  raw: string | null | undefined,
  { linkText, tails = true }: { linkText?: string; tails?: boolean } = {},
): CitationShape | null {
  if (!raw) return null;
  let s = decodePath(raw.trim());
  if (s.startsWith("file://")) s = s.slice("file://".length);
  const suffix = SUFFIX.exec(s);
  const loc = location(suffix);
  if (suffix) s = s.slice(0, suffix.index);
  if (!s || SCHEME.test(s)) return null;
  if (!loc.page && !loc.line && linkText) {
    const p = TEXT_PAGE.exec(linkText);
    if (p) loc.page = Number(p[1]);
  }

  if (s.startsWith("/")) {
    const m = ABSOLUTE.exec(s) ?? ABSOLUTE_COURSES.exec(s);
    return m && !COMMAND.test(s) ? { kind: "path", path: m[1], ...loc } : null;
  }
  if (/\s/.test(s)) return null;
  const rel = RELATIVE.exec(s);
  if (rel) return { kind: "path", path: rel[1], ...loc };
  const cwd = AGENT_CWD.exec(s);
  if (cwd) return { kind: "path", path: `agents/${cwd[1] ?? cwd[2]}`, ...loc };
  if (tails && (COURSE_RELATIVE.test(s) || BARE_NAME.test(s)))
    return { kind: "tail", tail: s, ...loc };
  return null;
}

/** A citation with its file known from the shape alone, or null. */
export function fullCitation(raw: string | null | undefined): Citation | null {
  const c = parseCitation(raw, { tails: false });
  return c?.kind === "path" ? { path: c.path, line: c.line, page: c.page } : null;
}

/** The citation as one string, `path` plus `:97-120` or `#page=12` — what a
 *  chip copies as. */
export function citationText(c: Citation): string {
  if (c.line) {
    const { from, to } = c.line;
    return `${c.path}:${from}${to !== from ? `-${to}` : ""}`;
  }
  return c.page ? `${c.path}#page=${c.page}` : c.path;
}

/** A parse artifact the student knows by its source: a `.md` under a course's
 *  `files/` or `uploads/` → the PDF or Office file it came from. */
export function parsedSourceOf(path: string): string | null {
  if (!/^courses\/[^/]+\/(?:files|uploads)\//.test(path)) return null;
  return parsedMdSource(path);
}
