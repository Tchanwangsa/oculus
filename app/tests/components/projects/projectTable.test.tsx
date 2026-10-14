import { matchesProjectFilter, projectSubjectCounts } from "@/components/projects/lists/projectFilters";
import { DEFAULT_FILTER } from "@/components/projects/tasks/filterRules";
import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { DEFAULT_COLUMNS, type DbProject } from "@/lib/planning/projects";
import { projectTableData } from "@/components/projects/lists/projectTableData";
import { ProjectsIndexTable } from "@/components/projects/lists/ProjectsIndexTable";
import { task } from "./fixtures";

const project: DbProject = {id: 40, subject_id: 1, subject_code: "SUBJ101", name: "The report", brief: null,
  status: "active", starts_at: null, due_at: "2026-10-16 00:00:00", columns: [...DEFAULT_COLUMNS],
  tags: ["Assignment"], event_id: null, position: 0, source: "manual", created_at: "", updated_at: ""};
const other = {...project, id: 41, subject_id: 2, subject_code: "SUBJ102", name: "The presentation", due_at: null};
const parent = task({id: 80, project_id: 40, project_name: project.name, project_subject_id: 1, project_subject_code: project.subject_code, title: "Draft the report"});
const child = task({...parent, id: 81, parent_id: 80, title: "Proofread section three", column_id: "done", done_at: "2026-10-01 00:00:00"});
const sibling = task({...parent, id: 82, title: "Review the bibliography"});
const tasks = [parent, child, sibling];
const sort = {key: "due", dir: "asc"} as const;
const tagName = () => undefined;

const actions = {onRename: () => {}, onArchive: () => {}, onUnarchive: () => {}, onDelete: () => {}};
const taskActions = {onMove: () => {}, onRefile: () => {}, onTags: () => {}, onCreateTag: async () => {throw new Error("unused");}};

test("searching a subtask retains its project and parent while progress includes every task", () => {
  const branches = projectTableData([project, other], tasks, "Proofread", sort, tagName);
  expect(branches.map(b => b.project.id)).toEqual([40]);
  expect(branches[0].nodes.map(n => n.task.id)).toEqual([80]);
  expect(branches[0].nodes[0].children.map(t => t.id)).toEqual([81]);
  expect(branches[0].progress).toEqual({done: 1, total: 3, pct: 33});
});

test("matching a project shows its full task tree; empty projects stay discoverable", () => {
  expect(projectTableData([project], tasks, "Assignment", sort, tagName)[0].nodes).toHaveLength(2);
  expect(projectTableData([other], [], "", sort, tagName)[0].progress.total).toBe(0);
  expect(projectTableData([other], [], "missing", sort, tagName)).toEqual([]);
});

test("undated projects sort last in either direction", () => {
  expect(projectTableData([other, project], tasks, "", sort, tagName).map(b => b.project.id)).toEqual([40, 41]);
  expect(projectTableData([other, project], tasks, "", {...sort, dir: "desc"}, tagName).map(b => b.project.id)).toEqual([40, 41]);
});

function render(expanded: Set<string>, query = "") {
  return renderToStaticMarkup(<MemoryRouter><ProjectsIndexTable branches={projectTableData([project], tasks, query, sort, tagName)}
    tasks={tasks} projects={[project]} projectById={new Map([[40, project]])} tags={[]} tagById={new Map()}
    expanded={expanded} onToggle={() => {}} query={query} sort={sort} onSort={() => {}}
    actions={actions} taskActions={taskActions}/></MemoryRouter>);
}

test("expanding a project renders the shared editable task rows and their subtasks", () => {
  const collapsed = render(new Set());
  expect(collapsed).toContain('Expand tasks in The report');
  expect(collapsed).not.toContain('Draft the report');
  const expanded = render(new Set(["40"]));
  expect(expanded).toContain('Draft the report');
  expect(expanded).toContain('Proofread section three');
  expect(expanded).toContain('aria-label="Change status"');
  expect(expanded).toContain('padding-left:48px');
  expect(expanded).toContain('/projects/40/tasks/81');
});

test("search opens matching branches without requiring saved expansion", () => {
  const html = render(new Set(), "Proofread");
  expect(html).toContain('Proofread section three');
  expect(html).toContain('Draft the report');
  expect(html).not.toContain('Review the bibliography');
});


test("project filters compose due, shared tags, subject, archive and task status", () => {
  const tags = new Map([[1, {id: 1, name: "assignment", color: "indigo" as const, position: 1}]]);
  const byId = new Map([[project.id, project]]);
  const [branch] = projectTableData([project], tasks, "", sort, tagName);
  const now = Date.parse("2026-10-15T00:00:00Z");
  expect(matchesProjectFilter(branch, {...DEFAULT_FILTER, tags: [1], subject: "SUBJ101"}, tags, byId, now)).toBe(true);
  expect(matchesProjectFilter(branch, {...DEFAULT_FILTER, tags: [2]}, tags, byId, now)).toBe(false);
  expect(matchesProjectFilter(branch, {...DEFAULT_FILTER, due: "undated"}, tags, byId, now)).toBe(false);
  expect(matchesProjectFilter({...branch, project: {...project, status: "archived"}}, DEFAULT_FILTER, tags, byId, now)).toBe(false);
  expect(matchesProjectFilter({...branch, project: {...project, status: "archived"}}, {...DEFAULT_FILTER, showArchived: true}, tags, byId, now)).toBe(true);
  expect(matchesProjectFilter(branch, {...DEFAULT_FILTER, status: ["done"]}, tags, byId, now)).toBe(true);
  const empty = projectTableData([other], [], "", sort, tagName)[0];
  expect(matchesProjectFilter(empty, {...DEFAULT_FILTER, status: ["backlog"]}, tags, byId, now)).toBe(true);
  expect(matchesProjectFilter(empty, {...DEFAULT_FILTER, status: ["done"]}, tags, byId, now)).toBe(false);
  const counts = projectSubjectCounts([branch, empty], {...DEFAULT_FILTER, subject: "SUBJ101", tags: [1]}, tags, byId, now);
  expect(counts.anySubject).toBe(2);
  expect(counts.subjects.get("SUBJ102")).toBe(1);
});
