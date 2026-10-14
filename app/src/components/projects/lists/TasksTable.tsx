import { useMemo, useState, type CSSProperties } from "react";
import { CaretDown, CaretUp } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { GridTable } from "@/components/ui/table/GridTable";
import type { DbProject, DbTaskTag, DbTaskWithProject } from "@/lib/planning/projects";
import { sortByHeader, type TasksDisplay } from "../display/settings";
import { TagDot } from "../tags/TagDot";
import { subtaskProgress, taskTree } from "../tasks/taskTree";
import { groupTasks, type TaskGroup } from "../tasks/taskGroups";
import type { TaskSort } from "../tasks/taskSort";
import { columnVariant } from "../tasks/columnVariant";
import { TaskGlyph } from "../tasks/TaskMarks";
import { UNIVERSAL_COLUMNS } from "../tasks/universalTasks";
import type { CellActions } from "./TaskCells";
import { TaskRow } from "./TaskRow";
import { COLUMNS, columnIds, gridTemplate, tableMinWidth } from "./tasksColumns";

/**
 * Every task in the library as one `GridTable` whose columns follow the
 * Display settings: Name, then the shown properties. Rows arrive filtered and
 * sorted; this groups them if asked. Not drag-reorderable — `position` only
 * compares within one project's column (`../tasks/universalTasks.ts`).
 * Subtasks can be flat rows or nested under their visible parent. A child
 * whose parent is filtered out stays visible as a top-level row.
 */

const COLS = "grid items-center gap-3 px-5";

export function TasksTable({
  tasks,
  allTasks = tasks,
  projects,
  projectById,
  tags,
  tagById,
  display,
  onSort,
  empty,
  actions,
}: {
  tasks: DbTaskWithProject[];
  /** Unfiltered tasks, so hidden completed children still count as progress. */
  allTasks?: DbTaskWithProject[];
  /** Archived included; `projectById` is the same list keyed by id. */
  projects: DbProject[];
  projectById: Map<number, DbProject>;
  tags: DbTaskTag[];
  tagById: Map<number, DbTaskTag>;
  display: TasksDisplay;
  /** A click on a sortable header. */
  onSort: (sort: TaskSort) => void;
  empty: string | false;
  actions: CellActions;
}) {
  const columns = useMemo(() => columnIds(display.properties), [display.properties]);
  const colsStyle = useMemo<CSSProperties>(() => ({ gridTemplateColumns: gridTemplate(columns) }), [columns]);

  const byId = useMemo(() => new Map(tasks.map((t) => [t.id, t])), [tasks]);
  // One clock per pass, so "Today" and "overdue" agree across rows.
  const now = useMemo(() => new Date(), [tasks]);

  const progressById = useMemo(() => new Map(taskTree(allTasks).map((node) => [node.task.id, subtaskProgress(node)])), [allTasks]);
  const nodes = useMemo(() => taskTree(tasks), [tasks]);
  const nodeById = useMemo(() => new Map(nodes.map((n) => [n.task.id, n])), [nodes]);
  const [foldedParents, setFoldedParents] = useState<ReadonlySet<number>>(new Set());

  const groups = useMemo(
    () =>
      display.group === "none"
        ? null
        : groupTasks(display.groupSubtasks ? nodes.map((n) => n.task) : tasks, display.group, { projectById, tags, now: now.getTime() }),
    [tasks, nodes, display.group, display.groupSubtasks, projectById, tags, now],
  );

  // Local and unsaved: groups open again on the next visit.
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  // Keyed by the grouping too: "No date" and "No tag" are both "none".
  const collapseKey = (key: string) => `${display.group}:${key}`;
  const toggle = (key: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (!next.delete(key)) next.add(key);
      return next;
    });

  const row = (task: DbTaskWithProject, nested = false) => (
    <TaskRow
      key={task.id}
      task={task}
      parent={task.parent_id != null ? byId.get(task.parent_id) ?? null : null}
      nested={nested}
      childrenProgress={progressById.get(task.id)}
      childrenCount={display.groupSubtasks ? nodeById.get(task.id)?.children.length ?? 0 : 0}
      childrenOpen={!foldedParents.has(task.id)}
      onToggleChildren={() => setFoldedParents((prev) => {
        const next = new Set(prev);
        if (!next.delete(task.id)) next.add(task.id);
        return next;
      })}
      properties={display.properties}
      cols={COLS}
      colsStyle={colsStyle}
      now={now}
      projects={projects}
      projectById={projectById}
      tags={tags}
      tagById={tagById}
      actions={actions}
    />
  );

  const rows = (items: DbTaskWithProject[]) => {
    if (!display.groupSubtasks) return items.map((task) => row(task));
    return items.flatMap((task) => [
      row(task),
      ...(!foldedParents.has(task.id) ? (nodeById.get(task.id)?.children ?? []).map((child) => row(child, true)) : []),
    ]);
  };

  return (
    <GridTable
      cols={COLS}
      colsStyle={colsStyle}
      minWidth={tableMinWidth(columns)}
      header={columns.map((id) => {
        const { label, sort } = COLUMNS[id];
        if (sort == null) {
          return (
            <span key={id} className="truncate text-[11px] font-medium text-muted-foreground">
              {label}
            </span>
          );
        }
        const current = display.sort.key === sort;
        return (
          <button
            key={id}
            type="button"
            aria-label={`Sort by ${label}`}
            onClick={() => onSort(sortByHeader(display.sort, sort))}
            className={cn(
              "flex min-w-0 cursor-pointer items-center gap-1 text-left text-[11px] font-medium transition-colors",
              current ? "text-foreground" : "text-muted-foreground hover:text-foreground",
            )}
          >
            <span className="truncate">{label}</span>
            {current &&
              (display.sort.dir === "asc" ? (
                <CaretUp size={9} weight="bold" className="shrink-0" />
              ) : (
                <CaretDown size={9} weight="bold" className="shrink-0" />
              ))}
          </button>
        );
      })}
      empty={tasks.length === 0 && empty}
    >
      {groups ? (
        groups.map((group) => (
          <GroupSection
            key={group.key}
            group={group}
            open={!collapsed.has(collapseKey(group.key))}
            onToggle={() => toggle(collapseKey(group.key))}
          >
            {rows(group.tasks)}
          </GroupSection>
        ))
      ) : (
        <div className="divide-y divide-border-subtle">{rows(display.groupSubtasks ? nodes.map((n) => n.task) : tasks)}</div>
      )}
    </GridTable>
  );
}

function GroupSection({
  group,
  open,
  onToggle,
  children,
}: {
  group: TaskGroup;
  open: boolean;
  onToggle: () => void;
  children: React.ReactNode;
}) {
  const status = UNIVERSAL_COLUMNS.find((c) => c.id === group.key);
  return (
    <section aria-label={group.label}>
      <button
        type="button"
        aria-expanded={open}
        onClick={onToggle}
        className="flex h-8 w-full cursor-pointer items-center gap-2 border-b border-border-subtle bg-surface/50 px-5 text-left outline-none transition-colors hover:bg-surface focus-visible:ring-[3px] focus-visible:ring-inset focus-visible:ring-ring/25"
      >
        <CaretDown
          size={10}
          weight="bold"
          className={cn("shrink-0 text-muted-foreground transition-transform will-change-transform", !open && "-rotate-90")}
        />
        {group.tag && <TagDot color={group.tag.color} />}
        {status && <TaskGlyph variant={columnVariant(UNIVERSAL_COLUMNS, status.id)} />}
        <span className="min-w-0 truncate text-xs font-medium text-foreground">{group.label}</span>
        <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground">{group.tasks.length}</span>
      </button>
      {open && <div className="divide-y divide-border-subtle border-b border-border-subtle">{children}</div>}
    </section>
  );
}
