import { useMemo, useRef } from "react";
import { ArrowElbowDownRight } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import type { ColumnKind, DbProject } from "@/lib/projects";
import { useCardDrag, useSettledList } from "@/hooks/useCardDrag";
import { BoardView } from "./BoardParts";
import { CardTitle } from "./CardTitle";
import { InlineAdd } from "./InlineAdd";
import { taskHref } from "./taskHref";
import { AgentMark, DueChip, SubtaskProgressBar, TaskGlyph } from "./TaskMarks";
import {
  boardColumns,
  columnEntries,
  siblingDropSlot,
  subtaskProgress,
  type ColumnEntry,
  type TaskNode,
} from "./taskTree";

/** A margin rather than a box around parent and children: a nested container
 *  would have to be dragged out of as well as within. */
const SUBTASK_INSET = "ml-4";

/**
 * The project's columns side by side, backlog at the left. A subtask is a card
 * of its own, indented under its parent and dragged in its own right —
 * `columnEntries` (`./taskTree.ts`) does the grouping.
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
  const columns = useMemo(() => boardColumns(project), [project]);

  // The drop resolves against a ref: the grouping below needs the gesture's
  // state, which needs this callback. At drop time both are the same list.
  const nodesRef = useRef(nodes);
  nodesRef.current = nodes;

  // The hook's before/after are neighbours in DOM order, which interleaves
  // parents and subtasks; `siblingDropSlot` re-derives them at the card's level.
  const drag = useCardDrag(
    ({ id, from, containerId, index }) => {
      const tree = nodesRef.current;
      const slot = siblingDropSlot(tree, containerId, id, index);
      // The hook skips a drop in the same *flat* slot; this skips one that
      // lands between the same two siblings after crossing strangers.
      if (from === containerId) {
        const at = columnEntries(tree, containerId).findIndex((e) => e.task.id === id);
        if (at >= 0) {
          const now = siblingDropSlot(tree, containerId, id, at);
          if (now.before === slot.before && now.after === slot.after) return;
        }
      }
      onMove(id, containerId, slot.before, slot.after);
      return true;
    },
    { settleOn: nodes },
  );
  const live = drag.drag;

  // During a settle, draw the tree the gesture was measured against — see
  // `CardDragState.settling`.
  const settledNodes = useSettledList(nodes, live);

  // Pointer movement updates `live` every frame, while the settled list stays
  // the same. Keep the expensive tree grouping out of those drag renders.
  const byColumn = useMemo(
    () => new Map(columns.map((c) => [c.id, columnEntries(settledNodes, c.id)])),
    [columns, settledNodes],
  );

  const lifted = live
    ? byColumn.get(live.containerId)?.find((e) => e.task.id === live.id) ?? null
    : null;
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
    <BoardView
      columns={columns}
      byColumn={byColumn}
      drag={drag}
      lifted={lifted}
      idOf={(entry) => entry.task.id}
      hrefOf={(entry) => taskHref(project.id, entry.task)}
      cardClassName={(entry) => entry.depth === 1 ? SUBTASK_INSET : undefined}
      renderCard={(entry) => (
        <CardBody
          projectId={project.id}
          entry={entry}
          kind={columns.find((c) => c.id === entry.task.column_id)?.kind ?? null}
        />
      )}
      footer={(column) => (
        <InlineAdd
          label="New task"
          placeholder="Task title"
          onAdd={(title) => onCreate({ title, columnId: column.id })}
        />
      )}
    />
  );
}

/**
 * What is on a card — its own component because the card is drawn twice while
 * dragged. A subtask gets a parent's affordances a notch quieter, minus the
 * progress meter: there is only one level of subtask.
 */
function CardBody({
  projectId,
  entry,
  kind,
}: {
  projectId: number;
  entry: ColumnEntry;
  kind: ColumnKind | null;
}) {
  const sub = entry.depth === 1;
  const progress = entry.node ? subtaskProgress(entry.node) : null;
  return (
    <>
      {/* A subtask whose parent is in another column names its parent. */}
      {entry.orphaned && entry.parent && (
        <div className="mb-0.5 flex min-w-0 items-center gap-1 text-[11px] text-muted-foreground/60">
          <ArrowElbowDownRight size={10} className="shrink-0" />
          <span className="truncate">{entry.parent.title}</span>
        </div>
      )}

      <div className="flex min-w-0 items-start gap-1.5">
        <TaskGlyph kind={kind} size={sub ? 11 : 13} className="mt-0.5" />
        <CardTitle
          title={entry.task.title}
          href={taskHref(projectId, entry.task)}
          done={entry.task.done_at != null}
          small={sub}
        />
        <AgentMark source={entry.task.source} className="mt-0.5" />
      </div>

      {(entry.task.due_at || (progress?.total ?? 0) > 0) && (
        <div
          className={cn(
            "mt-1.5 flex items-center gap-2.5",
            sub ? "pl-[16px]" : "pl-[18px]",
          )}
        >
          <DueChip dueAt={entry.task.due_at} />
          {progress && progress.total > 0 && <SubtaskProgressBar progress={progress} />}
        </div>
      )}
    </>
  );
}
