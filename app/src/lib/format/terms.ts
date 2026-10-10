/**
 * Canvas term names ("2026 Semester 1", "2026 Summer Term", "2026 June") do not
 * sort as text: Summer (Jan–Feb) comes first in its year, and an intensive named
 * after its month belongs to the term it falls in. Only the term needs a rank.
 */

/** Position within a year; the first pattern contained wins, so semesters match
 *  before the month aliases ("Semester 2 (July intensive)" is a semester). */
const TERM_RANK: ReadonlyArray<readonly [string, number]> = [
  ["summer", 0],
  ["semester 1", 1],
  ["winter", 2],
  ["semester 2", 3],
  ["january", 0],
  ["february", 0],
  ["june", 2],
  ["july", 2],
];

const UNKNOWN_RANK = 9;

/** `"2026 Summer Term"` → `0`. Anything unrecognised ranks after real terms. */
function termRank(termName: string | null): number {
  if (!termName) return UNKNOWN_RANK;
  const haystack = termName.toLowerCase();
  for (const [needle, rank] of TERM_RANK) {
    if (haystack.includes(needle)) return rank;
  }
  return UNKNOWN_RANK;
}

/** `"2026 Summer Term"` → `2026`; missing or malformed years sort oldest. */
function termYear(termName: string | null): number {
  const year = Number(termName?.slice(0, 4));
  return Number.isFinite(year) ? year : 0;
}

/** Newest term first, matching the `ORDER BY` in `getSubjects`. */
export function compareTermsNewestFirst(a: string | null, b: string | null): number {
  return termYear(b) - termYear(a) || termRank(b) - termRank(a);
}

/** The same ranking as a SQL `CASE` (the plugin cannot register a custom
 *  function); `WHEN`s keep the array's order so the first match agrees. */
export function TERM_RANK_SQL(column: string): string {
  const whens = TERM_RANK.map(
    ([needle, rank]) => `WHEN lower(${column}) LIKE '%${needle}%' THEN ${rank}`
  ).join(" ");
  return `CASE ${whens} ELSE ${UNKNOWN_RANK} END`;
}
