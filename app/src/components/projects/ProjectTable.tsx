import { useState } from "react";
import { Link } from "react-router-dom";
import { CaretRight, Plus } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { TablePagination, usePagedRows } from "@/components/ui/TablePagination";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { displayCode } from "@/lib/format";
import type { DbProject, DbProjectTask } from "@/lib/projects";
import { InlineAdd } from "./InlineAdd";
import { StatusPill } from "./StatusPill";
import { taskHref } from "./taskHref";
import { AgentMark, DueChip, SubtaskProgressBar, TaskGlyph } from "./TaskMarks";
import {
  appendSlot,
  columnOf,
  promotionTarget,
  subtaskProgress,
  type TaskNode,
} from "./taskTree";

/**
 * Every task of the project as one table — the view for reading the plan
 * rather than working it. A parent expands into its subtasks in place, so the
 * whole tree is one column of rows instead of a drill-down.
 *
 * Structurally this is `app/src/components/sync/SyncHistoryTable.tsx`: one
 * `COLS` template shared by the header and every row, the header outside the
 * scroller, and a pinned `TablePagination` footer.
 */

/** Column template shared by the header and every row — the one thing that
 *  keeps them in column. */
const COLS =
  "grid grid-cols-[28px_minmax(0,1fr)_110px_130px_120px_150px] items-center gap-3 px-5";

const HEADERS = ["", "Title", "Subject", "Status", "Due", "Subtasks"];

const PAGE_SIZE = 25;

function SubjectCell({ project }: { project: DbProject }) {
  if (!project.subject_code) {
    return <span className="text-[11px] text-muted-foreground/60">Personal</span>;
  }
  return (
    <span className="inline-flex min-w-0 items-center gap-1.5 rounded-full border border-border-subtle bg-surface px-2 py-0.5 text-[11px] text-foreground">
      <SubjectIcon code={project.subject_code} size={11} />
      <span className="truncate">{displayCode(project.subject_code)}</span>
    </span>
  );
}

/** The add-subtask control, which on a subtask is a dead button rather than an
 *  error: `createTask` refuses a grandchild, and a refusal you can see before
 *  you click is not a refusal at all. */
function AddSubtaskButton({
  disabled,
  onClick,
}: {
  disabled: boolean;
  onClick: () => void;
}) {
  const button = (
    <button
      type="button"
      disabled={disabled}
      aria-label="Add subtask"
      onClick={onClick}
      className={cn(
        "shrink-0 rounded p-0.5 text-muted-foreground transition-opacity",
        disabled
          ? "cursor-not-allowed opacity-30"
          : "cursor-pointer opacity-0 hover:text-foreground group-hover/row:opacity-100",
      )}
    >
      <Plus size={11} weight="bold" />
    </button>
  );
  if (!disabled) return button;
  return (
    <Tooltip>
      {/* A disabled button swallows pointer events, so the tooltip hangs off a
          span that still gets them — the SyncPage pattern. */}
      <TooltipTrigger asChild>
        <span className="shrink-0">{button}</span>
      </TooltipTrigger>
      <TooltipContent>Subtasks are one level deep</TooltipContent>
    </Tooltip>
  );
}

function TaskRow({
  project,
  task,
  index,
  depth,
  expandable,
  expanded,
  onToggle,
  progress,
  onMove,
  onAddSubtask,
}: {
  project: DbProject;
  task: DbProjectTask;
  /** 1-based position across the whole list, or null on a subtask. */
  index: number | null;
  depth: 0 | 1;
  expandable: boolean;
  expanded: boolean;
  onToggle: () => void;
  progress: ReturnType<typeof subtaskProgress> | null;
  onMove: (columnId: string) => void;
  onAddSubtask: (() => void) | null;
}) {
  return (
    <div className={cn(COLS, "group/row py-2 transition-colors hover:bg-surface/60")}>
      <span className="text-[11px] tabular-nums text-muted-foreground/60">
        {index ?? ""}
      </span>

      <div className={cn("flex min-w-0 items-center gap-1.5", depth === 1 && "pl-5")}>
        {expandable ? (
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

        <TaskGlyph kind={columnOf(project, task.column_id)?.kind ?? null} size={depth ? 11 : 13} />
        {/* The title is the way into the task's own page, on a subtask row as
            well as a parent's: a subtask is a task, with the same page. The
            row's other controls stay where they are — a whole-row link would
            have swallowed the status pill and the add-subtask button. */}
        <Link
          to={taskHref(project.id, task)}
          className={cn(
            "truncate text-xs hover:underline",
            task.done_at ? "text-muted-foreground line-through" : "text-foreground",
          )}
        >
          {task.title}
        </Link>
        <AgentMark source={task.source} />
        <span className="flex-1" />
        <AddSubtaskButton disabled={onAddSubtask == null} onClick={() => onAddSubtask?.()} />
      </div>

      <SubjectCell project={project} />

      <StatusPill project={project} columnId={task.column_id} onPick={onMove} />

      {task.due_at ? (
        <DueChip dueAt={task.due_at} />
      ) : (
        <span className="text-[11px] text-muted-foreground/50">—</span>
      )}

      {progress ? (
        <SubtaskProgressBar progress={progress} />
      ) : (
        <span className="text-[11px] text-muted-foreground/50">—</span>
      )}
    </div>
  );
}

export function ProjectTable({
  project,
  nodes,
  onMove,
  onCreate,
}: {
  project: DbProject;
  nodes: TaskNode[];
  onMove: (id: number, columnId: string, before: number | null, after: number | null) => void;
  onCreate: (input: { title: string; columnId: string; parentId?: number }) => void;
}) {
  const [expanded, setExpanded] = useState<Set<number>>(new Set());
  const [composing, setComposing] = useState<number | null>(null);
  const { page, pageCount, setPage, pageRows } = usePagedRows(nodes, PAGE_SIZE);

  const toggle = (id: number) =>
    setExpanded((prev) => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });

  // A new top-level row joins the first column you actually work in, not the
  // backlog the project's column list happens to start with.
  const defaultColumn = promotionTarget(project) ?? project.columns[0];

  const move = (id: number, columnId: string) => {
    const slot = appendSlot(nodes, columnId);
    onMove(id, columnId, slot.before, slot.after);
  };

  return (
    <div className="flex h-full flex-col">
      {/* The header sits OUTSIDE the scroll container, not sticky inside it:
          the scrollbar is a 6px classic bar that takes its gutter from the
          scroller's full height, so a header within it gets a bar drawn down
          its right edge. The wrapper's `pr-1.5` re-creates that gutter's width
          for the header, and `scrollbar-gutter: stable` on the body keeps it
          reserved when there is nothing to scroll — without both, the header
          and its rows sit 6px out of column. */}
      <div className="shrink-0 pr-1.5">
        <div className={cn(COLS, "border-b border-border-subtle bg-card py-2")}>
          {HEADERS.map((h, i) => (
            <span key={i} className="text-[11px] font-medium text-muted-foreground">
              {h}
            </span>
          ))}
        </div>
      </div>

      <div className="flex-1 min-h-0 overflow-y-auto [scrollbar-gutter:stable]">
        {nodes.length === 0 && (
          <p className="px-5 py-16 text-center text-xs text-muted-foreground">
            No tasks yet — break the project into the first few pieces below.
          </p>
        )}

        <div className="divide-y divide-border-subtle">
          {pageRows.map((node, i) => {
            const open = expanded.has(node.task.id);
            const progress = subtaskProgress(node);
            return (
              <div key={node.task.id}>
                <TaskRow
                  project={project}
                  task={node.task}
                  index={(page - 1) * PAGE_SIZE + i + 1}
                  depth={0}
                  expandable={node.children.length > 0}
                  expanded={open}
                  onToggle={() => toggle(node.task.id)}
                  progress={progress}
                  onMove={(columnId) => move(node.task.id, columnId)}
                  onAddSubtask={() => {
                    setExpanded((prev) => new Set(prev).add(node.task.id));
                    setComposing(node.task.id);
                  }}
                />

                {open && (
                  <div className="divide-y divide-border-subtle border-t border-border-subtle bg-surface/30">
                    {node.children.map((child) => (
                      <TaskRow
                        key={child.id}
                        project={project}
                        task={child}
                        index={null}
                        depth={1}
                        expandable={false}
                        expanded={false}
                        onToggle={() => {}}
                        progress={null}
                        onMove={(columnId) => move(child.id, columnId)}
                        onAddSubtask={null}
                      />
                    ))}
                    {composing === node.task.id && (
                      <div className={cn(COLS, "py-1.5")}>
                        <span />
                        <div className="pl-5">
                          <InlineAdd
                            defaultEditing
                            label="New subtask"
                            placeholder="Subtask title"
                            onAdd={(title) =>
                              onCreate({
                                title,
                                columnId: node.task.column_id,
                                parentId: node.task.id,
                              })
                            }
                          />
                        </div>
                      </div>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>

        {defaultColumn && (
          <div className={cn(COLS, "border-t border-border-subtle py-1.5")}>
            <span />
            <InlineAdd
              label="New task"
              placeholder="Task title"
              onAdd={(title) => onCreate({ title, columnId: defaultColumn.id })}
            />
          </div>
        )}
      </div>

      <TablePagination
        page={page}
        pageCount={pageCount}
        onPage={setPage}
        total={nodes.length}
        unit="task"
      />
    </div>
  );
}
