import { getDb } from "./connection";
import { terms } from "./match";
import type { DbFile } from "./types";
import { PDF_BACKED_SQL_LIST } from "@/lib/files/fileTypes";
import { COURSE_DIR, COURSES_DIR } from "@/lib/files/libraryLayout";

/** A file the chat composer's `@` can point the agent at. */
export interface MentionFile {
  id: number;
  subject_id: number;
  subject_code: string;
  filename: string;
  /** Library-root path (`courses/<subject>/…`), as `oculus read` takes it. */
  relative_path: string;
  category: string | null;
}

/**
 * The `@` query's words as AND-ed `LIKE` predicates over `f.filename` (any
 * order), with params bound from `$from` on. Shared by the menu and its
 * "no markdown yet" count so both agree on what matches.
 *
 * Callers must number placeholders in the order the statement's *text*
 * mentions them: SQLite treats `$1` as a name and assigns indices by first
 * appearance, so an out-of-order placeholder binds a neighbour's value.
 */
function mentionMatch(
  query: string,
  from: number,
  column = "f.filename",
): { where: string; params: string[]; prefix: string } {
  const words = terms(query);
  return {
    where: words.length
      ? words.map((_, i) => `${column} LIKE $${from + i} ESCAPE '\\'`).join(" AND ")
      : "1",
    params: words.map((w) => `%${w}%`),
    prefix: `${words[0] ?? ""}%`,
  };
}

/**
 * Candidates for an `@` mention, narrowed to the chat's subject when it has
 * one. Only files the agent can read: `.md`, or a parsed document
 * (`parse_status = 'quality'` is the finished-parse marker). Ranked by a
 * prefix match on the first word, then recency.
 */
export async function searchMentionFiles(
  subjectId: number | null,
  query: string,
  limit = 8,
): Promise<MentionFile[]> {
  const db = await getDb();
  const { where, params, prefix } = mentionMatch(query, 2);
  return db.select<MentionFile[]>(
    `SELECT f.id, f.subject_id, s.code AS subject_code, f.filename,
            f.relative_path, f.category
     FROM files f
     JOIN subjects s ON s.id = f.subject_id
     WHERE (f.file_type = 'md' OR f.parse_status = 'quality')
       AND ($1 IS NULL OR f.subject_id = $1)
       AND (${where})
     ORDER BY (f.filename LIKE $${params.length + 2} ESCAPE '\\') DESC,
              f.last_accessed_at DESC,
              f.filename ASC
     LIMIT $${params.length + 3}`,
    [subjectId, ...params, prefix, limit],
  );
}

/** A folder the chat composer's `@` can point the agent at; the agent lists it
 *  itself. */
export interface MentionFolder {
  /** Library-root path ending in `/`: `courses/<subject>/Course/Pages/`, or
   *  a whole subject's `courses/<subject>/` in a General thread. */
  path: string;
  /** Its own name: the folder's, or the subject's folder's. */
  name: string;
  subject_id: number;
  subject_code: string;
}

/**
 * The folders an `@` offers, first in its menu: a subject's top-level
 * folders, with `Course/` opened one level (`Course/Pages/`, `Course/Files/`,
 * the student's own `Exam prep/`…), or in a General thread each subject's
 * folder. Read off the files table, so a folder holding no file is
 * not offered. Most recently opened first; the query's words are ANDed
 * against the folder's name.
 */
export async function searchMentionFolders(
  subjectId: number | null,
  query: string,
  limit = 6,
): Promise<MentionFolder[]> {
  const db = await getDb();
  const { sql, params } = mentionFoldersQuery(subjectId, query, limit);
  return db.select<MentionFolder[]>(sql, params);
}

/** `searchMentionFolders`' statement and its parameters. */
export function mentionFoldersQuery(
  subjectId: number | null,
  query: string,
  limit: number,
): { sql: string; params: (string | number | null)[] } {
  const { where, params } = mentionMatch(query, 2, "name");
  return {
    sql: `WITH d AS (
       SELECT f.subject_id, s.code AS subject_code, f.last_accessed_at AS seen,
              substr(f.relative_path, ${COURSES_DIR.length + 2}) AS rest
       FROM files f
       JOIN subjects s ON s.id = f.subject_id
       WHERE f.relative_path LIKE '${COURSES_DIR}/%/%'
         AND ($1 IS NULL OR f.subject_id = $1)
     ), t AS (
       SELECT subject_id, subject_code, seen,
              substr(rest, 1, instr(rest, '/')) AS top,
              substr(rest, instr(rest, '/') + 1) AS inner
       FROM d
     ), u AS (
       SELECT subject_id, subject_code, seen, top,
              CASE WHEN substr(inner, 1, ${COURSE_DIR.length + 1}) = '${COURSE_DIR}/' THEN '${COURSE_DIR}/' ELSE '' END AS lead,
              CASE WHEN substr(inner, 1, ${COURSE_DIR.length + 1}) = '${COURSE_DIR}/' THEN substr(inner, ${COURSE_DIR.length + 2})
                   ELSE inner END AS below
       FROM t
     ), n AS (
       SELECT subject_id, subject_code, seen,
              CASE WHEN $1 IS NULL THEN '${COURSES_DIR}/' || top
                   ELSE '${COURSES_DIR}/' || top || lead || substr(below, 1, instr(below, '/')) END AS path,
              CASE WHEN $1 IS NULL THEN substr(top, 1, length(top) - 1)
                   ELSE substr(below, 1, instr(below, '/') - 1) END AS name
       FROM u
       WHERE $1 IS NULL OR instr(below, '/') > 0
     )
     SELECT path, name, subject_id, subject_code
     FROM n
     WHERE (${where})
     GROUP BY path
     ORDER BY MAX(seen) DESC, path ASC
     LIMIT $${params.length + 2}`,
    params: [subjectId, ...params, limit],
  };
}

/** What an opened `@` folder shows under it. */
export interface MentionFolderContents {
  /** Its own subfolders, by name. */
  folders: MentionFolder[];
  /** The files directly in it the agent can read, by name, up to
   *  `MENTION_FOLDER_FILES`. */
  files: MentionFile[];
  /** How many readable files the cap left out. */
  more: number;
}

/** The most files one opened folder lists; the rest are found by typing. */
export const MENTION_FOLDER_FILES = 100;

const byName = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

/**
 * One level of a folder (`path` ends in `/`), for an `@` folder opened in the
 * menu: subfolders holding any file, as `searchMentionFolders` offers them,
 * then the readable files directly inside (`searchMentionFiles`' rule).
 * Sorted as a listing, so `Week_2` comes before `Week_10`.
 */
export async function listMentionFolder(path: string): Promise<MentionFolderContents> {
  const db = await getDb();
  const q = mentionFolderQueries(path);
  const [folders, files] = await Promise.all([
    db.select<MentionFolder[]>(q.folders, q.params),
    db.select<MentionFile[]>(q.files, q.params),
  ]);
  folders.sort((a, b) => byName.compare(a.name, b.name));
  files.sort((a, b) => byName.compare(a.filename, b.filename));
  return {
    folders,
    files: files.slice(0, MENTION_FOLDER_FILES),
    more: Math.max(0, files.length - MENTION_FOLDER_FILES),
  };
}

/** `listMentionFolder`'s two statements. The prefix is compared with `substr`,
 *  not `LIKE`, whose `_` wildcard nearly every path holds. */
export function mentionFolderQueries(path: string): { folders: string; files: string; params: string[] } {
  const inside = `FROM files f
       JOIN subjects s ON s.id = f.subject_id
       WHERE substr(f.relative_path, 1, length($1)) = $1`;
  return {
    folders: `WITH d AS (
       SELECT f.subject_id, s.code AS subject_code,
              substr(f.relative_path, length($1) + 1) AS rest
       ${inside}
     )
     SELECT $1 || substr(rest, 1, instr(rest, '/')) AS path,
            substr(rest, 1, instr(rest, '/') - 1) AS name,
            subject_id, subject_code
     FROM d
     WHERE instr(rest, '/') > 0
     GROUP BY path`,
    files: `SELECT f.id, f.subject_id, s.code AS subject_code, f.filename,
            f.relative_path, f.category
     ${inside}
       AND instr(substr(f.relative_path, length($1) + 1), '/') = 0
       AND (f.file_type = 'md' OR f.parse_status = 'quality')`,
    params: [path],
  };
}

/** A file a note's `@` offers, with its subject's code for the menu. */
export type NoteLinkFile = DbFile & { subject_code: string };

/** Files a note's `@` can mention: every file of `subjectId`, or of the whole
 *  library when it is null, parsed or not (a mention opens the file, nothing
 *  reads it), `excludePath` (the note itself) left out. Ranked like
 *  `searchMentionFiles`, so an empty query lists recently opened files. */
export async function searchNoteLinkFiles(
  subjectId: number | null,
  excludePath: string | null,
  query: string,
  limit = 8,
): Promise<NoteLinkFile[]> {
  const db = await getDb();
  const { where, params, prefix } = mentionMatch(query, 3);
  return db.select<NoteLinkFile[]>(
    `SELECT f.*, s.code AS subject_code
     FROM files f
     JOIN subjects s ON s.id = f.subject_id
     WHERE ($1 IS NULL OR f.subject_id = $1)
       AND ($2 IS NULL OR f.relative_path != $2)
       AND (${where})
     ORDER BY (f.filename LIKE $${params.length + 3} ESCAPE '\\') DESC,
              f.last_accessed_at DESC,
              f.filename ASC
     LIMIT $${params.length + 4}`,
    [subjectId, excludePath, ...params, prefix, limit],
  );
}

/** How many PDF-backed files the `@` query would match if they were parsed,
 *  so the menu can say why they're missing rather than look like a typo. */
export async function countUnparsedMentionMatches(
  subjectId: number | null,
  query: string,
): Promise<number> {
  const db = await getDb();
  const { where, params } = mentionMatch(query, 2);
  const rows = await db.select<{ n: number }[]>(
    `SELECT COUNT(*) AS n
     FROM files f
     WHERE lower(f.file_type) IN ${PDF_BACKED_SQL_LIST}
       AND (f.parse_status IS NULL OR f.parse_status != 'quality')
       AND ($1 IS NULL OR f.subject_id = $1)
       AND (${where})`,
    [subjectId, ...params],
  );
  return rows[0]?.n ?? 0;
}
