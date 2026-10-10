import { NO_SUBJECT, type CalEvent } from "./model";

/** Chart tokens, redefined for dark mode in `index.css`. */
const PALETTE = [
  "var(--color-chart-1)",
  "var(--color-chart-2)",
  "var(--color-chart-3)",
  "var(--color-chart-4)",
  "var(--color-chart-5)",
];

/**
 * A stable colour per subject, indexed by sorted id. {@link NO_SUBJECT} stays
 * out of the indexing (and takes `primary`), or the first personal note would
 * recolour every subject.
 */
export function subjectColors(events: CalEvent[]): Map<number, string> {
  const ids = [...new Set(events.map((e) => e.subjectId))]
    .filter((id) => id !== NO_SUBJECT)
    .sort((a, b) => a - b);
  const colors = new Map(ids.map((id, i) => [id, PALETTE[i % PALETTE.length]]));
  colors.set(NO_SUBJECT, "var(--color-primary)");
  return colors;
}
