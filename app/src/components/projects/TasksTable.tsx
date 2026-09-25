import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { ArrowElbowDownRight, CaretDown, CaretUp } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { usePagedRows } from "@/components/ui/TablePagination";
import { GridTable } from "@/components/ui/GridTable";
import { displayCode, sqliteUtcToMs } from "@/lib/format";
import type { DbProject, DbTaskWithProject } from "@/lib/projects";
import { ProjectPicker } from "./ProjectPicker";
import { StatusPill } from "./StatusPill";
import { taskHref } from "./taskHref";
import { AgentMark, DueChip, TaskGlyph } from "./TaskMarks";
import {
  UNIVERSAL_COLUMNS,
  projectLabel,
  projectOf,
  universalColumnOf,
} from "./universalTasks";

/**
 * Every task in the library as one flat `GridTable`. Sortable, not
 * drag-reorderable: `position` only compares within one project's column
 * (`./universalTasks.ts`). Subtasks are flat rows marked with their parent.
 * Status and Project cells write; a subtask's Project is text, since
 * `refileTask` won't move one alone.
 */

const COLS =
  "grid grid-cols-[28px_minmax(0,1fr)_150px_110px_130px_120px] items-center gap-3 px-5";

const PAGE_SIZE = 25;

type SortKey = "title" | "project" | "subject" | "status" | "due";

interface Sort {
  key: SortKey;
  dir: "asc" | "desc";
}

const HEADERS: ReadonlyArray<{ key: SortKey | null; label: string }> = [
  { key: null, label: "" },
  { key: "title", label: "Title" },
  { key: "project", label: "Project" },
  { key: "subject", label: "Subject" },
  { key: "status", label: "Status" },
  { key: "due", label: "Due" },
];

/** `UNIVERSAL_ORDER` from `app/src/lib/projects.ts`, mirrored by hand — keep
 *  the two in step. The tiebreak under every sort; Due ascending reproduces
 *  the arrival order. */
function defaultCmp(a: DbTaskWithProject, b: DbTaskWithProject): number {
  const due = dueCmp(a, b);
  if (due !== 0) return due;
  // Unfiled first.
  if ((a.project_id == null) !== (b.project_id == null)) return a.project_id == null ? -1 : 1;
  if (a.project_id != null && b.project_id != null && a.project_id !== b.project_id) {
    return a.project_id - b.project_id;
  }
  return a.position - b.position || a.id - b.id;
}

/** Undated last. Parsed, not string-compared: `due_at` is ISO from the UI or
 *  SQLite's "YYYY-MM-DD HH:MM:SS" from the CLI, which don't sort together. */
function dueCmp(a: DbTaskWithProject, b: DbTaskWithProject): number {
  const am = sqliteUtcToMs(a.due_at);
  const bm = sqliteUtcToMs(b.due_at);
  if (am == null || bm == null) return am == null ? (bm == null ? 0 : 1) : -1;
  return am - bm;
}

export function TasksTable({
  tasks,
  projects,
  projectById,
  empty,
  onMove,
  onRefile,
}: {
  tasks: DbTaskWithProject[];
  /** Archived included; `projectById` is the same list keyed by id. */
  projects: DbProject[];
  projectById: Map<number, DbProject>;
  empty: string;
  /** A column id from the task's own board. */
  onMove: (task: DbTaskWithProject, columnId: string) => void;
  /** `null` unfiles. Top-level tasks only. */
  onRefile: (task: DbTaskWithProject, projectId: number | null) => void;
}) {
  const [sort, setSort] = useState<Sort>({ key: "due", dir: "asc" });

  const byId = useMemo(() => new Map(tasks.map((t) => [t.id, t])), [tasks]);

  const sorted = useMemo(() => {
    const keyCmp = (a: DbTaskWithProject, b: DbTaskWithProject): number => {
      switch (sort.key) {
        case "title":
          return a.title.localeCompare(b.title);
        // By the cell's text, so "Unfiled" sorts among the names.
        case "project":
          return projectLabel(a).localeCompare(projectLabel(b));
        // By code, so subject-less rows (Personal and unfiled) group.
        case "subject":
          return (a.project_subject_code ?? "").localeCompare(b.project_subject_code ?? "");
        // By board column order — not by name, and not by kind (Todo and In
        // progress share a kind but are separate columns).
        case "status":
          return (
            UNIVERSAL_COLUMNS.indexOf(universalColumnOf(a, projectById)) -
            UNIVERSAL_COLUMNS.indexOf(universalColumnOf(b, projectById))
          );
        case "due":
          return dueCmp(a, b);
      }
    };
    const rows = [...tasks];
    rows.sort((a, b) => {
      // Undated stays last in both directions.
      if (sort.key === "due") {
        const am = a.due_at == null;
        const bm = b.due_at == null;
        if (am !== bm) return am ? 1 : -1;
      }
      const k = keyCmp(a, b);
      if (k !== 0) return sort.dir === "asc" ? k : -k;
      return defaultCmp(a, b);
    });
    return rows;
  }, [tasks, sort, projectById]);

  const { page, pageCount, setPage, pageRows } = usePagedRows(sorted, PAGE_SIZE);

  // No "unsorted" third state: Due ascending is the natural order.
  const pick = (key: SortKey) =>
    setSort((prev) =>
      prev.key === key
        ? { key, dir: prev.dir === "asc" ? "desc" : "asc" }
        : { key, dir: "asc" },
    );

  return (
    <GridTable
      cols={COLS}
      header={HEADERS.map((h, i) =>
        h.key == null ? (
          <span key={i} />
        ) : (
          <button
            key={h.key}
            type="button"
            onClick={() => pick(h.key as SortKey)}
            className={cn(
              "flex cursor-pointer items-center gap-1 text-left text-[11px] font-medium transition-colors",
              sort.key === h.key
                ? "text-foreground"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            <span className="truncate">{h.label}</span>
            {sort.key === h.key &&
              (sort.dir === "asc" ? (
                <CaretUp size={9} weight="bold" className="shrink-0" />
              ) : (
                <CaretDown size={9} weight="bold" className="shrink-0" />
              ))}
          </button>
        ),
      )}
      empty={tasks.length === 0 && empty}
      pagination={{ page, pageCount, onPage: setPage, total: sorted.length, unit: "task" }}
    >
      <div className="divide-y divide-border-subtle">
        {pageRows.map((task, i) => {
          const project = projectOf(task, projectById);
          const parent =
            task.parent_id != null ? byId.get(task.parent_id) ?? null : null;
          return (
            <div
              key={task.id}
              className={cn(COLS, "py-2 transition-colors hover:bg-surface/60")}
            >
              <span className="text-[11px] tabular-nums text-muted-foreground/60">
                {(page - 1) * PAGE_SIZE + i + 1}
              </span>

              <div className="flex min-w-0 items-center gap-1.5">
                <TaskGlyph kind={universalColumnOf(task, projectById).kind} />
                {parent && (
                  <span
                    title={`Subtask of ${parent.title}`}
                    className="shrink-0 text-muted-foreground/60"
                  >
                    <ArrowElbowDownRight size={11} />
                  </span>
                )}
                <Link
                  to={taskHref(task.project_id, task)}
                  className={cn(
                    "truncate text-xs hover:underline",
                    task.done_at
                      ? "text-muted-foreground line-through"
                      : "text-foreground",
                  )}
                >
                  {task.title}
                </Link>
                <AgentMark source={task.source} />
              </div>

              {/* Gated on `parent_id`, not on the parent being in this
                  list: `refileTask` refuses any subtask. */}
              {task.parent_id != null ? (
                <span
                  className={cn(
                    "truncate text-[11px]",
                    task.project_id == null
                      ? "text-muted-foreground/60"
                      : "text-foreground",
                  )}
                  title={`${projectLabel(task)} — a subtask sits in its parent's project`}
                >
                  {projectLabel(task)}
                </span>
              ) : (
                <ProjectPicker
                  projects={projects}
                  value={task.project_id}
                  label="Move to"
                  unfiledHint="Take it out of every project"
                  onPick={(projectId) => onRefile(task, projectId)}
                >
                  <button
                    type="button"
                    aria-label="Change project"
                    title={`${projectLabel(task)} — click to refile`}
                    className={cn(
                      "min-w-0 cursor-pointer truncate rounded-md px-1.5 py-0.5 text-left text-[11px] transition-colors hover:bg-accent",
                      task.project_id == null
                        ? "text-muted-foreground/60"
                        : "text-foreground",
                    )}
                  >
                    {projectLabel(task)}
                  </button>
                </ProjectPicker>
              )}

              {task.project_id == null ? (
                <span className="text-[11px] text-muted-foreground/50">—</span>
              ) : task.project_subject_code ? (
                <span className="inline-flex min-w-0 items-center gap-1.5 rounded-full border border-border-subtle bg-surface px-2 py-0.5 text-[11px] text-foreground">
                  <SubjectIcon code={task.project_subject_code} size={11} />
                  <span className="truncate">
                    {displayCode(task.project_subject_code)}
                  </span>
                </span>
              ) : (
                <span className="text-[11px] text-muted-foreground/60">Personal</span>
              )}

              {/* The task's own board — the default four when unfiled. */}
              <StatusPill
                project={project}
                columnId={task.column_id}
                onPick={(columnId) => onMove(task, columnId)}
              />

              {task.due_at ? (
                <DueChip dueAt={task.due_at} />
              ) : (
                <span className="text-[11px] text-muted-foreground/50">—</span>
              )}
            </div>
          );
        })}
      </div>
    </GridTable>
  );
}
