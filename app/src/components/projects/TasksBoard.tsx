import { useRef } from "react";
import { createPortal } from "react-dom";
import { useNavigate } from "react-router-dom";
import { ArrowElbowDownRight } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { DRAG_SURFACE, useCardDrag, useSettledList } from "@/hooks/useCardDrag";
import { displayCode } from "@/lib/format";
import type { DbProject, DbTaskWithProject } from "@/lib/projects";
import { CardTitle } from "./CardTitle";
import { taskHref } from "./taskHref";
import { AgentMark, DueChip, TaskGlyph } from "./TaskMarks";
import {
  KIND_COLUMNS,
  columnForKind,
  kindOf,
  projectLabel,
} from "./universalTasks";

/**
 * Every task there is, on three columns — Backlog, In progress, Done.
 *
 * The columns are **kinds**, not columns: this board spans projects, and the
 * only vocabulary every project's board shares is what a column means
 * (`KIND_COLUMNS` in `./universalTasks.ts`). Dropping a card in another column
 * therefore writes a change of kind, resolved against the card's *own* board —
 * so a project that distinguishes Todo from In progress keeps that
 * distinction, and a board with no Done column refuses a drop onto Done
 * instead of inventing one.
 *
 * **Within a column there is no manual order, so a drag inside one is a
 * no-op.** `./universalTasks.ts` carries the long version: `position` is only
 * comparable inside one project's column, so this column is *sorted* — by due
 * date (nulls last), then project, then position — rather than arranged. The
 * order is `UNIVERSAL_ORDER`'s, carried through from `getAllTasks`, the same
 * trade `columnEntries` makes with `getTasks`'s ORDER BY: the rows arrive in
 * the order they are drawn in, and this file only groups them. Nothing here
 * re-sorts, so handing it rows in another order would quietly change what the
 * board means.
 *
 * **A subtask is a plain card, not an indented one.** `ProjectBoard` draws it
 * under its parent because within one project the parent is right there and
 * the indent is the cheapest way to say so. Here the sort has already put the
 * two wherever their due dates put them — possibly pages apart, possibly in
 * different columns — so there is nothing to indent *under*, and the card
 * names its parent on a line of its own instead.
 */
const CARD = cn(
  "rounded-lg border border-border-subtle bg-card px-2.5 py-2 cursor-grab active:cursor-grabbing",
  // The whole card opens the task now, so the whole card answers the pointer.
  "hover:border-border",
  // The whole card is the drag surface, so nothing in it may be selectable —
  // see `DRAG_SURFACE`, which is CSS precisely because cancelling the press
  // would take the title's `click` with it.
  DRAG_SURFACE,
);

export function TasksBoard({
  tasks,
  projectById,
  onMove,
}: {
  /** In `UNIVERSAL_ORDER` — see above. */
  tasks: DbTaskWithProject[];
  projectById: Map<number, DbProject>;
  /** Given a column id off the task's own board, never an invented one. */
  onMove: (task: DbTaskWithProject, columnId: string) => void;
}) {
  // The rows the *drop* resolves against, in a ref rather than read from the
  // render: the grouping below is built from the list the settle is drawn
  // against, which needs the gesture's own state, which needs this callback.
  // At the moment of a drop the two are the same list.
  const tasksRef = useRef(tasks);
  tasksRef.current = tasks;

  // The pane's own router, which is what the title anchor navigated through
  // when it was the only way in — so a click on the card and a click on the
  // title land identically.
  const navigate = useNavigate();

  const drag = useCardDrag(
    ({ id, from, containerId }) => {
      // The whole no-manual-order rule, in one line: a card that came back to
      // the kind it started in has nothing to write, however far it travelled or
      // which two cards it was hovering between. The hook already drops a
      // gesture that ends in the slot it began in; this drops the rest.
      if (from === containerId) return;
      const task = tasksRef.current.find((t) => t.id === id);
      const target = KIND_COLUMNS.find((c) => c.kind === containerId);
      if (!task || !target) return;
      const column = columnForKind(task, projectById, target.kind);
      // No column of that kind on this task's own board. Nothing is written and
      // nothing is faked — the card springs back to where it was.
      if (!column) return;
      onMove(task, column.id);
      return true;
    },
    { settleOn: tasks },
  );
  const live = drag.drag;

  /** The list the board draws: during a settle, the one the gesture was
   *  measured against rather than the one the store has just re-read — see
   *  `CardDragState.settling`. Re-grouping the columns mid-glide would put the
   *  new grouping underneath transforms worked out for the old one. */
  const settledTasks = useSettledList(tasks, live);

  const byId = new Map(settledTasks.map((t) => [t.id, t]));

  // Grouped once per render and shared by the columns: two passes could
  // disagree about what the user was looking at.
  const byKind = new Map(KIND_COLUMNS.map((c) => [c.kind, [] as DbTaskWithProject[]]));
  for (const task of settledTasks) byKind.get(kindOf(task, projectById))?.push(task);

  /** The pointer is back over the column it was lifted from, where a drop
   *  would do nothing — so nothing moves. The cards used to slide apart and
   *  then close again on release, which is a promise the write cannot keep. */
  const idle = live != null && live.targetContainerId === live.containerId;

  const lifted = live ? byId.get(live.id) ?? null : null;

  return (
    <div className="flex h-full gap-3 overflow-x-auto px-5 py-4">
      {KIND_COLUMNS.map((column) => {
        const cards = byKind.get(column.kind) ?? [];
        return (
          <section
            key={column.kind}
            ref={drag.containerRef(column.kind)}
            className={cn(
              "flex w-72 shrink-0 flex-col rounded-xl border bg-surface/40 transition-colors",
              live?.targetContainerId === column.kind && !idle
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

            <div className="flex min-h-0 flex-1 flex-col gap-1.5 overflow-y-auto px-2 pb-2 pt-1">
              {cards.length === 0 && (
                <p className="px-1 py-3 text-[11px] text-muted-foreground/60">
                  Nothing here yet.
                </p>
              )}

              {cards.map((task, i) => {
                const grabbed = live?.id === task.id;
                const shift = grabbed || idle ? 0 : drag.shiftFor(column.kind, i);
                const href = taskHref(task.project_id, task);
                return (
                  <article
                    key={task.id}
                    ref={drag.itemRef(column.kind, task.id)}
                    /* The card opens the task, and stays a plain `<article>`
                       to do it: it holds controls of its own — the title's
                       anchor, the Show more toggle — and interactive content
                       inside an anchor or a button is invalid markup WebKit
                       repairs by closing the outer control early, stranding
                       the rest of the card (see
                       `components/markdown/FileChip.tsx`). `data-tab-href` is
                       what gives ⌘-click a route to open in its own tab
                       (`app/src/lib/newTabClicks.ts`) from anywhere on the
                       card rather than only from the title. */
                    data-tab-href={href}
                    onPointerDown={(e) =>
                      drag.onPointerDown(e, { id: task.id, containerId: column.kind })
                    }
                    onClick={(e) => {
                      // What is *on* the card keeps its own click: the title
                      // anchor navigates here by itself, and the toggle is not
                      // a navigation at all.
                      if (e.target instanceof Element && e.target.closest("a[href], button")) {
                        return;
                      }
                      navigate(href);
                    }}
                    /* A plain click navigates — it is the way into the task's
                       page — so only a click that was really a drag is
                       swallowed. */
                    onClickCapture={(e) => {
                      if (!drag.didDrag()) return;
                      e.preventDefault();
                      e.stopPropagation();
                    }}
                    style={shift ? { transform: `translateY(${shift}px)` } : undefined}
                    className={cn(
                      CARD,
                      grabbed
                        ? // This card is invisible but still holds the pointer
                          // capture, so its cursor is the one on screen for the
                          // whole gesture — including over another column.
                          "cursor-grabbing opacity-0"
                        : live
                          ? "transition-transform duration-200 ease-out"
                          : "transition-colors",
                    )}
                  >
                    <TaskCard
                      task={task}
                      parent={task.parent_id != null ? byId.get(task.parent_id) ?? null : null}
                      projectById={projectById}
                    />
                  </article>
                );
              })}
            </div>
          </section>
        );
      })}

      {/* The lifted card is drawn a second time in a fixed overlay, because
          each column's list is its own scroller and a scroller clips on both
          axes — a card translated towards the next column would be cut off at
          its own column's edge. `z-50` rather than `z-10`: this is a sibling of
          `#root`, competing with the palette and the dialogs rather than with
          anything inside the board. */}
      {live &&
        lifted &&
        createPortal(
          <div
            aria-hidden
            className={cn(
              "pointer-events-none fixed z-50",
              // No easing while it tracks the pointer — the copy *is* the hand.
              // Once the pointer is up it glides into the column it was dropped
              // in, so that when the re-read lands and this copy goes, the card
              // in the list is already where this one is.
              live.settling && "transition-transform duration-200 ease-out",
            )}
            style={{
              left: live.rect.left,
              top: live.rect.top,
              width: live.rect.width,
              transform: `translate(${live.dx}px, ${live.dy}px)`,
            }}
          >
            <article className={cn(CARD, "shadow-md")}>
              <TaskCard
                task={lifted}
                parent={lifted.parent_id != null ? byId.get(lifted.parent_id) ?? null : null}
                projectById={projectById}
              />
            </article>
          </div>,
          document.body,
        )}
    </div>
  );
}

/**
 * What is on a card. Its own component because the card is drawn twice while
 * it is dragged — once in its slot, once in the overlay riding the pointer —
 * and the two must not be able to drift apart.
 *
 * The line a project's own board does not need and this one cannot do without
 * is the first one: which project this is. It leads with the subject's glyph
 * where there is one, because across a whole semester the subject is what
 * tells two "Read chapter 4"s apart faster than a project name does.
 */
function TaskCard({
  task,
  parent,
  projectById,
}: {
  task: DbTaskWithProject;
  /** The parent row, when this card is a subtask and the parent is in the
   *  list. `null` otherwise — including for a subtask whose parent was
   *  filtered out, which then reads as a plain task. */
  parent: DbTaskWithProject | null;
  projectById: Map<number, DbProject>;
}) {
  return (
    <>
      <div className="flex min-w-0 items-center gap-1.5 text-[11px] text-muted-foreground">
        {task.project_subject_code && (
          <SubjectIcon code={task.project_subject_code} size={11} />
        )}
        <span
          className={cn(
            "truncate",
            task.project_id == null && "text-muted-foreground/60",
          )}
        >
          {task.project_subject_code
            ? `${displayCode(task.project_subject_code)} · ${projectLabel(task)}`
            : projectLabel(task)}
        </span>
      </div>

      {/* A subtask has no parent above it to belong to here — the column is
          sorted, not grouped — so it names the task it is part of. Quiet: this
          is the ordinary state of a subtask in a cross-project list, not a
          warning about one. */}
      {parent && (
        <div className="mt-0.5 flex min-w-0 items-center gap-1 text-[11px] text-muted-foreground/60">
          <ArrowElbowDownRight size={10} className="shrink-0" />
          <span className="truncate">{parent.title}</span>
        </div>
      )}

      <div className="mt-1 flex min-w-0 items-start gap-1.5">
        <TaskGlyph kind={kindOf(task, projectById)} size={13} className="mt-0.5" />
        {/* Folded to three lines and broken so an unbreakable token — a pasted
            URL — cannot leave the card. Still an anchor: it is the keyboard way
            in and what ⌘-click reads. */}
        <CardTitle
          title={task.title}
          href={taskHref(task.project_id, task)}
          done={task.done_at != null}
        />
        <AgentMark source={task.source} className="mt-0.5" />
      </div>

      {task.due_at && (
        <div className="mt-1.5 pl-[18px]">
          <DueChip dueAt={task.due_at} />
        </div>
      )}
    </>
  );
}
