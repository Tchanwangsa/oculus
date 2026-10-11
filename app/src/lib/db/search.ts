import { getDb } from "./connection";
import { MAX_TERMS, matchSql } from "./match";
import type { DbFile } from "./types";

/** What the palette matches against: the filename with slug separators
 *  flattened to spaces, plus the subject code, so "comp30026 workshop" is one
 *  query. */
const FILE_HAYSTACK = `replace(replace(f.filename, '-', ' '), '_', ' ') || ' ' || s.code`;
const LECTURE_HAYSTACK = `l.title || ' ' || s.code`;

/** Narrows a palette search (`in:` / `type:` in `app/src/lib/search/index.ts`). */
export interface SearchScope {
  subjectId?: number;
  /** A `files.category` (`category_from_path` in `app/src-tauri/src/library/paths/categories.rs`). */
  category?: string;
}

export interface LibraryFileHit extends DbFile {
  subject_code: string;
}

/** Enough of a `Lecture` for the palette to build its route. */
export interface LibraryLectureHit {
  id: string;
  subject_id: number;
  subject_code: string;
  title: string;
  date: string;
}

/** Files matching a palette query, best first. Unlike the `@` menu, every file
 *  — a person can read an unparsed PDF. Ties go to this term, then recency. */
export async function searchLibraryFiles(
  query: string,
  limit = 8,
  scope: SearchScope = {},
): Promise<LibraryFileHit[]> {
  const db = await getDb();
  const { where, rank, params } = matchSql(FILE_HAYSTACK, query, [
    ["f.subject_id", scope.subjectId],
    ["f.category", scope.category],
  ]);
  return db.select<LibraryFileHit[]>(
    `SELECT f.*, s.code AS subject_code
     FROM files f
     JOIN subjects s ON s.id = f.subject_id
     WHERE ${where}
     ORDER BY ${rank} DESC,
              s.is_current DESC,
              f.last_accessed_at DESC,
              f.filename ASC
     LIMIT $${params.length + 1}`,
    [...params, limit],
  );
}

/** Lectures matching a palette query — same ranking, newest capture first. */
export async function searchLibraryLectures(
  query: string,
  limit = 4,
  scope: Pick<SearchScope, "subjectId"> = {},
): Promise<LibraryLectureHit[]> {
  const db = await getDb();
  const { where, rank, params } = matchSql(LECTURE_HAYSTACK, query, [
    ["l.subject_id", scope.subjectId],
  ]);
  return db.select<LibraryLectureHit[]>(
    `SELECT l.id, l.subject_id, s.code AS subject_code, l.title, l.date
     FROM lectures l
     JOIN subjects s ON s.id = l.subject_id
     WHERE ${where}
     ORDER BY ${rank} DESC,
              s.is_current DESC,
              l.date DESC
     LIMIT $${params.length + 1}`,
    [...params, limit],
  );
}

/** One file whose page text matched: the best page and FTS5's snippet. */
export interface PageTextHit {
  file_id: number;
  subject_id: number;
  subject_code: string;
  relative_path: string;
  filename: string;
  category: string | null;
  page_no: number;
  /** The matched line, with each hit fenced by {@link SNIP_OPEN} /
   *  {@link SNIP_CLOSE}. Parsed by `snippetParts`, never rendered raw. */
  snippet: string;
}

/** The fences `snippet()` wraps a hit in — control characters, because the
 *  markdown can contain any printable delimiter. */
export const SNIP_OPEN = "\u0001";
export const SNIP_CLOSE = "\u0002";

/** Shorter prefix terms match most of the library. */
const MIN_TEXT_QUERY = 2;

/** Terms FTS5 can tokenise (a letter or digit): a token-less phrase like
 *  `"--"` is a syntax error, not an empty result. */
function ftsTerms(query: string): string[] {
  return query
    .trim()
    .split(/\s+/)
    .filter((w) => /[\p{L}\p{N}]/u.test(w))
    .slice(0, MAX_TERMS);
}

/** FTS5 MATCH for what was typed: every word required, each a prefix. Terms
 *  are quoted as FTS5 strings so `AND`/`NOT`/`-`/`:` etc. in typed text are
 *  literal, not query syntax. */
function ftsMatch(query: string): string | null {
  const words = ftsTerms(query);
  if (words.length === 0) return null;
  if (words.join("").length < MIN_TEXT_QUERY) return null;
  return words.map((w) => `"${w.replace(/"/g, '""')}"*`).join(" ");
}

/**
 * Files whose page text matches, best first — the lexical half of search, over
 * `pages_fts` (parsed documents only; see docs/retrieval.md).
 *
 * One row per file: SQLite takes the bare columns beside a single `MIN()` from
 * that row, so `page_no` and `snippet` belong to the best-scoring page. The
 * join onto `pages` also drops stale FTS entries (see `retrieval::PAGES_FTS_SQL`).
 */
export async function searchPageText(
  query: string,
  limit = 5,
  scope: SearchScope = {},
): Promise<PageTextHit[]> {
  const match = ftsMatch(query);
  if (!match) return [];
  const db = await getDb();
  // Scope placeholders follow MATCH's `$3`, in text order.
  const params: (string | number)[] = [SNIP_OPEN, SNIP_CLOSE, match];
  let scoped = "";
  for (const [column, value] of [
    ["f.subject_id", scope.subjectId],
    ["f.category", scope.category],
  ] as const) {
    if (value === undefined) continue;
    params.push(value);
    scoped += ` AND ${column} = $${params.length}`;
  }
  try {
    return await db.select<PageTextHit[]>(
      `SELECT p.file_id        AS file_id,
              f.subject_id     AS subject_id,
              s.code           AS subject_code,
              f.relative_path  AS relative_path,
              f.filename       AS filename,
              f.category       AS category,
              p.page_no        AS page_no,
              snippet(pages_fts, 0, $1, $2, '…', 14) AS snippet,
              MIN(bm25(pages_fts)) AS score
         FROM pages_fts
         JOIN pages p    ON p.id = pages_fts.rowid
         JOIN files f    ON f.id = p.file_id
         JOIN subjects s ON s.id = f.subject_id
        WHERE pages_fts MATCH $3${scoped}
        GROUP BY p.file_id
        ORDER BY score ASC, s.is_current DESC
        LIMIT $${params.length + 1}`,
      [...params, limit],
    );
  } catch (e) {
    // A malformed MATCH is the user still typing, not a broken index.
    console.warn("[oculus] page text search", e);
    return [];
  }
}
