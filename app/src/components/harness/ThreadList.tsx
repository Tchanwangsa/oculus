import { memo, useEffect, useMemo, useRef, useState } from "react";
import { CaretRight, CircleNotch, Plus, SidebarSimple, Trash, VideoCamera, X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { ProviderMark } from "@/components/harness/ProviderMark";
import type { HarnessThread } from "@/lib/harness";
import type { Subject } from "@/lib/db";
import { displayCode, displayName } from "@/lib/format";
import { cn } from "@/lib/utils";

const COLLAPSED_KEY = "oculus-chat-groups-collapsed";
const ORDER_KEY = "oculus-chat-groups-order";

/** How many threads a group shows before it has to be asked for more, and how
 *  many each ask adds. Long-running subjects accumulate dozens of threads, and
 *  a column that lists every one of them buries the groups under it. */
const PAGE = 5;

/** Which groups are folded away, by group key. What is stored is the collapsed
 *  ones rather than the open ones, so a subject scoped for the first time
 *  arrives expanded without having to be listed anywhere first. */
function loadCollapsed(): Set<string> {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? "[]");
    return new Set(Array.isArray(raw) ? raw.filter((k): k is string => typeof k === "string") : []);
  } catch {
    return new Set();
  }
}

/** The order the groups have been dragged into, by group key. Only the keys
 *  that have been arranged are stored — a subject scoped for the first time
 *  has never been dragged anywhere, so it is not in here and sorts by recency
 *  like it always did. */
function loadOrder(): string[] {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(ORDER_KEY) ?? "[]");
    return Array.isArray(raw) ? raw.filter((k): k is string => typeof k === "string") : [];
  } catch {
    return [];
  }
}

/** Threads under the scope they were opened with, most recent group first.
 *  Threads arrive ordered by `updated_at`, so first appearance is recency for
 *  the groups too, and a subject that has since been deleted falls back into
 *  General rather than disappearing with its heading. */
function group(
  threads: HarnessThread[],
  subjects: Subject[],
): {
  key: string;
  label: string;
  title: string;
  subjectId: number | null;
  threads: HarnessThread[];
}[] {
  const out: ReturnType<typeof group> = [];
  const byKey = new Map<string, (typeof out)[number]>();
  for (const t of threads) {
    const subject = t.subject_id == null ? null : subjects.find((s) => s.id === t.subject_id) ?? null;
    const key = subject ? String(subject.id) : "general";
    let g = byKey.get(key);
    if (!g) {
      g = {
        key,
        label: subject ? displayCode(subject.code) : "General",
        title: subject ? displayName(subject.name, subject.code) : "Not scoped to a subject",
        subjectId: subject?.id ?? null,
        threads: [],
      };
      byKey.set(key, g);
      out.push(g);
    }
    g.threads.push(t);
  }
  return out;
}

/**
 * Recency, overruled by the arrangement the student dragged the headers into.
 *
 * Only the groups named in `order` are placed by it; anything else keeps the
 * recency position it arrived in, **above** them — a subject scoped for the
 * first time has a conversation in it right now, and burying it under an
 * arrangement made before it existed would hide the thread that put it there.
 * A drop writes every key back, so the surprise lasts exactly until the next
 * drag. `sort` is stable in every engine this runs on, which is what keeps
 * the unplaced ones in recency order among themselves.
 */
function arrange<T extends { key: string }>(groups: T[], order: string[]): T[] {
  if (!order.length) return groups;
  // `indexOf` answers -1 for a key nobody has placed, which is what sorts it
  // above every key somebody has.
  return groups.slice().sort((a, b) => order.indexOf(a.key) - order.indexOf(b.key));
}

/**
 * The conversations column: threads grouped by the subject they are scoped
 * to, each row its agent's mark and the thread's name.
 *
 * Its width is the page's (`useResizablePanel` in `ChatPage`); this only draws
 * it. Folded means gone rather than narrowed — the app's own sidebar pattern —
 * so the way back in is the button in the chat header, not a rail left behind
 * here. The inner box keeps the unfolded width throughout, so the fold clips
 * the list instead of reflowing every row on its way out.
 *
 * A group header doubles as a way in: its `+` opens a new thread already
 * scoped to that subject, which is the whole reason to pick a scope and the
 * one place the choice is already made.
 *
 * The name is the model's own — asked for once the first exchange is done
 * (`Harness::name_thread`) and, until it lands, the first line of the first
 * message. The model the thread runs on is not shown here: it can change per
 * send, it is already in the composer, and this list answers "which
 * conversation", not "on what".
 */
export const ThreadList = memo(function ThreadList({
  threads,
  subjects,
  activeId,
  runningIds,
  width,
  restWidth,
  collapsed,
  animate,
  onToggle,
  onOpen,
  onNew,
  onDelete,
}: {
  threads: HarnessThread[];
  subjects: Subject[];
  activeId: number | null;
  /** Drawn width: 0 while folded. */
  width: number;
  /** The width it unfolds back to — what the contents are laid out at. */
  restWidth: number;
  collapsed: boolean;
  /** Off mid-drag: a width that eased toward every mouse position lagged the
   *  handle by a frame and felt like dragging elastic. */
  animate: boolean;
  onToggle: () => void;
  /** Which threads have a turn in flight. A set of ids rather than the
   *  store's live map: that map changes with every streamed token, and this
   *  column only ever asks it one yes/no question per row. */
  runningIds: Set<number>;
  onOpen: (id: number) => void;
  /** No argument is the composer's current scope; a subject id (or null for
   *  General) starts the new thread in that group instead. */
  onNew: (subjectId?: number | null) => void;
  onDelete: (id: number) => void;
}) {
  const [confirming, setConfirming] = useState<number | null>(null);
  // Which *groups* are folded — not to be confused with the panel's own
  // `collapsed` prop above.
  const [folded, setFolded] = useState<Set<string>>(loadCollapsed);
  // How many threads each group has been asked to show, over the default page.
  // Deliberately not persisted: a fresh window starts every group short again.
  const [shown, setShown] = useState<Record<string, number>>({});
  // The arrangement the headers have been dragged into, and the drag in
  // flight: which group is lifted and which gap it would drop into.
  const [order, setOrder] = useState<string[]>(loadOrder);
  const [drag, setDrag] = useState<{ key: string; at: number } | null>(null);
  // Each group's box, for the maths a drag does. A ref rather than state:
  // it is read when a drag starts and never drawn from.
  const boxes = useRef(new Map<string, HTMLDivElement>());
  // A drag ends in a `click` on the header it started from, since the pointer
  // went down and up on the same button. Without this that click would fold
  // the group you had just finished moving.
  const dragged = useRef(false);

  useEffect(() => {
    localStorage.setItem(COLLAPSED_KEY, JSON.stringify([...folded]));
  }, [folded]);

  const groups = useMemo(
    () => arrange(group(threads, subjects), order),
    [threads, subjects, order],
  );

  /**
   * Dragging a group header to rearrange the column, on pointer events rather
   * than HTML5 drag-and-drop — the same shape the tab strip's reorder uses
   * (`components/tabs/TopTabBar.tsx`), and for the same reason: a `dragstart`
   * that sets no `dataTransfer` is cancelled outright by WebKit, and the
   * payload nothing reads that buys it back is a trap to maintain.
   *
   * A group is as tall as the threads under it, so nothing slides out of the
   * way here the way the tabs do: what moves is a line drawn in the gap the
   * group would land in, which is Notion's own answer to the same problem and
   * survives a list whose boxes are all different heights. Positions are all
   * read once, when the lift starts, so nothing reflows mid-drag.
   */
  const onHeaderPointerDown = (e: React.PointerEvent<HTMLDivElement>, key: string) => {
    if (e.button !== 0 || groups.length < 2) return;
    const el = e.currentTarget;
    const pointerId = e.pointerId;
    const startY = e.clientY;
    let edges: number[] = [];
    let latest: number | null = null;
    dragged.current = false;

    const onMove = (ev: PointerEvent) => {
      if (!dragged.current) {
        if (Math.abs(ev.clientY - startY) < 4) return;
        dragged.current = true;
        // The gaps a group can land in: the top of the first box, then the
        // bottom of each. One more edge than there are groups.
        const rects = groups.map((g) => boxes.current.get(g.key)!.getBoundingClientRect());
        edges = [rects[0].top, ...rects.map((r) => r.bottom)];
      }
      // The nearest gap to the pointer, which is how a drop reads at the
      // boundary between two groups rather than only over a box's middle.
      let at = 0;
      for (let i = 1; i < edges.length; i++) {
        if (Math.abs(ev.clientY - edges[i]) < Math.abs(ev.clientY - edges[at])) at = i;
      }
      latest = at;
      setDrag({ key, at });
    };
    const end = () => {
      el.removeEventListener("pointermove", onMove);
      el.removeEventListener("pointerup", end);
      el.removeEventListener("pointercancel", end);
      if (el.hasPointerCapture(pointerId)) el.releasePointerCapture(pointerId);
      if (latest != null) {
        const from = groups.findIndex((g) => g.key === key);
        const keys = groups.map((g) => g.key);
        keys.splice(from, 1);
        // The gap indices are into the list *with* the dragged group still in
        // it, so a drop below its own position has shifted up by one now that
        // it is out.
        keys.splice(latest > from ? latest - 1 : latest, 0, key);
        setOrder(keys);
        localStorage.setItem(ORDER_KEY, JSON.stringify(keys));
      }
      setDrag(null);
    };
    el.setPointerCapture(pointerId);
    el.addEventListener("pointermove", onMove);
    el.addEventListener("pointerup", end);
    el.addEventListener("pointercancel", end);
  };

  const setGroupOpen = (key: string, open: boolean) =>
    setFolded((prev) => {
      if (open === !prev.has(key)) return prev;
      const next = new Set(prev);
      if (open) next.delete(key);
      else next.add(key);
      return next;
    });

  const setGroupOpenAndReset = (key: string, open: boolean) => {
    setGroupOpen(key, open);
    // Folding a group away is also the way back to a short list: it comes back
    // at one page rather than at whatever it had been expanded to.
    if (!open) setShown(({ [key]: _dropped, ...rest }) => rest);
  };

  return (
    <aside
      /* width/min/max move together so flexbox cannot clamp the box to its
         min-content size mid-fold. */
      style={{ width, minWidth: width, maxWidth: width }}
      className={cn(
        "flex shrink-0 grow-0 flex-col overflow-hidden",
        // The divider is the panel's right edge, so it goes when the panel
        // does — a 1px rule left standing on a zero-width box reads as a
        // second border beside the card's own.
        !collapsed && "border-r border-border-subtle",
        animate && "transition-[width,min-width,max-width] duration-200 ease-out",
      )}
    >
      <div className="flex h-full flex-col" style={{ width: restWidth, minWidth: restWidth }}>
        <div className="flex items-center gap-1 p-2">
          <Button variant="ghost" size="xs" className="min-w-0 flex-1 justify-start" onClick={() => onNew()}>
            <Plus size={13} /> New thread
          </Button>
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label="Hide conversations"
            title="Hide conversations (⌘⌥B)"
            className="shrink-0 text-muted-foreground"
            onClick={onToggle}
          >
            <SidebarSimple size={14} />
          </Button>
        </div>
        {/* `overflow-y: scroll`, not `auto`: the gutter has to be reserved whether
            or not the list overflows, because a bar that appears only once it does
            takes its width out of every row and jogs the column sideways as
            threads come and go. `scrollbar-gutter: stable` is the modern spelling
            and is a no-op in this WebKit — measured: it reports support and
            computes to `stable`, and reserves nothing. Always-on overflow does. */}
        <div className="flex flex-1 flex-col overflow-y-scroll px-2 pb-2">
          {groups.map((g, gi) => {
            const open = !folded.has(g.key);
            const busy = g.threads.some((t) => runningIds.has(t.id));
            // The open thread is always drawn, however far down the group it
            // sits: a list that hides the conversation you are reading would
            // leave the page with no row highlighted at all.
            const activeAt = g.threads.findIndex((t) => t.id === activeId);
            const limit = Math.max(shown[g.key] ?? PAGE, activeAt + 1);
            const rest = g.threads.length - limit;
            return (
              <div
                key={g.key}
                ref={(el) => {
                  if (el) boxes.current.set(g.key, el);
                  else boxes.current.delete(g.key);
                }}
                className={cn(
                  "relative mb-1.5",
                  // The group being carried, lightened so the column reads as
                  // one box lifted out of it rather than two in two places.
                  drag?.key === g.key && "opacity-40",
                )}
              >
                {/* Where the drop would land. Drawn on the group above or
                    below the gap rather than as a row of its own, so nothing
                    in the list changes height mid-drag and the edges the maths
                    was captured from stay where they were measured. */}
                {drag?.at === gi && <DropLine className="-top-1" />}
                {/* The gap past the last group is the only one with nothing
                    below it to carry the line. */}
                {drag?.at === groups.length && gi === groups.length - 1 && (
                  <DropLine className="-bottom-1" />
                )}
                {/* The label and its caret are one control on the left — the
                    caret says folded or not, which belongs beside the name and
                    not beside the `+`, where it read as a second button doing
                    something to the group. The right-hand slot holds the count
                    until hover swaps it for the new-thread `+`. */}
                <div
                  onPointerDown={(e) => onHeaderPointerDown(e, g.key)}
                  className="group/head flex select-none items-center gap-1 pl-2.5 pr-1 py-1">
                  <button
                    type="button"
                    title={g.title}
                    aria-expanded={open}
                    onClick={() => {
                      // The click that ends a drag is not a click on the
                      // header; it is the tail of the move.
                      if (dragged.current) return;
                      setGroupOpenAndReset(g.key, !open);
                    }}
                    className="flex min-w-0 flex-1 items-center gap-0.5 text-left text-[11px] font-medium tracking-wide text-muted-foreground transition-colors hover:text-foreground"
                  >
                    <span className="truncate">{g.label}</span>
                    <CaretRight
                      size={11}
                      className={cn(
                        "shrink-0 transition-[transform,opacity]",
                        // Folded, the caret is the state and stays; open, it is
                        // only an affordance and waits for the pointer.
                        open ? "rotate-90 opacity-0 group-hover/head:opacity-100" : "opacity-60",
                      )}
                    />
                  </button>
                  <div className="relative flex size-4 shrink-0 items-center justify-center">
                    <button
                      type="button"
                      aria-label={`New thread in ${g.label}`}
                      title={`New thread in ${g.label}`}
                      /* A thread started from a folded group would be started out
                         of sight, so the group opens with it. */
                      onClick={() => {
                        if (dragged.current) return;
                        setGroupOpen(g.key, true);
                        onNew(g.subjectId);
                      }}
                      className="absolute inset-0 hidden items-center justify-center rounded text-muted-foreground transition-colors hover:text-foreground group-hover/head:flex"
                    >
                      <Plus size={11} weight="bold" />
                    </button>
                    {/* Folded, the group is the only place left to say that
                        something inside it is running — the rows carrying their
                        own spinners are folded away with it. */}
                    {busy && !open ? (
                      <CircleNotch size={11} className="animate-spin text-muted-foreground group-hover/head:hidden" />
                    ) : (
                      <span className="text-[10px] tabular-nums text-muted-foreground opacity-60 group-hover/head:hidden">
                        {g.threads.length}
                      </span>
                    )}
                  </div>
                </div>
                {open && (
                  <div className="flex flex-col gap-0.5">
                    {g.threads.slice(0, limit).map((t) => {
                      const running = runningIds.has(t.id);
                      const active = t.id === activeId;
                      return (
                        <div
                          key={t.id}
                          className={cn(
                            "group/thread flex rounded-lg text-xs transition-colors",
                            active
                              ? "bg-accent text-foreground"
                              : "text-muted-foreground hover:bg-accent hover:text-foreground",
                          )}
                        >
                          {/* The padding and the provider mark live *inside* the
                              button, and the row is left to stretch it — so the
                              whole row opens the thread, the way the sidebar's
                              own rows do. With the padding on this div instead,
                              the hit target was the title's 16px line box inside
                              a 28px row while the hover fill covered the dead 6px
                              strips above and below it: a click that looked
                              aimed, on a row that had lit up under the pointer,
                              landed on the div and did nothing. It read as the
                              app dropping clicks, and it showed up most when
                              switching threads fast, which is when you stop
                              settling on the text. */}
                          <button
                            type="button"
                            onClick={() => onOpen(t.id)}
                            className="flex min-w-0 flex-1 items-center gap-2 rounded-lg py-1.5 pl-2.5 text-left"
                          >
                            {/* Monochrome and `currentColor`, so it dims with the row
                                rather than sitting on it as a second colour. */}
                            <ProviderMark provider={t.provider} className="size-3.5 shrink-0 opacity-70" />
                            <span className="min-w-0 flex-1 truncate">{t.title || "Untitled"}</span>
                            {/* A thread opened in the lecture player's dock is
                                grouped under its subject like any other — it is
                                the same conversation and the same list — so the
                                one thing the row has to add is which kind of
                                scope it was given. The app's lecture icon,
                                because that is what it means everywhere else. */}
                            {t.lecture_id && (
                              <VideoCamera
                                size={11}
                                className="shrink-0 opacity-60"
                                aria-label="Lecture thread"
                              />
                            )}
                          </button>
                          {/* The one strip of the row that is not the thread: its
                              own controls, and a spinner in their place while a
                              turn runs. */}
                          <div className="flex shrink-0 items-center pl-1 pr-1.5">
                            {running ? (
                              <CircleNotch size={12} className="animate-spin text-muted-foreground" />
                            ) : confirming === t.id ? (
                              <ConfirmDelete
                                label={"Yes"}
                                onConfirm={() => {
                                  setConfirming(null);
                                  onDelete(t.id);
                                }}
                                onCancel={() => setConfirming(null)}
                              />
                            ) : (
                              <button
                                type="button"
                                aria-label="Delete thread"
                                onClick={() => setConfirming(t.id)}
                                className="rounded p-0.5 opacity-0 transition-opacity hover:text-foreground group-hover/thread:opacity-100"
                              >
                                <Trash size={12} />
                              </button>
                            )}
                          </div>
                        </div>
                      );
                    })}
                    {/* Quiet, and shaped like a row rather than a button: it is
                        the tail of the list, not a control beside it. The count
                        says how big a step it is, and the group header above
                        already carries the total. */}
                    {rest > 0 && (
                      <button
                        type="button"
                        onClick={() => setShown((prev) => ({ ...prev, [g.key]: limit + PAGE }))}
                        className="rounded-lg py-1.5 pl-2.5 text-left text-[11px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                      >
                        Show {Math.min(PAGE, rest)} more
                      </button>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </aside>
  );
});

/** Where a dragged group would land: the accent as a line, not the fill —
 *  `brand`, since this is the in-flight version of something, and `primary`
 *  is the colour of a button. Absolute, so it costs the list no height. */
function DropLine({ className }: { className: string }) {
  return (
    <div
      aria-hidden
      className={cn("pointer-events-none absolute inset-x-1 h-0.5 rounded-full bg-brand", className)}
    />
  );
}

/**
 * The armed state of a row's delete: the confirm itself, and a way out.
 *
 * It takes focus on mount so that clicking anywhere else backs out — WebKit
 * does not focus a button when you click it, so the blur this leans on never
 * fired, and the row simply stuck on the confirm with no way back. Escape
 * cancels too, and the ✕ is the visible version of the same, for when the way
 * out should not have to be guessed at.
 *
 * **Every button here commits on `mousedown`, and that is what makes the
 * delete work at all.** The same WebKit rule cuts the other way once this
 * button holds focus: pressing it is a mousedown on something WebKit will not
 * focus, so it clears the focus it *had* — this button's — and the blur fires
 * before the click does. The blur cancels, React unmounts the confirm
 * synchronously, and the click then lands on a node no longer in the tree. So
 * *Yes* was never delivered and deleting a thread silently did nothing, on
 * every click, for the same reason the ✕ beside it was already written this
 * way.
 */
function ConfirmDelete({
  label,
  onConfirm,
  onCancel,
}: {
  label: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const ref = useRef<HTMLButtonElement>(null);
  // Mount only: a re-render mid-turn must not yank focus back out of whatever
  // the user has since started typing in.
  useEffect(() => ref.current?.focus(), []);
  return (
    <div className="flex shrink-0 items-center gap-0.5">
      <button
        ref={ref}
        type="button"
        onMouseDown={(e) => {
          e.preventDefault();
          onConfirm();
        }}
        onBlur={onCancel}
        /* And the keyboard has to be handled here rather than left to the
           click a button would normally synthesise, since there is no click
           handler left to synthesise it into. It is focused on mount, so
           Enter is the fastest way to confirm and Escape the way out. */
        onKeyDown={(e) => {
          if (e.key === "Escape") onCancel();
          else if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onConfirm();
          }
        }}
        className="rounded px-1 text-[10.5px] text-destructive hover:bg-destructive/10"
      >
        {label}
      </button>
      <button
        type="button"
        aria-label="Keep thread"
        title="Keep"
        /* mousedown, not click: the confirm button's blur lands first and
           would unmount this one before a click could land. */
        onMouseDown={(e) => {
          e.preventDefault();
          onCancel();
        }}
        className="rounded p-0.5 text-muted-foreground hover:text-foreground"
      >
        <X size={11} weight="bold" />
      </button>
    </div>
  );
}
