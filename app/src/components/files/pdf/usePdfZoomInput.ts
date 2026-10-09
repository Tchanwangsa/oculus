import { useEffect, type RefObject } from "react";

/** WebKit sometimes drops `gestureend`, which would latch `gestureActive` and
 *  kill ⌘-scroll zoom; the flag releases itself after this much quiet. */
const GESTURE_LAPSE = 400;

/** Pixels per line when `deltaMode === DOM_DELTA_LINE`. */
const LINE_HEIGHT = 16;

/** ⌘-scroll and trackpad pinch on the scroller zoom by a factor about the
 *  pointer (`zoomBy`). */
export function usePdfZoomInput(
  containerRef: RefObject<HTMLDivElement | null>,
  zoomBy: (factor: number, clientX?: number, clientY?: number) => void,
) {
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    // WebKit reports one pinch twice — as `gesture*` events and as ctrlKey
    // wheel events — so the wheel path stands down while a gesture is live.
    const gestureActive = { current: false };
    let pinchBase = 1;
    let lapse: ReturnType<typeof setTimeout> | null = null;
    const armRelease = () => {
      if (lapse != null) clearTimeout(lapse);
      lapse = setTimeout(() => {
        lapse = null;
        gestureActive.current = false;
      }, GESTURE_LAPSE);
    };
    const disarm = () => {
      if (lapse != null) clearTimeout(lapse);
      lapse = null;
    };

    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey && !e.metaKey) return;
      e.preventDefault();
      if (gestureActive.current) return;
      // Exponential and clamped: a linear factor goes negative on one ±120
      // ⌘+wheel notch.
      const raw = e.deltaY * (e.deltaMode === 1 ? LINE_HEIGHT : 1);
      const d = Math.max(-50, Math.min(50, raw));
      zoomBy(Math.exp(-d * 0.01), e.clientX, e.clientY);
    };

    // Gesture `scale` is cumulative from gesturestart, so zoom by the ratio
    // to the previous frame.
    const onGestureStart = (e: Event) => {
      e.preventDefault();
      gestureActive.current = true;
      pinchBase = 1;
      armRelease();
    };
    const onGestureChange = (e: Event) => {
      e.preventDefault();
      armRelease();
      const g = e as unknown as {
        scale: number;
        clientX?: number;
        clientY?: number;
      };
      if (!g.scale) return;
      const factor = g.scale / pinchBase;
      pinchBase = g.scale;
      zoomBy(factor, g.clientX, g.clientY);
    };
    const onGestureEnd = (e: Event) => {
      e.preventDefault();
      disarm();
      gestureActive.current = false;
    };

    el.addEventListener("wheel", onWheel, { passive: false });
    el.addEventListener("gesturestart", onGestureStart);
    el.addEventListener("gesturechange", onGestureChange);
    el.addEventListener("gestureend", onGestureEnd);
    return () => {
      el.removeEventListener("wheel", onWheel);
      el.removeEventListener("gesturestart", onGestureStart);
      el.removeEventListener("gesturechange", onGestureChange);
      el.removeEventListener("gestureend", onGestureEnd);
      disarm();
    };
  }, [zoomBy]);
}
