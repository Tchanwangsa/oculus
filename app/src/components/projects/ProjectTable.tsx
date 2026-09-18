import { useState } from "react";
import type { CSSProperties, PointerEvent as ReactPointerEvent } from "react";
import { Link } from "react-router-dom";
import { CaretRight, DotsSixVertical, Plus } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { TablePagination, usePagedRows } from "@/components/ui/TablePagination";
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
 * Every task of the project as one table — the view for reading the plan
 * rather than working it. A parent expands into its subtasks in place, so the
 * whole tree is one column of rows instead of a drill-down.
 *
 * Structurally this is `app/src/components/sync/SyncHistoryTable.tsx`: one
 * `COLS` template shared by the header and every row, the header outside the
 * scroller, and a pinned `TablePagination` footer.
 *
 * **Rows reorder by dragging the grip that takes the index cell over on
 * hover** — GitHub Projects' idiom, and the reason the number and the handle
 * share one 28px cell without the row twitching as the pointer crosses it. The
 * gesture is `useCardDrag` (`app/src/hooks/useCardDrag.ts`), the same one the
 * board runs on. A drop writes `moveTask(id, task.column_id, before, after)`:
 * the row keeps the column it is in, and only its `position` moves — which is
 * exactly what this table is ordered by, since `getTasks` deliberately leaves
 * `column_id` out of its ORDER BY (`docs/projects.md`). So the midpoint between
 * two rows *is* this table's order, however many different columns the rows
 * around the drop happen to be sitting in.
 *
 * The cost of that is worth knowing before it surprises someone: one `position`
 * serves two orders. A later drag on the *board* whose gap underflows renumbers
 * that whole column to whole numbers, which can scramble an order hand-set
 * here; and two rows in different columns can already carry the same number, in
 * which case there is no midpoint between them to take and `moveTask` refuses
 * the write outright (`ProjectPage` logs it and nothing moves). A second column
 * holding the table's own order was considered and turned down — see the plan
 * in `data/plans/`.
 */

/** Column template shared by the header and every row — the one thing that
 *  keeps them in column. The first cell is 28px and holds the row number and
 *  the drag grip, one at a time. */
const COLS =
  "grid grid-cols-[28px_minmax(0,1fr)_110px_130px_120px_150px] items-center gap-3 px-5";

const HEADERS = ["", "Title", "Subject", "Status", "Due", "Subtasks"];

const PAGE_SIZE = 25;

/**
 * The drag engine's container ids. Every top-level row is in one list; each
 * expanded parent's children are a list of their own, so a subtask is only ever
 * reordered among its siblings.
 */
const TOP_LIST = "top";
const childList = (parentId: number) => `sub-${parentId}`;

/** How a row looks while it is the one being dragged. It needs a ground of its
 *  own — a row has none until it is hovered — and a stacking context, or the
 *  siblings it travels over would paint on top of it. */
const LIFTED = "relative z-10 bg-card shadow-md";

/** The neighbours a drop lands between, the way `moveTask` wants them. */
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
  /** Starts the reorder gesture. It hangs off the grip alone, never the row:
   *  a row carries a caret, a title link, a status popover and an add button,
   *  and a press on any of them has to keep doing what it does. */
  onGrab: (e: ReactPointerEvent<HTMLElement>) => void;
  /** This row is the one being dragged — it drops its hover ground, since the
   *  caller has given it a lifted one. */
  lifted: boolean;
  /** A reorder is in flight, so no row's number is the truth: the numbers are
   *  positions in a list that is being rearranged, and they all change at once
   *  when it lands. They fade out for the length of the gesture and fade back
   *  in already renumbered — which is cheaper to watch than five numbers
   *  flicking over in the frame the row lands in. */
  numbersHidden: boolean;
  /** Set on a subtask row, where the row itself is the draggable item. A
   *  top-level row is dragged by the block that holds it *and its expanded
   *  children*, so there the caller registers that block instead. */
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
      {/* Number and grip share the cell: the number sits in flow and keeps the
          28px honest, the grip is laid over it and fades in on row hover, so
          neither the header nor any other row shifts when the pointer arrives.
          `aria-hidden` because it is a pointer affordance with nothing behind
          it — the drag is mouse-only here, as it is on the board. */}
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
            // The press is not cancelled — doing that costs the row's links
            // their `click` in WebKit — so the grip is made unselectable
            // instead, or a drag begun on it would drag a text selection
            // across the table behind the row. See `DRAG_SURFACE`.
            DRAG_SURFACE,
            // The grip names the grabbing cursor rather than leaving it to
            // `:active`: it is the element holding the pointer capture, so its
            // cursor is the one on screen for the whole gesture — including
            // the stretches where the pointer has left the row entirely.
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

  /**
   * The reorder.
   *
   * `containerRef` is deliberately never called here, which is the one thing
   * about this that looks like an omission. The engine hit-tests the pointer
   * against the container boxes that *are* registered — first match wins, in
   * registration order — and in this table the lists nest: every expanded
   * parent's child block sits inside the top-level list's own box. The
   * top-level list is on screen before any child block can be, since the table
   * opens with everything collapsed, so it would win every hit test and no
   * subtask could ever be dropped into its own list. With no boxes registered
   * the engine leaves `containerId` at the list the gesture started in, which
   * is the confinement this table wants anyway: a row is reordered among its
   * own kind, or not at all.
   */
  const drag = useCardDrag((drop) => {
    // A drop that changed lists would be a *re-parent*, and `moveTask` cannot
    // write one — `parent_id` is not among the three columns it owns. Moving
    // the row's `position` without re-parenting it would be worse than doing
    // nothing, since the row would sort to a slot it is not drawn in. So it is
    // refused outright rather than half made. This is also the invariant
    // everything below leans on, and the reason no container boxes are
    // registered above.
    if (drop.from !== drop.containerId) return;

    if (drop.containerId === TOP_LIST) {
      // The engine only ever saw the 25 rows this page renders, so its index —
      // and the `before`/`after` it derived from it — is page-local. Dropping
      // at the top of page 2 hands back `before: null`, which `moveTask` reads
      // as "before everything" and would land the row above page 1. The page
      // offset maps that slot onto the full `nodes` list, and the neighbours
      // come from there instead. The offset is the same either side of the
      // dragged row: the engine has already taken the row out of its own
      // neighbour list, and so has `rest`.
      const from = nodes.findIndex((n) => n.task.id === drop.id);
      if (from < 0) return;
      const rest = nodes.filter((n) => n.task.id !== drop.id);
      const slot = (page - 1) * PAGE_SIZE + drop.index;
      // Already between those two rows: a write plus a project-wide re-read to
      // change nothing on screen.
      if (slot === from) return;
      const task = nodes[from].task;
      const { before, after } = slotIn(rest, slot, (n) => n.task.id);
      onMove(task.id, task.column_id, before, after);
      return true;
    }

    // A subtask, among the siblings it is drawn with. Its parent's children are
    // all on screen together — the block is not paginated — so the engine's
    // index needs no offset here.
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

  /**
   * The rows the *settle* is drawn against — see `CardDragState.settling`.
   *
   * `onMove`'s write comes back as a project-wide re-read, which lands while
   * the dropped row is still gliding into its slot. Drawing the new order
   * there would put it underneath the transforms that were worked out for the
   * old one, moving every row twice; so the table keeps the list it had when
   * the pointer went up until the gesture lets go, and takes the new one in
   * the same commit the transforms come off in — where they are the same
   * picture and the swap cannot be seen.
   *
   * Only the rows are held. `page` is not: a reorder cannot change how many
   * rows there are, so the paging around it is the same either way.
   */
  const settledNodes = useSettledList(nodes, live);
  const rows =
    settledNodes === nodes
      ? pageRows
      : settledNodes.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);
  /** A row's number is a position in the list being rearranged — see
   *  `TaskRow`'s `numbersHidden`. Only a top-level drag renumbers anything;
   *  a subtask's siblings are unnumbered. */
  const numbersHidden = live != null && live.containerId === TOP_LIST;

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
          {rows.map((node, i) => {
            const open = expanded.has(node.task.id);
            const progress = subtaskProgress(node);
            const grabbed = live?.id === node.task.id;
            // **The grabbed block's own travel, and the reason it needs a
            // branch of its own.** `shiftFor` answers 0 for it — it describes
            // how far the *other* rows move to open the gap, and the grabbed
            // row is the gap — so reading it here left the one row the hand is
            // on sitting still while its neighbours slid around it. The lift
            // comes from the live gesture instead. Vertically only: a row
            // spans the table's width, so there is nowhere sideways to go, and
            // `dx` would only tear it out of its columns.
            const shift = grabbed && live ? live.dy : drag.shiftFor(TOP_LIST, i);
            return (
              // The draggable item is this block, not the row inside it, so an
              // expanded parent travels with its subtasks instead of leaving
              // them behind mid-gesture. The engine's displacement is the
              // grabbed item's own height, so a block that is taller than a row
              // still opens and closes exactly the gap it occupies.
              //
              // It is translated where it sits rather than drawn into a portal
              // the way the board's lifted card is. The board portals because
              // each of its columns is a scroller that clips on both axes and a
              // card crosses between them; a row here only ever travels up and
              // down inside one scroller, where being clipped at that
              // scroller's edge is the right answer rather than a bug.
              <div
                key={node.task.id}
                ref={drag.itemRef(TOP_LIST, node.task.id)}
                style={shift ? { transform: `translateY(${shift}px)` } : undefined}
                className={cn(
                  // The lift's shadow fades rather than vanishing, so the row
                  // setting down is one movement. It is on the base class
                  // because a transition only runs on a property the style it
                  // lands in still transitions, and `LIFTED` is *removed* here.
                  "transition-[box-shadow] duration-200",
                  grabbed && LIFTED,
                  // The neighbours glide as the gap opens and closes. The
                  // grabbed row glides only once the hand is off it: while the
                  // pointer is down it must track it with no easing at all, or
                  // the row lags behind the grip.
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
                      // Same as the block above: the grabbed subtask rides the
                      // pointer, its siblings ride `shiftFor`.
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
