import { sqliteUtcToMs } from "@/lib/format/format";
import type { RevisionItem } from "./types";

export function isRevisionDue(item: Pick<RevisionItem, "status" | "due">, now: number): boolean {
  const due = sqliteUtcToMs(item.due);
  return item.status === "active" && due != null && due <= now;
}

export function subjectRevisionDue(items: RevisionItem[], subjectId: number, now: number): number {
  return items.filter((item) => item.subject_id === subjectId && isRevisionDue(item, now)).length;
}
