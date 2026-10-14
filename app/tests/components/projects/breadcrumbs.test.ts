import { expect, test } from "bun:test";
import { taskBreadcrumbs } from "@/components/projects/nav/taskBreadcrumbs";
import { DEFAULT_COLUMNS, type DbProject } from "@/lib/planning/projects";
import { task } from "./fixtures";

const project: DbProject = { id: 40, subject_id: 1, subject_code: "SUBJ101", name: "The report",
  brief: null, status: "active", starts_at: null, due_at: null, columns: [...DEFAULT_COLUMNS],
  tags: [], event_id: null, position: 0, source: "manual", created_at: "", updated_at: "" };

test("a filed subtask's full path includes project ancestors and its direct parent", () => {
  const parent = task({ id: 80, project_id: 40, title: "Draft the report" });
  const child = task({ id: 81, project_id: 40, parent_id: 80, title: "Proofread section three" });
  const trail = taskBreadcrumbs(child, project, parent);
  expect(trail.map(i => i.label)).toEqual(["Projects", "SUBJ101", "The report", "Draft the report", "Proofread section three"]);
  expect(trail.at(-2)?.href).toBe("/projects/40/tasks/80?n=Draft%20the%20report");
  expect(trail.at(-1)?.href).toBeUndefined();
});

test("an unfiled subtask retains its direct parent and the Tasks list", () => {
  const parent = task({ id: 80, title: "Read chapter three" });
  const child = task({ parent_id: 80, title: "Take notes" });
  const trail = taskBreadcrumbs(child, null, parent);
  expect(trail.map(i => i.label)).toEqual(["Tasks", "Read chapter three", "Take notes"]);
  expect(trail.at(-2)?.href).toBe("/tasks/80?n=Read%20chapter%20three");
});

test("a root unfiled task links directly to the Tasks list", () => {
  const trail = taskBreadcrumbs(task({title: "Read chapter three"}), null, null);
  expect(trail.map(i => i.label)).toEqual(["Tasks", "Read chapter three"]);
  expect(trail[0].href).toBe("/tasks");
});
