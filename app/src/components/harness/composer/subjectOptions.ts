import type { Subject } from "@/lib/db";
import { displayCode, displayName } from "@/lib/format/format";
import { GENERAL_SCOPE, type ChatScope } from "@/lib/harness/scope";

/** One subject choice, in `SubjectPicker` and the new-thread dialog: General,
 *  or a subject by its short code. */
export interface SubjectOption {
  scope: ChatScope;
  /** What the row shows: "General" or the short code (`displayCode`). */
  label: string;
  /** The full name, for the tooltip and `aria-label`. */
  title: string;
  /** Null for General. */
  subject: Subject | null;
}

const GENERAL_OPTION: SubjectOption = {
  scope: GENERAL_SCOPE,
  label: "General",
  title: "General — the whole library",
  subject: null,
};

function option(s: Subject): SubjectOption {
  return { scope: s.id, label: displayCode(s.code), title: displayName(s.name, s.code), subject: s };
}

/** Every word of the query appears in the label, the code or the name. */
function matches(o: SubjectOption, words: string[]): boolean {
  const hay = [o.label, o.subject?.code ?? "", o.title].join(" ").toLowerCase();
  return words.every((w) => hay.includes(w));
}

/**
 * The picker's rows in order: General first, then this term's subjects as the
 * list has them, then past terms' subjects (newest term first) for the fold.
 * A query keeps the rows whose code or name holds each of its words.
 */
export function subjectOptions(
  subjects: readonly Subject[],
  query: string,
): { listed: SubjectOption[]; past: SubjectOption[] } {
  const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const keep = (o: SubjectOption) => matches(o, words);
  const past = subjects
    .filter((s) => !s.is_current)
    .map((s, i) => ({ s, i }))
    // Newest term first; `term_sort` is `YYYY-MM`, so strings compare as dates.
    .sort((a, b) => (b.s.term_sort ?? "").localeCompare(a.s.term_sort ?? "") || a.i - b.i)
    .map(({ s }) => option(s));
  return {
    listed: [GENERAL_OPTION, ...subjects.filter((s) => s.is_current).map(option)].filter(keep),
    past: past.filter(keep),
  };
}

/** The row a scope names; null when it names none (unchosen, or a subject
 *  that is gone). */
export function subjectOption(subjects: readonly Subject[], scope: ChatScope | null): SubjectOption | null {
  if (scope == null) return null;
  if (scope === GENERAL_SCOPE) return GENERAL_OPTION;
  const subject = subjects.find((s) => s.id === scope);
  return subject ? option(subject) : null;
}
