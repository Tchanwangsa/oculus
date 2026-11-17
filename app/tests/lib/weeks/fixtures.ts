import type { DbCalendarEvent, Lecture, Subject } from "@/lib/db";

export const subject = (id = 1, term_name: string | null = "2026 Semester 2"): Subject => ({
  id, term_name, code: `SUBJ${id}`, name: `Subject ${id}`, term_sort: "2026-07",
  is_current: true, workflow_state: "available", selected: true,
  last_synced_at: null, created_at: "2026-01-01",
});

export const lecture = (date: string, subject_id = 1, id = `${subject_id}-${date}`): Lecture => ({
  id, lesson_id: id, subject_id, title: "Lecture", date, duration_seconds: 3600,
  video_path: null, video2_path: null, has_source2: 0, transcript_path: null,
  progress_seconds: 0, last_watched_at: null, completed: 0, synced_at: "2026-01-01",
  chapter_status: null, chaptered_at: null, chapter_error: null,
  content_end_seconds: null, content_end_quote: null, content_end_status: null, content_end_error: null,
});

export const event = (date: string, subject_id = 1, extra: Partial<DbCalendarEvent> = {}): DbCalendarEvent => ({
  id: `${subject_id}-${date}`, subject_id, subject_code: `SUBJ${subject_id}`, kind: "class",
  title: "Lecture", start_at: new Date(date).toISOString(), end_at: null, all_day: 0,
  location: null, url: null, description: null, ...extra,
});
