import { TableGroup } from "@/components/ui/table/TableGroup";
import { TaskTableHeaders } from "./TaskTableHeaders";
import { useMemo, useState, type CSSProperties } from "react";
import { GridTable } from "@/components/ui/table/GridTable";
import type { DbProject, DbTaskTag, DbTaskWithProject } from "@/lib/planning/projects";
import { type TasksDisplay } from "../display/settings";
import { TagDot } from "../tags/TagDot";
import { subtaskProgress, taskTree } from "../tasks/taskTree";
import { groupTasks, type TaskGroup } from "../tasks/taskGroups";
import type { TaskSort } from "../tasks/taskSort";
import { columnVariant } from "../tasks/columnVariant";
import { TaskGlyph } from "../tasks/TaskMarks";
import { UNIVERSAL_COLUMNS } from "../tasks/universalTasks";
import type { CellActions } from "./TaskCells";
import { TaskRow } from "./TaskRow";
import { columnIds, gridTemplate, tableMinWidth } from "./tasksColumns";

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
      reserveToggle={display.groupSubtasks}
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
      header={<TaskTableHeaders columns={columns} sort={display.sort} onSort={onSort} />}
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
  return <TableGroup label={group.label} count={group.tasks.length} open={open} onToggle={onToggle}
    icon={<>{group.tag && <TagDot color={group.tag.color} />}{status && <TaskGlyph variant={columnVariant(UNIVERSAL_COLUMNS, status.id)} />}</>}>
    {children}
  </TableGroup>;
}
