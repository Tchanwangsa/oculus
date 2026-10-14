import { sqliteUtcToMs, startOfDay } from "@/lib/format/format";
import { addDays, startOfWeek } from "@/lib/planning/calendar/dates";
import type { DbProject, DbTaskTag, DbTaskWithProject } from "@/lib/planning/projects";
import { UNIVERSAL_COLUMNS, projectLabel, universalColumnOf } from "./universalTasks";

export const GROUP_BYS = ["none", "status", "project", "tag", "due"] as const;

export type GroupBy = (typeof GROUP_BYS)[number];

export const GROUP_LABELS: Record<GroupBy, string> = {
  none: "None",
  status: "Status",
  project: "Project",
  tag: "Tag",
  due: "Due date",
};

export function isGroupBy(value: unknown): value is GroupBy {
  return (GROUP_BYS as readonly unknown[]).includes(value);
}

export interface TaskGroup {
  /** Stable across renders, for the collapsed set. */
  key: string;
  label: string;
  /** Set on a tag's group, which draws the tag's dot. */
  tag?: DbTaskTag;
  /** In the order the caller passed them. */
  tasks: DbTaskWithProject[];
}

const DUE_BUCKETS = [
  { key: "overdue", label: "Overdue" },
  { key: "today", label: "Today" },
  { key: "week", label: "This week" },
  { key: "later", label: "Later" },
  { key: "earlier", label: "Earlier" },
  { key: "none", label: "No date" },
] as const;

type DueBucket = (typeof DUE_BUCKETS)[number]["key"];

/**
 * Where a task's due date falls. Overdue means unfinished and past the
 * moment, like the red on `DueChip`; a finished task that is past due is
 * Earlier, so Overdue never lists work that is done. The rest are calendar
 * days, "This week" being the calendar's week after today.
 */
export function dueBucket(task: DbTaskWithProject, now: number): DueBucket {
  const ms = sqliteUtcToMs(task.due_at);
  if (ms == null) return "none";
  if (ms < now && task.done_at == null) return "overdue";
  const today = new Date(now);
  const from = startOfDay(today).getTime();
  if (ms < from) return "earlier";
  if (ms < addDays(today, 1).getTime()) return "today";
  if (ms < addDays(startOfWeek(today), 7).getTime()) return "week";
  return "later";
}

/** `tasks` split into non-empty groups; a task with several tags is in each
 *  tag's group. `"none"` is one unlabeled group. */
export function groupTasks(
  tasks: readonly DbTaskWithProject[],
  by: GroupBy,
  ctx: { projectById: Map<number, DbProject>; tags: readonly DbTaskTag[]; now: number },
): TaskGroup[] {
  const groups = new Map<string, TaskGroup>();
  const add = (key: string, label: string, task: DbTaskWithProject, tag?: DbTaskTag) => {
    let group = groups.get(key);
    if (!group) groups.set(key, (group = { key, label, tag, tasks: [] }));
    group.tasks.push(task);
  };

  let order: string[] | null = null;
  switch (by) {
    case "none":
      for (const t of tasks) add("all", "", t);
      break;
    case "status":
      for (const t of tasks) {
        const column = universalColumnOf(t, ctx.projectById);
        add(column.id, column.name, t);
      }
      order = UNIVERSAL_COLUMNS.map((c) => c.id);
      break;
    case "project":
      for (const t of tasks) add(t.project_id == null ? "unfiled" : `p${t.project_id}`, projectLabel(t), t);
      break;
    case "tag":
      for (const t of tasks) {
        const known = t.tag_ids.flatMap((id) => ctx.tags.find((g) => g.id === id) ?? []);
        if (known.length === 0) add("none", "No tag", t);
        for (const tag of known) add(`t${tag.id}`, tag.name, t, tag);
      }
      order = [...ctx.tags.map((g) => `t${g.id}`), "none"];
      break;
    case "due":
      for (const t of tasks) {
        const bucket = dueBucket(t, ctx.now);
        add(bucket, DUE_BUCKETS.find((b) => b.key === bucket)!.label, t);
      }
      order = DUE_BUCKETS.map((b) => b.key);
      break;
  }

  const list = [...groups.values()];
  if (order) {
    const rank = (key: string) => order.indexOf(key);
    list.sort((a, b) => rank(a.key) - rank(b.key));
  } else if (by === "project") {
    // Unfiled first, then by name.
    list.sort((a, b) =>
      a.key === "unfiled" ? -1 : b.key === "unfiled" ? 1 : a.label.localeCompare(b.label),
    );
  }
  return list;
}
