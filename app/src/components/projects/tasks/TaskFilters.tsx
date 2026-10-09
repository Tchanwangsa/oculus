import { useMemo, useState } from "react";
import { CalendarBlank, CaretUpDown, Check, Kanban, Tray } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { useSubjects } from "@/hooks/data/useSubjects";
import { addDays, startOfWeek } from "@/lib/planning/calendar";
import { displayCode, sqliteUtcToMs, startOfDay } from "@/lib/format/format";
import type { DbProject, DbTaskWithProject, ProjectColumn } from "@/lib/planning/projects";
import { UNIVERSAL_COLUMNS, universalColumnOf } from "./universalTasks";

/**
 * The universal Tasks view's filters. The page holds the state; filtering is
 * {@link matchesFilter} over the one `getAllTasks` list, not a query per filter.
 */

type DueFilter = "any" | "overdue" | "today" | "week" | "undated";

export interface TaskFilter {
  /** {@link UNIVERSAL_COLUMNS} ids to show — on the board, the columns that
   *  exist. Never empty: the control won't uncheck the last one. */
  status: readonly string[];
  /** Not `number | null`: `ProjectPicker`'s `null` already means Unfiled. */
  project: "any" | "unfiled" | number;
  /** `"personal"` is no subject at all; otherwise a subject code. */
  subject: "any" | "personal" | string;
  due: DueFilter;
}

export const DEFAULT_FILTER: TaskFilter = {
  status: ["todo"],
  project: "any",
  subject: "any",
  due: "any",
};

/** Includes the default, whose status set is narrowed to Todo. */
export function isFiltered(filter: TaskFilter): boolean {
  return (
    filter.status.length !== UNIVERSAL_COLUMNS.length ||
    filter.project !== "any" ||
    filter.subject !== "any" ||
    filter.due !== "any"
  );
}

export function filteredColumns(filter: TaskFilter): readonly ProjectColumn[] {
  return UNIVERSAL_COLUMNS.filter((c) => filter.status.includes(c.id));
}

/** `now` is passed in so one pass over the list reads one clock. */
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

/** Parsed, not string-compared: `due_at` may be ISO or SQLite's format (see
 *  `TasksTable`'s `dueCmp`). "This week" is the calendar's Monday-first week,
 *  not a rolling seven days. */
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

export function TaskFilters({
  filter,
  projects,
  onChange,
  className,
}: {
  filter: TaskFilter;
  /** Archived included. */
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

  // The last checked status can't be unchecked — see `TaskFilter.status`.
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
            {/* Multi-select: picking doesn't close. */}
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

/** `children` is a render prop so a single-choice row can close the popover. */
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
