import { memo, useEffect, useMemo, useRef, useState } from "react";
import { CaretRight, CircleNotch, Plus, SidebarSimple, Trash, VideoCamera, X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { ProviderMark } from "@/components/harness/ProviderMark";
import type { HarnessThread } from "@/lib/harness";
import type { Subject } from "@/lib/db";
import { displayCode, displayName } from "@/lib/format";
import { cn } from "@/lib/utils";
import { usePointerDrag } from "@/hooks/usePointerDrag";

const COLLAPSED_KEY = "oculus-chat-groups-collapsed";
const ORDER_KEY = "oculus-chat-groups-order";

/** Threads a group shows at first, and how many each "Show more" adds. */
const PAGE = 5;

/** Stores the collapsed groups, so a newly scoped subject arrives expanded. */
function loadCollapsed(): Set<string> {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? "[]");
    return new Set(Array.isArray(raw) ? raw.filter((k): k is string => typeof k === "string") : []);
  } catch {
    return new Set();
  }
}

/** The dragged order, by group key; unlisted groups sort by recency. */
function loadOrder(): string[] {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(ORDER_KEY) ?? "[]");
    return Array.isArray(raw) ? raw.filter((k): k is string => typeof k === "string") : [];
  } catch {
    return [];
  }
}

/** Groups by scope in first-appearance (i.e. `updated_at`) order; a deleted
 *  subject's threads fall back into General. */
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

/** Recency, overruled by the dragged `order`. Unplaced groups (index -1) sort
 *  above placed ones — a new subject has a live conversation — and stay in
 *  recency order among themselves because `sort` is stable. */
function arrange<T extends { key: string }>(groups: T[], order: string[]): T[] {
  if (!order.length) return groups;
  return groups.slice().sort((a, b) => order.indexOf(a.key) - order.indexOf(b.key));
}

/**
 * The conversations column, threads grouped by subject scope. Width is owned by
 * `ChatPage`'s `useResizablePanel`; folded means width 0, and the inner box keeps
 * `restWidth` so the fold clips rather than reflows. Titles come from
 * `Harness::name_thread`.
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
  /** The unfolded width the contents are laid out at. */
  restWidth: number;
  collapsed: boolean;
  /** Off while resizing, so the width tracks the handle without lag. */
  animate: boolean;
  onToggle: () => void;
  /** Ids, not the store's live map, which changes on every streamed token. */
  runningIds: Set<number>;
  onOpen: (id: number) => void;
  /** No argument: the composer's scope; otherwise that group's subject (null = General). */
  onNew: (subjectId?: number | null) => void;
  onDelete: (id: number) => void;
}) {
  const [confirming, setConfirming] = useState<number | null>(null);
  // Folded *groups* — not the panel's own `collapsed`.
  const [folded, setFolded] = useState<Set<string>>(loadCollapsed);
  // Per-group page expansion; not persisted.
  const [shown, setShown] = useState<Record<string, number>>({});
  const [order, setOrder] = useState<string[]>(loadOrder);
  // The lifted group and the gap it would drop into.
  const [drag, setDrag] = useState<{ key: string; at: number } | null>(null);
  const boxes = useRef(new Map<string, HTMLDivElement>());
  // `didDrag` swallows the `click` that ends a drag, which would otherwise fold the group.
  const gesture = usePointerDrag("y");

  useEffect(() => {
    localStorage.setItem(COLLAPSED_KEY, JSON.stringify([...folded]));
  }, [folded]);

  const groups = useMemo(
    () => arrange(group(threads, subjects), order),
    [threads, subjects, order],
  );

  // Header reorder on pointer events (see CLAUDE.md: no HTML5 drag). Groups
  // differ in height, so a drop line marks the gap instead of sliding boxes;
  // edges are measured once at lift.
  const onHeaderPointerDown = (e: React.PointerEvent<HTMLDivElement>, key: string) => {
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
          setOrder(keys);
          localStorage.setItem(ORDER_KEY, JSON.stringify(keys));
        }
        setDrag(null);
      },
    });
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
    // Folding resets the group to one page.
    if (!open) setShown(({ [key]: _dropped, ...rest }) => rest);
  };

  return (
    <aside
      /* width/min/max move together so flexbox cannot clamp the box to its
         min-content size mid-fold. */
      style={{ width, minWidth: width, maxWidth: width }}
      className={cn(
        "flex shrink-0 grow-0 flex-col overflow-hidden",
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
        {/* `overflow-y: scroll`, not `auto`, reserves the gutter so rows don't jog
            when the bar appears; `scrollbar-gutter: stable` is a no-op in WebKit. */}
        <div className="flex flex-1 flex-col overflow-y-scroll px-2 pb-2">
          {groups.map((g, gi) => {
            const open = !folded.has(g.key);
            const busy = g.threads.some((t) => runningIds.has(t.id));
            // The open thread is always drawn, however far down it sits.
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
                  drag?.key === g.key && "opacity-40",
                )}
              >
                {/* Absolute on a neighbouring group, so no height changes mid-drag. */}
                {drag?.at === gi && <DropLine className="-top-1" />}
                {drag?.at === groups.length && gi === groups.length - 1 && (
                  <DropLine className="-bottom-1" />
                )}
                {/* Label + caret fold; the right slot shows the count until hover swaps in `+`. */}
                <div
                  onPointerDown={(e) => onHeaderPointerDown(e, g.key)}
                  className="group/head flex select-none items-center gap-1 pl-2.5 pr-1 py-1">
                  <button
                    type="button"
                    title={g.title}
                    aria-expanded={open}
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
                        "shrink-0 transition-[transform,opacity]",
                        open ? "rotate-90 opacity-0 group-hover/head:opacity-100" : "opacity-60",
                      )}
                    />
                  </button>
                  <div className="relative flex size-4 shrink-0 items-center justify-center">
                    <button
                      type="button"
                      aria-label={`New thread in ${g.label}`}
                      title={`New thread in ${g.label}`}
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
                          {/* Padding lives inside the button so the whole lit row is the hit target. */}
                          <button
                            type="button"
                            onClick={() => onOpen(t.id)}
                            className="flex min-w-0 flex-1 items-center gap-2 rounded-lg py-1.5 pl-2.5 text-left"
                          >
                            <ProviderMark provider={t.provider} className="size-3.5 shrink-0 opacity-70" />
                            <span className="min-w-0 flex-1 truncate">{t.title || "Untitled"}</span>
                            {t.lecture_id && (
                              <VideoCamera
                                size={11}
                                className="shrink-0 opacity-60"
                                aria-label="Lecture thread"
                              />
                            )}
                          </button>
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
