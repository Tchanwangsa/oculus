import type { DbCalendarEvent, DbFile, Lecture } from "@/lib/db";
import { threadTitle, type EdThread } from "@/lib/ed/threads";
import { filePagePath, fileTitle } from "@/lib/files/labels";
import { lecturePagePath } from "@/lib/lectures/pagePath";
import { recordingStatus, type SubjectWeeks } from "@/lib/weeks";
import { inWeek, localRowDate, sourceDate } from "./dates";
import type { ActivityItem } from "./types";

export function deriveActivity(
  subjectId: number, strip: SubjectWeeks, key: number | null, files: DbFile[],
  lectures: Lecture[], classes: DbCalendarEvent[], threads: EdThread[], since: number | null,
): ActivityItem[] {
  const out: ActivityItem[] = [];
  const ed = new Map(threads.map((thread) => [thread.file.id, thread]));
  for (const file of files) {
    if (file.subject_id !== subjectId) continue;
    const kind = file.category;
    if (kind !== "file" && kind !== "page" && kind !== "home" && kind !== "syllabus"
      && kind !== "announcement" && kind !== "ed") continue;
    const created = file.source_created_at == null
      ? localRowDate(file.first_seen_at) : sourceDate(file.source_created_at);
    const updated = sourceDate(file.source_updated_at);
    const thread = ed.get(file.id);
    let date = inWeek(strip, key, created) ? created : null;
    let badge = kind === "ed" ? "New thread" : "New";
    // Ed updates represent replies, so even a reply minutes after posting counts.
    const changed = updated && (kind === "ed" || (created && updated.getTime() - created.getTime() > 3_600_000));
    if (kind !== "announcement" && changed && inWeek(strip, key, updated)
      && (!date || updated > date)) {
      date = updated;
      badge = kind === "ed"
        ? thread?.replyCount != null ? `${thread.replyCount} replies` : "Replies"
        : "Updated";
    }
    if (!date) continue;
    out.push({
      id: `file:${file.id}`, kind: kind === "home" || kind === "syllabus" ? "page" : kind,
      title: thread ? threadTitle(thread) : fileTitle(file), date,
      href: filePagePath(subjectId, file.relative_path), badge,
      isNew: since == null || date.getTime() > since, recordingStatus: null,
    });
  }
  for (const lecture of lectures) {
    if (lecture.subject_id !== subjectId) continue;
    const date = sourceDate(lecture.date);
    if (!inWeek(strip, key, date)) continue;
    out.push({
      id: `recording:${lecture.id}`, kind: "recording", title: lecture.title, date,
      href: lecturePagePath(lecture), badge: "New recording",
      isNew: since == null || date.getTime() > since,
      recordingStatus: recordingStatus(lecture, classes),
    });
  }
  return out.sort((a, b) => b.date.getTime() - a.date.getTime() || a.id.localeCompare(b.id));
}
