import { describe, expect, test } from "bun:test";
import {
  DEFAULT_FILTER,
  activeFilterCount,
  filteredColumns,
  isDefaultFilter,
  matchesFilter,
  normaliseRange,
  popoverFilterCount,
  toggleStatusFilter,
} from "@/components/projects/tasks/filterRules";
import { dueBucket, groupTasks } from "@/components/projects/tasks/taskGroups";
import { sortTasks } from "@/components/projects/tasks/taskSort";
import {
  matchesSearch,
  matchesView,
  parseTagParam,
  parseView,
  tasksHref,
} from "@/components/projects/tasks/taskViews";
import { DEFAULT_COLUMNS, type DbProject } from "@/lib/planning/projects";
import { taskTree } from "@/components/projects/tasks/taskTree";
import { NO_PROJECTS, tag, task } from "./fixtures";

// Wednesday 14 Oct 2026, noon: the calendar week runs Mon 12 to Sun 18.
const NOW = new Date(2026, 9, 14, 12).getTime();
const at = (day: number, hour = 9) => new Date(2026, 9, day, hour).toISOString();

describe("the nav column's URL state", () => {
  test("an unknown view is All tasks", () => {
    expect(parseView("agent")).toBe("agent");
    expect(parseView("week")).toBe("all");
    expect(parseView("nonsense")).toBe("all");
    expect(parseView(null)).toBe("all");
  });

  test("a tag param is a whole positive id", () => {
    expect(parseTagParam("3")).toBe(3);
    expect(parseTagParam("0")).toBeNull();
    expect(parseTagParam("-1")).toBeNull();
    expect(parseTagParam("2.5")).toBeNull();
    expect(parseTagParam("abc")).toBeNull();
    expect(parseTagParam(null)).toBeNull();
  });

  test("a link names only what is off the default", () => {
    expect(tasksHref("all", null)).toBe("/tasks");
    expect(tasksHref("agent", null)).toBe("/tasks?view=agent");
    expect(tasksHref("all", 4)).toBe("/tasks?tag=4");
    expect(tasksHref("agent", 4)).toBe("/tasks?view=agent&tag=4");
  });
});

describe("the views", () => {
  const dated = task({ due_at: at(15) });
  const undated = task();
  const agent = task({ source: "agent" });

  test("All tasks keeps everything, including finished ones", () => {
    const done = task({ done_at: "2026-10-10 00:00:00" });
    expect([dated, undated, done].every((t) => matchesView(t, "all"))).toBe(true);
  });

  test("Added by the agent", () => {
    expect(matchesView(agent, "agent")).toBe(true);
    expect(matchesView(undated, "agent")).toBe(false);
  });
});

describe("search", () => {
  const names = new Map([[1, "Launch"], [2, "Customers"]]);
  const lookup = (id: number) => names.get(id);
  const t = task({ title: "Draft essay outline", project_name: "Politics 101", tag_ids: [1] });

  test("matches the title, the project name or a tag name, case-insensitively", () => {
    expect(matchesSearch(t, "ESSAY", lookup)).toBe(true);
    expect(matchesSearch(t, "politics", lookup)).toBe(true);
    expect(matchesSearch(t, "launch", lookup)).toBe(true);
    expect(matchesSearch(t, "customers", lookup)).toBe(false);
  });

  test("every word must match somewhere, in any order", () => {
    expect(matchesSearch(t, "launch essay", lookup)).toBe(true);
    expect(matchesSearch(t, "launch zebra", lookup)).toBe(false);
  });

  test("an empty or blank query matches everything", () => {
    expect(matchesSearch(t, "", lookup)).toBe(true);
    expect(matchesSearch(t, "   ", lookup)).toBe(true);
  });

  test("a task with no project or tags still matches on its title", () => {
    expect(matchesSearch(task({ title: "Read" }), "read", lookup)).toBe(true);
    expect(matchesSearch(task({ title: "Read" }), "null", lookup)).toBe(false);
  });
});

describe("the filter", () => {
  test("the default narrows nothing, including finished work", () => {
    expect(activeFilterCount(DEFAULT_FILTER)).toBe(0);
    expect(matchesFilter(task({ column_id: "done" }), DEFAULT_FILTER, NO_PROJECTS, NOW)).toBe(true);
  });

  test("tags are any-of", () => {
    const filter = { ...DEFAULT_FILTER, tags: [1, 2] };
    expect(activeFilterCount(filter)).toBe(1);
    expect(matchesFilter(task({ tag_ids: [2, 9] }), filter, NO_PROJECTS, NOW)).toBe(true);
    expect(matchesFilter(task({ tag_ids: [9] }), filter, NO_PROJECTS, NOW)).toBe(false);
    expect(matchesFilter(task(), filter, NO_PROJECTS, NOW)).toBe(false);
  });

  test("sections compose, and each counts once", () => {
    const filter = { ...DEFAULT_FILTER, status: ["todo"], due: "overdue" as const, project: "unfiled" as const };
    expect(activeFilterCount(filter)).toBe(3);
    const hit = task({ column_id: "todo", due_at: at(10) });
    expect(matchesFilter(hit, filter, NO_PROJECTS, NOW)).toBe(true);
    expect(matchesFilter(task({ column_id: "todo", due_at: at(20) }), filter, NO_PROJECTS, NOW)).toBe(false);
    expect(matchesFilter(task({ column_id: "backlog", due_at: at(10) }), filter, NO_PROJECTS, NOW)).toBe(false);
  });
});

describe("the custom due range", () => {
  const day = (d: number) => new Date(2026, 9, d).getTime();
  const range = (from: number | null, to: number | null) => ({
    ...DEFAULT_FILTER,
    due: "custom" as const,
    dueRange: { from: from == null ? null : day(from), to: to == null ? null : day(to) },
  });
  const hit = (dueAt: string | null, filter: ReturnType<typeof range>) =>
    matchesFilter(task({ due_at: dueAt }), filter, NO_PROJECTS, NOW);

  test("the last day counts whole, the next midnight does not", () => {
    const f = range(12, 14);
    expect(hit(new Date(2026, 9, 14, 23, 59, 59).toISOString(), f)).toBe(true);
    expect(hit(new Date(2026, 9, 15, 0, 0, 0).toISOString(), f)).toBe(false);
    expect(hit(new Date(2026, 9, 12, 0, 0, 0).toISOString(), f)).toBe(true);
    expect(hit(new Date(2026, 9, 11, 23, 59, 59).toISOString(), f)).toBe(false);
  });

  test("either end may be open", () => {
    expect(hit(at(2), range(null, 14))).toBe(true);
    expect(hit(at(15), range(null, 14))).toBe(false);
    expect(hit(at(30), range(20, null))).toBe(true);
    expect(hit(at(19), range(20, null))).toBe(false);
  });

  test("an SQLite UTC due date is read as UTC, an ISO one as its instant", () => {
    const f = range(12, 12);
    const local = new Date(2026, 9, 12, 10);
    const sqlite = local.toISOString().slice(0, 19).replace("T", " ");
    expect(hit(sqlite, f)).toBe(true);
    expect(hit(local.toISOString(), f)).toBe(true);
    const next = new Date(2026, 9, 13, 10);
    expect(hit(next.toISOString().slice(0, 19).replace("T", " "), f)).toBe(false);
  });

  test("a task without a due date never matches a range", () => {
    expect(hit(null, range(12, 14))).toBe(false);
    expect(hit(null, range(null, 14))).toBe(false);
  });

  test("a Custom range without ends matches Any time and still counts as changed", () => {
    const f = range(null, null);
    expect(hit(null, f)).toBe(true);
    expect(hit(at(30), f)).toBe(true);
    expect(popoverFilterCount(f)).toBe(1);
    expect(isDefaultFilter(f)).toBe(false);
    expect(isDefaultFilter(DEFAULT_FILTER)).toBe(true);
    expect(popoverFilterCount(range(12, null))).toBe(1);
  });

  test("swapped bounds are swapped back, in the predicate too", () => {
    expect(normaliseRange({ from: day(14), to: day(12) })).toEqual({ from: day(12), to: day(14) });
    expect(hit(at(13), range(14, 12))).toBe(true);
    expect(hit(at(15), range(14, 12))).toBe(false);
  });

  test("project and subject are not the Filter popover's", () => {
    const f = { ...DEFAULT_FILTER, project: "unfiled" as const, subject: "personal" };
    expect(popoverFilterCount(f)).toBe(0);
    expect(activeFilterCount(f)).toBe(2);
  });
});

describe("sorting", () => {
  const a = task({ id: 101, title: "b", due_at: at(20), updated_at: "2026-10-03 00:00:00", created_at: "2026-10-05 00:00:00" });
  const b = task({ id: 102, title: "a", due_at: at(16), updated_at: "2026-10-04 00:00:00", created_at: "2026-10-02 00:00:00" });
  const c = task({ id: 103, title: "c", due_at: null, updated_at: "2026-10-02 00:00:00", created_at: "2026-10-03 00:00:00" });
  const ids = (list: { id: number }[]) => list.map((t) => t.id);

  test("Due puts undated last in both directions", () => {
    expect(ids(sortTasks([c, a, b], { key: "due", dir: "asc" }, NO_PROJECTS))).toEqual([102, 101, 103]);
    expect(ids(sortTasks([c, a, b], { key: "due", dir: "desc" }, NO_PROJECTS))).toEqual([101, 102, 103]);
  });

  test("Updated and Created compare as instants, newest first when descending", () => {
    expect(ids(sortTasks([a, b, c], { key: "updated", dir: "desc" }, NO_PROJECTS))).toEqual([102, 101, 103]);
    expect(ids(sortTasks([a, b, c], { key: "created", dir: "asc" }, NO_PROJECTS))).toEqual([102, 103, 101]);
  });

  test("Title sorts by name and does not mutate its input", () => {
    const input = [a, b, c];
    expect(ids(sortTasks(input, { key: "title", dir: "asc" }, NO_PROJECTS))).toEqual([102, 101, 103]);
    expect(ids(input)).toEqual([101, 102, 103]);
  });

  test("Status follows the board's column order, ties falling to the default order", () => {
    const todo = task({ id: 201, column_id: "todo" });
    const backlog = task({ id: 202, column_id: "backlog" });
    const done = task({ id: 203, column_id: "done" });
    expect(ids(sortTasks([done, todo, backlog], { key: "status", dir: "asc" }, NO_PROJECTS))).toEqual([202, 201, 203]);
    expect(ids(sortTasks([done, todo, backlog], { key: "status", dir: "desc" }, NO_PROJECTS))).toEqual([203, 201, 202]);
  });
});

describe("grouping", () => {
  const ctx = { projectById: NO_PROJECTS, tags: [tag(1, "Launch"), tag(2, "Product")], now: NOW };

  test("none is one group holding everything", () => {
    const groups = groupTasks([task(), task()], "none", ctx);
    expect(groups).toHaveLength(1);
    expect(groups[0].tasks).toHaveLength(2);
  });

  test("a task with several tags appears under each, untagged ones under No tag last", () => {
    const both = task({ id: 301, tag_ids: [1, 2] });
    const only = task({ id: 302, tag_ids: [2] });
    const none = task({ id: 303 });
    const groups = groupTasks([both, only, none], "tag", ctx);
    expect(groups.map((g) => [g.label, g.tasks.map((t) => t.id)])).toEqual([
      ["Launch", [301]],
      ["Product", [301, 302]],
      ["No tag", [303]],
    ]);
    expect(groups[0].tag?.id).toBe(1);
  });

  test("status groups follow the board and skip empty ones", () => {
    const groups = groupTasks([task({ column_id: "done" }), task({ column_id: "backlog" })], "status", ctx);
    expect(groups.map((g) => g.label)).toEqual(["Planning", "Done"]);
  });

  test("project groups put No project first, then names", () => {
    const groups = groupTasks(
      [
        task({ project_id: 2, project_name: "Zed" }),
        task({ project_id: 1, project_name: "Alpha" }),
        task(),
      ],
      "project",
      ctx,
    );
    expect(groups.map((g) => g.label)).toEqual(["No project", "Alpha", "Zed"]);
  });

  test("due groups: Overdue, Today, This week, Later, No date", () => {
    const rows = {
      overdue: task({ due_at: at(13) }),
      today: task({ due_at: at(14, 18) }),
      week: task({ due_at: at(17) }),
      later: task({ due_at: at(25) }),
      undated: task(),
    };
    expect(dueBucket(rows.overdue, NOW)).toBe("overdue");
    expect(dueBucket(rows.today, NOW)).toBe("today");
    expect(dueBucket(rows.week, NOW)).toBe("week");
    expect(dueBucket(rows.later, NOW)).toBe("later");
    expect(dueBucket(rows.undated, NOW)).toBe("none");
    const groups = groupTasks(Object.values(rows).reverse(), "due", ctx);
    expect(groups.map((g) => g.label)).toEqual(["Overdue", "Today", "This week", "Later", "No date"]);
  });

  test("a finished task past its due date is Earlier, never Overdue", () => {
    const done = task({ due_at: at(10), done_at: "2026-10-10 00:00:00" });
    expect(dueBucket(done, NOW)).toBe("earlier");
    const earlierToday = task({ due_at: at(14, 8), done_at: "2026-10-14 08:00:00" });
    expect(dueBucket(earlierToday, NOW)).toBe("today");
  });
});


describe("archived visibility and status toggles", () => {
  const project: DbProject = {
    id: 7, name: "Archive", status: "archived", columns: [...DEFAULT_COLUMNS],
    subject_id: null, subject_code: null, brief: null, starts_at: null, due_at: null,
    tags: [], event_id: null, position: 0, source: "manual",
    created_at: "2026-10-01 00:00:00", updated_at: "2026-10-01 00:00:00",
  };
  const projects = new Map([[project.id, project]]);

  test("archived parents and children are hidden by default; unfiled work stays", () => {
    for (const parent_id of [null, 20]) {
      const archived = task({ project_id: 7, parent_id });
      expect(matchesFilter(archived, DEFAULT_FILTER, projects, NOW)).toBe(false);
      expect(matchesFilter(archived, { ...DEFAULT_FILTER, showArchived: true }, projects, NOW)).toBe(true);
    }
    expect(matchesFilter(task(), DEFAULT_FILTER, projects, NOW)).toBe(true);
    expect(matchesFilter(task({ project_id: 7 }), DEFAULT_FILTER, new Map([[7, { ...project, status: "active" }]]), NOW)).toBe(true);
  });

  test("picking from Any isolates one status; clearing the last restores Any", () => {
    expect(toggleStatusFilter(DEFAULT_FILTER.status, "todo")).toEqual(["todo"]);
    expect(toggleStatusFilter(["todo"], "doing")).toEqual(["todo", "doing"]);
    expect(toggleStatusFilter(["todo", "doing"], "todo")).toEqual(["doing"]);
    expect(toggleStatusFilter(["doing"], "doing")).toEqual(DEFAULT_FILTER.status);
    expect(popoverFilterCount({ ...DEFAULT_FILTER, showArchived: true })).toBe(1);
  });

  test("explicit Select all stays selected and can be narrowed, while Reset means Any", () => {
    const all = filteredColumns(DEFAULT_FILTER).map((c) => c.id);
    const selected = { ...DEFAULT_FILTER, status: all };
    expect(activeFilterCount(selected)).toBe(1);
    expect(isDefaultFilter(selected)).toBe(false);
    expect(filteredColumns(selected)).toEqual(filteredColumns(DEFAULT_FILTER));
    expect(matchesFilter(task({ column_id: "done" }), selected, NO_PROJECTS, NOW)).toBe(true);
    expect(toggleStatusFilter(all, "todo")).toEqual(all.filter((id) => id !== "todo"));
    expect(filteredColumns({ ...selected, status: DEFAULT_FILTER.status })).toEqual(filteredColumns(DEFAULT_FILTER));
  });
});

describe("nesting filtered subtasks", () => {
  const parent = task({ id: 800, title: "Parent", column_id: "todo" });
  const child = task({ id: 801, parent_id: 800, title: "Child", column_id: "doing" });
  const other = task({ id: 802, title: "Other" });

  test("a child sorted before its parent nests under it, preserving sibling order", () => {
    const second = task({ id: 803, parent_id: 800 });
    const nodes = taskTree([child, other, parent, second]);
    expect(nodes.map((n) => n.task.id)).toEqual([802, 800]);
    expect(nodes[1].children.map((t) => t.id)).toEqual([801, 803]);
    expect(nodes[1].children[0].project_name).toBeNull();
  });

  test("filtering out a parent promotes its matching child instead of hiding it", () => {
    const nodes = taskTree([child, other]);
    expect(nodes.map((n) => n.task.id)).toEqual([801, 802]);
    expect(nodes[0].children).toEqual([]);
  });

  test("parent grouping keeps a child of another status under its parent", () => {
    const nodes = taskTree([child, parent]);
    const groups = groupTasks(nodes.map((n) => n.task), "status", { projectById: NO_PROJECTS, tags: [], now: NOW });
    expect(groups.map((g) => g.key)).toEqual(["todo"]);
    expect(nodes[0].children[0].column_id).toBe("doing");
  });
});
