import { z } from "zod/v4";
import type { Reads } from "./reads";
import { limit, subject, subjectIds, subjectEvents, tool, revisionEvent, calendarEvent } from "./shared";

export function studyTools(read: Reads, now: () => number) {
  const revisionScope = async (wanted?: string | number) => wanted === undefined ? "all" as const
    : wanted === "general" ? "general" as const : subjectIds(await read.getSubjects(), wanted, true)![0];
  const revisionSchema = z.object({ subject, topic: z.number().int().positive().optional(), limit }).strict();
  return {
    subjects: tool(z.object({ limit }).strict(), subjectEvents, async () =>
      (await read.getSubjects()).map(s => ({ id: s.id, code: s.code, name: s.name, term: s.term_name, current: s.is_current, selected: s.selected, last_synced_at: s.last_synced_at }))),
    lectures: tool(z.object({ subject, limit }).strict(), [calendarEvent, "lecture-chapters", "oculus:file-scraped"], async a => {
      const ids = a.subject === undefined ? undefined : subjectIds(await read.getSubjects(), a.subject, true);
      return (await read.getAllLectures()).filter(l => !ids || ids.includes(l.subject_id)).sort((a, b) => a.date.localeCompare(b.date) || a.id.localeCompare(b.id))
        .map(l => ({ id: l.id, title: l.title, subject_id: l.subject_id, subject_code: l.subject_code, date: l.date, duration_seconds: l.duration_seconds, has_video: l.video_path != null, has_transcript: l.transcript_path != null }));
    }),
    "revision due": tool(revisionSchema, [revisionEvent], async a =>
      (await read.getItems(await revisionScope(a.subject))).sort((a, b) => a.id - b.id).filter(i => i.status === "active" && Date.parse(i.due) <= now() && (!a.topic || i.topic_ids.includes(a.topic)))),
    "cheat notes": tool(revisionSchema, [revisionEvent], async a =>
      (await read.getNotes(await revisionScope(a.subject))).filter(n => !a.topic || n.topic_ids.includes(a.topic))),
  };
}
