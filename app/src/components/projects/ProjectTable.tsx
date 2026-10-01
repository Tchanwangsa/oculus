import { useState } from "react";
import type { CSSProperties, PointerEvent as ReactPointerEvent } from "react";
import { Link } from "react-router-dom";
import { CaretRight, DotsSixVertical, Plus } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { usePagedRows } from "@/components/ui/TablePagination";
import { GridTable, HeaderLabels } from "@/components/ui/GridTable";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { DRAG_SURFACE, useCardDrag, useSettledList } from "@/hooks/useCardDrag";
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
 * Every task of the project as one table; a parent expands into its subtasks
 * in place, in the `GridTable` shell.
 *
 * Rows reorder by the grip that replaces the row number on hover
 * (`useCardDrag`). A drop keeps the row's column and moves only its
 * `position`, which is all `getTasks` orders by (`docs/projects.md`). The board
 * shares that `position`: a board renumber can scramble an order set here, and
 * equal positions across columns leave no midpoint, so `moveTask` refuses.
 */

/** Shared by the header and every row. The 28px first cell holds the row
 *  number or the drag grip. */
const COLS =
  "grid grid-cols-[28px_minmax(0,1fr)_110px_130px_120px_150px] items-center gap-3 px-5";

const HEADERS = ["", "Title", "Subject", "Status", "Due", "Subtasks"];

const PAGE_SIZE = 25;

/** Drag container ids: one list for top-level rows, one per expanded parent,
 *  so a subtask only reorders among its siblings. */
const TOP_LIST = "top";
const childList = (parentId: number) => `sub-${parentId}`;

/** A background and stacking context, or siblings paint over the dragged row. */
const LIFTED = "relative z-10 bg-card shadow-md";

function slotIn<T>(rest: T[], slot: number, idOf: (item: T) => number) {
  return {
    before: slot > 0 ? idOf(rest[slot - 1]) : null,
    after: slot < rest.length ? idOf(rest[slot]) : null,
  };
}

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

/** Disabled on a subtask: `createTask` refuses a grandchild. */
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
      {/* A disabled button gets no pointer events; the span does. */}
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
  onGrab,
  lifted,
  numbersHidden,
  rowRef,
  rowStyle,
  rowClassName,
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
  /** On the grip only, so the row's own controls keep their presses. */
  onGrab: (e: ReactPointerEvent<HTMLElement>) => void;
  lifted: boolean;
  /** Numbers fade out during a reorder and back in already renumbered. */
  numbersHidden: boolean;
  /** Subtask rows only; a top-level row is dragged by its enclosing block. */
  rowRef?: (node: HTMLDivElement | null) => void;
  rowStyle?: CSSProperties;
  rowClassName?: string;
}) {
  return (
    <div
      ref={rowRef}
      style={rowStyle}
      className={cn(
        COLS,
        "group/row py-2",
        !lifted && "transition-colors hover:bg-surface/60",
        rowClassName,
      )}
    >
      {/* The grip overlays the number so hovering shifts nothing. */}
      <div className="relative flex items-center">
        <span
          className={cn(
            "text-[11px] tabular-nums text-muted-foreground/60 transition-opacity",
            numbersHidden ? "opacity-0" : "group-hover/row:opacity-0",
          )}
        >
          {index ?? ""}
        </span>
        <span
          aria-hidden
          onPointerDown={onGrab}
          className={cn(
            "absolute inset-0 flex items-center text-muted-foreground/70 transition-opacity",
            // Not preventDefault on press — see docs/frontend.md (WebKit click).
            DRAG_SURFACE,
            // Set here, not via `:active`: the grip holds pointer capture, so
            // its cursor shows for the whole gesture.
            lifted
              ? "cursor-grabbing opacity-100"
              : "cursor-grab opacity-0 hover:text-foreground group-hover/row:opacity-100",
          )}
        >
          <DotsSixVertical size={13} />
        </span>
      </div>

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
        {/* Title-only link: a whole-row link would swallow the other controls. */}
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

  // `containerRef` is deliberately never registered: the lists nest, and the
  // engine's first-match hit test would let the top-level box swallow every
  // subtask drop. Unregistered, a drag stays in the list it started in.
  const drag = useCardDrag((drop) => {
    // A list change would be a re-parent, which `moveTask` cannot write.
    if (drop.from !== drop.containerId) return;

    if (drop.containerId === TOP_LIST) {
      // The engine's index is page-local; map it onto the full `nodes` list,
      // or a drop at the top of page 2 would land above page 1.
      const from = nodes.findIndex((n) => n.task.id === drop.id);
      if (from < 0) return;
      const rest = nodes.filter((n) => n.task.id !== drop.id);
      const slot = (page - 1) * PAGE_SIZE + drop.index;
      if (slot === from) return;
      const task = nodes[from].task;
      const { before, after } = slotIn(rest, slot, (n) => n.task.id);
      onMove(task.id, task.column_id, before, after);
      return true;
    }

    // Subtasks aren't paginated, so no offset.
    const parent = nodes.find((n) => childList(n.task.id) === drop.containerId);
    if (!parent) return;
    const from = parent.children.findIndex((c) => c.id === drop.id);
    if (from < 0 || drop.index === from) return;
    const rest = parent.children.filter((c) => c.id !== drop.id);
    const child = parent.children[from];
    const { before, after } = slotIn(rest, drop.index, (c) => c.id);
    onMove(child.id, child.column_id, before, after);
    return true;
  }, { settleOn: nodes });
  const live = drag.drag;

  // Hold the pre-drop order until the settle animation ends, or the re-read
  // would land under transforms computed for the old order (see
  // `useSettledList`).
  const settledNodes = useSettledList(nodes, live);
  const rows =
    settledNodes === nodes
      ? pageRows
      : settledNodes.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);
  // Subtasks are unnumbered, so only a top-level drag hides numbers.
  const numbersHidden = live != null && live.containerId === TOP_LIST;

  const toggle = (id: number) =>
    setExpanded((prev) => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });

  // New rows join the first working column, not the backlog.
  const defaultColumn = promotionTarget(project) ?? project.columns[0];

  const move = (id: number, columnId: string) => {
    const slot = appendSlot(nodes, columnId);
    onMove(id, columnId, slot.before, slot.after);
  };

  return (
    <GridTable
      cols={COLS}
      header={<HeaderLabels labels={HEADERS} />}
      empty={
        nodes.length === 0 &&
        "No tasks yet — break the project into the first few pieces below."
      }
      pagination={{ page, pageCount, onPage: setPage, total: nodes.length, unit: "task" }}
    >

      <div className="divide-y divide-border-subtle">
        {rows.map((node, i) => {
          const open = expanded.has(node.task.id);
          const progress = subtaskProgress(node);
          const grabbed = live?.id === node.task.id;
          // The grabbed block follows the pointer's `dy`; `shiftFor` is 0 for it.
          const shift = grabbed && live ? live.dy : drag.shiftFor(TOP_LIST, i);
          return (
            // The block, not the row, is the item, so an expanded parent
            // carries its subtasks.
            <div
              key={node.task.id}
              ref={drag.itemRef(TOP_LIST, node.task.id)}
              style={shift ? { transform: `translateY(${shift}px)` } : undefined}
              className={cn(
                // On the base class so the shadow fades out with `LIFTED`.
                "transition-[box-shadow] duration-200",
                grabbed && LIFTED,
                // The grabbed row eases only while settling, never under the pointer.
                (grabbed ? live?.settling : live != null) &&
                  "transition-[transform,box-shadow] ease-out",
              )}
            >
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
                onGrab={(e) =>
                  drag.onPointerDown(e, { id: node.task.id, containerId: TOP_LIST })
                }
                lifted={grabbed}
                numbersHidden={numbersHidden}
              />

              {open && (
                <div className="divide-y divide-border-subtle border-t border-border-subtle bg-surface/30">
                  {node.children.map((child, j) => {
                    const list = childList(node.task.id);
                    const childGrabbed = live?.id === child.id;
                    const childShift =
                      childGrabbed && live ? live.dy : drag.shiftFor(list, j);
                    return (
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
                        onGrab={(e) =>
                          drag.onPointerDown(e, { id: child.id, containerId: list })
                        }
                        lifted={childGrabbed}
                        numbersHidden={numbersHidden}
                        rowRef={drag.itemRef(list, child.id)}
                        rowStyle={
                          childShift ? { transform: `translateY(${childShift}px)` } : undefined
                        }
                        rowClassName={cn(
                          "transition-[box-shadow] duration-200",
                          childGrabbed && LIFTED,
                          (childGrabbed ? live?.settling : live != null) &&
                            "transition-[transform,box-shadow] ease-out",
                        )}
                      />
                    );
                  })}
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
    </GridTable>
  );
}
