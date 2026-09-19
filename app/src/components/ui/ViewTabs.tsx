import { useRef, useState } from "react";

import { cn } from "@/lib/utils";

export interface ViewTab<T extends string> {
  value: T;
  label: string;
  /** Optional trailing element — a count chip, a dot. */
  badge?: React.ReactNode;
}

/** How far the pointer travels before a press becomes a drag. The same 4px
 *  `TopTabBar` and `useCardDrag` use — small enough that the lift reads as
 *  having happened when the hand moved, large enough that a click never
 *  becomes one. */
const THRESHOLD = 4;

/** One tab's captured geometry. Nothing is re-measured during a drag, so these
 *  numbers are the whole world the gesture reasons about. */
interface TabBox {
  left: number;
  mid: number;
  width: number;
}

/**
 * A non-routed underline tab strip: the same shape as `SubjectLayout`'s nav,
 * for switching what a page renders rather than where it navigates. Inactive
 * tabs are greyed out and only the active one carries the indigo rule, so a
 * page's views read as siblings instead of hiding inside a dropdown.
 *
 * The strip is meant to sit on a container's bottom border — `-mb-px` pulls
 * the active underline down onto that line rather than floating above it.
 *
 * **Reordering is opt-in.** Pass `onReorder` and the tabs become draggable;
 * leave it off and they are plain buttons, which is what a settings page's
 * three fixed views want. Only the dock's strip asks for it — four tabs in a
 * 220px header where which one sits under your thumb is worth choosing.
 */
export function ViewTabs<T extends string>({
  tabs,
  value,
  onChange,
  onReorder,
  className,
}: {
  tabs: ReadonlyArray<ViewTab<T>>;
  value: T;
  onChange: (value: T) => void;
  /** Hand back the whole strip in its new order. A list rather than a pair of
   *  indices: the caller may be showing a subset of what it stores, and only
   *  it knows how to fold this back into the rest. */
  onReorder?: (next: T[]) => void;
  className?: string;
}) {
  /**
   * The live gesture: the grabbed tab rides the pointer (`dx`) while the ones
   * between its old slot and `target` slide out of / into its way.
   *
   * **This is `TopTabBar`'s drag** (`app/src/components/tabs/TopTabBar.tsx`),
   * which is the gesture the window's own tab strip uses and the one these
   * tabs are meant to feel like. It replaces an HTML5 drag-and-drop reorder
   * that *worked* and still read as the wrong gesture: WebKit drew its own
   * translucent copy of the label under the pointer, the cursor carried a
   * copy badge rather than a move one, and the strip itself never moved — the
   * tabs sat still until the drop, so which gap the tab would land in was
   * only ever a recoloured underline. Here the strip rearranges live under the
   * hand, which is what the window's tab strip does and what these were asked
   * to match. It is also the gesture every other reorder in this app uses;
   * `app/src/hooks/useCardDrag.ts` gives the WebKit reasons at length.
   *
   * `width` is the grabbed tab's own, and it is the whole displacement a
   * neighbour takes: what slides is the *hole* the grabbed tab leaves, so
   * tabs of different widths — which these are, being labels — all travel the
   * same distance. `gap` is the strip's own `gap-*`, measured rather than
   * assumed because the caller sets it.
   */
  const [drag, setDrag] = useState<{
    value: T;
    dx: number;
    from: number;
    target: number;
    width: number;
    gap: number;
  } | null>(null);
  const tabRefs = useRef(new Map<T, HTMLButtonElement>());
  /** Whether the gesture that just ended crossed the threshold, so the click
   *  WebKit raises on release can be told from a real one. */
  const moved = useRef(false);

  const onPointerDown = (e: React.PointerEvent<HTMLButtonElement>, tab: T) => {
    if (!onReorder || e.button !== 0) return;
    // **Nothing is cancelled here.** In WebKit a `click` is raised from a
    // `mousedown`/`mouseup` pair, and cancelling `pointerdown` suppresses the
    // `mousedown` — so a `preventDefault()` here would take the tab's own
    // click with it and the strip would stop switching tabs entirely. The
    // selection a press would otherwise start is held off by `select-none` on
    // the strip and by cancelling the *move* below instead.
    moved.current = false;
    const el = e.currentTarget;
    const pointerId = e.pointerId;
    const startX = e.clientX;
    let boxes: TabBox[] = [];
    let gap = 0;
    let from = -1;
    let latest: T[] | null = null;

    const onMove = (ev: PointerEvent) => {
      if (from === -1) {
        if (Math.abs(ev.clientX - startX) < THRESHOLD) return;
        const measured = tabs.map((t) => tabRefs.current.get(t.value));
        if (measured.some((n) => !n)) return;
        boxes = measured.map((node) => {
          const r = node!.getBoundingClientRect();
          return { left: r.left, mid: r.left + r.width / 2, width: r.width };
        });
        gap = boxes.length > 1 ? boxes[1].left - (boxes[0].left + boxes[0].width) : 0;
        from = tabs.findIndex((t) => t.value === tab);
        if (from === -1) return;
        moved.current = true;
      }

      // WebKit extends a selection with the compatibility `mousemove`, and
      // `index.css` hands `span` — which a badge is — `user-select: text` back
      // inside `body { user-select: none }`. Cancelling the move suppresses
      // that while leaving the press and release, and so the click, alone.
      ev.preventDefault();
      const selection = document.getSelection();
      if (selection && !selection.isCollapsed) selection.removeAllRanges();

      const me = boxes[from];
      const last = boxes[boxes.length - 1];
      // Keep the lifted tab inside the strip.
      const dx = Math.min(
        Math.max(ev.clientX - startX, boxes[0].left - me.left),
        last.left + last.width - (me.left + me.width),
      );
      // Where it would land: the grabbed tab's **leading edge** against the
      // captured midpoints of its neighbours — the right edge going right, the
      // left edge going left. An edge and not the centre, which is what
      // `TopTabBar` compares, because a centre cannot reach the far slot.
      //
      // The clamp above stops the grabbed tab with its right edge on the last
      // tab's right edge, so the furthest its centre can get is
      // `last.right - me.width / 2`. Passing `last.mid` from there needs
      // `last.width > me.width` — the end tab must be strictly *wider* than the
      // one in your hand. Equal widths land exactly on the midpoint, where `>`
      // is false. So with the centre rule the last slot is unreachable for any
      // tab at least as wide as the one sitting in it, and only just reachable
      // for a narrow one: Chat cleared Chapters by about 13px, which is why it
      // stopped dead right as it looked like it needed to keep going.
      //
      // A leading edge has no such dead zone: at the clamp the right edge *is*
      // `last.right`, which is past `last.mid` by half a tab whatever the
      // widths, and the left edge at the other end is `boxes[0].left`, likewise
      // past `boxes[0].mid`. Both ends are always reachable. It also swaps on
      // half the neighbour being covered rather than on all of it, which is the
      // more usual feel — the centre rule only swaps once the grabbed tab is
      // sitting square on top of its neighbour.
      const lead = me.left + dx;
      const trail = me.left + me.width + dx;
      let target = from;
      for (let i = from - 1; i >= 0; i--) if (lead < boxes[i].mid) target = i;
      for (let i = from + 1; i < boxes.length; i++) if (trail > boxes[i].mid) target = i;

      if (target !== from) {
        const next = tabs.map((t) => t.value);
        next.splice(target, 0, next.splice(from, 1)[0]);
        latest = next;
      } else {
        latest = null;
      }
      setDrag({ value: tab, dx, from, target, width: me.width, gap });
    };

    const end = () => {
      el.removeEventListener("pointermove", onMove);
      el.removeEventListener("pointerup", end);
      el.removeEventListener("pointercancel", end);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
      if (el.hasPointerCapture(pointerId)) el.releasePointerCapture(pointerId);
      // The write and the end of the gesture land in one commit — React
      // batches them, and the caller's store is synchronous — so the strip
      // re-renders already in its new order as the transforms come off. That
      // is why there is no *settle* here, which the board needs only because
      // its new order comes back from SQLite a few frames later.
      if (latest) onReorder(latest);
      setDrag(null);
    };

    el.setPointerCapture(pointerId);
    el.addEventListener("pointermove", onMove);
    el.addEventListener("pointerup", end);
    el.addEventListener("pointercancel", end);
    // Capture retargets the move and the release onto `el`, so its own
    // listeners are enough — except when `el` is unmounted mid-gesture, which
    // drops the capture and leaves nothing to hear the release. These are the
    // net; whichever fires first removes both sets.
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
  };

  return (
    <div
      role="tablist"
      className={cn(
        "flex items-center gap-4",
        // A press must not start a text selection: in WebKit a selection
        // pre-empts the gesture, and the strip is nothing but text.
        onReorder && "select-none [&_*]:select-none",
        className,
      )}
    >
      {tabs.map((tab, i) => {
        const active = tab.value === value;
        const grabbed = drag?.value === tab.value;
        // The grabbed tab rides the pointer; every tab between its old slot and
        // the one it would land in slides by the width of the hole it left.
        let dragStyle: React.CSSProperties | undefined;
        if (drag) {
          if (grabbed) {
            dragStyle = { transform: `translateX(${drag.dx}px)` };
          } else {
            const shift = drag.width + drag.gap;
            if (drag.from < i && i <= drag.target)
              dragStyle = { transform: `translateX(-${shift}px)` };
            else if (drag.target <= i && i < drag.from)
              dragStyle = { transform: `translateX(${shift}px)` };
          }
        }
        return (
          <button
            key={tab.value}
            type="button"
            role="tab"
            aria-selected={active}
            ref={(node) => {
              if (node) tabRefs.current.set(tab.value, node);
              else tabRefs.current.delete(tab.value);
            }}
            // Activation stays on the click rather than moving to the press
            // the way `TopTabBar`'s does: these are real `<button role="tab">`s
            // and a keyboard activation arrives as a click, which a pointerdown
            // handler would never see. A click raised by the release at the end
            // of a drag is the one to drop.
            onClick={() => {
              if (moved.current) return;
              onChange(tab.value);
            }}
            onPointerDown={(e) => onPointerDown(e, tab.value)}
            style={dragStyle}
            className={cn(
              "-mb-px flex cursor-pointer items-center gap-1.5 border-b-2 pb-2.5 pt-1 text-[13px] font-medium transition-colors",
              active
                ? "border-primary text-foreground"
                : "border-transparent text-muted-foreground hover:text-foreground",
              grabbed
                ? // Lifted: above its neighbours, tracking the pointer with no
                  // easing lag, and in the foreground colour whether or not it
                  // is the active tab — it is the one in hand.
                  "relative z-10 text-foreground"
                : drag && "transition-transform duration-200 ease-out",
            )}
          >
            {tab.label}
            {tab.badge}
          </button>
        );
      })}
    </div>
  );
}
