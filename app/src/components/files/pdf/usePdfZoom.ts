import { useCallback, useEffect, type RefObject } from "react";
import {
  DEFAULT_FIT,
  DRAW_DELAY,
  FIT_VALUES,
  GESTURE_LAPSE,
  LINE_HEIGHT,
} from "@/components/files/pdf/constants";
import type { Engine } from "@/components/files/pdf/types";

/** Zoom: the toolbar's steps, pinch and ⌘-wheel (which pdf.js does not bind
 *  itself), and re-fitting when the container resizes. */
export function usePdfZoom(
  engineRef: RefObject<Engine | null>,
  containerRef: RefObject<HTMLDivElement | null>,
) {
  /** Scale by a ratio about a point (or the view's centre). pdf.js wants
   *  `origin` in the container's offset space — it subtracts `offsetTop`/
   *  `offsetLeft` — so client coordinates are converted, not passed through. */
  const zoomBy = useCallback(
    (factor: number, clientX?: number, clientY?: number) => {
      const viewer = engineRef.current?.viewer;
      const container = containerRef.current;
      if (!viewer || !container) return;
      let origin: [number, number] | undefined;
      if (clientX != null && clientY != null) {
        const rect = container.getBoundingClientRect();
        origin = [
          clientX - rect.left + container.offsetLeft,
          clientY - rect.top + container.offsetTop,
        ];
      }
      viewer.updateScale({
        scaleFactor: factor,
        origin,
        drawingDelay: DRAW_DELAY,
      });
    },
    [],
  );

  const resetZoom = useCallback(() => {
    const viewer = engineRef.current?.viewer;
    if (viewer) viewer.currentScaleValue = DEFAULT_FIT;
  }, []);

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

  // Re-apply a fit when the container resizes; pdf.js leaves re-fitting to
  // the host viewer.
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      const viewer = engineRef.current?.viewer;
      const value = viewer?.currentScaleValue;
      if (viewer && value && FIT_VALUES.has(value)) {
        viewer.currentScaleValue = value;
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  return { zoomBy, resetZoom };
}
