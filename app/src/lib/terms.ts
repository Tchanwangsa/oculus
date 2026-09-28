/**
 * Canvas term names sort chronologically as strings — right up until Summer.
 *
 * Canvas hands back `"2026 Semester 1"`, `"2026 Semester 2"`, `"2026 Summer
 * Term"`. Compared as text, `"Summer"` beats `"Semester"` (`Su` > `Se`), so a
 * plain `ORDER BY term_name` puts Summer *last* in its year. It runs first:
 * UniMelb's Summer Term is January–February, ahead of Semester 1, with Winter
 * Term sitting between the two semesters.
 *
 * Not every term is named after a semester. A short intensive comes back as
 * the month it runs in — `"2026 June"` for `THTR30042_2026_JUN_STH_2` — so the
 * months map onto the term they fall inside: January/February are Summer,
 * June/July are Winter. Without that they ranked as unknown, which sorts after
 * every real term and made a one-off intensive outrank the semester actually
 * being studied.
 *
 * The year prefix is still fine to compare as text or as an integer; only the
 * term within a year needs a rank.
 */

/** Chronological position within an academic year. Unknown terms sort last.
 *  Ordered: the first pattern a term name contains wins, so the semesters are
 *  matched before the month aliases — `"Semester 2 (July intensive)"` is a
 *  semester, not a winter term. */
export const TERM_RANK: ReadonlyArray<readonly [string, number]> = [
  ["summer", 0],
  ["semester 1", 1],
  ["winter", 2],
  ["semester 2", 3],
  ["january", 0],
  ["february", 0],
  ["june", 2],
  ["july", 2],
];

const TERM_TOKENS: ReadonlyArray<readonly [number, readonly string[]]> = [
  [0, ["summer", "sum"]],
  [1, ["semester 1", "sm1"]],
  [2, ["winter", "win"]],
  [3, ["semester 2", "sm2"]],
  [0, ["january", "jan", "february", "feb"]],
  [2, ["june", "jun", "july", "jul"]],
];

const UNKNOWN_RANK = 9;

/** `"2026 Summer Term"` → `0`. Anything unrecognised ranks after real terms. */
export function termRank(termName: string | null): number {
  if (!termName) return UNKNOWN_RANK;
  const haystack = ` ${termName.toLowerCase().split(/[\s_()-]+/).join(" ")} `;
  for (const [rank, tokens] of TERM_TOKENS) {
    if (tokens.some((token) => haystack.includes(` ${token} `))) return rank;
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

interface TermSubject {
  id: number;
  term_name: string | null;
  workflow_state: string;
}

/** Current means the newest available term, using academic order even if an
 *  older app or a Canvas event supplied incorrect current flags. */
export function currentSubjectIds(subjects: readonly TermSubject[]): Set<number> {
  const latest = subjects
    .filter((s) => s.workflow_state === "available")
    .reduce<string | null>(
      (best, s) => compareTermsNewestFirst(s.term_name, best) < 0 ? s.term_name : best,
      null,
    );
  return new Set(subjects
    .filter((s) => s.workflow_state === "available" && compareTermsNewestFirst(s.term_name, latest) === 0)
    .map((s) => s.id));
}

/** Recognize only the obsolete automatic selection. A custom set, including
 *  selecting nothing, stays untouched. The stored flags are repaired with it,
 *  so later reads cannot repeatedly reset a person's choices. */
export function hasLegacyDefaultSelection(
  subjects: readonly (TermSubject & { is_current: boolean | number; selected: boolean | number })[],
  currentIds: ReadonlySet<number>,
): boolean {
  return subjects.some((s) => !!s.is_current)
    && subjects.every((s) => !!s.selected === !!s.is_current)
    && subjects.some((s) => !!s.is_current !== currentIds.has(s.id));
}

/**
 * The same ranking as a SQLite expression, so ordering can stay in the query
 * that fetches subjects rather than becoming a second sort in JS. Inlined as a
 * `CASE` because the plugin has no way to register a custom SQL function. The
 * `WHEN`s keep the array's order, so a `CASE` matches the same pattern
 * `termRank` would.
 */
export function TERM_RANK_SQL(column: string): string {
  let normalized = `replace(replace(replace(replace(replace(replace(replace(lower(${column}), '_', ' '), '-', ' '), '(', ' '), ')', ' '), char(9), ' '), char(10), ' '), char(13), ' ')`;
  for (let i = 0; i < 4; i++) normalized = `replace(${normalized}, '  ', ' ')`;
  normalized = `(' ' || ${normalized} || ' ')`;
  const whens = TERM_TOKENS
    .map(([rank, tokens]) => `WHEN ${tokens.map((token) => `${normalized} LIKE '% ${token} %'`).join(" OR ")} THEN ${rank}`)
    .join(" ");
  return `CASE ${whens} ELSE ${UNKNOWN_RANK} END`;
}
