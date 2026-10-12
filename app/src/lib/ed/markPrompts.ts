/**
 * What to ask before a Mark, as Ed's own UI does: a submission past the due
 * date is late, a challenge may ask for confirmation (`confirm_submit`), and
 * the last attempt is warned about. Decided from the live standing fetched
 * just before, so a due date or attempt count changed on Ed counts.
 */

export type MarkPrompt = "late" | "confirm" | "lastAttempt";

export interface MarkFacts {
  now: number;
  dueAt: string | null;
  confirmSubmit: boolean;
  /** Null when attempts are unlimited. */
  attemptsRemaining: number | null;
}

/** The prompts to show, most serious first; none means Mark goes straight on. */
export function markPrompts({ now, dueAt, confirmSubmit, attemptsRemaining }: MarkFacts): MarkPrompt[] {
  const prompts: MarkPrompt[] = [];
  const due = dueAt ? Date.parse(dueAt) : NaN;
  if (Number.isFinite(due) && due < now) prompts.push("late");
  if (attemptsRemaining === 1) prompts.push("lastAttempt");
  if (confirmSubmit) prompts.push("confirm");
  return prompts;
}

export interface MarkDialogCopy {
  title: string;
  lines: string[];
  confirmLabel: string;
}

/** One dialog for every prompt that applies, titled by the most serious.
 *  `due` is the due date already formatted for display. */
export function markDialogCopy(prompts: MarkPrompt[], due: string | null): MarkDialogCopy {
  const lines: string[] = [];
  if (prompts.includes("late")) {
    lines.push(due ? `This challenge was due ${due}, so this submission is late.` : "This challenge is past its due date, so this submission is late.");
  }
  if (prompts.includes("lastAttempt")) lines.push("This is your last attempt.");
  lines.push("Your saved files are submitted for marking, and the submission counts on Ed.");
  const title = prompts.includes("late")
    ? "Submit late?"
    : prompts.includes("lastAttempt")
      ? "Use your last attempt?"
      : "Submit for marking?";
  return { title, lines, confirmLabel: prompts.includes("late") ? "Submit late" : "Submit" };
}
