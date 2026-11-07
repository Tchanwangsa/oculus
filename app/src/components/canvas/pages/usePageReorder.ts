import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent, type RefObject } from 'react';
import { usePointerDrag } from '@/hooks/gestures/usePointerDrag';

/** How long a finger or Pencil holds a thumbnail still before it lifts; until then a drag scrolls the list. */
const HOLD_MS = 350;
/** Pixels from the list's top or bottom edge where a held thumbnail scrolls the list. */
const EDGE = 40;

/** A drag in progress: the page lifted, and the gap it would drop into, with that gap's offset in the list's content. */
export interface PageDrag { from: number; slot: number; top: number }

/** Where a page dropped into gap `slot` (0 is before the first) ends up, or null when that leaves it where it was. */
export function dropTarget(from: number, slot: number): number | null {
  if (slot === from || slot === from + 1) return null;
  return slot > from ? slot - 1 : slot;
}

/**
 * Drag a thumbnail to reorder the pages, on pointer events (`usePointerDrag`), not HTML5 drag, so it works the same with
 * a mouse, a finger and a Pencil on the companion. A mouse lifts after a few pixels; a finger or Pencil must hold still
 * for a moment first, so an ordinary swipe still scrolls the list, and once lifted its touch moves stop scrolling. The
 * list scrolls itself while a thumbnail is held near its top or bottom edge.
 */
export function usePageReorder(scroller: RefObject<HTMLElement | null>, count: number, onMove: (from: number, to: number) => void) {
  const [drag, setDrag] = useState<PageDrag | null>(null);
  const [held, setHeld] = useState<number | null>(null);
  const items = useRef(new Map<number, HTMLElement>());
  const latest = useRef({ count, onMove });
  latest.current = { count, onMove };
  const cleanup = useRef<(() => void) | null>(null);
  const gesture = usePointerDrag('y');
  useEffect(() => () => cleanup.current?.(), []);

  const itemRef = (index: number) => (node: HTMLElement | null) => {
    if (node) items.current.set(index, node); else items.current.delete(index);
  };

  const onPointerDown = (event: ReactPointerEvent<HTMLElement>, index: number) => {
    if (event.button !== 0) return;
    cleanup.current?.();
    let armed = event.pointerType === 'mouse', lifted = false, frame = 0, y = event.clientY, slot = index;
    /** Each gap's offset from the top of the list's content: gap i is above page i, the last below every page. */
    let gaps: number[] = [];
    const list = () => scroller.current;
    const timer = armed ? 0 : window.setTimeout(() => { armed = true; setHeld(index); }, HOLD_MS);
    const holdTouch = (touch: TouchEvent) => { if (armed) touch.preventDefault(); };
    window.addEventListener('touchmove', holdTouch, { passive: false });
    const place = () => {
      const element = list();
      if (!element || !gaps.length) return;
      const content = y - element.getBoundingClientRect().top + element.scrollTop;
      let nearest = 0;
      gaps.forEach((gap, i) => { if (Math.abs(gap - content) < Math.abs(gaps[nearest] - content)) nearest = i; });
      slot = nearest;
      setDrag({ from: index, slot, top: gaps[slot] });
    };
    const scroll = () => {
      const element = list();
      if (element) {
        const box = element.getBoundingClientRect();
        const speed = y < box.top + EDGE ? (y - box.top - EDGE) / 4 : y > box.bottom - EDGE ? (y - box.bottom + EDGE) / 4 : 0;
        if (speed) { element.scrollTop += speed; place(); }
      }
      frame = requestAnimationFrame(scroll);
    };
    const finish = () => {
      window.clearTimeout(timer);
      cancelAnimationFrame(frame);
      window.removeEventListener('touchmove', holdTouch);
      cleanup.current = null;
      setHeld(null); setDrag(null);
    };
    cleanup.current = finish;
    gesture.start(event, {
      lift: () => {
        const element = list();
        if (!armed || !element) return false;
        const top = element.getBoundingClientRect().top - element.scrollTop;
        const boxes = Array.from({ length: latest.current.count }, (_, i) => items.current.get(i)?.getBoundingClientRect());
        if (boxes.some(box => !box)) return false;
        const rects = boxes as DOMRect[];
        gaps = rects.map((box, i) => i ? (rects[i - 1].bottom + box.top) / 2 - top : box.top - top);
        gaps.push(rects[rects.length - 1].bottom - top);
        lifted = true;
        frame = requestAnimationFrame(scroll);
        return true;
      },
      move: moveEvent => { y = moveEvent.clientY; place(); },
      end: () => {
        const to = lifted ? dropTarget(index, slot) : null;
        finish();
        if (to !== null) latest.current.onMove(index, to);
      },
    });
  };

  return { drag, held, itemRef, onPointerDown, didDrag: gesture.didDrag };
}
