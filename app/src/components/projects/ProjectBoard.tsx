import { useState } from "react";
import { cn } from "@/lib/utils";
import type { DbProject } from "@/lib/projects";
import { InlineAdd } from "./InlineAdd";
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
 * The Backlog view is not made redundant by that column: it is the same pile
 * read as a list, with a promote button per stub. Dragging a stub out of the
 * backlog column is the same `moveTask`, reached the way a board is normally
 * reached — which is the gesture a kanban board exists for, so leaving it out
 * cost more than the tidiness was worth.
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

            <div className="flex min-h-0 flex-1 flex-col gap-1.5 overflow-y-auto px-2 pb-1">
              {cards.length === 0 && (
                <p className="px-1 py-3 text-[11px] text-muted-foreground/60">
                  Nothing here yet.
                </p>
              )}

              {cards.map((node) => {
                const progress = subtaskProgress(node);
                return (
                  <article
                    key={node.task.id}
                    draggable
                    onDragStart={() => setDragId(node.task.id)}
                    onDragEnd={clear}
                    onDragOver={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      setOverColumn(column.id);
                      setOverTask(node.task.id);
                    }}
                    onDrop={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      drop(column.id, node.task.id);
                    }}
                    className={cn(
                      "cursor-grab rounded-lg border bg-card px-2.5 py-2 transition-colors active:cursor-grabbing",
                      dragId === node.task.id && "opacity-50",
                      overTask === node.task.id && dragId != null && dragId !== node.task.id
                        ? "border-brand"
                        : "border-border-subtle",
                    )}
                  >
                    <div className="flex items-start gap-1.5">
                      <TaskGlyph kind={column.kind} className="mt-0.5" />
                      <span
                        className={cn(
                          "min-w-0 flex-1 text-xs leading-snug",
                          node.task.done_at
                            ? "text-muted-foreground line-through"
                            : "text-foreground",
                        )}
                      >
                        {node.task.title}
                      </span>
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
