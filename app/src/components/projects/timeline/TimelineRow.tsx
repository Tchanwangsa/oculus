import { useMemo } from "react";
import { Flag } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { fmtClock, sqliteUtcToMs } from "@/lib/format/format";
import { packLanes } from "@/lib/planning/lanes";
import type { DbProject, DbProjectTask } from "@/lib/planning/projects";
import { SubtaskToggle, TaskGlyph } from "../tasks/TaskMarks";
import { columnOf } from "../tasks/taskTree";
import { BAR_PX, NAME_PX, ROW_PX, SUB_BAR_PX, SUB_ROW_PX } from "./geometry";
import { laneSpan, type Row } from "./place";

function toneOf(task: DbProjectTask, now: Date): string {
  if (task.done_at != null) return "var(--color-chart-other)";
  const due = sqliteUtcToMs(task.due_at);
  if (due != null && due < now.getTime()) return "var(--color-destructive)";
  return "var(--color-brand)";
}

function rangeLabel(task: DbProjectTask): string {
  const s = sqliteUtcToMs(task.starts_at);
  const d = sqliteUtcToMs(task.due_at);
  const est = task.estimate_minutes ? ` · ${task.estimate_minutes} min` : "";
  if (s != null && d != null) return `${task.title}\n${fmtClock(s, true)} → ${fmtClock(d, true)}${est}`;
  if (d != null) return `${task.title}\nDue ${fmtClock(d, true)}${est}`;
  if (s != null) return `${task.title}\nStarts ${fmtClock(s, true)} · no due date${est}`;
  return task.title;
}

export function TimelineRow({
  project,
  row,
  now,
  total,
  expanded,
  onToggle,
}: {
  project: DbProject;
  row: Row;
  now: Date;
  total: number;
  expanded: boolean;
  onToggle: () => void;
}) {
  const height = row.depth === 0 ? ROW_PX : SUB_ROW_PX;
  const laid = useMemo(() => packLanes(row.items, laneSpan), [row.items]);

  return (
    <div
      className="group/row flex border-b border-border-subtle/60 transition-colors hover:bg-surface"
      style={{ height }}
    >
      <div
        className={cn(
          "sticky left-0 z-20 flex shrink-0 items-center gap-1.5 bg-card px-5 group-hover/row:bg-surface",
          row.depth === 1 && "pl-10",
        )}
        style={{ width: NAME_PX }}
      >
        <SubtaskToggle expandable={row.depth === 0 && row.expandable} expanded={expanded} onToggle={onToggle} />
        <TaskGlyph
          kind={columnOf(project, row.task.column_id)?.kind ?? null}
          size={row.depth === 0 ? 12 : 10}
        />
        <span
          title={row.task.title}
          className={cn(
            "truncate",
            row.depth === 0 ? "text-[11.5px]" : "text-[11px]",
            row.task.done_at
              ? "text-muted-foreground line-through"
              : row.depth === 0
                ? "text-foreground"
                : "text-muted-foreground",
          )}
        >
          {row.task.title}
        </span>
      </div>

      <div className="relative" style={{ width: total, height }}>
        {laid.map(({ item, lane, of }) => {
          const lanePx = height / of;
          const base = row.depth === 0 ? BAR_PX : SUB_BAR_PX;
          const h = Math.max(6, Math.min(base, lanePx - 3));
          const top = lane * lanePx + (lanePx - h) / 2;
          const tone = toneOf(item.task, now);
          const title = rangeLabel(item.task);

          if (item.instant) {
            const size = row.depth === 0 && !item.rollup ? 11 : 9;
            return (
              <div
                key={item.task.id}
                title={title}
                className="absolute flex items-center justify-center"
                style={{ left: item.left, width: item.width, top, height: h }}
              >
                {item.deadline ? (
                  <Flag size={size} weight="fill" className="shrink-0" style={{ color: tone }} />
                ) : (
                  <span
                    className="shrink-0 rounded-full"
                    style={{
                      width: size - 3,
                      height: size - 3,
                      backgroundColor: tone,
                    }}
                  />
                )}
              </div>
            );
          }

          return (
            <div
              key={item.task.id}
              title={title}
              className="absolute overflow-hidden rounded-[4px] border-l-2 px-1.5"
              style={{
                left: item.left,
                width: item.width,
                top,
                height: h,
                borderLeftColor: tone,
                backgroundColor: `color-mix(in srgb, ${tone} ${
                  item.rollup ? 12 : 18
                }%, var(--color-card))`,
              }}
            >
              {item.width > 56 && h >= 11 && (
                <span
                  className={cn(
                    "block truncate text-[10px] leading-[11px]",
                    item.task.done_at ? "text-muted-foreground" : "text-foreground",
                  )}
                >
                  {item.task.title}
                </span>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
