/**
 * How an agent cites the library, as one grammar: the path spellings it
 * writes (`courses/…`, `../courses/…`, absolute, `agents/…`, `lectures/…`,
 * agent-cwd-relative), a line or page suffix, and the partial spellings
 * (course-relative, bare filename) that need a lookup. Parsing is shape-only
 * and sync, so rendering a reply costs no queries; partials resolve through
 * `resolveCitation`, cached. Parsed `.md` ↔ PDF page arithmetic lives here
 * too (`citedPage`), since the chip label and the opener both need it.
 */
import { findFilesByTail } from "@/lib/db";
import { readCourseFile } from "@/lib/courseFiles";
import { parsedMdSource } from "@/lib/fileTypes";

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

const ROOTS = "(?:courses|agents|lectures)";

/** From the filesystem root, cut at the data dir (`IDENTIFIER` in
 *  `app/src-tauri/src/paths.rs`); any `/…/courses/…` also counts, but
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
  /^[^\s/\\:]+\.(?:pdf|md|docx?|pptx?|xlsx?|txt|csv|png|jpe?g|gif|webp|svg)$/i;

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

// ── Prose ────────────────────────────────────────────────────────────────────

/** A full-shape library path sitting bare in prose: `courses/…`,
 *  `../courses/…`, or absolute (which may cross "Application Support").
 *  It must end in a file extension, plus an optional line suffix. */
const PROSE_PATH = new RegExp(
  "(?<![\\w/.`])(?:(?:\\.\\.\\/)?courses\\/[^\\s`]+|\\/(?:[^\\s/`]+\\/|Application Support\\/)+?" +
    `${ROOTS}\\/[^\\s\`]+)`,
  "g",
);
const TRAILING = /[.,;:!?)\]'"]+$/;
const ENDS_IN_FILE = /\.[A-Za-z0-9]{1,5}(?::L?\d+(?:-L?\d+)?)?$/;

type MdNode = { type: string; value?: string; children?: MdNode[] };

/** Prose text never rewritten: code and maths carry `value`, not children,
 *  so only links' text needs keeping out. */
const SKIP = new Set(["link", "linkReference", "definition", "footnoteDefinition"]);

/** Splits one text value into text and `inlineCode` nodes, or null. */
function splitProse(value: string): MdNode[] | null {
  const out: MdNode[] = [];
  let at = 0;
  for (const m of value.matchAll(PROSE_PATH)) {
    const path = m[0].replace(TRAILING, "");
    if (!ENDS_IN_FILE.test(path) || !fullCitation(path)) continue;
    const i = m.index ?? 0;
    if (i > at) out.push({ type: "text", value: value.slice(at, i) });
    out.push({ type: "inlineCode", value: path });
    at = i + path.length;
  }
  if (!out.length) return null;
  if (at < value.length) out.push({ type: "text", value: value.slice(at) });
  return out;
}

function walkProse(node: MdNode): void {
  const kids = node.children;
  if (!kids || SKIP.has(node.type)) return;
  for (let i = 0; i < kids.length; i++) {
    const kid = kids[i];
    if (kid.type === "text" && kid.value) {
      const parts = splitProse(kid.value);
      if (parts) {
        kids.splice(i, 1, ...parts);
        i += parts.length - 1;
      }
    } else {
      walkProse(kid);
    }
  }
}

/** remark plugin: bare library paths in prose become `inlineCode`, which the
 *  `code` renderer draws as a chip. Runs after remark-math, so maths is
 *  already out of the text nodes. */
export function remarkProsePaths() {
  return (tree: MdNode) => walkProse(tree);
}

// ── Resolving tails ──────────────────────────────────────────────────────────

const resolved = new Map<string, Citation | null>();
const resolving = new Map<string, Promise<Citation | null>>();

function shapeKey(s: CitationShape): string {
  return s.kind === "path" ? s.path : `tail:${s.tail}`;
}

/** A tail's file, already looked up: the citation, null for no unique hit,
 *  undefined while unknown. */
export function resolvedNow(shape: CitationShape): Citation | null | undefined {
  if (shape.kind === "path") return { path: shape.path, line: shape.line, page: shape.page };
  const hit = resolved.get(shapeKey(shape));
  return hit === undefined ? undefined : hit && { ...hit, line: shape.line, page: shape.page };
}

/** The library path a tail names, accepting only a unique hit. A parsed
 *  `.md` has no row, so its source is looked up instead — but the citation
 *  keeps the `.md` path, which its line numbers refer to. */
async function lookupTail(tail: string): Promise<string | null> {
  const direct = await findFilesByTail(tail);
  if (direct.length === 1) return direct[0].relative_path;
  if (direct.length > 1) return null;
  const source = parsedMdSource(tail);
  if (!source) return null;
  const rows = await findFilesByTail(source);
  if (rows.length !== 1) return null;
  return rows[0].relative_path.slice(0, -source.length) + tail;
}

export function resolveCitation(shape: CitationShape): Promise<Citation | null> {
  const now = resolvedNow(shape);
  if (now !== undefined) return Promise.resolve(now);
  if (shape.kind === "path") return Promise.resolve(null);
  const key = shapeKey(shape);
  let p = resolving.get(key);
  if (!p) {
    p = lookupTail(shape.tail)
      .then((path) => (path ? { path } : null))
      .catch((e) => {
        console.error(e);
        return null;
      })
      .then((c) => {
        resolved.set(key, c);
        return c;
      });
    resolving.set(key, p);
  }
  return p.then((c) => c && { ...c, line: shape.line, page: shape.page });
}

// ── Parsed markdown ↔ PDF pages ──────────────────────────────────────────────

/** A parsed `.md` with its pages: `lines` 0-based, `start` 1-based. */
interface PagedMd {
  lines: string[];
  pages: { pageNo: number; start: number; text: string }[];
}

interface PagesJson {
  pages: { page_no: number; markdown: string }[];
}

const paged = new Map<string, Promise<PagedMd | null>>();

/**
 * The parser writes the `.md` as the `.pages.json` pages' markdown joined
 * with "\n\n" (`document_markdown` in `app/src-tauri/src/parse/mod.rs`), blank
 * pages keeping their slot, so page k starts at line
 * 1 + Σ_{j<k} (lines(md_j) + 1). Cached per path; a failed read is retried.
 */
function loadPaged(mdPath: string): Promise<PagedMd | null> {
  let p = paged.get(mdPath);
  if (!p) {
    const json = mdPath.replace(/\.md$/i, ".pages.json");
    p = Promise.all([readCourseFile(mdPath), readCourseFile(json)])
      .then(([md, raw]) => {
        const pagesJson = JSON.parse(raw) as PagesJson;
        let start = 1;
        const pages = pagesJson.pages.map((pg) => {
          const at = start;
          start += pg.markdown.split("\n").length + 1;
          return { pageNo: pg.page_no, start: at, text: pg.markdown };
        });
        return { lines: md.split("\n"), pages };
      })
      .catch(() => {
        // Not parsed yet, perhaps: the next ask reads again.
        paged.delete(mdPath);
        return null;
      });
    paged.set(mdPath, p);
  }
  return p;
}

const pageMaps = new WeakMap<PagedMd, Map<number, string>>();

/** A parsed `.md`'s pages, `page_no` (1-based) → that page's markdown, from
 *  the same cached read as `citedPage`; null when the file is not parsed. */
export async function loadParsedPages(mdPath: string): Promise<Map<number, string> | null> {
  const doc = await loadPaged(mdPath);
  if (!doc) return null;
  let map = pageMaps.get(doc);
  if (!map) {
    map = new Map(doc.pages.map((p) => [p.pageNo, p.text]));
    pageMaps.set(doc, map);
  }
  return map;
}

/** Letters and digits only, lowercased, ligatures and accents folded — the
 *  form a quote and a PDF's text layer are compared in. */
export function normalizeText(s: string): string {
  return s.normalize("NFKD").toLowerCase().replace(/[^\p{L}\p{N}]+/gu, "");
}

/** One markdown line as the reader sees it: no heading, list, emphasis,
 *  code, maths, table, image, link or HTML syntax. */
function plainLine(line: string): string {
  return line
    .replace(/<[^>]+>/g, " ")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, " ")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\$\$[^$]*\$\$|\$[^$\n]*\$/g, " ")
    .replace(/^\s*[|:\-\s]+$/, "")
    .replace(/^\s{0,3}(?:#{1,6}\s+|>\s?)/, "")
    .replace(/^\s*(?:[-*+•]|\d+[.)])\s+/, "")
    .replace(/[|*_`~]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

/** The cited lines' text for highlighting, at most three lines. */
export function quoteFromLines(lines: string[], range: LineRange): string {
  const to = Math.min(range.to, range.from + 2, lines.length);
  return lines
    .slice(range.from - 1, to)
    .map(plainLine)
    .filter(Boolean)
    .join(" ");
}

/** The PDF page a line of a parsed `.md` falls on, checked against the
 *  page's text and searched for when it disagrees; null if unknowable. */
function pageOfLine(doc: PagedMd, range: LineRange): { page: number; quote: string } | null {
  if (range.from < 1 || range.from > doc.lines.length || !doc.pages.length) return null;
  const quote = quoteFromLines(doc.lines, range);
  let k = doc.pages.length - 1;
  while (k > 0 && doc.pages[k].start > range.from) k--;
  const needle = normalizeText(quote);
  if (needle && !normalizeText(doc.pages[k].text).includes(needle)) {
    const found = doc.pages.find((p) => normalizeText(p.text).includes(needle));
    if (found) return { page: found.pageNo, quote };
  }
  return { page: doc.pages[k].pageNo, quote };
}

/** Page and quote for a line of a parsed `.md`. */
export async function citedPage(
  mdPath: string,
  range: LineRange,
): Promise<{ page: number; quote: string } | null> {
  const doc = await loadPaged(mdPath);
  return doc ? pageOfLine(doc, range) : null;
}
