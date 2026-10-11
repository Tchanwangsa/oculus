import { getDb } from "./connection";
import { terms } from "./match";
import type { DbFile } from "./types";
import { PDF_BACKED_SQL_LIST } from "@/lib/files/fileTypes";

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
): { where: string; params: string[]; prefix: string } {
  const words = terms(query);
  return {
    where: words.length
      ? words.map((_, i) => `f.filename LIKE $${from + i} ESCAPE '\\'`).join(" AND ")
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
