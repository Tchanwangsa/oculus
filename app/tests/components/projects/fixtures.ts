import type { DbProject, DbTaskTag, DbTaskWithProject } from "@/lib/planning/projects";

let next = 1;

/** A task with the fields a test cares about; the rest is inert. */
export function task(over: Partial<DbTaskWithProject> = {}): DbTaskWithProject {
  const id = over.id ?? next++;
  return {
    id,
    project_id: null,
    parent_id: null,
    title: `Task ${id}`,
    body: null,
    column_id: "backlog",
    position: id,
    starts_at: null,
    due_at: null,
    estimate_minutes: null,
    done_at: null,
    tag_ids: [],
    source: "manual",
    created_at: "2026-10-01 00:00:00",
    updated_at: "2026-10-01 00:00:00",
    project_name: null,
    project_subject_id: null,
    project_subject_code: null,
    ...over,
  };
}

export function tag(id: number, name: string): DbTaskTag {
  return { id, name, color: "slate", position: id };
}

export const NO_PROJECTS = new Map<number, DbProject>();
