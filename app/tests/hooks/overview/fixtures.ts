import type { DbFile } from "@/lib/db";
import type { HarnessThread } from "@/lib/harness/types";
import type { DbOpenTask } from "@/lib/planning/projects";
import { subjectWeeks } from "@/lib/weeks";
import { event, lecture, subject } from "../../lib/weeks/fixtures";
export { event, lecture, subject };

export const classes = [event("2026-10-05T10:00:00"), event("2026-10-19T10:00:00")];
export const strip = subjectWeeks({ subjectId: 1, subjects: [subject()], classes, lectures: [], now: new Date("2026-10-06T12:00:00") });
export const first = strip.weeks[0].key;
export const second = strip.weeks[2].key;
export const date = (day = 6, hour = 10) => new Date(2026, 9, day, hour);
export const iso = (day = 6, hour = 10) => date(day, hour).toISOString();
export const sqlite = (day = 6, hour = 10) => iso(day, hour).replace("T", " ").replace(".000Z", "");

export const file = (extra: Partial<DbFile> = {}): DbFile => ({
  id: 1, subject_id: 1, filename: "lecture-slides.pdf", relative_path: "courses/SUBJ1/files/lecture-slides.pdf",
  file_type: "pdf", size_bytes: 10, category: "file", canvas_id: null, source_url: null,
  source_created_at: null, source_updated_at: null, first_seen_at: sqlite(), content_changed_at: null,
  modified_at: null, scraped_at: sqlite(), last_accessed_at: null, parse_status: null, parsed_at: null,
  embed_status: null, embedded_at: null, ...extra,
});

export const thread = (extra: Partial<HarnessThread> = {}): HarnessThread => ({
  id: 1, provider: "codex", provider_session_id: null, model: null, subject_id: 1, lecture_id: null,
  title: "My chat", status: "idle", usage: null, created_at: sqlite(), updated_at: sqlite(), ...extra,
});

export const task = (extra: Partial<DbOpenTask> = {}): DbOpenTask => ({
  id: 1, project_id: 1, parent_id: null, title: "Draft report", body: null, column_id: "todo", position: 0,
  starts_at: null, due_at: sqlite(7), estimate_minutes: null, done_at: null, tag_ids: [], source: "manual",
  created_at: sqlite(), updated_at: sqlite(), project_name: "Report", project_subject_id: 1,
  project_subject_code: "SUBJ1", ...extra,
});
