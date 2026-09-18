/**
 * Canvas term names sort chronologically as strings — right up until Summer.
 *
 * Canvas hands back `"2026 Semester 1"`, `"2026 Semester 2"`, `"2026 Summer
 * Term"`. Compared as text, `"Summer"` beats `"Semester"` (`Su` > `Se`), so a
 * plain `ORDER BY term_name` puts Summer *last* in its year. It runs first:
 * UniMelb's Summer Term is January–February, ahead of Semester 1, with Winter
 * Term sitting between the two semesters.
 *
 * The year prefix is still fine to compare as text or as an integer; only the
 * term within a year needs a rank.
 */

/** Chronological position within an academic year. Unknown terms sort last. */
export const TERM_RANK: Record<string, number> = {
  summer: 0,
  "semester 1": 1,
  winter: 2,
  "semester 2": 3,
};

const UNKNOWN_RANK = 9;

/** `"2026 Summer Term"` → `0`. Anything unrecognised ranks after real terms. */
export function termRank(termName: string | null): number {
  if (!termName) return UNKNOWN_RANK;
  const haystack = termName.toLowerCase();
  for (const [needle, rank] of Object.entries(TERM_RANK)) {
    if (haystack.includes(needle)) return rank;
  }
  return UNKNOWN_RANK;
}

/** `"2026 Summer Term"` → `2026`; missing or malformed years sort oldest. */
export function termYear(termName: string | null): number {
  const year = Number(termName?.slice(0, 4));
  return Number.isFinite(year) ? year : 0;
}

/** Newest term first, matching the `ORDER BY` in `getSubjects`. */
export function compareTermsNewestFirst(a: string | null, b: string | null): number {
  return termYear(b) - termYear(a) || termRank(b) - termRank(a);
}

/**
 * The same ranking as a SQLite expression, so ordering can stay in the query
 * that fetches subjects rather than becoming a second sort in JS. Inlined as a
 * `CASE` because the plugin has no way to register a custom SQL function.
 */
export function TERM_RANK_SQL(column: string): string {
  const whens = Object.entries(TERM_RANK)
    .map(([needle, rank]) => `WHEN lower(${column}) LIKE '%${needle}%' THEN ${rank}`)
    .join(" ");
  return `CASE ${whens} ELSE ${UNKNOWN_RANK} END`;
}
