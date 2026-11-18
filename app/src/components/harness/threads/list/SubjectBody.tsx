import { useMemo, useRef, useState, type ReactNode } from "react";
import { Books, CaretRight, Plus } from "@phosphor-icons/react";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { SIDE_NAV_FOLDS, SIDE_NAV_ROW, SIDE_NAV_ROW_IDLE } from "@/components/ui/layout/SideNav";
import type { HarnessThread } from "@/lib/harness";
import type { Subject } from "@/lib/db";
import { GENERAL_SCOPE, type ChatScope } from "@/lib/harness/scope";
import { groupBySubject } from "@/lib/ui/subjectGroups";
import { usePointerDrag } from "@/hooks/gestures/usePointerDrag";
import { useStoredSet } from "@/hooks/ui/useStoredState";
import { cn } from "@/lib/utils";
import { arrange, ORDER_KEY, PAGE } from "../order";
import { SECTION_HEAD } from "../rowType";
import { DropLine } from "../parts";
import { WorkingRing } from "../StatusPill";
import { byNewest } from "./RecentsBody";

/**
 * The Subject view: the threads the store has read, grouped by subject
 * scope, groups in dragged order (recency for unplaced ones), each folding
 * and paging by `PAGE`. A group's header is its subject's glyph and code,
 * which the rows then leave out; its right slot is the count, `+` on hover.
 * Once a group shows all it holds, its Show more reads older threads
 * (`onMore`) while the database has them.
 */
export function SubjectBody({
  threads,
  subjects,
  activeId,
  runningIds,
  folded,
  onGroupOpen,
  collapsed,
  onNew,
  more,
  onMore,
  row,
}: {
  threads: HarnessThread[];
  subjects: Subject[];
  activeId: number | null;
  runningIds: Set<number>;
  /** Folded group keys, kept with the column's other folds. */
  folded: Set<string>;
  onGroupOpen: (key: string, open: boolean) => void;
  collapsed: boolean;
  onNew: (scope?: ChatScope) => void;
  /** Older threads exist beyond those read. */
  more: boolean;
  onMore: () => void;
  row: (thread: HarnessThread) => ReactNode;
}) {
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
  const codes = useMemo(() => new Map(subjects.map((s) => [s.id, s.code])), [subjects]);

  const groups = useMemo(
    () => arrange(groupBySubject(byNewest(threads), subjects, { key: "general", label: "General" }), order),
    [threads, subjects, order],
  );

  // Header reorder on pointer events (see docs/ui.md). Groups differ in
  // height, so a drop line marks the gap instead of sliding boxes; edges are
  // measured once at lift.
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

  const setOpen = (key: string, open: boolean) => {
    onGroupOpen(key, open);
    // Folding resets the group to one page.
    if (!open) setShown(({ [key]: _dropped, ...rest }) => rest);
  };

  return (
    <>
      {groups.map((g, gi) => {
        const open = !folded.has(g.key);
        const busy = g.items.some((t) => runningIds.has(t.id));
        const code = g.subjectId == null ? undefined : codes.get(g.subjectId);
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
            className={cn("relative", drag?.key === g.key && "opacity-40")}
          >
            {/* Absolute on a neighbouring group, so no height changes mid-drag. The
                first group's line sits inside its box: above it is the scroller's
                padding, which clips. */}
            {drag?.at === gi && <DropLine className={gi === 0 ? "top-0" : "-top-1.5"} />}
            {drag?.at === groups.length && gi === groups.length - 1 && <DropLine className="-bottom-1.5" />}
            <div
              onPointerDown={(e) => onHeaderPointerDown(e, g.key)}
              className={cn(
                "group/head flex select-none items-center gap-1.5 pr-2 pl-2.5 group-data-[collapsed=true]/nav:pointer-events-none",
                // The first heading sits under the header, which already spaces it.
                gi === 0 ? "pt-1 pb-1" : SECTION_HEAD,
              )}
            >
              {/* The glyph is what a folding column leaves longest. */}
              {code ? <SubjectIcon code={code} size={14} className="shrink-0" /> : <Books size={14} className="shrink-0 text-muted-foreground" />}
              <button
                type="button"
                title={g.title}
                aria-expanded={open}
                tabIndex={collapsed ? -1 : undefined}
                onClick={() => {
                  if (gesture.didDrag()) return;
                  setOpen(g.key, !open);
                }}
                className={cn(
                  "flex min-w-0 flex-1 items-center gap-1 text-left text-[12px] font-medium text-muted-foreground transition-colors hover:text-foreground",
                  SIDE_NAV_FOLDS,
                )}
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
              <div className={cn("relative flex size-4 shrink-0 items-center justify-center", SIDE_NAV_FOLDS)}>
                <button
                  type="button"
                  data-side-nav-close
                  aria-label={`New thread in ${g.label}`}
                  title={`New thread in ${g.label}`}
                  tabIndex={collapsed ? -1 : undefined}
                  onClick={() => {
                    if (gesture.didDrag()) return;
                    setOpen(g.key, true);
                    onNew(g.subjectId ?? GENERAL_SCOPE);
                  }}
                  className="absolute inset-0 hidden items-center justify-center rounded text-muted-foreground transition-colors hover:text-foreground group-hover/head:flex"
                >
                  <Plus size={11} weight="bold" />
                </button>
                {/* Folded, the rows' rings are hidden, so the header shows one. */}
                {busy && !open ? (
                  <WorkingRing className="size-[11px] text-brand group-hover/head:hidden" />
                ) : (
                  <span className="text-[11px] tabular-nums text-muted-foreground opacity-70 group-hover/head:hidden">
                    {g.items.length || ""}
                  </span>
                )}
              </div>
            </div>
            {open && (
              <div className="flex flex-col gap-0.5">
                {g.items.slice(0, limit).map(row)}
                {(rest > 0 || more) && (
                  <button
                    type="button"
                    tabIndex={collapsed ? -1 : undefined}
                    onClick={() => {
                      setShown((prev) => ({ ...prev, [g.key]: limit + PAGE }));
                      // Every thread read is shown: read older ones, which may
                      // hold more of this subject's.
                      if (rest <= 0) onMore();
                    }}
                    className={cn(
                      SIDE_NAV_ROW,
                      SIDE_NAV_ROW_IDLE,
                      SIDE_NAV_FOLDS,
                      "px-2.5 text-[11.5px] group-data-[collapsed=true]/nav:pointer-events-none",
                    )}
                  >
                    {/* One child: the row's flex gap would split the words. */}
                    {rest > 0 ? (
                      <span>
                        Show <span className="tabular-nums">{Math.min(PAGE, rest)}</span> more
                      </span>
                    ) : (
                      "Show more"
                    )}
                  </button>
                )}
              </div>
            )}
          </div>
        );
      })}
    </>
  );
}
