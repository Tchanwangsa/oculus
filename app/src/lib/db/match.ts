/** Words beyond this are ignored (noise from a pasted line). */
export const MAX_TERMS = 6;

export function likeEscape(s: string): string {
  return s.replace(/[%_\\]/g, (c) => `\\${c}`);
}

/** The typed words, escaped for LIKE. Empty matches every row. */
export function terms(query: string): string[] {
  return query.trim().split(/\s+/).filter(Boolean).slice(0, MAX_TERMS).map(likeEscape);
}

/**
 * `AND`-ed substring predicates (every word, any order) plus a rank: whether
 * some haystack word *starts* with the first term (a leading space is
 * prepended so the first word counts).
 *
 * `scope` adds `column = value` predicates (skipped when the value is
 * undefined), numbered before the rank so placeholders keep text order (see
 * `mentionMatch`).
 */
export function matchSql(
  haystack: string,
  query: string,
  scope: [column: string, value: string | number | undefined][] = [],
): { where: string; rank: string; params: (string | number)[] } {
  const words = terms(query);
  const preds = words.map((_, i) => `${haystack} LIKE $${i + 1} ESCAPE '\\'`);
  const params: (string | number)[] = words.map((w) => `%${w}%`);
  for (const [column, value] of scope) {
    if (value === undefined) continue;
    params.push(value);
    preds.push(`${column} = $${params.length}`);
  }
  params.push(`% ${words[0] ?? ""}%`);
  const where = preds.length ? preds.join(" AND ") : "1";
  const rank = `((' ' || ${haystack}) LIKE $${params.length} ESCAPE '\\')`;
  return { where, rank, params };
}
