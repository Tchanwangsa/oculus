import { describe, expect, test } from "bun:test";
import {
  DEFAULT_FILTER,
  activeFilterCount,
  matchesFilter,
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

  test("project groups put Unfiled first, then names", () => {
    const groups = groupTasks(
      [
        task({ project_id: 2, project_name: "Zed" }),
        task({ project_id: 1, project_name: "Alpha" }),
        task(),
      ],
      "project",
      ctx,
    );
    expect(groups.map((g) => g.label)).toEqual(["Unfiled", "Alpha", "Zed"]);
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
