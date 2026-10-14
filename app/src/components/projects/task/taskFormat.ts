import { fmtAgo } from "@/lib/format/format";

/** "90 min", or null when there is no estimate. */
export function estimateLabel(minutes: number | null): string | null {
  return minutes == null ? null : `${minutes} min`;
}

/**
 * What an estimate field's text means: whole minutes, `null` for empty or zero
 * (clear it), `undefined` for text that is not a non-negative number (leave
 * the stored value alone).
 */
export function parseEstimate(text: string): number | null | undefined {
  const trimmed = text.trim();
  if (trimmed === "") return null;
  const n = Number(trimmed);
  if (!Number.isFinite(n) || n < 0) return undefined;
  const minutes = Math.round(n);
  return minutes === 0 ? null : minutes;
}

/** How long ago `ms` was: minutes, hours and days up to a week, then the date. */
export function ageLabel(ms: number, now = Date.now()): string {
  const mins = Math.floor((now - ms) / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  return fmtAgo(ms);
}

/** Done and total of a task's checklist; `pct` is 0 rather than NaN when empty. */
export function checklistProgress(items: { done_at: string | null }[]): {
  done: number;
  total: number;
  pct: number;
} {
  const total = items.length;
  const done = items.filter((i) => i.done_at != null).length;
  return { done, total, pct: total ? Math.round((done / total) * 100) : 0 };
}

/**
 * The folder a library file sits in under its subject: `files/week-1` for
 * `courses/MULT20015_2026_SM2/files/week-1/notes.pdf`, empty for a file at
 * the subject's top.
 */
export function fileFolder(relativePath: string): string {
  const inSubject = relativePath.replace(/^courses\/[^/]+\//, "");
  const cut = inSubject.lastIndexOf("/");
  return cut < 0 ? "" : inSubject.slice(0, cut);
}
