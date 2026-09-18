import { useRef } from "react";
import { createPortal } from "react-dom";
import { useNavigate } from "react-router-dom";
import { ArrowElbowDownRight } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import type { ColumnKind, DbProject } from "@/lib/projects";
import { DRAG_SURFACE, useCardDrag, useSettledList } from "@/hooks/useCardDrag";
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

/**
 * The project's columns side by side, cards in each — the backlog at the left
 * end, where a card's life starts.
 *
 * It is the only place a backlog stub is promoted now — there was a Backlog
 * list beside this with a promote button per row, and it went once the drag
 * below actually worked, because dragging a card out of the backlog into Todo
 * is the same `moveTask` reached by the gesture a kanban board exists for.
 *
 * **A subtask is a card of its own here**, indented under its parent and
 * dragged in its own right, rather than a line nested inside the parent's card:
 * a subtask is the thing you actually work on, so it has to be movable between
 * columns like anything else, and a card holding a list of other cards would
 * have made a board of one column's worth of boards. `columnEntries`
 * (`./taskTree.ts`) does the grouping and says why it is structural.
 *
 * The drag is `useCardDrag` (`app/src/hooks/useCardDrag.ts`), which is the tab
 * strip's pointer-capture gesture generalised across columns: press, lift past
 * 4px, ride the pointer, and the neighbours slide out of the way — the gap
 * opening up *is* the drop feedback, in place of the rule this used to draw
 * above the card being landed on. That hook's doc comment carries why HTML5
 * drag-and-drop had to go; the short version is that pressing on a due chip
 * started a text selection and WebKit then refused to lift the card at all.
 */
const CARD = cn(
  "rounded-lg border border-border-subtle bg-card px-2.5 py-2 cursor-grab active:cursor-grabbing",
  // The whole card opens the task now, so the whole card answers the pointer.
  "hover:border-border",
  // The whole card is the drag surface, so nothing inside it may be selectable:
  // a press that started a text selection would smear a highlight across the
  // board behind the card as it travels. `DRAG_SURFACE` is why that is done in
  // CSS rather than by cancelling the press — cancelling it took the card
  // title's `click` with it, and with that the only way into a task's page.
  DRAG_SURFACE,
);

/** The step a subtask card is inset by. A left margin rather than a box drawn
 *  around a parent and its children: the indent is the only thing that has to
 *  say "this belongs to the card above", and a nested container would have to
 *  be dragged out of as well as within. */
const SUBTASK_INSET = "ml-4";

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
  const columns = boardColumns(project);
  // The pane's own router, which is what the title anchor navigated through
  // when it was the only way in — so a click on the card and a click on the
  // title land identically.
  const navigate = useNavigate();

  // The tree the *drop* resolves against, in a ref rather than read from the
  // render: the grouping below is built from the tree the settle is drawn
  // against, which needs the gesture's own state, which needs this callback.
  // At the moment of a drop the two are the same list — the settle has not
  // started yet — so the ref is the honest way round that knot.
  const nodesRef = useRef(nodes);
  nodesRef.current = nodes;

  // The hook hands back a flat slot index plus the neighbours either side of
  // it *in DOM order*, and DOM order here interleaves parents and subtasks — so
  // its pair is the wrong pair. `siblingDropSlot` re-derives it from the index
  // against the dragged card's own level and group, which is the only pair the
  // grouping will not immediately re-sort away.
  const drag = useCardDrag(
    ({ id, from, containerId, index }) => {
      const tree = nodesRef.current;
      const slot = siblingDropSlot(tree, containerId, id, index);
      // The hook already drops a gesture that ends in the slot it started in,
      // but that is the *flat* slot: a card can cross several strangers and
      // still land between the same two siblings. Writing that move would spend
      // a write and a board-wide re-read to change nothing on screen.
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

  /**
   * The tree the board draws, which during a settle is the one the gesture was
   * measured against rather than the one the store has just re-read — see
   * `CardDragState.settling`. Re-grouping the columns mid-glide would put the
   * new order underneath transforms worked out for the old one.
   */
  const settledNodes = useSettledList(nodes, live);

  // Shaped once per render and shared: the cards each column draws, and the
  // lists the lifted copy is found in. Two passes over the tree could disagree
  // about what the user was looking at.
  const byColumn = new Map(
    columns.map((c) => [c.id, columnEntries(settledNodes, c.id)]),
  );

  // The lifted card is drawn a second time, in a fixed overlay, because each
  // column's card list is its own `overflow-y-auto` scroller and a scroller
  // clips on both axes — a card translated towards the next column would
  // simply be cut off at its own column's edge. The copy in the list stays in
  // flow (invisible) so the slot it came from is still there to animate around.
  const lifted = live
    ? [...byColumn.values()].flat().find((e) => e.task.id === live.id) ?? null
    : null;
  const liftedKind = lifted
    ? columns.find((c) => c.id === lifted.task.column_id)?.kind ?? null
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
    <div className="flex h-full gap-3 overflow-x-auto px-5 py-4">
      {columns.map((column) => {
        const cards = byColumn.get(column.id) ?? [];
        return (
          <section
            key={column.id}
            ref={drag.containerRef(column.id)}
            className={cn(
              "flex w-72 shrink-0 flex-col rounded-xl border bg-surface/40 transition-colors",
              live?.targetContainerId === column.id
                ? "border-brand/50"
                : "border-border-subtle",
            )}
          >
            <div className="flex shrink-0 items-center gap-2 px-3 pb-1.5 pt-2.5">
              <span className="truncate text-[11px] font-medium text-muted-foreground">
                {column.name}
              </span>
              {/* Every card in the column, subtasks included — it used to be
                  the same number either way, and now that a subtask is a card
                  it is not. The count reads as "how much is sitting here", so
                  it has to agree with what you can count below it; a header
                  saying 3 over five cards is a header nobody trusts again.
                  Progress elsewhere still counts top-level tasks only
                  (`boardProgress`), which is a different question. */}
              <span className="text-[11px] tabular-nums text-muted-foreground/60">
                {cards.length}
              </span>
            </div>

            <div className="flex min-h-0 flex-1 flex-col gap-1.5 overflow-y-auto px-2 pb-1 pt-1">
              {cards.length === 0 && (
                <p className="px-1 py-3 text-[11px] text-muted-foreground/60">
                  Nothing here yet.
                </p>
              )}

              {cards.map((entry, i) => {
                const grabbed = live?.id === entry.task.id;
                const shift = grabbed ? 0 : drag.shiftFor(column.id, i);
                const href = taskHref(project.id, entry.task);
                return (
                  <article
                    key={entry.task.id}
                    ref={drag.itemRef(column.id, entry.task.id)}
                    /* **Why the card is not an `<a>` or a `<button>`.** The
                       whole card opens the task, but it holds controls of its
                       own — the title's anchor, the Show more toggle — and
                       interactive content nested inside an anchor or a button
                       is invalid markup that WebKit repairs by closing the
                       outer control early, stranding the rest of the card
                       outside it (see `components/markdown/FileChip.tsx`). So
                       the card stays a plain `<article>` that navigates on
                       click, and carries `data-tab-href` so ⌘-click still
                       finds a route to open in its own tab
                       (`app/src/lib/newTabClicks.ts`) from anywhere on it,
                       not only from the title. */
                    data-tab-href={href}
                    onPointerDown={(e) =>
                      drag.onPointerDown(e, { id: entry.task.id, containerId: column.id })
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
                       swallowed, and only because the press that started it is
                       the same press. */
                    onClickCapture={(e) => {
                      if (!drag.didDrag()) return;
                      e.preventDefault();
                      e.stopPropagation();
                    }}
                    style={shift ? { transform: `translateY(${shift}px)` } : undefined}
                    className={cn(
                      CARD,
                      entry.depth === 1 && SUBTASK_INSET,
                      grabbed
                        ? // Named rather than left to `active:cursor-grabbing`:
                          // this card is invisible but still the element
                          // holding the pointer capture, so its cursor is the
                          // one on screen for the whole gesture, including the
                          // stretches where the pointer is over another column.
                          "cursor-grabbing opacity-0"
                        : live
                          ? "transition-transform duration-200 ease-out"
                          : "transition-colors",
                    )}
                  >
                    <CardBody projectId={project.id} entry={entry} kind={column.kind} />
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

      {/* Lifted: a floating card above the board, tracking the pointer with no
          easing lag, exactly as a dragged tab does. Portalled to the body
          rather than positioned in place so no column's scroller can clip it,
          and inert so the pointer keeps reaching the card that captured it.
          The overlay takes its width from the captured rect, so a subtask's
          copy is already the indented width and needs no inset of its own. */}
      {live &&
        lifted &&
        createPortal(
          <div
            aria-hidden
            /* z-50 rather than z-10: this is a sibling of `#root` under
               `<body>`, so it is competing with the app's own top tier — the
               command palette and the dialogs — and not with anything inside
               the board. A lifted card that paints *under* the content card is
               indistinguishable from a card that never lifted. */
            className={cn(
              "pointer-events-none fixed z-50",
              // Tracking the pointer takes no easing — the copy *is* the hand.
              // Once the pointer is up it glides into the slot the column has
              // been holding open, so that when the re-read lands and this copy
              // goes, the card in the list is already where this one is.
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
              <CardBody projectId={project.id} entry={lifted} kind={liftedKind} />
            </article>
          </div>,
          document.body,
        )}
    </div>
  );
}

/**
 * What is on a card. Its own component because the card is drawn twice while
 * it is being dragged — once in its slot, once in the overlay riding the
 * pointer — and the two must not be able to drift apart.
 *
 * A subtask gets the same affordances a parent does — the column's glyph, a
 * link to its own page, the agent mark, its due date — a notch quieter, since
 * the indent alone is a weak signal once a card has been dragged away from the
 * parent it belongs to. What it does not get is a progress meter: one level of
 * subtask is all there is, so a subtask has nothing to report on.
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
      {/* A subtask whose parent is in another column is drawn at the end of
          this one with nothing above it to belong to, so it names its parent.
          This is an ordinary state — finishing one piece of a task that is
          still in progress is the whole point of subtasks — so the line is
          quiet rather than a warning. */}
      {entry.orphaned && entry.parent && (
        <div className="mb-0.5 flex min-w-0 items-center gap-1 text-[11px] text-muted-foreground/60">
          <ArrowElbowDownRight size={10} className="shrink-0" />
          <span className="truncate">{entry.parent.title}</span>
        </div>
      )}

      <div className="flex min-w-0 items-start gap-1.5">
        <TaskGlyph kind={kind} size={sub ? 11 : 13} className="mt-0.5" />
        {/* Folded to three lines, broken so an unbreakable token cannot leave
            the card, and still an anchor: it is the keyboard way in and what
            ⌘-click reads. A subtask is a task, so it has a page of its own at
            the same route. */}
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
