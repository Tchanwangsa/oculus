import { useEffect, useMemo, useRef, useState } from "react";
import { cn } from "@/lib/utils";
import { useNow } from "@/hooks/ui/useNow";
import { DAY_MS } from "@/lib/planning/calendar";
import { sqliteUtcToMs, startOfDay } from "@/lib/format/format";
import type { DbProject, DbProjectTask } from "@/lib/planning/projects";
import { columnOf, type TaskNode } from "../tasks/taskTree";
import { NAME_PX, PX_PER_DAY, bandsFor, ticksFor, timelineRange } from "./geometry";
import { place, type Placed, type Row } from "./place";
import { TimelineRow } from "./TimelineRow";
import { UnscheduledRail } from "./UnscheduledRail";
import type { TimelineZoom } from "./zoom";

export {
  TIMELINE_ZOOM_KEY,
  TimelineZoomControl,
  isTimelineZoom,
  type TimelineZoom,
} from "./zoom";

/**
 * The roadmap: one row per top-level task on a horizontal time axis.
 * Read-only by design — tasks move on the board and table, through `moveTask`.
 * Follows `WeekView`'s conventions: span = bar, single date = marker, overlaps
 * packed into lanes, elapsed time washed grey, range fitted to contents.
 */
export function ProjectTimeline({
  project,
  nodes,
  zoom,
}: {
  project: DbProject;
  nodes: TaskNode[];
  zoom: TimelineZoom;
}) {
  const now = useNow();
  const todayMs = startOfDay(now).getTime();
  const [expanded, setExpanded] = useState<Set<number>>(new Set());
  const scroller = useRef<HTMLDivElement>(null);

  const toggle = (id: number) =>
    setExpanded((prev) => {
      const next = new Set(prev);
      if (!next.delete(id)) next.add(id);
      return next;
    });

  // Backlog tasks (and their subtasks) are uncommitted, so not scheduled.
  const onBoard = useMemo(
    () => nodes.filter((n) => columnOf(project, n.task.column_id)?.kind !== "backlog"),
    [nodes, project],
  );
  const parked = nodes.length - onBoard.length;

  const [rangeStart, rangeEnd] = useMemo(() => {
    const points: number[] = [];
    for (const n of onBoard) {
      for (const t of [n.task, ...n.children]) {
        const s = sqliteUtcToMs(t.starts_at);
        const d = sqliteUtcToMs(t.due_at);
        if (s != null) points.push(s);
        if (d != null) points.push(d);
      }
    }
    return timelineRange(points, new Date(todayMs), zoom);
  }, [onBoard, todayMs, zoom]);

  const origin = rangeStart.getTime();
  const px = PX_PER_DAY[zoom];
  const x = useMemo(
    () => (ms: number) => ((ms - origin) / DAY_MS) * px,
    [origin, px],
  );
  const total = x(rangeEnd.getTime());

  const ticks = useMemo(
    () => ticksFor(zoom, rangeStart, rangeEnd, new Date(todayMs)),
    [zoom, rangeStart, rangeEnd, todayMs],
  );
  const bands = useMemo(() => bandsFor(zoom, rangeStart, rangeEnd), [zoom, rangeStart, rangeEnd]);

  const { rows, unscheduled } = useMemo(() => {
    const rows: Row[] = [];
    const unscheduled: DbProjectTask[] = [];

    for (const node of onBoard) {
      const own = place(node.task, x, total, false);
      const kids = node.children.map((c) => ({ task: c, at: place(c, x, total, false) }));
      const dated = kids.filter((k) => k.at != null);
      const undated = kids.filter((k) => k.at == null).map((k) => k.task);

      if (!own && dated.length === 0) {
        unscheduled.push(node.task, ...undated);
        continue;
      }
      unscheduled.push(...undated);

      const open = expanded.has(node.task.id);
      const rollups = open
        ? []
        : dated.map((k) => place(k.task, x, total, true)).filter((p): p is Placed => p != null);

      rows.push({
        node,
        task: node.task,
        depth: 0,
        items: [...(own ? [own] : []), ...rollups],
        expandable: dated.length > 0,
      });

      if (open) {
        for (const k of dated) {
          rows.push({
            node,
            task: k.task,
            depth: 1,
            items: [k.at as Placed],
            expandable: false,
          });
        }
      }
    }
    return { rows, unscheduled };
  }, [onBoard, expanded, x, total]);

  // Open on today. Not keyed on `now`, or each minute tick would reset the
  // scroll.
  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    el.scrollLeft = Math.max(0, ((Date.now() - origin) / DAY_MS) * px - NAME_PX);
  }, [origin, px, project.id]);

  const nowX = Math.min(Math.max(x(now.getTime()), 0), total);

  if (rows.length === 0 && unscheduled.length === 0) {
    return (
      <p className="px-6 py-16 text-center text-xs text-muted-foreground">
        {parked > 0
          ? "Every task is still in the backlog. Commit one to a column and give it dates, and it will take a row here."
          : "No tasks yet. Break the project up on the board first — a task lands here once it has a start or a due date."}
      </p>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <div ref={scroller} className="min-h-0 flex-1 overflow-auto">
        <div style={{ width: NAME_PX + total, minWidth: "100%" }}>
          <div className="sticky top-0 z-30 flex bg-card">
            <div
              className="sticky left-0 z-10 shrink-0 border-b border-border-subtle bg-card"
              style={{ width: NAME_PX }}
            />
            <div
              className="relative border-b border-border-subtle"
              style={{ width: total, height: 42 }}
            >
              {bands.map((b) => {
                const left = Math.max(0, x(b.at.getTime()));
                const width = Math.min(total, x(b.end.getTime())) - left;
                if (width <= 0) return null;
                return (
                  <div
                    key={b.at.toISOString()}
                    className="absolute top-0 flex h-5 items-center overflow-hidden border-l border-border-subtle px-1.5"
                    style={{ left, width }}
                  >
                    {width >= 44 && (
                      <span className="truncate text-[10.5px] font-medium text-muted-foreground">
                        {width >= 104 ? b.label : b.label.split(" ")[0]}
                      </span>
                    )}
                  </div>
                );
              })}
              {ticks.map((t) => (
                <div
                  key={t.at.toISOString()}
                  className={cn(
                    "absolute bottom-0 top-5 flex items-center justify-center overflow-hidden border-l",
                    t.major ? "border-border" : "border-border-subtle",
                  )}
                  style={{
                    left: x(t.at.getTime()),
                    width: x(t.end.getTime()) - x(t.at.getTime()),
                  }}
                >
                  <span
                    className={cn(
                      "inline-flex h-4 min-w-4 items-center justify-center rounded-full px-1 text-[10px] tabular-nums",
                      t.today
                        ? "bg-primary font-semibold text-primary-foreground"
                        : "text-muted-foreground",
                    )}
                  >
                    {t.label}
                  </span>
                </div>
              ))}
            </div>
          </div>

          {/* One shared overlay of gridlines, past wash and now line. */}
          <div className="relative">
            <div
              className="pointer-events-none absolute inset-y-0"
              style={{ left: NAME_PX, width: total }}
            >
              {ticks.map((t) => (
                <div
                  key={t.at.toISOString()}
                  className={cn(
                    "absolute inset-y-0 border-l",
                    t.major ? "border-border" : "border-border-subtle/60",
                  )}
                  style={{ left: x(t.at.getTime()) }}
                />
              ))}
              {/* Mixed from muted-foreground: `surface` is invisible as a wash. */}
              {nowX > 0 && (
                <div
                  className="absolute inset-y-0 left-0"
                  style={{
                    width: nowX,
                    backgroundColor:
                      "color-mix(in srgb, var(--color-muted-foreground) 9%, transparent)",
                  }}
                />
              )}
              <div
                className="absolute inset-y-0 z-10 border-l border-destructive"
                style={{ left: nowX }}
              >
                <span className="absolute -left-[3px] -top-px block h-1.5 w-1.5 rounded-full bg-destructive" />
              </div>
            </div>

            {rows.map((row) => (
              <TimelineRow
                key={`${row.task.id}-${row.depth}`}
                project={project}
                row={row}
                now={now}
                total={total}
                onToggle={() => toggle(row.node.task.id)}
                expanded={expanded.has(row.node.task.id)}
              />
            ))}
          </div>
        </div>
      </div>

      {unscheduled.length > 0 && (
        <UnscheduledRail project={project} tasks={unscheduled} />
      )}
    </div>
  );
}
