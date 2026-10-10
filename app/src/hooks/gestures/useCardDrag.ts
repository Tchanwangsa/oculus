import { useCallback, useEffect, useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";
import { usePointerDrag } from "@/hooks/gestures/usePointerDrag";

export { DRAG_SURFACE } from "@/hooks/gestures/usePointerDrag";

/**
 * Drag for vertical lists spanning several containers, on `usePointerDrag`:
 * the lifted item rides the pointer, neighbours slide aside, the caller writes
 * once on drop, and the item then settles into its new slot
 * ({@link CardDragState.settling}). It deals in `(containerId, itemId)` and
 * knows nothing about tasks.
 */

/** The settle's glide time; matches the callers' `duration-200` transition. */
const SETTLE_MS = 200;

/** Longest the settle waits for the re-read, so a write that fails costs a
 *  spring-back rather than a list frozen in its pre-drop order. */
const SETTLE_CAP_MS = 600;

/** The item a gesture starts on; ids are per-container. */
interface CardDragItem {
  id: number;
  containerId: string;
}

export interface CardDragState {
  id: number;
  /** Pointer travel since the press — what the lifted item is translated by. */
  dx: number;
  dy: number;
  containerId: string;
  targetContainerId: string;
  /** Landing index among the target's items, the dragged one excluded. */
  targetIndex: number;
  /** Viewport box at lift, for callers whose lists clip and so draw the lifted
   *  copy in a fixed overlay. */
  rect: { left: number; top: number; width: number; height: number };
  /**
   * Pointer is up and the item is gliding into its drop slot (`dx`/`dy` are now
   * that slot's offset). Ending on release instead would snap the item back
   * until the re-read lands, then jump. Callers add a transform transition and
   * keep drawing the pre-drop list via {@link useSettledList}.
   */
  settling: boolean;
}

export interface CardDrop {
  id: number;
  from: string;
  containerId: string;
  /** Index among the destination's items, the dragged one excluded. */
  index: number;
  /**
   * Item ids either side of the landing slot (`null` at an end), dragged item
   * excluded — `moveTask` positions by neighbours, see `siblingDropSlot`. They
   * are in DOM order; a grouped list re-derives them from `index`.
   */
  before: number | null;
  after: number | null;
}

export interface CardDragHandle {
  /** `null` between gestures; kept through the settle, so "pointer is down"
   *  is `drag && !drag.settling`. */
  drag: CardDragState | null;
  /** Ref for a container — the box the pointer is hit-tested against. */
  containerRef: (containerId: string) => (node: HTMLElement | null) => void;
  itemRef: (containerId: string, itemId: number) => (node: HTMLElement | null) => void;
  onPointerDown: (e: ReactPointerEvent<HTMLElement>, item: CardDragItem) => void;
  /** px to translate a non-grabbed item at render `index` in `containerId`
   *  to open or close the gap; 0 between gestures. */
  shiftFor: (containerId: string, index: number) => number;
  /** Whether the last gesture crossed the threshold, so the title `Link` can
   *  ignore the click that ends a drag. Stays true until the next press. */
  didDrag: () => boolean;
}

/** One item's geometry, captured at lift and never re-measured. */
interface ItemBox {
  id: number;
  left: number;
  top: number;
  width: number;
  height: number;
  mid: number;
}

interface Snapshot {
  containers: { id: string; rect: DOMRect }[];
  /** Per container, its items sorted top to bottom (render order). */
  lists: Map<string, ItemBox[]>;
  gap: number;
  from: { containerId: string; index: number; box: ItemBox };
}

/** The first adjacent gap found in any list — every column shares one `gap-*`. */
function measureGap(lists: Map<string, ItemBox[]>): number {
  for (const list of lists.values()) {
    for (let i = 1; i < list.length; i++) {
      const gap = list[i].top - (list[i - 1].top + list[i - 1].height);
      if (gap >= 0) return gap;
    }
  }
  return 0;
}

export interface CardDragOptions {
  /**
   * Anything whose identity changes when the caller's list is re-read. The
   * settle ends once {@link SETTLE_MS} has run and this has changed; leave it
   * out and there is no settle.
   */
  settleOn?: unknown;
}

/**
 * Offset from the lifted box to the drop slot, read off the same captured
 * rects `shiftFor` uses so the two agree. `null` for an empty container:
 * nothing measured to land against.
 */
function settleTo(s: Snapshot, drop: CardDrop): { dx: number; dy: number } | null {
  const others = (s.lists.get(drop.containerId) ?? []).filter((b) => b.id !== drop.id);
  const sameList = drop.containerId === s.from.containerId;
  const step = s.from.box.height + s.gap;

  let top: number;
  if (drop.index >= others.length) {
    const last = others[others.length - 1];
    if (!last) return null;
    // End of the list; in its own list everything below has slid up a step.
    top = last.top + last.height + s.gap - (sameList ? step : 0);
  } else if (sameList && drop.index > s.from.index) {
    // Down its own list: the item above has slid up a step.
    const above = others[drop.index - 1];
    top = above.top + above.height + s.gap - step;
  } else {
    // Otherwise it lands on the captured top of the item below it.
    top = others[drop.index].top;
  }

  // Sideways from container to container, not neighbour to neighbour: a card
  // keeps its own inset (a subtask's `ml-4`).
  const fromBox = s.containers.find((c) => c.id === s.from.containerId);
  const toBox = s.containers.find((c) => c.id === drop.containerId);
  return {
    dx: fromBox && toBox ? toBox.rect.left - fromBox.rect.left : 0,
    dy: top - s.from.box.top,
  };
}

/**
 * Holds the pre-drop list while a drop settles: the re-read lands mid-glide,
 * and drawing it under transforms computed for the old order would displace
 * everything twice.
 */
export function useSettledList<T>(list: T, drag: CardDragState | null): T {
  const held = useRef(list);
  if (!drag?.settling) held.current = list;
  return held.current;
}

/**
 * @param onDrop Called once when a gesture lands somewhere new. Return `true`
 * if it wrote something; otherwise no new order is coming and the item springs
 * back instead of settling.
 */
export function useCardDrag(
  onDrop: (drop: CardDrop) => boolean | void,
  options: CardDragOptions = {},
): CardDragHandle {
  const [drag, setDrag] = useState<CardDragState | null>(null);
  // Declared before the settle-timer cleanup below: an unmount ends the
  // gesture first, then clears whatever timer that end started.
  const gesture = usePointerDrag("xy");

  const containers = useRef(new Map<string, HTMLElement>());
  const items = useRef(new Map<string, { containerId: string; id: number; node: HTMLElement }>());
  // Ref callbacks cached by key, so every pointer move's re-render does not
  // re-register every item's ref.
  const containerCbs = useRef(new Map<string, (node: HTMLElement | null) => void>());
  const itemCbs = useRef(new Map<string, (node: HTMLElement | null) => void>());

  const snapshot = useRef<Snapshot | null>(null);
  // Read at drop time, not closed over at press time.
  const dropRef = useRef(onDrop);
  dropRef.current = onDrop;

  /** The settle in flight: `settleOn` at the drop, and when the glide began. */
  const settle = useRef<{ key: unknown; at: number } | null>(null);
  const settleTimer = useRef<number | null>(null);
  const settleKey = useRef(options.settleOn);
  settleKey.current = options.settleOn;

  /** Ends the gesture, settle included. Every path out goes through here. */
  const stop = useCallback(() => {
    if (settleTimer.current != null) {
      window.clearTimeout(settleTimer.current);
      settleTimer.current = null;
    }
    settle.current = null;
    snapshot.current = null;
    setDrag(null);
  }, []);

  useEffect(
    () => () => {
      if (settleTimer.current != null) window.clearTimeout(settleTimer.current);
    },
    [],
  );

  // The new order has arrived: end the settle once the glide has had its
  // SETTLE_MS, or the item would move twice.
  useEffect(() => {
    const phase = settle.current;
    if (!phase || options.settleOn === phase.key) return;
    const left = Math.max(0, SETTLE_MS - (performance.now() - phase.at));
    const t = window.setTimeout(stop, left);
    return () => window.clearTimeout(t);
  }, [options.settleOn, stop]);

  const containerRef = (containerId: string) => {
    let cb = containerCbs.current.get(containerId);
    if (!cb) {
      cb = (node: HTMLElement | null) => {
        if (node) containers.current.set(containerId, node);
        else {
          containers.current.delete(containerId);
          containerCbs.current.delete(containerId);
        }
      };
      containerCbs.current.set(containerId, cb);
    }
    return cb;
  };

  const itemRef = (containerId: string, itemId: number) => {
    // Keep `\0` as an escape: a raw NUL byte makes grep/rg treat this file as
    // binary and skip it silently.
    const key = `${containerId}\0${itemId}`;
    let cb = itemCbs.current.get(key);
    if (!cb) {
      cb = (node: HTMLElement | null) => {
        if (node) items.current.set(key, { containerId, id: itemId, node });
        else {
          items.current.delete(key);
          itemCbs.current.delete(key);
        }
      };
      itemCbs.current.set(key, cb);
    }
    return cb;
  };

  /** Every rect, once, at lift: items only move by `transform` during a drag,
   *  so re-measuring would read back displaced positions. */
  const capture = (item: CardDragItem): Snapshot | null => {
    const boxes: { id: string; rect: DOMRect }[] = [];
    for (const [id, node] of containers.current) {
      boxes.push({ id, rect: node.getBoundingClientRect() });
    }
    const lists = new Map<string, ItemBox[]>();
    for (const entry of items.current.values()) {
      const r = entry.node.getBoundingClientRect();
      const box: ItemBox = {
        id: entry.id,
        left: r.left,
        top: r.top,
        width: r.width,
        height: r.height,
        mid: r.top + r.height / 2,
      };
      const list = lists.get(entry.containerId);
      if (list) list.push(box);
      else lists.set(entry.containerId, [box]);
    }
    for (const list of lists.values()) list.sort((a, b) => a.top - b.top);

    const own = lists.get(item.containerId);
    const index = own ? own.findIndex((b) => b.id === item.id) : -1;
    if (!own || index < 0) return null;
    return {
      containers: boxes,
      lists,
      gap: measureGap(lists),
      from: { containerId: item.containerId, index, box: own[index] },
    };
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLElement>, item: CardDragItem) => {
    if (e.button !== 0) return;
    // A new press interrupts any settle in flight.
    if (settle.current) stop();
    let snap: Snapshot | null = null;
    let latest: CardDrop | null = null;
    // Between containers (a gutter), the last container hit still stands.
    let lastTarget = item.containerId;

    gesture.start(e, {
      lift: () => {
        snap = capture(item);
        snapshot.current = snap;
        return snap != null;
      },
      move: (ev, dx, dy) => {
        const s = snap!;
        for (const c of s.containers) {
          if (
            ev.clientX >= c.rect.left &&
            ev.clientX <= c.rect.right &&
            ev.clientY >= c.rect.top &&
            ev.clientY <= c.rect.bottom
          ) {
            lastTarget = c.id;
            break;
          }
        }

        // The item's centre, not the pointer's, against the captured midpoints.
        const centre = s.from.box.mid + dy;
        const others = (s.lists.get(lastTarget) ?? []).filter((b) => b.id !== item.id);
        let index = 0;
        while (index < others.length && others[index].mid < centre) index++;

        latest = {
          id: item.id,
          from: s.from.containerId,
          containerId: lastTarget,
          index,
          before: index > 0 ? others[index - 1].id : null,
          after: index < others.length ? others[index].id : null,
        };
        setDrag({
          id: item.id,
          dx,
          dy,
          containerId: s.from.containerId,
          targetContainerId: lastTarget,
          targetIndex: index,
          rect: s.from.box,
          settling: false,
        });
      },
      end: () => {
        const drop = latest;
        const s = snap;
        snap = null;
        if (!drop || !s) return stop();
        if (drop.containerId === s.from.containerId && drop.index === s.from.index) {
          return stop();
        }
        // Glide only if the caller wrote (see `@param onDrop`) and named `settleOn`.
        const wrote = dropRef.current(drop) === true;
        const to = wrote && settleKey.current !== undefined ? settleTo(s, drop) : null;
        if (!to) return stop();
        settle.current = { key: settleKey.current, at: performance.now() };
        settleTimer.current = window.setTimeout(stop, SETTLE_CAP_MS);
        // Only offset and phase change; neighbours keep the gap they hold open.
        setDrag((prev) =>
          prev ? { ...prev, dx: to.dx, dy: to.dy, settling: true } : prev,
        );
      },
    });
  };

  const shiftFor = (containerId: string, index: number): number => {
    const s = snapshot.current;
    if (!drag || !s) return 0;
    const step = s.from.box.height + s.gap;
    const source = containerId === drag.containerId;
    const target = containerId === drag.targetContainerId;
    if (source && target) {
      // Within one list, only the run between old and new slot moves.
      if (s.from.index < index && index <= drag.targetIndex) return -step;
      if (drag.targetIndex <= index && index < s.from.index) return step;
      return 0;
    }
    if (source) return index > s.from.index ? -step : 0;
    if (target) return index >= drag.targetIndex ? step : 0;
    return 0;
  };

  return {
    drag,
    containerRef,
    itemRef,
    onPointerDown,
    shiftFor,
    didDrag: gesture.didDrag,
  };
}
