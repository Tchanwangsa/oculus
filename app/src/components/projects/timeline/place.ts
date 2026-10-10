import { sqliteUtcToMs } from "@/lib/format/format";
import type { DbProjectTask } from "@/lib/planning/projects";
import type { TaskNode } from "../tasks/taskTree";
import { MARKER_PX, MIN_BAR_PX } from "./geometry";

export interface Placed {
  task: DbProjectTask;
  /** Only one date, drawn as a marker. */
  instant: boolean;
  /** The one date is `due_at`, not `starts_at`. */
  deadline: boolean;
  left: number;
  width: number;
  /** A subtask drawn on its collapsed parent's row. */
  rollup: boolean;
}

export interface Row {
  node: TaskNode;
  task: DbProjectTask;
  depth: 0 | 1;
  items: Placed[];
  expandable: boolean;
}

/** Null means undated (the Unscheduled rail). `due_at <= starts_at` is bad
 *  data and falls back to a deadline marker. */
export function place(
  task: DbProjectTask,
  x: (ms: number) => number,
  total: number,
  rollup: boolean,
): Placed | null {
  const s = sqliteUtcToMs(task.starts_at);
  const d = sqliteUtcToMs(task.due_at);
  if (s == null && d == null) return null;

  const clamp = (left: number, width: number) => ({
    left: Math.min(Math.max(left, 0), Math.max(0, total - width)),
    width,
  });

  if (s != null && d != null && d > s) {
    const left = x(s);
    return {
      task,
      instant: false,
      deadline: false,
      rollup,
      ...clamp(left, Math.max(MIN_BAR_PX, x(d) - left)),
    };
  }
  const at = d ?? (s as number);
  return {
    task,
    instant: true,
    deadline: d != null,
    rollup,
    ...clamp(x(at) - MARKER_PX / 2, MARKER_PX),
  };
}

export function laneSpan(p: Placed) {
  return { start: p.left, end: p.left + p.width + 2 };
}
