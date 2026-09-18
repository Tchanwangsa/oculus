import { useMemo, useState } from "react";
import { CalendarBlank, CaretUpDown, Check, Kanban, Tray } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { useSubjects } from "@/hooks/useSubjects";
import { addDays, startOfDay, startOfWeek } from "@/lib/calendar";
import { displayCode, sqliteUtcToMs } from "@/lib/format";
import type { DbProject, DbTaskWithProject, ProjectColumn } from "@/lib/projects";
import { UNIVERSAL_COLUMNS, universalColumnOf } from "./universalTasks";

/**
 * The four questions the universal Tasks view asks, and the predicate that
 * answers them.
 *
 * The page holds the state; this file holds the meaning — the same split
 * `ProjectPicker` makes with its callers. It replaces the `All tasks · Unfiled`
 * strip that used to sit above the toolbar: Unfiled is a *value* of the project
 * filter here, which is strictly more useful, because there was previously no
 * way to ask for one project's tasks in this view at all.
 *
 * **The filter is a read, in the page.** One `getAllTasks` still, narrowed by
 * {@link matchesFilter} over the list the page already has — not a second
 * query variant per question. Four SQL shapes that have to agree with each
 * other is how the old two-query strip told a lie about Unfiled.
 */

/** Any · Overdue · Today · This week · Undated. */
export type DueFilter = "any" | "overdue" | "today" | "week" | "undated";

export interface TaskFilter {
  /**
   * Which of {@link UNIVERSAL_COLUMNS} to show, by id — and so, on the board,
   * **which columns exist at all**. A filtered-out status simply has no column
   * there; a board that ignored a filter the toolbar says is on would be
   * worse than one you cannot drag onto.
   *
   * Never empty: the control keeps the last checked status checked, since an
   * empty set is a board with no columns and a table with no rows and no way
   * back but the toolbar.
   */
  status: readonly string[];
  /**
   * `"any"`, `"unfiled"`, or one project's id.
   *
   * Three states, so not `ProjectPicker`'s `number | null` — `null` there
   * already *means* Unfiled (it is what files a task nowhere), and reusing it
   * for Any would turn this filter into the mode this control replaced.
   */
  project: "any" | "unfiled" | number;
  /** `"any"`, `"personal"` (no subject at all), or a subject's code. */
  subject: "any" | "personal" | string;
  due: DueFilter;
}

/** Opens on Todo: what there is to do next, which is the question this view
 *  exists to answer. Everything else opens on Any. */
export const DEFAULT_FILTER: TaskFilter = {
  status: ["todo"],
  project: "any",
  subject: "any",
  due: "any",
};

/** Whether any question is narrower than Any — what the empty copy and the
 *  `done/total` counter both key off. The status set counts as narrowed when
 *  it is not all four, which includes the default. */
export function isFiltered(filter: TaskFilter): boolean {
  return (
    filter.status.length !== UNIVERSAL_COLUMNS.length ||
    filter.project !== "any" ||
    filter.subject !== "any" ||
    filter.due !== "any"
  );
}

/** The columns the board draws: {@link UNIVERSAL_COLUMNS} in their own order,
 *  minus the statuses filtered out. */
export function filteredColumns(filter: TaskFilter): readonly ProjectColumn[] {
  return UNIVERSAL_COLUMNS.filter((c) => filter.status.includes(c.id));
}

/**
 * Whether a task survives the filter.
 *
 * `now` is passed in rather than read here so one pass over the list asks one
 * clock — a `Date.now()` per row could put two tasks a millisecond either side
 * of "overdue" in the same render.
 */
export function matchesFilter(
  task: DbTaskWithProject,
  filter: TaskFilter,
  projectById: Map<number, DbProject>,
  now: number,
): boolean {
  if (!filter.status.includes(universalColumnOf(task, projectById).id)) return false;

  if (filter.project === "unfiled") {
    if (task.project_id != null) return false;
  } else if (filter.project !== "any") {
    if (task.project_id !== filter.project) return false;
  }

  if (filter.subject === "personal") {
    if (task.project_subject_code != null) return false;
  } else if (filter.subject !== "any") {
    if (task.project_subject_code !== filter.subject) return false;
  }

  return matchesDue(task.due_at, filter.due, now);
}

/**
 * The due question, through `sqliteUtcToMs` rather than by comparing strings:
 * `due_at` is an ISO stamp when the UI wrote it and SQLite's own
 * "YYYY-MM-DD HH:MM:SS" when the CLI did, and those two do not compare
 * lexicographically — the `T` beats the space and reorders a day. `TasksTable`'s
 * `dueCmp` makes the same point.
 *
 * **This week is the calendar's own week** — Monday-first, `startOfWeek` in
 * `app/src/lib/calendar.ts` — so it names the same seven days the calendar page
 * draws rather than a rolling seven from now. A task due on Tuesday is in this
 * week on Friday, and overdue as well; those are two true things about it, not
 * a conflict.
 */
function matchesDue(dueAt: string | null, due: DueFilter, now: number): boolean {
  if (due === "any") return true;
  const ms = sqliteUtcToMs(dueAt);
  if (due === "undated") return ms == null;
  if (ms == null) return false;
  if (due === "overdue") return ms < now;
  const today = new Date(now);
  if (due === "today") {
    const from = startOfDay(today).getTime();
    return ms >= from && ms < addDays(today, 1).getTime();
  }
  const from = startOfWeek(today).getTime();
  return ms >= from && ms < addDays(startOfWeek(today), 7).getTime();
}

const DUE_LABELS: Record<DueFilter, string> = {
  any: "Any time",
  overdue: "Overdue",
  today: "Today",
  week: "This week",
  undated: "Undated",
};

/** The toolbar's four controls. Nothing here holds state but the popovers'
 *  own open flags — the filter itself is the page's. */
export function TaskFilters({
  filter,
  projects,
  onChange,
  className,
}: {
  filter: TaskFilter;
  /** Every project, archived included — the same list the table's Project cell
   *  offers, so a task filed in last semester's work is still askable for. */
  projects: DbProject[];
  onChange: (next: TaskFilter) => void;
  className?: string;
}) {
  const { subjects } = useSubjects();

  const projectName = useMemo(() => {
    if (filter.project === "any") return "Any project";
    if (filter.project === "unfiled") return "Unfiled";
    return projects.find((p) => p.id === filter.project)?.name ?? "Project";
  }, [filter.project, projects]);

  const subjectName = useMemo(() => {
    if (filter.subject === "any") return "Any subject";
    if (filter.subject === "personal") return "Personal";
    return displayCode(filter.subject);
  }, [filter.subject]);

  const statusName =
    filter.status.length === UNIVERSAL_COLUMNS.length
      ? "Any status"
      : filter.status.length === 1
        ? UNIVERSAL_COLUMNS.find((c) => c.id === filter.status[0])?.name ?? "Status"
        : `${filter.status.length} statuses`;

  /** Checking and unchecking, with the last one pinned: see `TaskFilter.status`. */
  const toggleStatus = (id: string) => {
    const on = filter.status.includes(id);
    if (on && filter.status.length === 1) return;
    onChange({
      ...filter,
      status: on
        ? filter.status.filter((s) => s !== id)
        : UNIVERSAL_COLUMNS.filter((c) => c.id === id || filter.status.includes(c.id)).map(
            (c) => c.id,
          ),
    });
  };

  return (
    <div className={cn("flex min-w-0 items-center gap-1.5", className)}>
      <FilterPopover label={statusName} active={filter.status.length !== UNIVERSAL_COLUMNS.length}>
        {() => (
          <>
            {/* Multi-select, so picking does not close it — you are usually
                after two of these at once. */}
            {UNIVERSAL_COLUMNS.map((c) => (
              <FilterRow
                key={c.id}
                label={c.name}
                selected={filter.status.includes(c.id)}
                onPick={() => toggleStatus(c.id)}
              />
            ))}
            <FilterRow
              label="Any status"
              selected={filter.status.length === UNIVERSAL_COLUMNS.length}
              onPick={() =>
                onChange({ ...filter, status: UNIVERSAL_COLUMNS.map((c) => c.id) })
              }
              className="mt-1 border-t border-border-subtle pt-1.5"
            />
          </>
        )}
      </FilterPopover>

      <FilterPopover label={projectName} active={filter.project !== "any"}>
        {(close) => (
          <div className="-mx-1 max-h-52 overflow-y-auto px-1">
            <FilterRow
              label="Any project"
              selected={filter.project === "any"}
              onPick={() => {
                close();
                onChange({ ...filter, project: "any" });
              }}
            />
            <FilterRow
              label="Unfiled"
              icon={<Tray size={13} className="shrink-0" />}
              selected={filter.project === "unfiled"}
              onPick={() => {
                close();
                onChange({ ...filter, project: "unfiled" });
              }}
            />
            {projects.map((p) => (
              <FilterRow
                key={p.id}
                label={p.name}
                icon={
                  p.subject_code ? (
                    <SubjectIcon code={p.subject_code} size={13} />
                  ) : (
                    <Kanban size={13} className="shrink-0" />
                  )
                }
                selected={filter.project === p.id}
                onPick={() => {
                  close();
                  onChange({ ...filter, project: p.id });
                }}
              />
            ))}
          </div>
        )}
      </FilterPopover>

      <FilterPopover label={subjectName} active={filter.subject !== "any"}>
        {(close) => (
          <div className="-mx-1 max-h-52 overflow-y-auto px-1">
            <FilterRow
              label="Any subject"
              selected={filter.subject === "any"}
              onPick={() => {
                close();
                onChange({ ...filter, subject: "any" });
              }}
            />
            {/* No subject at all — a personal project's tasks, and every
                unfiled one, which is the same absence read from the row. */}
            <FilterRow
              label="Personal"
              hint="No subject — personal projects, and anything unfiled"
              selected={filter.subject === "personal"}
              onPick={() => {
                close();
                onChange({ ...filter, subject: "personal" });
              }}
            />
            {subjects.map((s) => (
              <FilterRow
                key={s.id}
                label={displayCode(s.code)}
                icon={<SubjectIcon code={s.code} size={13} />}
                selected={filter.subject === s.code}
                onPick={() => {
                  close();
                  onChange({ ...filter, subject: s.code });
                }}
              />
            ))}
          </div>
        )}
      </FilterPopover>

      <FilterPopover
        label={DUE_LABELS[filter.due]}
        active={filter.due !== "any"}
        icon={<CalendarBlank size={12} className="shrink-0 text-muted-foreground" />}
      >
        {(close) => (
          <>
            {(Object.keys(DUE_LABELS) as DueFilter[]).map((d) => (
              <FilterRow
                key={d}
                label={DUE_LABELS[d]}
                selected={filter.due === d}
                onPick={() => {
                  close();
                  onChange({ ...filter, due: d });
                }}
              />
            ))}
          </>
        )}
      </FilterPopover>
    </div>
  );
}

/**
 * One filter's trigger and popover. The trigger is the sync page's compact
 * `h-7` outline button (`components/sync/SubjectPicker.tsx`), and it takes the
 * brand as an *accent* — a border and a text colour, never a fill — when its
 * question is narrower than Any, so which filters are on is legible without
 * reading four labels.
 *
 * `children` is a render prop rather than a node so a row can close the
 * popover: a single-choice pick is finished the moment it is made, and the
 * status list — which is not — simply ignores the argument.
 */
function FilterPopover({
  label,
  active,
  icon,
  children,
}: {
  label: string;
  active: boolean;
  icon?: React.ReactNode;
  children: (close: () => void) => React.ReactNode;
}) {
  const [open, setOpen] = useState(false);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          size="sm"
          className={cn(
            "h-7 shrink-0 gap-1.5 px-2.5 text-xs font-normal",
            active ? "border-brand/40 text-brand" : "text-foreground",
          )}
        >
          {icon}
          <span className="max-w-32 truncate">{label}</span>
          <CaretUpDown size={11} className="shrink-0 text-muted-foreground/60" />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-56 p-1.5">
        {children(() => setOpen(false))}
      </PopoverContent>
    </Popover>
  );
}

/** One option. `ProjectDestinations`' row at this toolbar's weight — the same
 *  tick in the same place, so the two lists read as one vocabulary. */
function FilterRow({
  label,
  hint,
  icon,
  selected,
  onPick,
  className,
}: {
  label: string;
  hint?: string;
  icon?: React.ReactNode;
  selected: boolean;
  onPick: () => void;
  className?: string;
}) {
  return (
    <button
      type="button"
      title={hint ?? label}
      onClick={onPick}
      className={cn(
        "flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-[12.5px] transition-colors",
        selected
          ? "bg-accent text-foreground"
          : "text-muted-foreground hover:bg-accent hover:text-foreground",
        className,
      )}
    >
      {icon}
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {selected && <Check size={12} weight="bold" className="shrink-0 text-brand" />}
    </button>
  );
}
