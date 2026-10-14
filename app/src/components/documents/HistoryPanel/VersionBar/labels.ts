import { displayLocale } from "@/lib/format/locale";

/** "3 of 12 changes" (`current` counts from 0), "12 changes" before one is
 *  picked, or "No changes". */
export function changesLabel({ hunks, current }: { hunks: number; current: number | null }): string {
  if (hunks === 0) return "No changes";
  const noun = hunks === 1 ? "change" : "changes";
  return current == null ? `${hunks} ${noun}` : `${current + 1} of ${hunks} ${noun}`;
}

/** "+412 −38 words" as its two signed halves, a real minus on the second. */
export function wordStats({ inserted, deleted }: { inserted: number; deleted: number }): {
  inserted: string;
  deleted: string;
} {
  return {
    inserted: `+${inserted.toLocaleString(displayLocale)}`,
    deleted: `−${deleted.toLocaleString(displayLocale)}`,
  };
}

/** The compare pill: what the marks are against, or that they're off. */
export function compareLabel(showChanges: boolean, compare: "previous" | "current"): string {
  if (!showChanges) return "Changes hidden";
  return compare === "previous" ? "vs previous" : "vs current note";
}
