import type { DbFile } from "@/lib/db";
import type { HarnessThread } from "@/lib/harness/types";
import { chatHref } from "@/lib/harness/route";
import { filePagePath, fileTitle } from "@/lib/files/labels";
import type { SubjectWeeks } from "@/lib/weeks";
import { inWeek, localRowDate } from "./dates";
import type { YoursItem } from "./types";

function latestInWeek(strip: SubjectWeeks, key: number | null, created: string | null, updated: string | null): Date | null {
  const dates = [localRowDate(created), localRowDate(updated)]
    .filter((date) => inWeek(strip, key, date));
  return dates.sort((a, b) => b.getTime() - a.getTime())[0] ?? null;
}

export function deriveYours(subjectId: number, strip: SubjectWeeks, key: number | null, files: DbFile[], live: HarnessThread[], subjectThreads: HarnessThread[]): YoursItem[] {
  const out: YoursItem[] = [];
  for (const file of files) {
    if (file.subject_id !== subjectId) continue;
    const kind = file.category;
    if (kind !== "document" && kind !== "whiteboard" && kind !== "notebook" && kind !== "upload") continue;
    const date = latestInWeek(strip, key, file.first_seen_at, file.content_changed_at);
    if (!date) continue;
    out.push({ id: `file:${file.id}`, kind, title: fileTitle(file), date, href: filePagePath(subjectId, file.relative_path) });
  }
  // The store's copy is the fresher: a live run touches it before any read.
  const threads = new Map([...subjectThreads, ...live].map((thread) => [thread.id, thread]));
  for (const thread of threads.values()) {
    if (thread.subject_id !== subjectId) continue;
    const date = latestInWeek(strip, key, thread.created_at, thread.updated_at);
    if (!date) continue;
    out.push({ id: `chat:${thread.id}`, kind: "chat", title: thread.title || "Untitled chat", date, href: chatHref(thread.id, thread.title) });
  }
  return out.sort((a, b) => b.date.getTime() - a.date.getTime() || a.id.localeCompare(b.id));
}
