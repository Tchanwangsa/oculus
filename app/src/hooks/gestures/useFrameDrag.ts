import { useEffect, useRef } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";

type PointerPoint = { clientX: number; clientY: number };

/** Starts a `useFrameDrag` gesture on a primary-button press: cancels the
 *  press and captures the pointer. False for any other button. */
export function capturePress(e: ReactPointerEvent): boolean {
  if (e.button !== 0) return false;
  e.preventDefault();
  (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
  return true;
}

/**
 * Window-level tracking for a resize-style drag begun with `capturePress`.
 * While `active()`, moves coalesce to one `move` per frame (each sets state
 * and may force layout); a release or cancel drops any queued move before
 * `end`, so nothing applies after it. Handlers are read through a ref.
 */
export function useFrameDrag(handlers: {
  active: () => boolean;
  move: (p: PointerPoint) => void;
  end: () => void;
}) {
  const ref = useRef(handlers);
  ref.current = handlers;

  useEffect(() => {
    let frame = 0;
    let pending: PointerPoint | null = null;

    const onMove = (e: PointerEvent) => {
      if (!ref.current.active()) return;
      pending = { clientX: e.clientX, clientY: e.clientY };
      if (frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        const p = pending;
        pending = null;
        if (p) ref.current.move(p);
      });
    };

    const onUp = () => {
      if (!ref.current.active()) return;
      if (frame) cancelAnimationFrame(frame);
      frame = 0;
      pending = null;
      ref.current.end();
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
    return () => {
      if (frame) cancelAnimationFrame(frame);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
    };
  }, []);
}
