import { useState } from "react";
import { Link } from "react-router-dom";
import { cn } from "@/lib/utils";
import type { DbProject } from "@/lib/projects";
import { InlineAdd } from "./InlineAdd";
import { taskHref } from "./taskHref";
import { AgentMark, DueChip, SubtaskProgressBar, TaskGlyph } from "./TaskMarks";
import {
  boardColumns,
  dropSlot,
  nodesIn,
  subtaskProgress,
  type TaskNode,
} from "./taskTree";

/**
 * The project's columns side by side, cards in each — the backlog at the left
 * end, where a card's life starts.
 *
 * It is the only place a backlog stub is promoted now — there was a Backlog
 * list beside this with a promote button per row, and it went once the drag
 * below actually worked, because dragging a card out of the backlog into Todo
 * is the same `moveTask` reached by the gesture a kanban board exists for.
 *
 * Dragging is hand-rolled HTML5 (`draggable` + dragstart/dragover/drop), the
 * same shape as `app/src/components/llm/FallbackList.tsx` — a board of tens of
 * cards does not earn a drag library.
 */
export function ProjectBoard({
  project,
  nodes,
  onMove,
  onCreate,
}: {
  project: DbProject;
  nodes: TaskNode[];
  onMove: (id: number, columnId: string, before: number | null, after: number | null) => void;
  onCreate: (input: { title: string; columnId: string }) => void;
}) {
  const [dragId, setDragId] = useState<number | null>(null);
  const [overColumn, setOverColumn] = useState<string | null>(null);
  const [overTask, setOverTask] = useState<number | null>(null);

  const columns = boardColumns(project);

  const clear = () => {
    setDragId(null);
    setOverColumn(null);
    setOverTask(null);
  };

  /** `targetId` null means the column's empty space — append. */
  const drop = (columnId: string, targetId: number | null) => {
    const id = dragId;
    clear();
    if (id == null || id === targetId) return;
    const slot = dropSlot(nodesIn(nodes, columnId), id, targetId);
    onMove(id, columnId, slot.before, slot.after);
  };

  if (columns.length === 0) {
    return (
      <div className="flex h-full items-center justify-center px-6">
        <p className="text-xs text-muted-foreground">
          This project has no columns, so there is no board to draw.
        </p>
      </div>
    );
  }

  return (
    <div className="flex h-full gap-3 overflow-x-auto px-5 py-4">
      {columns.map((column) => {
        const cards = nodesIn(nodes, column.id);
        return (
          <section
            key={column.id}
            onDragOver={(e) => {
              e.preventDefault();
              e.dataTransfer.dropEffect = "move";
              setOverColumn(column.id);
            }}
            onDrop={(e) => {
              e.preventDefault();
              drop(column.id, null);
            }}
            className={cn(
              "flex w-72 shrink-0 flex-col rounded-xl border bg-surface/40 transition-colors",
              overColumn === column.id && dragId != null
                ? "border-brand/50"
                : "border-border-subtle",
            )}
          >
            <div className="flex shrink-0 items-center gap-2 px-3 pb-1.5 pt-2.5">
              <span className="truncate text-[11px] font-medium text-muted-foreground">
                {column.name}
              </span>
              <span className="text-[11px] tabular-nums text-muted-foreground/60">
                {cards.length}
              </span>
            </div>

            {/* `pt-1` is the drop indicator's room: the rule is drawn 4px above
                its card, and on the first card of a column that lands outside
                a scroller that clips on both axes. */}
            <div className="flex min-h-0 flex-1 flex-col gap-1.5 overflow-y-auto px-2 pb-1 pt-1">
              {cards.length === 0 && (
                <p className="px-1 py-3 text-[11px] text-muted-foreground/60">
                  Nothing here yet.
                </p>
              )}

              {cards.map((node) => {
                const progress = subtaskProgress(node);
                const landsHere =
                  overTask === node.task.id && dragId != null && dragId !== node.task.id;
                return (
                  <article
                    key={node.task.id}
                    draggable
                    onDragStart={(e) => {
                      // Load-bearing, and not obvious: WebKit — which is what
                      // a Tauri WKWebView is — aborts a drag whose dragstart
                      // sets no data, so without this call no dragover and no
                      // drop ever fire and a card cannot be moved at all. The
                      // payload is never read (the id is in state); setting
                      // *something* is the whole point. Do not "clean it up".
                      // `app/src/components/llm/FallbackList.tsx` carries the
                      // same line for the same reason.
                      e.dataTransfer.setData("text/plain", String(node.task.id));
                      e.dataTransfer.effectAllowed = "move";
                      setDragId(node.task.id);
                    }}
                    onDragEnd={clear}
                    onDragOver={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      e.dataTransfer.dropEffect = "move";
                      setOverColumn(column.id);
                      setOverTask(node.task.id);
                    }}
                    onDrop={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      drop(column.id, node.task.id);
                    }}
                    className={cn(
                      "relative cursor-grab rounded-lg border border-border-subtle bg-card px-2.5 py-2 transition-colors active:cursor-grabbing",
                      dragId === node.task.id && "opacity-50",
                    )}
                  >
                    {/* Where the card would land, rather than merely which
                        card is under the pointer: `dropSlot` puts the dragged
                        card *above* the one it is over, so the feedback is a
                        rule in that gap. It sits in the column's gap-1.5 as an
                        absolute child so showing it moves nothing. */}
                    {landsHere && (
                      <span
                        aria-hidden
                        className="pointer-events-none absolute inset-x-0 -top-1 h-0.5 rounded-full bg-brand"
                      />
                    )}

                    <div className="flex items-start gap-1.5">
                      <TaskGlyph kind={column.kind} className="mt-0.5" />
                      {/* The card is `draggable`, so the way into the task's
                          page is its title rather than a wrapper around the
                          whole card, which would fight the drag. And an
                          <a href> is draggable in its own right in WebKit,
                          so grabbing the title would start a link drag and
                          hijack the card's dragstart — hence the opt-out. */}
                      <Link
                        to={taskHref(project.id, node.task)}
                        draggable={false}
                        className={cn(
                          "min-w-0 flex-1 text-xs leading-snug hover:underline",
                          node.task.done_at
                            ? "text-muted-foreground line-through"
                            : "text-foreground",
                        )}
                      >
                        {node.task.title}
                      </Link>
                      <AgentMark source={node.task.source} className="mt-0.5" />
                    </div>

                    {(node.task.due_at || progress.total > 0) && (
                      <div className="mt-1.5 flex items-center gap-2.5 pl-[18px]">
                        <DueChip dueAt={node.task.due_at} />
                        {progress.total > 0 && <SubtaskProgressBar progress={progress} />}
                      </div>
                    )}
                  </article>
                );
              })}
            </div>

            <div className="shrink-0 px-2 pb-2 pt-0.5">
              <InlineAdd
                label="New task"
                placeholder="Task title"
                onAdd={(title) => onCreate({ title, columnId: column.id })}
              />
            </div>
          </section>
        );
      })}
    </div>
  );
}
