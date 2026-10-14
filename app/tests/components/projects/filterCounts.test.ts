import { describe, expect, test } from "bun:test";
import { pickSubjectFilter, taskFilterCounts } from "@/components/projects/tasks/filterCounts";
import { DEFAULT_FILTER } from "@/components/projects/tasks/filterRules";
import { DEFAULT_COLUMNS, type DbProject } from "@/lib/planning/projects";
import { task } from "./fixtures";

const project = (id: number, code: string, status = "active"): DbProject => ({
  id, name: code, subject_id: id, subject_code: code, status,
  columns: [...DEFAULT_COLUMNS], tags: [], brief: null, starts_at: null, due_at: null,
  event_id: null, position: id, source: "manual", created_at: "", updated_at: "",
});
const projects = new Map([[1, project(1, "SCI10001")], [2, project(2, "ART10001")], [3, project(3, "SCI10001", "archived")], [4, project(4, "SCI10001")]]);
const rows = [
  task({ id: 1, project_id: 1, project_subject_code: "SCI10001", column_id: "todo" }),
  task({ id: 2, project_id: 1, project_subject_code: "SCI10001", parent_id: 1, column_id: "doing" }),
  task({ id: 3, project_id: 2, project_subject_code: "ART10001", column_id: "todo" }),
  task({ id: 4, project_id: 3, project_subject_code: "SCI10001", column_id: "todo" }),
  task({ id: 5 }),
];
const now = Date.now();

describe("task filter option counts", () => {
  test("projects are limited to the selected subject, with empty and unfiled choices absent", () => {
    const counts = taskFilterCounts(rows, { ...DEFAULT_FILTER, subject: "SCI10001" }, projects, now);
    expect([...counts.projects]).toEqual([[1, 2], [3, 1]]);
    expect(counts.projects.has(null)).toBe(false);
    expect(counts.projects.has(4)).toBe(false);
    expect(counts.anyProject).toBe(2);
  });

  test("counts ignore the current project, honor other filters, and include matching subtasks", () => {
    const counts = taskFilterCounts(rows, { ...DEFAULT_FILTER, project: 1, status: ["todo"] }, projects, now);
    expect([...counts.projects]).toEqual([[1, 1], [2, 1], [3, 1]]);
    expect(counts.anyProject).toBe(2);
    expect([...counts.subjects]).toEqual([["SCI10001", 1], ["ART10001", 1]]);
    expect(counts.anySubject).toBe(2);
  });

  test("archived choices count tasks they reveal; subject counts follow visibility", () => {
    const counts = taskFilterCounts(rows, DEFAULT_FILTER, projects, now);
    expect(counts.projects.get(3)).toBe(1);
    expect(counts.projects.get(null)).toBe(1);
    expect(counts.subjects.get("SCI10001")).toBe(2);
    expect(counts.subjects.get(null)).toBe(1);
    const archived = taskFilterCounts(rows, { ...DEFAULT_FILTER, showArchived: true }, projects, now);
    expect(archived.subjects.get("SCI10001")).toBe(3);
  });

  test("choosing a subject clears Project; choosing it again restores Any subject", () => {
    const picked = pickSubjectFilter({ ...DEFAULT_FILTER, project: 2 }, "SCI10001");
    expect(picked.subject).toBe("SCI10001");
    expect(picked.project).toBe("any");
    expect(pickSubjectFilter(picked, "SCI10001").subject).toBe("any");
  });
});
