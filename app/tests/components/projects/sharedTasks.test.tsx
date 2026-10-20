import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import type { DbProject } from "@/lib/planning/projects";
import { DEFAULT_DISPLAY, PROJECT_DISPLAY, isDefaultDisplay, readDisplay, resetDisplay, showProperty, writeDisplay } from "@/components/projects/display/settings";
import { DEFAULT_FILTER, filteredColumns, matchesFilter, toggleStatusFilter } from "@/components/projects/tasks/filterRules";
import { groupTasks } from "@/components/projects/tasks/taskGroups";
import { sortTasks } from "@/components/projects/tasks/taskSort";
import { TasksView } from "@/components/projects/tasks/TasksView";
import { task, tag } from "./fixtures";

const project: DbProject = {
  id: 901, subject_id: 91, subject_code: "SUBJ901", name: "A scoped project", brief: null,
  status: "active", starts_at: null, due_at: null, tags: ["Assignment"], event_id: null,
  position: 1, source: "manual", created_at: "", updated_at: "",
  columns: [{id: "ideas", name: "Ideas", kind: "backlog"}, {id: "draft", name: "Drafting", kind: "active"},
    {id: "review", name: "In review", kind: "active"}, {id: "shipped", name: "Shipped", kind: "done"}],
};
const byId = new Map([[project.id, project]]);
const parent = task({id: 101, title: "Prepare draft", project_id: project.id, project_name: project.name,
  project_subject_id: project.subject_id, project_subject_code: project.subject_code, column_id: "draft", tag_ids: [1]});
const child = task({...parent, id: 102, parent_id: parent.id, title: "Check evidence", column_id: "shipped", done_at: "2026-10-17 00:00:00"});
const inReview = task({...parent, id: 103, title: "Ask for feedback", column_id: "review"});
const tags = [tag(1, "Assignment")];
const actions = {onMove: () => {}, onRefile: () => {}, onTags: () => {}, onCreateTag: async () => tags[0]};

test("project defaults hide only the context identities and keep tags; choices round trip independently", () => {
  const defaults = readDisplay(null, PROJECT_DISPLAY);
  expect(defaults.properties).toEqual(["tags", "due", "updated"]);
  expect(defaults.groupSubtasks).toBe(true);
  expect(defaults.showCompleted).toBe(true);
  const chosen = showProperty(showProperty(defaults, "project"), "subject");
  expect(readDisplay(writeDisplay(chosen), PROJECT_DISPLAY).properties).toContain("subject");
  expect(isDefaultDisplay(resetDisplay(PROJECT_DISPLAY), PROJECT_DISPLAY)).toBe(true);
  expect(readDisplay(null)).toEqual(DEFAULT_DISPLAY);
});

test("project filters retain distinct custom active columns and allow an explicitly opened archived project", () => {
  const filter = {...DEFAULT_FILTER, status: ["review"]};
  expect(matchesFilter(inReview, filter, byId, Date.now(), project)).toBe(true);
  expect(matchesFilter(parent, filter, byId, Date.now(), project)).toBe(false);
  expect(filteredColumns(filter, project.columns).map(c => c.id)).toEqual(["review"]);
  expect(toggleStatusFilter(["review"], "draft", project.columns)).toEqual(["draft", "review"]);
  const archived = {...project, status: "archived"};
  expect(matchesFilter(parent, DEFAULT_FILTER, new Map([[project.id, archived]]), Date.now(), archived)).toBe(true);
  expect(matchesFilter(parent, DEFAULT_FILTER, new Map([[project.id, archived]]), Date.now())).toBe(false);
});

test("status sorting and grouping keep project names and column order, including multiple active columns", () => {
  expect(sortTasks([inReview, parent], {key: "status", dir: "asc"}, byId, project.columns).map(t => t.id)).toEqual([parent.id, inReview.id]);
  const groups = groupTasks([inReview, parent, child], "status", {projectById: byId, tags, now: Date.now(), columns: project.columns});
  expect(groups.map(g => g.label)).toEqual(["Drafting", "In review", "Shipped"]);
});

function render(view: "table" | "board", identities = false) {
  const display = {...PROJECT_DISPLAY, view, properties: identities ? [...PROJECT_DISPLAY.properties, "project", "subject"] as typeof PROJECT_DISPLAY.properties : PROJECT_DISPLAY.properties};
  return renderToStaticMarkup(<MemoryRouter><TasksView tasks={[parent, inReview]} allTasks={[parent, child, inReview]}
    projects={[project]} projectById={byId} tags={tags} tagById={new Map(tags.map(t => [t.id, t]))}
    display={display} onSort={() => {}} columns={project.columns} project={project} onCreate={() => {}}
    empty="No tasks" actions={actions}/></MemoryRouter>);
}

for (const view of ["table", "board"] as const) {
  test(`the shared ${view} keeps tags and progress, hides context fields and allows showing them`, () => {
    const html = render(view);
    expect(html).toContain(parent.title);
    expect(html).toContain("Assignment");
    expect(html).toContain(">1/1<");
    expect(html).toContain("New task");
    expect(html).not.toContain(project.name);
    expect(html).not.toContain(project.subject_code!);
    const explicit = render(view, true);
    expect(explicit).toContain(project.name);
    expect(explicit).toContain(project.subject_code!);
    if (view === "board") expect(html).toContain("In review");
  });
}
