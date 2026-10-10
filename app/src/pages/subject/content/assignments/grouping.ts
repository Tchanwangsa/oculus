import { humanizeSlug } from "@/lib/format/format";
import type { DbFile } from "@/lib/db";

export interface TaskDoc {
  file: DbFile;
  title: string;
  kind: "quiz" | "assignment";
  /** Parsed from the doc's `**Due:**` line; null when Canvas set no due date. */
  due: Date | null;
  /** `**Available until:**` — when Canvas stops accepting submissions. */
  lock: Date | null;
  /** `**Status:** submitted|graded` — the user has handed it in. */
  submitted: boolean;
  points: string | null;
}

/** `**<label>:** 2026-09-12 13:59 UTC` (written by sync/render.rs) → a local Date. */
function parseTs(md: string, label: string): Date | null {
  const m = new RegExp(
    `^\\*\\*${label}:\\*\\* (\\d{4}-\\d{2}-\\d{2}) (\\d{2}:\\d{2}) UTC\\s*$`,
    "m",
  ).exec(md);
  if (!m) return null;
  const d = new Date(`${m[1]}T${m[2]}:00Z`);
  return Number.isNaN(d.getTime()) ? null : d;
}

export const WEEK_MS = 7 * 24 * 60 * 60 * 1000;

export const GROUPS = ["due-soon", "overdue", "upcoming", "closed", "done"] as const;
export type Group = (typeof GROUPS)[number];

export const GROUP_LABELS: Record<Group, string> = {
  "due-soon": "Due soon",
  overdue: "Overdue",
  upcoming: "Upcoming",
  closed: "Closed",
  done: "Done",
};

export function groupOf(t: TaskDoc, now: number): Group {
  if (t.submitted) return "done";
  if (t.lock != null && t.lock.getTime() < now) return "closed";
  if (t.due != null && t.due.getTime() < now) return "overdue";
  if (t.due != null && t.due.getTime() - now < WEEK_MS) return "due-soon";
  return "upcoming";
}

export function parseTaskDoc(md: string, file: DbFile): TaskDoc {
  return {
    file,
    title: /^# (.+)$/m.exec(md)?.[1] ?? humanizeSlug(file.filename),
    kind: file.category === "quiz" ? "quiz" : "assignment",
    due: parseTs(md, "Due"),
    lock: parseTs(md, "Available until"),
    submitted: /^\*\*Status:\*\* (submitted|graded)\s*$/m.test(md),
    points: /^\*\*Points:\*\* (.+?)\s*$/m.exec(md)?.[1] ?? null,
  };
}

export function fmtDue(d: Date): string {
  return d.toLocaleString("en-AU", {
    weekday: "short",
    day: "numeric",
    month: "short",
    hour: "numeric",
    minute: "2-digit",
  });
}
