import { useEffect, useRef, useState } from "react";
import type { CSSProperties, PointerEvent as ReactPointerEvent } from "react";

/**
 * The pointer-capture gesture every in-window drag shares: past a small
 * threshold the press lifts and captures the pointer, and every exit — release, cancel, a release the
 * captured element never hears because it unmounted, the owner unmounting —
 * runs the caller's `end` once. Not HTML5 DnD: see docs/ui.md,
 * "Gotchas". `useCardDrag` builds lists on it; `useStripReorder` below
 * builds tab strips and the sidebar rail.
 */

/** Pointer travel (px) before a press becomes a drag rather than a click. */
const THRESHOLD = 4;

/**
 * Put this on the drag surface. It holds text selection off in CSS because
 * cancelling `pointerdown` would also kill a `click` in WebKit (see
 * docs/ui.md). The descendant half beats `index.css`'s `span`/`p` reset only
 * while that reset stays inside `@layer base`.
 */
export const DRAG_SURFACE = "select-none [&_*]:select-none";

export interface PointerDragHandlers {
  /** Past the threshold: measure. `false` stays unlifted and retries on the next move. */
  lift: () => boolean;
  /** Every move once lifted; `dx`/`dy` are travel since the press. */
  move: (ev: PointerEvent, dx: number, dy: number) => void;
  /** Once per gesture, lifted or not. A pointercancel ends it like a release. */
  end: () => void;
}

/** @param axis Which travel counts toward the threshold. */
export function usePointerDrag(axis: "x" | "y" | "xy") {
  const moved = useRef(false);
  const teardown = useRef<(() => void) | null>(null);

  useEffect(() => () => teardown.current?.(), []);

  const start = (e: ReactPointerEvent<HTMLElement>, h: PointerDragHandlers) => {
    // A gesture whose release never arrived must not outlive the next press.
    teardown.current?.();
    moved.current = false;
    if (e.button !== 0) return;
    // Don't preventDefault the press — see docs/ui.md: it kills the click.
    const el = e.currentTarget;
    const pointerId = e.pointerId;
    const startX = e.clientX;
    const startY = e.clientY;

    const onMove = (ev: PointerEvent) => {
      if (ev.pointerId !== pointerId) return;
      // The button is up but no release was heard: end rather than follow
      // the bare pointer around.
      if ((ev.buttons & 1) === 0) return end();
      const dx = ev.clientX - startX;
      const dy = ev.clientY - startY;
      if (!moved.current) {
        const travel =
          axis === "x" ? Math.abs(dx) : axis === "y" ? Math.abs(dy) : Math.hypot(dx, dy);
        if (travel < THRESHOLD || !h.lift()) return;
        moved.current = true;
        // Capture only once lifted: a capture taken on the press retargets the
        // click to `el`, so a button inside it never hears its own click.
        el.setPointerCapture(pointerId);
      }
      // Cancel the move, not the press: it stops WebKit extending a selection
      // while keeping the click alive. removeAllRanges clears one anchored
      // before the lift.
      ev.preventDefault();
      const selection = document.getSelection();
      if (selection && !selection.isCollapsed) selection.removeAllRanges();
      h.move(ev, dx, dy);
    };

    const end = (ev?: PointerEvent) => {
      if (ev && ev.pointerId !== pointerId) return;
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
      if (el.hasPointerCapture(pointerId)) el.releasePointerCapture(pointerId);
      teardown.current = null;
      h.end();
    };

    // On the window, so a press that drifts off `el` before the lift still
    // hears its moves, and an `el` unmounted mid-gesture still hears the release.
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
    teardown.current = () => end();
  };

  return {
    start,
    /** Whether the last gesture lifted, so a click that ends a drag can be
     *  ignored. Stays true until the next press. */
    didDrag: () => moved.current,
  };
}

/** A strip's live drag: the grabbed item rides the pointer by `delta` along
 *  the strip's axis, and the items between `from` and `target` slide by the
 *  hole it left. */
export interface StripDrag<K> {
  key: K;
  delta: number;
  from: number;
  target: number;
  size: number;
  gap: number;
}

/** One item's extent along the strip's axis. */
interface Box {
  start: number;
  mid: number;
  size: number;
}

/**
 * Reorder for a strip of tabs or rail icons, rects captured once at lift and
 * the order written only on drop. `swapOn` is deliberately per strip — see
 * docs/ui.md: `ViewTabs` swaps on the leading edge, `TopTabBar` on the centre.
 */
export function useStripReorder<K>({
  keys,
  axis = "x",
  swapOn,
  gap: fixedGap,
  onDrop,
}: {
  keys: readonly K[];
  /** `x` for a row, `y` for a column (the sidebar rail). */
  axis?: "x" | "y";
  swapOn: "edge" | "centre";
  /** px between items; measured off the first pair when omitted. */
  gap?: number;
  /** Only when the item lands somewhere new; `order` is `keys` rearranged. */
  onDrop: (drop: { key: K; target: number; order: K[] }) => void;
}) {
  const [drag, setDrag] = useState<StripDrag<K> | null>(null);
  const nodes = useRef(new Map<K, HTMLElement>());
  const latest = useRef({ keys, onDrop });
  latest.current = { keys, onDrop };
  const gesture = usePointerDrag(axis);

  const itemRef = (key: K) => (node: HTMLElement | null) => {
    if (node) nodes.current.set(key, node);
    else nodes.current.delete(key);
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLElement>, key: K) => {
    let order: K[] = [];
    let boxes: Box[] = [];
    let gap = 0;
    let from = -1;
    let target = -1;

    gesture.start(e, {
      lift: () => {
        order = [...latest.current.keys];
        const measured = order.map((k) => nodes.current.get(k));
        if (measured.some((n) => !n)) return false;
        boxes = measured.map((node) => {
          const r = node!.getBoundingClientRect();
          const start = axis === "x" ? r.left : r.top;
          const size = axis === "x" ? r.width : r.height;
          return { start, mid: start + size / 2, size };
        });
        gap =
          fixedGap ??
          (boxes.length > 1 ? boxes[1].start - (boxes[0].start + boxes[0].size) : 0);
        from = order.indexOf(key);
        return from !== -1;
      },
      move: (_ev, dx, dy) => {
        const travel = axis === "x" ? dx : dy;
        const me = boxes[from];
        const last = boxes[boxes.length - 1];
        const delta = Math.min(
          Math.max(travel, boxes[0].start - me.start),
          last.start + last.size - (me.start + me.size),
        );
        target = from;
        if (swapOn === "edge") {
          const lead = me.start + delta;
          const trail = me.start + me.size + delta;
          for (let i = from - 1; i >= 0; i--) if (lead < boxes[i].mid) target = i;
          for (let i = from + 1; i < boxes.length; i++) if (trail > boxes[i].mid) target = i;
        } else {
          const centre = me.mid + delta;
          for (let i = from - 1; i >= 0; i--) if (centre < boxes[i].mid) target = i;
          for (let i = from + 1; i < boxes.length; i++) if (centre > boxes[i].mid) target = i;
        }
        setDrag({ key, delta, from, target, size: me.size, gap });
      },
      end: () => {
        if (from !== -1 && target !== -1 && target !== from) {
          const next = [...order];
          next.splice(target, 0, next.splice(from, 1)[0]);
          latest.current.onDrop({ key, target, order: next });
        }
        // No settle step: callers' stores are synchronous, so the new order
        // and the cleared transforms land in one commit.
        setDrag(null);
      },
    });
  };

  /** The transform for the item `key` at render `index`; none between gestures. */
  const styleFor = (key: K, index: number): CSSProperties | undefined => {
    if (!drag) return undefined;
    const translate = axis === "x" ? "translateX" : "translateY";
    if (drag.key === key) return { transform: `${translate}(${drag.delta}px)` };
    const shift = drag.size + drag.gap;
    if (drag.from < index && index <= drag.target)
      return { transform: `${translate}(-${shift}px)` };
    if (drag.target <= index && index < drag.from)
      return { transform: `${translate}(${shift}px)` };
    return undefined;
  };

  return { drag, itemRef, onPointerDown, styleFor, didDrag: gesture.didDrag };
}
