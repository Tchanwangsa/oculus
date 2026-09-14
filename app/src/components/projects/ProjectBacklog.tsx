import { ArrowRight } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import type { DbProject } from "@/lib/projects";
import { InlineAdd } from "./InlineAdd";
import { AgentMark, DueChip, SubtaskProgressBar, TaskGlyph } from "./TaskMarks";
import {
  appendSlot,
  backlogColumn,
  nodesIn,
  promotionTarget,
  subtaskProgress,
  type TaskNode,
} from "./taskTree";

/**
 * The `kind: "backlog"` column on its own: everything planned but not
 * committed to, as a list of stubs rather than a column of cards.
 *
 * Its one verb is promotion — a stub becomes work by landing in the first real
 * column, which is a `moveTask` like any other and so stamps or clears
 * `done_at` by the destination's kind for free. The shape is
 * `app/src/components/calendar/AgendaView.tsx`: a centred column of hairline
 * rows, because this is a list you read down rather than a surface you arrange.
 */
export function ProjectBacklog({
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
  const column = backlogColumn(project);
  const target = promotionTarget(project);

  if (!column) {
    return (
      <div className="flex h-full items-center justify-center px-6">
        <p className="text-xs text-muted-foreground">
          This project has no backlog column — every column on it is work in progress.
        </p>
      </div>
    );
  }

  const stubs = nodesIn(nodes, column.id);

  const promote = (id: number) => {
    if (!target) return;
    const slot = appendSlot(nodes, target.id);
    onMove(id, target.id, slot.before, slot.after);
  };

  return (
    <div className="page-scroll">
      <div className="mx-auto max-w-3xl px-6 py-5">
        <p className="mb-2.5 px-0.5 text-[11px] text-muted-foreground">
          Planned, not committed to. Promote a stub when it becomes work
          {target ? ` — it lands at the end of ${target.name}.` : "."}
        </p>

        <div className="divide-y divide-border-subtle overflow-hidden rounded-lg border border-border">
          {stubs.length === 0 && (
            <p className="px-3 py-6 text-center text-xs text-muted-foreground">
              Nothing in the backlog yet. Park the things you know are coming here, so the
              board only ever holds what you are actually doing.
            </p>
          )}

          {stubs.map((node) => {
            const progress = subtaskProgress(node);
            return (
              <div
                key={node.task.id}
                className="group/stub flex items-center gap-3 px-3 py-2.5 transition-colors hover:bg-surface"
              >
                <TaskGlyph kind={column.kind} />
                <span className="min-w-0 flex-1 truncate text-[12px] text-foreground">
                  {node.task.title}
                </span>
                <AgentMark source={node.task.source} />
                <DueChip dueAt={node.task.due_at} />
                {progress.total > 0 && <SubtaskProgressBar progress={progress} />}
                {target && (
                  <button
                    type="button"
                    onClick={() => promote(node.task.id)}
                    title={`Move to ${target.name}`}
                    className={cn(
                      "flex shrink-0 cursor-pointer items-center gap-1 rounded-full border border-border px-2 py-0.5 text-[11px] text-muted-foreground transition-[opacity,color]",
                      "opacity-0 hover:text-foreground focus-visible:opacity-100 group-hover/stub:opacity-100",
                    )}
                  >
                    {target.name}
                    <ArrowRight size={10} weight="bold" />
                  </button>
                )}
              </div>
            );
          })}

          <div className="px-1.5 py-1">
            <InlineAdd
              label="New stub"
              placeholder="Something you might do"
              onAdd={(title) => onCreate({ title, columnId: column.id })}
            />
          </div>
        </div>
      </div>
    </div>
  );
}
