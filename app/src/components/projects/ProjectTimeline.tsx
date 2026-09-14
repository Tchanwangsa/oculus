import { useEffect, useMemo, useRef, useState } from "react";
import { CaretRight, Flag } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { useNow } from "@/hooks/useNow";
import {
  DAY_MS,
  addDays,
  addMonths,
  fmtMonth,
  startOfDay,
  startOfWeek,
} from "@/lib/calendar";
import { packLanes } from "@/lib/lanes";
import { fmtClock, sqliteUtcToMs } from "@/lib/format";
import type { DbProject, DbProjectTask } from "@/lib/projects";
import { TaskGlyph } from "./TaskMarks";
import { columnOf, type TaskNode } from "./taskTree";

/**
 * The roadmap: one row per top-level task, laid along a horizontal time axis.
 *
 * This view **reads**. Tasks are edited on the board and in the table, which
 * own the one door `moveTask` provides for where a task sits; a bar you could
 * drag here would be a second one. Nothing in this file writes, and nothing
 * here reads the database either — the rows arrive as props from
 * `ProjectPage`, which is what listens for `PROJECTS_UPDATED_EVENT`.
 *
 * The shape borrows from `app/src/components/calendar/WeekView.tsx` wherever
 * the two are answering the same question: a span is a bar and a single date
 * is a marker (`isInstant`), overlapping bars are packed into lanes
 * (`app/src/lib/lanes.ts`), elapsed time carries a grey wash mixed from
 * `muted-foreground`, a finished item drops its colour for the neutral
 * `chart-other`, and the drawn range is fitted to its contents rather than
 * guessed.
 */

// ── Zoom ─────────────────────────────────────────────────────────────────────

export type TimelineZoom = "day" | "week" | "month";

/** The column the axis is ruled in. Not a CSS `zoom` on anything — this only
 *  changes how many pixels a day is worth, so pointer coordinates and element
 *  rects stay in the same space (see the root `CLAUDE.md`). */
export const TIMELINE_ZOOMS = [
  { id: "day", label: "Days" },
  { id: "week", label: "Weeks" },
  { id: "month", label: "Months" },
] as const satisfies ReadonlyArray<{ id: TimelineZoom; label: string }>;

export const TIMELINE_ZOOM_KEY = "oculus-project-timeline-zoom";

export function isTimelineZoom(v: string | null): v is TimelineZoom {
  return v === "day" || v === "week" || v === "month";
}

/** The segmented control for the axis, in the page's toolbar rather than in
 *  this component — it scales the view, so it belongs with the other chrome.
 *  Rectangular on purpose: a segmented toolbar is the one exception to the
 *  pill rule, and this is the same control `CalendarPage` uses. */
export function TimelineZoomControl({
  value,
  onChange,
}: {
  value: TimelineZoom;
  onChange: (value: TimelineZoom) => void;
}) {
  return (
    <div className="flex shrink-0 items-center rounded-md border border-border p-0.5">
      {TIMELINE_ZOOMS.map((z) => (
        <button
          key={z.id}
          type="button"
          onClick={() => onChange(z.id)}
          className={cn(
            "cursor-pointer rounded-[4px] px-2 py-1 text-[11.5px] font-medium transition-colors",
            value === z.id
              ? "bg-surface-raised text-foreground"
              : "text-muted-foreground hover:text-foreground",
          )}
        >
          {z.label}
        </button>
      ))}
    </div>
  );
}

// ── Geometry ─────────────────────────────────────────────────────────────────

/** Pixels a single day is worth at each zoom. Everything else — bar widths,
 *  tick spacing, where "now" falls — is this times a number of days. */
const PX_PER_DAY: Record<TimelineZoom, number> = { day: 52, week: 22, month: 5 };

/** Breathing room either side of the dated work, so the first bar does not
 *  start flush against the left edge. */
const PAD_DAYS: Record<TimelineZoom, number> = { day: 2, week: 7, month: 20 };

/** A floor on the drawn span, the way `hourRange` never goes narrower than
 *  8am–6pm: a project with one dated task should still read as a calendar
 *  rather than as a single column. */
const MIN_SPAN_DAYS: Record<TimelineZoom, number> = { day: 14, week: 56, month: 180 };

const NAME_PX = 208;
const ROW_PX = 30;
const SUB_ROW_PX = 22;
const BAR_PX = 13;
const SUB_BAR_PX = 9;
/** Width of a single-date marker, and the shortest a bar is ever drawn. Both
 *  feed the lane packing, which runs on drawn pixels rather than on times —
 *  lanes exist to stop things overlapping on screen, and a one-hour task and a
 *  deadline both occupy more screen than their duration. */
const MARKER_PX = 12;
const MIN_BAR_PX = 10;

/**
 * The span the axis covers: everything dated, padded either side, never
 * narrower than the zoom's floor, and always containing "now".
 *
 * The last clause is `hourRange`'s `includeHour` argument by another name — a
 * today marker the range has cropped off is worse than no marker at all.
 */
function timelineRange(points: number[], now: Date, zoom: TimelineZoom): [Date, Date] {
  let lo = now.getTime();
  let hi = now.getTime();
  for (const p of points) {
    lo = Math.min(lo, p);
    hi = Math.max(hi, p);
  }

  let start = addDays(startOfDay(new Date(lo)), -PAD_DAYS[zoom]);
  let end = addDays(startOfDay(new Date(hi)), PAD_DAYS[zoom] + 1);

  const short = MIN_SPAN_DAYS[zoom] - Math.round((end.getTime() - start.getTime()) / DAY_MS);
  if (short > 0) {
    start = addDays(start, -Math.floor(short / 2));
    end = addDays(end, Math.ceil(short / 2));
  }

  // Snap to the unit the axis is ruled in, so the first tick is a whole week
  // or a whole month rather than a stub.
  if (zoom === "week") {
    start = startOfWeek(start);
    end = addDays(startOfWeek(end), 7);
  } else if (zoom === "month") {
    start = addMonths(start, 0);
    end = addMonths(end, 1);
  }
  return [start, end];
}

interface Tick {
  at: Date;
  /** Where the next tick starts — the column's right edge. */
  end: Date;
  label: string;
  /** Ruled a little harder: a month boundary inside a day or week axis. */
  major: boolean;
  today: boolean;
}

function ticksFor(zoom: TimelineZoom, start: Date, end: Date, now: Date): Tick[] {
  const out: Tick[] = [];
  const limit = end.getTime();
  if (zoom === "month") {
    for (let d = addMonths(start, 0); d.getTime() < limit; d = addMonths(d, 1)) {
      out.push({
        at: d,
        end: addMonths(d, 1),
        label: d.toLocaleDateString("en-AU", { month: "short" }),
        major: d.getMonth() === 0,
        today: false,
      });
    }
    return out;
  }
  const step = zoom === "week" ? 7 : 1;
  for (let d = start; d.getTime() < limit; d = addDays(d, step)) {
    out.push({
      at: d,
      end: addDays(d, step),
      label:
        zoom === "week"
          ? d.toLocaleDateString("en-AU", { day: "numeric", month: "short" })
          : String(d.getDate()),
      major: zoom === "week" ? false : d.getDate() === 1,
      today:
        zoom === "day" &&
        d.getFullYear() === now.getFullYear() &&
        d.getMonth() === now.getMonth() &&
        d.getDate() === now.getDate(),
    });
  }
  return out;
}

interface Band {
  at: Date;
  end: Date;
  label: string;
}

/** The row above the ticks: months over a day or week axis, years over a
 *  month one — whichever unit the ticks are not already naming. */
function bandsFor(zoom: TimelineZoom, start: Date, end: Date): Band[] {
  const out: Band[] = [];
  if (zoom === "month") {
    for (
      let y = new Date(start.getFullYear(), 0, 1);
      y.getTime() < end.getTime();
      y = new Date(y.getFullYear() + 1, 0, 1)
    ) {
      out.push({
        at: y,
        end: new Date(y.getFullYear() + 1, 0, 1),
        label: String(y.getFullYear()),
      });
    }
    return out;
  }
  for (let m = addMonths(start, 0); m.getTime() < end.getTime(); m = addMonths(m, 1)) {
    out.push({ at: m, end: addMonths(m, 1), label: fmtMonth(m) });
  }
  return out;
}

// ── Placing tasks ────────────────────────────────────────────────────────────

/** A task drawn on a row: a bar between two dates, or a marker at one. */
interface Placed {
  task: DbProjectTask;
  /** One date to go on, so there is no span to draw — the distinction
   *  `isInstant` makes on the calendar, for the same reason: a deadline is a
   *  moment, not a stretch of time you are booked for. */
  instant: boolean;
  /** True on the `due_at` side of that, false when the only date is a start. */
  deadline: boolean;
  left: number;
  width: number;
  /** A subtask borrowed onto its collapsed parent's row, so folding a parent
   *  up never hides its children's dates. Drawn quieter than the parent's own. */
  rollup: boolean;
}

interface Row {
  node: TaskNode;
  /** The row's own task — the parent, or the subtask on an expanded child row. */
  task: DbProjectTask;
  depth: 0 | 1;
  items: Placed[];
  expandable: boolean;
}

/** Both dates and the task is a span; one date and it is a marker; neither and
 *  it belongs in the Unscheduled rail instead. A `due_at` at or before
 *  `starts_at` is bad data rather than a zero-length span, so it falls back to
 *  the deadline — the date that means something. */
function place(
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

function laneSpan(p: Placed) {
  // Two pixels of air, so bars that merely touch do not read as one.
  return { start: p.left, end: p.left + p.width + 2 };
}

// ── The view ─────────────────────────────────────────────────────────────────

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
  const [expanded, setExpanded] = useState<Set<number>>(new Set());
  const scroller = useRef<HTMLDivElement>(null);

  const toggle = (id: number) =>
    setExpanded((prev) => {
      const next = new Set(prev);
      if (!next.delete(id)) next.add(id);
      return next;
    });

  /**
   * Backlog-kind columns are left off the axis entirely. A stub you have not
   * committed to has no place on a schedule — it is a plan to make a plan, and
   * drawing it beside real work would say you had booked time for it. The
   * Backlog view is where that pile is read. A whole node goes with its
   * parent: the pieces of a stub are part of the stub.
   */
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
    return timelineRange(points, now, zoom);
  }, [onBoard, now, zoom]);

  const origin = rangeStart.getTime();
  const px = PX_PER_DAY[zoom];
  const x = useMemo(
    () => (ms: number) => ((ms - origin) / DAY_MS) * px,
    [origin, px],
  );
  const total = x(rangeEnd.getTime());

  const ticks = useMemo(
    () => ticksFor(zoom, rangeStart, rangeEnd, now),
    [zoom, rangeStart, rangeEnd, now],
  );
  const bands = useMemo(() => bandsFor(zoom, rangeStart, rangeEnd), [zoom, rangeStart, rangeEnd]);

  /** The rows, plus everything that has no date to be drawn at. */
  const { rows, unscheduled } = useMemo(() => {
    const rows: Row[] = [];
    const unscheduled: DbProjectTask[] = [];

    for (const node of onBoard) {
      const own = place(node.task, x, total, false);
      const kids = node.children.map((c) => ({ task: c, at: place(c, x, total, false) }));
      const dated = kids.filter((k) => k.at != null);
      const undated = kids.filter((k) => k.at == null).map((k) => k.task);

      // A parent with nothing dated anywhere in its subtree has no row to take;
      // it and its pieces go to the rail together, parent first.
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

  // Open on today rather than on the start of the range, which for a semester
  // plan is weeks behind. Keyed on the range and the zoom, never on `now`, or
  // the minute tick would drag the scroll back every sixty seconds.
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
          {/* Axis. Sticky vertically so the dates stay readable down a long
              project; its name cell is sticky both ways, the corner of the
              two. */}
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

          {/* Rows, over one shared layer of gridlines, past wash and now line —
              one overlay rather than the same three things per row. */}
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
              {/* Elapsed time, washed grey. `surface` sits within a couple of
                  percent of the page background and is invisible as a wash, so
                  this is mixed from the muted foreground — WeekView's PastWash,
                  turned on its side. */}
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

// ── Rows ─────────────────────────────────────────────────────────────────────

/** Done drops the colour for the neutral `chart-other`, overdue takes the same
 *  red `DueChip` already uses, and everything else is the brand indigo as an
 *  accent rather than a fill. */
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

function TimelineRow({
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
  const laid = packLanes(row.items, laneSpan);

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
        {row.depth === 0 && row.expandable ? (
          <button
            type="button"
            aria-label={expanded ? "Collapse subtasks" : "Expand subtasks"}
            aria-expanded={expanded}
            onClick={onToggle}
            className="shrink-0 cursor-pointer p-0.5 text-muted-foreground/50 transition-colors hover:text-foreground"
          >
            <CaretRight
              size={9}
              className={cn("transition-transform", expanded && "rotate-90")}
            />
          </button>
        ) : (
          <span aria-hidden className="w-[13px] shrink-0" />
        )}
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
                  // No cutoff to draw to, so it is the dot the calendar uses
                  // for anything that merely occupies time — see EventMark.
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

// ── The rail ─────────────────────────────────────────────────────────────────

/**
 * What has no date to be drawn at.
 *
 * The calendar's rule: nothing may hide outside the drawn range. A deadline
 * the week grid cannot place goes to the labelled "Due" strip rather than
 * being dropped, and a task with no dates is the same problem one step
 * further — there is no axis position to give it at all. It is listed, so a
 * plan with half its dates missing looks like a plan with half its dates
 * missing.
 */
function UnscheduledRail({
  project,
  tasks,
}: {
  project: DbProject;
  tasks: DbProjectTask[];
}) {
  return (
    <div className="shrink-0 border-t border-border-subtle px-5 py-2">
      <div className="flex items-baseline gap-2">
        <span className="text-[11px] font-medium text-muted-foreground">Unscheduled</span>
        <span className="text-[11px] tabular-nums text-muted-foreground/60">{tasks.length}</span>
      </div>
      <div className="mt-1.5 flex max-h-[68px] flex-wrap gap-1.5 overflow-y-auto">
        {tasks.map((t) => (
          <span
            key={t.id}
            title={t.title}
            className="inline-flex max-w-52 items-center gap-1.5 rounded-full border border-border-subtle bg-surface px-2 py-0.5 text-[11px]"
          >
            <TaskGlyph kind={columnOf(project, t.column_id)?.kind ?? null} size={10} />
            <span
              className={cn(
                "truncate",
                t.done_at ? "text-muted-foreground line-through" : "text-foreground",
              )}
            >
              {t.title}
            </span>
          </span>
        ))}
      </div>
    </div>
  );
}
