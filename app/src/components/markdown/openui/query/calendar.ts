import { z } from "zod/v4";
import type { Reads } from "./reads";
import { limit, subject, subjectIds, tool, calendarEvent, projectEvent } from "./shared";

export function calendarTools(read: Reads, now: () => number) {
  const schema = z.object({ subject, days: z.number().int().min(0).max(365).default(14), past: z.boolean().default(false), due: z.boolean().default(false), limit }).strict();
  const make = (dueOnly: boolean) => tool(schema, [calendarEvent, projectEvent], async a => {
    const ids = a.subject === undefined ? undefined : subjectIds(await read.getSubjects(), a.subject);
    const [canvas, local] = await Promise.all([read.getCalendarEvents(), read.getLocalEvents()]);
    const rows = [
      ...canvas.map(e => ({ ...e, notes: null as string | null })),
      ...local.map(e => ({ ...e, id: `local_${e.id}`, location: null, url: null })),
    ];
    const time = now();
    const today = localDate(time);
    return rows.filter(e => (!ids || ids.includes(e.subject_id!)) && (!(dueOnly || a.due) || e.kind === "due")
      && (a.past || Date.parse(e.start_at) >= time || (e.start_at.length === 10 && e.start_at === today))
      && Date.parse(e.start_at) < time + a.days * 86400000)
      .sort((a, b) => Date.parse(a.start_at) - Date.parse(b.start_at) || a.id.localeCompare(b.id))
      .map(e => ({ id: e.id, subject: e.subject_code, kind: e.kind, title: e.title, start_at: e.start_at, end_at: e.end_at,
        notes: e.notes, all_day: !!e.all_day, starts_local: e.start_at.length === 10 ? e.start_at : localStamp(e.start_at), location: e.location, url: e.url }));
  });
  return { calendar: make(false), upcoming: make(false), deadlines: make(true) };
}

function localDate(time: number) {
  const d = new Date(time);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}
function localStamp(iso: string) {
  const d = new Date(iso);
  return `${localDate(d.getTime())} ${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}
