import { useRef } from "react";

/**
 * Pointer handlers that report where a press and its drag are within the
 * element, as fractions of its box in [0, 1] (x across, y down). Both the
 * pointer and the box are in CSS pixels under the app's page zoom, so the
 * ratio holds at any zoom. The press captures the pointer at once: the
 * surface holds no button whose click a capture would steal.
 */
export function useFractionDrag(onMove: (x: number, y: number) => void) {
  const move = useRef(onMove);
  move.current = onMove;
  const report = (e: React.PointerEvent<HTMLElement>) => {
    const box = e.currentTarget.getBoundingClientRect();
    if (box.width <= 0 || box.height <= 0) return;
    const clamp = (n: number) => Math.min(1, Math.max(0, n));
    move.current(clamp((e.clientX - box.left) / box.width), clamp((e.clientY - box.top) / box.height));
  };
  return {
    onPointerDown: (e: React.PointerEvent<HTMLElement>) => {
      if (e.button !== 0) return;
      e.currentTarget.setPointerCapture(e.pointerId);
      e.currentTarget.focus();
      report(e);
    },
    onPointerMove: (e: React.PointerEvent<HTMLElement>) => {
      if (e.currentTarget.hasPointerCapture(e.pointerId)) report(e);
    },
    onPointerUp: (e: React.PointerEvent<HTMLElement>) => {
      if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId);
    },
  };
}

/** Arrow keys as a step in x and y, ten times larger with Shift; null for
 *  any other key. */
export function arrowStep(e: React.KeyboardEvent, step: number): [number, number] | null {
  const n = e.shiftKey ? step * 10 : step;
  switch (e.key) {
    case "ArrowLeft":
      return [-n, 0];
    case "ArrowRight":
      return [n, 0];
    case "ArrowUp":
      return [0, -n];
    case "ArrowDown":
      return [0, n];
    default:
      return null;
  }
}
