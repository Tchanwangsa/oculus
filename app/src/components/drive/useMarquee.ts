import { useEffect, useRef, type PointerEvent as ReactPointerEvent } from "react";
import { DRAG_SURFACE } from "@/hooks/gestures/usePointerDrag";
import type { ClickMods } from "@/lib/drive/selection";
import { isMac } from "@/lib/platform";

/** Pointer travel (px) before a press in empty space draws a box, not a click. */
const THRESHOLD = 4;
/** How near the scroller's top or bottom edge the pointer scrolls it, and how
 *  far a frame at the very edge goes. */
const EDGE = 40;
const MAX_STEP = 20;
const BOX = "pointer-events-none fixed z-50 rounded-[3px] border border-brand bg-brand/10";
/** A press on these is theirs, not the start of a box. */
const NOT_EMPTY = "[data-drive-item], button, a, input, textarea, select, [role=button], [contenteditable=true]";

interface Rect { left: number; top: number; right: number; bottom: number }

export interface MarqueeHandlers {
  /** The box has lifted: the selection it builds on is the one now. */
  start: () => void;
  /** The entries under the box changed; `hit` is in screen order. */
  box: (hit: string[], mods: ClickMods) => void;
}

/** The nearest element between `from` and `stop` that scrolls vertically.
 *  An `auto` one must overflow: a horizontal scroller computes `overflow-y`
 *  to `auto` too, though it grows with its rows. */
function scrollerOf(from: Element, stop: HTMLElement): HTMLElement {
  for (let el: Element | null = from; el && el !== stop; el = el.parentElement) {
    if (!(el instanceof HTMLElement)) continue;
    const y = getComputedStyle(el).overflowY;
    if (y === "scroll" || (y === "auto" && el.scrollHeight > el.clientHeight + 1)) return el;
  }
  return stop;
}

/**
 * Finder's rubber band for Drive: a drag that starts in empty space draws a
 * box, and every entry (`data-drive-item`) it touches is picked, ⇧ adding to
 * the selection and ⌘ toggling against it. Boxes are kept in the scroller's
 * content coordinates, so the box stays pinned to the entries while the
 * scroller scrolls (by wheel, or by holding the pointer near its top or
 * bottom). The box is a fixed element on `body`, moved directly rather than
 * re-rendered per move. The click that ends a drawn box is swallowed, so it
 * never clears what the box picked. Returns the column's `onPointerDown`.
 */
export function useMarquee(handlers: MarqueeHandlers) {
  const latest = useRef(handlers);
  latest.current = handlers;
  const teardown = useRef<(() => void) | null>(null);
  useEffect(() => () => teardown.current?.(), []);

  return (e: ReactPointerEvent<HTMLElement>) => {
    if (e.button !== 0 || e.pointerType === "touch") return;
    const target = e.target as Element;
    if (target.closest(NOT_EMPTY)) return;
    const column = e.currentTarget;
    const scroller = scrollerOf(target, column);
    // A press on the scrollbar scrolls; it never boxes.
    if (e.clientX >= scroller.getBoundingClientRect().left + scroller.clientLeft + scroller.clientWidth) return;
    // Don't preventDefault the press: WebKit would drop the click (docs/ui.md).
    teardown.current?.();

    const pointerId = e.pointerId;
    const mods: ClickMods = { toggle: isMac ? e.metaKey : e.ctrlKey, range: e.shiftKey };
    const at = (clientX: number, clientY: number) => {
      const r = scroller.getBoundingClientRect();
      return { x: clientX - r.left + scroller.scrollLeft, y: clientY - r.top + scroller.scrollTop };
    };
    const anchor = at(e.clientX, e.clientY);
    let px = e.clientX;
    let py = e.clientY;
    let lifted = false;
    let items: { key: string; rect: Rect }[] = [];
    let shown = "";
    let frame = 0;
    const box = document.createElement("div");
    box.className = BOX;
    const surface = DRAG_SURFACE.split(" ");

    const update = () => {
      const now = at(px, py);
      const area: Rect = {
        left: Math.min(anchor.x, now.x),
        top: Math.min(anchor.y, now.y),
        right: Math.max(anchor.x, now.x),
        bottom: Math.max(anchor.y, now.y),
      };
      const hit = items
        .filter(({ rect }) => rect.left < area.right && rect.right > area.left && rect.top < area.bottom && rect.bottom > area.top)
        .map((i) => i.key);
      const key = hit.join("\0");
      if (key !== shown) {
        shown = key;
        latest.current.box(hit, mods);
      }
      // Drawn in the viewport, clipped to the scroller's visible box.
      const r = scroller.getBoundingClientRect();
      const dx = r.left - scroller.scrollLeft;
      const dy = r.top - scroller.scrollTop;
      const left = Math.max(area.left + dx, r.left);
      const top = Math.max(area.top + dy, r.top);
      const right = Math.min(area.right + dx, r.right);
      const bottom = Math.min(area.bottom + dy, r.bottom);
      Object.assign(box.style, {
        display: right > left && bottom > top ? "block" : "none",
        left: `${left}px`,
        top: `${top}px`,
        width: `${Math.max(0, right - left)}px`,
        height: `${Math.max(0, bottom - top)}px`,
      });
    };

    const lift = () => {
      lifted = true;
      const r = scroller.getBoundingClientRect();
      items = [...scroller.querySelectorAll<HTMLElement>("[data-drive-item]")].map((el) => {
        const b = el.getBoundingClientRect();
        const left = b.left - r.left + scroller.scrollLeft;
        const top = b.top - r.top + scroller.scrollTop;
        return { key: el.dataset.driveItem!, rect: { left, top, right: left + b.width, bottom: top + b.height } };
      });
      window.getSelection()?.removeAllRanges();
      column.classList.add(...surface);
      document.body.append(box);
      latest.current.start();
      frame = requestAnimationFrame(edgeScroll);
    };

    /** Scrolls while the pointer holds near the top or bottom edge. */
    const edgeScroll = () => {
      const r = scroller.getBoundingClientRect();
      const into = py < r.top + EDGE ? py - (r.top + EDGE) : py > r.bottom - EDGE ? py - (r.bottom - EDGE) : 0;
      if (into) {
        const before = scroller.scrollTop;
        scroller.scrollTop += Math.max(-MAX_STEP, Math.min(MAX_STEP, (into / EDGE) * MAX_STEP));
        if (scroller.scrollTop !== before) update();
      }
      frame = requestAnimationFrame(edgeScroll);
    };

    const onMove = (ev: PointerEvent) => {
      if (ev.pointerId !== pointerId) return;
      if ((ev.buttons & 1) === 0) return end();
      px = ev.clientX;
      py = ev.clientY;
      if (!lifted) {
        if (Math.hypot(px - e.clientX, py - e.clientY) < THRESHOLD) return;
        lift();
      }
      // Cancelling the move, not the press, keeps text from selecting.
      ev.preventDefault();
      update();
    };
    const onScroll = () => { if (lifted) update(); };

    const end = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      scroller.removeEventListener("scroll", onScroll);
      cancelAnimationFrame(frame);
      box.remove();
      column.classList.remove(...surface);
      teardown.current = null;
      if (!lifted) return;
      const swallow = (ev: MouseEvent) => {
        ev.stopPropagation();
        ev.preventDefault();
      };
      window.addEventListener("click", swallow, { capture: true, once: true });
      setTimeout(() => window.removeEventListener("click", swallow, { capture: true }), 0);
    };
    const onUp = (ev: PointerEvent) => {
      if (ev.pointerId === pointerId) end();
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
    scroller.addEventListener("scroll", onScroll, { passive: true });
    teardown.current = end;
  };
}
