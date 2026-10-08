import { memo, useEffect, useMemo, useRef, useState } from "react";
import { CaretRight, Chat, CircleNotch, Plus, Trash, VideoCamera, X } from "@phosphor-icons/react";
import { ProviderMark } from "@/components/harness/ProviderMark";
import { useTabActive } from "@/components/tabs/TabContext";
import { ResizeHandle } from "@/components/ui/ResizeHandle";
import {
  SIDE_NAV_FOLDS,
  SIDE_NAV_ROW,
  SIDE_NAV_ROW_ACTIVE,
  SIDE_NAV_ROW_IDLE,
  SideNav,
} from "@/components/ui/SideNav";
import { chatHref, type HarnessThread } from "@/lib/harness";
import type { Subject } from "@/lib/db";
import { groupBySubject } from "@/lib/subjectGroups";
import { useCollapsedGroups } from "@/hooks/useCollapsedGroups";
import { useWindowEvent } from "@/hooks/useEvents";
import type { useResizablePanel } from "@/hooks/useResizablePanel";
import { useStoredSet } from "@/hooks/useStoredState";
import { cn } from "@/lib/utils";
import { usePointerDrag } from "@/hooks/usePointerDrag";

const COLLAPSED_KEY = "oculus-chat-groups-collapsed";
const ORDER_KEY = "oculus-chat-groups-order";

/** The column's width bounds; folded, it is gone entirely. `ChatPage` owns the
 *  panel so its header can hold the fold toggle. */
export const THREAD_LIST_PANEL = {
  defaultWidth: 224,
  minWidth: 168,
  maxWidth: 420,
  collapsedWidth: 0,
  storageKey: "oculus-chat-list-width",
};

/** Threads a group shows at first, and how many each "Show more" adds. */
const PAGE = 5;

/** Recency, overruled by the dragged `order`. Unplaced groups (index -1) sort
 *  above placed ones — a new subject has a live conversation — and stay in
 *  recency order among themselves because `sort` is stable. */
function arrange<T extends { key: string }>(groups: T[], order: string[]): T[] {
  if (!order.length) return groups;
  return groups.slice().sort((a, b) => order.indexOf(a.key) - order.indexOf(b.key));
}

/**
 * Chat's nav column on `SideNav`, the subject page's column: New thread on top
 * (lit while no thread is open), then threads grouped by subject scope. It
 * takes its width and fold from `panel` (`ChatPage` owns it); ⌘⌥B or the
 * page header's toggle folds it away completely. Titles come from
 * `Harness::name_thread`.
 */
export const ThreadList = memo(function ThreadList({
  panel,
  threads,
  subjects,
  activeId,
  runningIds,
  onOpen,
  onNew,
  onDelete,
}: {
  panel: ReturnType<typeof useResizablePanel>;
  threads: HarnessThread[];
  subjects: Subject[];
  activeId: number | null;
  /** Ids, not the store's live map, which changes on every streamed token. */
  runningIds: Set<number>;
  onOpen: (id: number) => void;
  /** No argument: the composer's scope; otherwise that group's subject (null = General). */
  onNew: (subjectId?: number | null) => void;
  onDelete: (id: number) => void;
}) {
  const { collapsed } = panel;
  const [confirming, setConfirming] = useState<number | null>(null);
  // Folded *groups* — not the column's own `collapsed`.
  const { folded, setOpen: setGroupOpen } = useCollapsedGroups(COLLAPSED_KEY);
  // Per-group page expansion; not persisted.
  const [shown, setShown] = useState<Record<string, number>>({});
  // A Set keeps insertion order, so the stored set is the dragged order.
  const [orderSet, setOrderSet] = useStoredSet(ORDER_KEY);
  const order = useMemo(() => [...orderSet], [orderSet]);
  // The lifted group and the gap it would drop into.
  const [drag, setDrag] = useState<{ key: string; at: number } | null>(null);
  const boxes = useRef(new Map<string, HTMLDivElement>());
  // `didDrag` swallows the `click` that ends a drag, which would otherwise fold the group.
  const gesture = usePointerDrag("y");

  const groups = useMemo(
    () => arrange(groupBySubject(threads, subjects, { key: "general", label: "General" }), order),
    [threads, subjects, order],
  );

  // ⌘⌥B folds the column, in the tab in front only: every tab stays mounted,
  // and the subject column answers the same chord. `e.code`, not `e.key`: on
  // macOS ⌥B arrives as `∫`.
  const active = useTabActive();
  useWindowEvent("keydown", (e) => {
    const k = e as KeyboardEvent;
    if (!active || !k.altKey || !(k.metaKey || k.ctrlKey) || k.code !== "KeyB") return;
    k.preventDefault();
    panel.toggle();
  });

  // Header reorder on pointer events (see docs/ui.md). Groups
  // differ in height, so a drop line marks the gap instead of sliding boxes;
  // edges are measured once at lift.
  const onHeaderPointerDown = (e: React.PointerEvent<HTMLDivElement>, key: string) => {
    if (collapsed) return;
    let edges: number[] = [];
    let latest: number | null = null;
    gesture.start(e, {
      lift: () => {
        if (groups.length < 2) return false;
        // Gaps: the first box's top, then each box's bottom.
        const rects = groups.map((g) => boxes.current.get(g.key)!.getBoundingClientRect());
        edges = [rects[0].top, ...rects.map((r) => r.bottom)];
        return true;
      },
      move: (ev) => {
        let at = 0;
        for (let i = 1; i < edges.length; i++) {
          if (Math.abs(ev.clientY - edges[i]) < Math.abs(ev.clientY - edges[at])) at = i;
        }
        latest = at;
        setDrag({ key, at });
      },
      end: () => {
        if (latest != null) {
          const from = groups.findIndex((g) => g.key === key);
          const keys = groups.map((g) => g.key);
          keys.splice(from, 1);
          // Gap indices count the dragged group; below it, shift up by one.
          keys.splice(latest > from ? latest - 1 : latest, 0, key);
          setOrderSet(new Set(keys));
        }
        setDrag(null);
      },
    });
  };

  const setGroupOpenAndReset = (key: string, open: boolean) => {
    setGroupOpen(key, open);
    // Folding resets the group to one page.
    if (!open) setShown(({ [key]: _dropped, ...rest }) => rest);
  };

  return (
    <>
      <SideNav
        aria-label="Conversations"
        width={panel.width}
        collapsed={collapsed}
        animate={!panel.dragging}
        header={
          <>
            <div className="px-4 pt-4 pb-3">
              <div className="-mx-2 flex items-center gap-2.5 px-2 py-1">
                <Chat size={16} className="shrink-0 text-foreground" />
                <h1
                  className={cn(
                    "min-w-0 flex-1 truncate font-display text-[16px] font-semibold leading-none tracking-tight text-foreground",
                    SIDE_NAV_FOLDS,
                  )}
                >
                  Chat
                </h1>
              </div>
            </div>
            <div className="px-2 pb-2">
              <button
                type="button"
                aria-current={activeId == null ? "page" : undefined}
                onClick={() => onNew()}
                className={cn(SIDE_NAV_ROW, activeId == null ? SIDE_NAV_ROW_ACTIVE : SIDE_NAV_ROW_IDLE)}
              >
                <Plus size={16} className="shrink-0" />
                <span className={cn("min-w-0 flex-1 truncate text-left", SIDE_NAV_FOLDS)}>New thread</span>
              </button>
            </div>
          </>
        }
      >
        {groups.map((g, gi) => {
          const open = !folded.has(g.key);
          const busy = g.items.some((t) => runningIds.has(t.id));
          // The open thread is always drawn, however far down it sits.
          const activeAt = g.items.findIndex((t) => t.id === activeId);
          const limit = Math.max(shown[g.key] ?? PAGE, activeAt + 1);
          const rest = g.items.length - limit;
          return (
            <div
              key={g.key}
              ref={(el) => {
                if (el) boxes.current.set(g.key, el);
                else boxes.current.delete(g.key);
              }}
              className={cn("relative mb-3", drag?.key === g.key && "opacity-40")}
            >
              {/* Absolute on a neighbouring group, so no height changes mid-drag. The
                  first group's line sits inside its box: above it is the scroller's
                  padding, which clips. */}
              {drag?.at === gi && <DropLine className={gi === 0 ? "top-0" : "-top-1.5"} />}
              {drag?.at === groups.length && gi === groups.length - 1 && (
                <DropLine className="-bottom-1.5" />
              )}
              {/* Label + caret fold; the right slot shows the count until hover swaps in `+`.
                  Folded, the column keeps the header's height so icons don't move. */}
              <div
                onPointerDown={(e) => onHeaderPointerDown(e, g.key)}
                className={cn(
                  "group/head mb-0.5 flex select-none items-center gap-1 py-1 pr-1 pl-2",
                  SIDE_NAV_FOLDS,
                  "group-data-[collapsed=true]/nav:pointer-events-none",
                )}
              >
                <button
                  type="button"
                  title={g.title}
                  aria-expanded={open}
                  tabIndex={collapsed ? -1 : undefined}
                  onClick={() => {
                    if (gesture.didDrag()) return;
                    setGroupOpenAndReset(g.key, !open);
                  }}
                  className="flex min-w-0 flex-1 items-center gap-0.5 text-left text-[11px] font-medium tracking-wide text-muted-foreground transition-colors hover:text-foreground"
                >
                  <span className="truncate">{g.label}</span>
                  <CaretRight
                    size={11}
                    className={cn(
                      "shrink-0 transition-[transform,opacity] will-change-[opacity,transform]",
                      open ? "rotate-90 opacity-0 group-hover/head:opacity-100" : "opacity-60",
                    )}
                  />
                </button>
                <div className="relative flex size-4 shrink-0 items-center justify-center">
                  <button
                    type="button"
                    aria-label={`New thread in ${g.label}`}
                    title={`New thread in ${g.label}`}
                    tabIndex={collapsed ? -1 : undefined}
                    onClick={() => {
                      if (gesture.didDrag()) return;
                      setGroupOpen(g.key, true);
                      onNew(g.subjectId);
                    }}
                    className="absolute inset-0 hidden items-center justify-center rounded text-muted-foreground transition-colors hover:text-foreground group-hover/head:flex"
                  >
                    <Plus size={11} weight="bold" />
                  </button>
                  {/* Folded, the rows' spinners are hidden, so the header shows one. */}
                  {busy && !open ? (
                    <CircleNotch size={11} className="animate-spin text-muted-foreground group-hover/head:hidden" />
                  ) : (
                    <span className="text-[10px] tabular-nums text-muted-foreground opacity-60 group-hover/head:hidden">
                      {g.items.length}
                    </span>
                  )}
                </div>
              </div>
              {open && (
                <div className="flex flex-col gap-0.5">
                  {g.items.slice(0, limit).map((t) => (
                    <ThreadRow
                      key={t.id}
                      thread={t}
                      active={t.id === activeId}
                      running={runningIds.has(t.id)}
                      confirming={confirming === t.id}
                      onOpen={onOpen}
                      onArm={setConfirming}
                      onDelete={onDelete}
                    />
                  ))}
                  {rest > 0 && (
                    <button
                      type="button"
                      tabIndex={collapsed ? -1 : undefined}
                      onClick={() => setShown((prev) => ({ ...prev, [g.key]: limit + PAGE }))}
                      className={cn(
                        SIDE_NAV_ROW,
                        SIDE_NAV_ROW_IDLE,
                        SIDE_NAV_FOLDS,
                        "text-[11px] group-data-[collapsed=true]/nav:pointer-events-none",
                      )}
                    >
                      {/* One child: the row's flex gap would split the words. */}
                      <span>
                        Show <span className="tabular-nums">{Math.min(PAGE, rest)}</span> more
                      </span>
                    </button>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </SideNav>
      {/* On the seam: negative margins cost no layout width. Folded away the
          seam is the page's own left edge, where a side panel's handle lives. */}
      {!collapsed && (
        <ResizeHandle
          onMouseDown={panel.onMouseDown}
          dragging={panel.dragging}
          label="Resize conversations"
          className="-mx-0.5"
        />
      )}
    </>
  );
});

/** A thread as a nav row: its provider's mark is the icon a fold leaves, and a
 *  running thread marks that icon's corner while folded. */
function ThreadRow({
  thread: t,
  active,
  running,
  confirming,
  onOpen,
  onArm,
  onDelete,
}: {
  thread: HarnessThread;
  active: boolean;
  running: boolean;
  confirming: boolean;
  onOpen: (id: number) => void;
  /** Arms this row's delete, or disarms with null. */
  onArm: (id: number | null) => void;
  onDelete: (id: number) => void;
}) {
  const title = t.title?.trim() || "Untitled";
  return (
    <div
      className={cn(
        SIDE_NAV_ROW,
        "group/thread relative gap-0 overflow-hidden p-0",
        active ? SIDE_NAV_ROW_ACTIVE : SIDE_NAV_ROW_IDLE,
      )}
    >
      {/* Padding lives inside the button so the whole lit row is the hit target. */}
      <button
        type="button"
        // ⌘-click opens the thread in a new tab (`app/src/lib/newTabClicks.ts`).
        data-tab-href={chatHref(t.id, t.title)}
        onClick={() => onOpen(t.id)}
        className="flex min-w-0 flex-1 items-center gap-2.5 py-1.5 pl-2 text-left"
      >
        <ProviderMark provider={t.provider} className="size-4 shrink-0" />
        <span className={cn("min-w-0 flex-1 truncate", SIDE_NAV_FOLDS)}>{title}</span>
        {t.lecture_id && (
          <VideoCamera
            size={11}
            className={cn("shrink-0 opacity-60", SIDE_NAV_FOLDS)}
            aria-label="Lecture thread"
          />
        )}
      </button>
      <div className={cn("flex shrink-0 items-center pr-1.5 pl-1", SIDE_NAV_FOLDS)}>
        {running ? (
          <CircleNotch size={12} className="animate-spin text-muted-foreground" aria-label="Running" />
        ) : confirming ? (
          <ConfirmDelete
            label="Yes"
            onConfirm={() => {
              onArm(null);
              onDelete(t.id);
            }}
            onCancel={() => onArm(null)}
          />
        ) : (
          <button
            type="button"
            aria-label="Delete thread"
            onClick={() => onArm(t.id)}
            className="rounded p-0.5 opacity-0 transition-opacity will-change-[opacity] hover:text-foreground focus-visible:opacity-100 group-hover/thread:opacity-100 group-data-[collapsed=true]/nav:pointer-events-none"
          >
            <Trash size={12} />
          </button>
        )}
      </div>
      {running && (
        <span
          aria-hidden
          className="absolute top-1 left-5 size-1.5 rounded-full bg-brand opacity-0 transition-opacity will-change-[opacity] duration-150 group-data-[collapsed=true]/nav:opacity-100"
        />
      )}
    </div>
  );
}

function DropLine({ className }: { className: string }) {
  return (
    <div
      aria-hidden
      className={cn("pointer-events-none absolute inset-x-1 h-0.5 rounded-full bg-brand", className)}
    />
  );
}

/**
 * A row's armed delete. Focused on mount so clicking elsewhere blurs to cancel.
 * Both buttons commit on `mousedown`: WebKit doesn't focus a clicked button, so
 * pressing one blurs the focused confirm first, which unmounts this before a
 * `click` could land.
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
  // Mount only, so a re-render never steals focus back.
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
        /* No onClick for the keyboard to synthesise into, so keys are handled here. */
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
