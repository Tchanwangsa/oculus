import { Fragment, useMemo, useState } from "react";
import { GridTable } from "@/components/ui/table/GridTable";
import { TableGroup } from "@/components/ui/table/TableGroup";
import { useStoredState } from "@/hooks/ui/useStoredState";
import type { DbProject, DbTaskTag, DbTaskWithProject } from "@/lib/planning/projects";
import type { PropertyId } from "../display/settings";
import { subtaskProgress, taskTree } from "../tasks/taskTree";
import type { TaskSort } from "../tasks/taskSort";
import type { CellActions } from "./TaskCells";
import { TaskRow } from "./TaskRow";
import { ProjectIndexRow } from "./ProjectIndexRow";
import { TaskTableHeaders } from "./TaskTableHeaders";
import { columnIds, gridTemplate, tableMinWidth } from "./tasksColumns";
import type { ProjectRowActions } from "./useProjectActions";
import type { ProjectBranch } from "./projectTableData";

const PROPERTIES: PropertyId[] = ["tags", "subject", "due", "updated"];
const COLUMNS = columnIds(PROPERTIES);
const COLS = "grid items-center gap-3 px-5";
const EMPTY_KEYS: ReadonlySet<string> = new Set();
const COLS_STYLE = { gridTemplateColumns: gridTemplate(COLUMNS) };

/** One scrolling table: project → task → subtask, using the Tasks page's rows. */
export function ProjectsIndexTable({ branches, tasks, projects, projectById, tags, tagById, expanded, onToggle,
  query, sort, onSort, actions, taskActions, empty }: {
  branches: ProjectBranch[];
  tasks: DbTaskWithProject[];
  projects: DbProject[];
  projectById: Map<number, DbProject>;
  tags: DbTaskTag[];
  tagById: Map<number, DbTaskTag>;
  expanded: ReadonlySet<string>;
  onToggle: (id: number) => void;
  query: string;
  empty?: string;
  sort: TaskSort;
  onSort: (sort: TaskSort) => void;
  actions: ProjectRowActions;
  taskActions: CellActions;
}) {
  const searching = Boolean(query.trim());
  const [searchState, setSearchState] = useState({ query: "", keys: EMPTY_KEYS });
  const searchClosed = searchState.query === query ? searchState.keys : EMPTY_KEYS;
  const toggleSearch = (key: string) => setSearchState(prev => {
    const keys = new Set(prev.query === query ? prev.keys : []);
    if (!keys.delete(key)) keys.add(key);
    return { query, keys };
  });
  const [archivedOpen, setArchivedOpen] = useStoredState("oculus-projects-archived-open", v => v === "true");
  const [foldedTasks, setFoldedTasks] = useState<ReadonlySet<number>>(new Set());
  const byId = useMemo(() => new Map(tasks.map(t => [t.id, t])), [tasks]);
  const progressById = useMemo(() => new Map(taskTree(tasks).map(n => [n.task.id, subtaskProgress(n)])), [tasks]);
  const now = useMemo(() => new Date(), [tasks, projects]);
  const active = branches.filter(b => b.project.status !== "archived");
  const archived = branches.filter(b => b.project.status === "archived");
  const toggleTask = (id: number) => {
    if (searching) { toggleSearch(`task:${id}`); return; }
    setFoldedTasks(prev => {
    const next = new Set(prev);
    if (!next.delete(id)) next.add(id);
    return next;
    });
  };
  const taskRow = (id: number, childrenCount: number, depth: number) => {
    const task = byId.get(id);
    if (!task) return null;
    return <TaskRow key={id} task={task} parent={task.parent_id != null ? byId.get(task.parent_id) ?? null : null}
      depth={depth} reserveToggle childrenCount={childrenCount} childrenProgress={progressById.get(id)}
      childrenOpen={searching ? !searchClosed.has(`task:${id}`) : !foldedTasks.has(id)} onToggleChildren={() => toggleTask(id)} properties={PROPERTIES}
      cols={COLS} colsStyle={COLS_STYLE} now={now} projects={projects} projectById={projectById}
      tags={tags} tagById={tagById} actions={taskActions} />;
  };
  const rows = (list: ProjectBranch[]) => list.map(branch => {
    const id = branch.project.id;
    const open = searching ? !searchClosed.has(`project:${id}`) : expanded.has(String(id));
    return <Fragment key={id}>
      <ProjectIndexRow branch={branch} open={open} onToggle={() => searching ? toggleSearch(`project:${id}`) : onToggle(id)} cols={COLS} colsStyle={COLS_STYLE} now={now} actions={actions} tags={tags} />
      {open && branch.nodes.flatMap(node => [taskRow(node.task.id, node.children.length, 1),
        ...((searching ? !searchClosed.has(`task:${node.task.id}`) : !foldedTasks.has(node.task.id)) ? node.children.map(child => taskRow(child.id, 0, 2)) : [])])}
    </Fragment>;
  });
  return <GridTable cols={COLS} colsStyle={COLS_STYLE} minWidth={tableMinWidth(COLUMNS)}
    header={<TaskTableHeaders columns={COLUMNS} sort={sort} onSort={onSort} />}
    empty={branches.length === 0 && (empty ?? (searching ? "No projects or tasks match your search." : "No projects yet. Create your first project to get started."))}>
    <div className="divide-y divide-border-subtle">{rows(active)}</div>
    {archived.length > 0 && <TableGroup label="Archived" count={archived.length} open={searching ? !searchClosed.has("archived") : archivedOpen} onToggle={() => searching ? toggleSearch("archived") : setArchivedOpen(v => !v)}>
      {rows(archived)}
    </TableGroup>}
  </GridTable>;
}
