import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { isVertical, usePlayerPrefs, type Dock } from "@/stores/playerPrefsStore";

export { isVertical, type Dock };

const MIN_H = 88;
const MIN_W = 220;
/** Room the video stack always keeps for itself. */
const KEEP_H = 140;
const KEEP_W = 300;
/** Pointer travel before a header press counts as a dock drag, not a click. */
const DRAG_SLOP = 5;

/** The edge nearest the pointer — the container split into four triangles. */
function nearestEdge(rect: DOMRect, x: number, y: number): Dock {
  const fx = Math.min(1, Math.max(0, (x - rect.left) / rect.width));
  const fy = Math.min(1, Math.max(0, (y - rect.top) / rect.height));
  const d: [Dock, number][] = [
    ["left", fx],
    ["right", 1 - fx],
    ["top", fy],
    ["bottom", 1 - fy],
  ];
  return d.reduce((a, b) => (b[1] < a[1] ? b : a))[0];
}

/**
 * Dock side + size for the lecture transcript panel: drag its header to any
 * edge of the player, drag the divider to resize. Both are global player
 * preferences (`playerPrefsStore`), so they carry across lectures and
 * sessions.
 *
 * Pointer maths stays in client coordinates against one `getBoundingClientRect`
 * of the player — safe here because the app zooms the webview page rather than
 * CSS-zooming a container (see docs/frontend.md).
 */
export function useTranscriptDock(containerRef: RefObject<HTMLDivElement | null>) {
  const dock = usePlayerPrefs((s) => s.dock);
  const height = usePlayerPrefs((s) => s.height);
  const width = usePlayerPrefs((s) => s.width);
  const setPrefs = usePlayerPrefs((s) => s.set);

  /** Edge highlighted under the pointer mid-drag; null when not dragging. */
  const [dropTarget, setDropTarget] = useState<Dock | null>(null);
  // Mirrored in a ref: the drop is committed from a pointer handler, and a
  // state updater is the wrong place for that side effect (StrictMode runs
  // updaters twice).
  const dropTargetRef = useRef<Dock | null>(null);

  /**
   * Mirrored into state because the panel needs it in its className: it slides
   * open and shut on a transition, and a transition during a resize drag means
   * the edge trailing 200ms behind the pointer.
   */
  const [resizing, setResizing] = useState(false);

  const drag = useRef<{ x: number; y: number; moved: boolean } | null>(null);
  const resize = useRef<{ dock: Dock; x: number; y: number; size: number } | null>(
    null,
  );

  const startDockDrag = useCallback((e: React.PointerEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    drag.current = { x: e.clientX, y: e.clientY, moved: false };
  }, []);

  const startResize = useCallback(
    (e: React.PointerEvent) => {
      if (e.button !== 0) return;
      e.preventDefault();
      (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
      resize.current = {
        dock,
        x: e.clientX,
        y: e.clientY,
        size: isVertical(dock) ? height : width,
      };
      setResizing(true);
      document.body.style.cursor = isVertical(dock) ? "row-resize" : "col-resize";
      document.body.style.userSelect = "none";
    },
    [dock, height, width],
  );

  useEffect(() => {
    // A trackpad emits pointermove faster than the display refreshes, and each
    // one here both forces layout (`getBoundingClientRect`) and sets state.
    // Coalescing to one frame is the difference between a smooth drag and a
    // drag that lays the player out several times per painted frame.
    let frame = 0;
    let pending: { clientX: number; clientY: number } | null = null;

    const apply = (e: { clientX: number; clientY: number }) => {
      const rect = containerRef.current?.getBoundingClientRect();

      if (resize.current && rect) {
        const r = resize.current;
        if (isVertical(r.dock)) {
          const dy = e.clientY - r.y;
          // Bottom dock grows upward, top dock downward.
          const next = r.size + (r.dock === "bottom" ? -dy : dy);
          setPrefs({
            height: Math.min(
              Math.max(MIN_H, next),
              Math.max(MIN_H, rect.height - KEEP_H),
            ),
          });
        } else {
          const dx = e.clientX - r.x;
          const next = r.size + (r.dock === "right" ? -dx : dx);
          setPrefs({
            width: Math.min(
              Math.max(MIN_W, next),
              Math.max(MIN_W, rect.width - KEEP_W),
            ),
          });
        }
        return;
      }

      if (drag.current && rect) {
        if (
          !drag.current.moved &&
          Math.abs(e.clientX - drag.current.x) < DRAG_SLOP &&
          Math.abs(e.clientY - drag.current.y) < DRAG_SLOP
        ) {
          return;
        }
        if (!drag.current.moved) {
          drag.current.moved = true;
          document.body.style.cursor = "grabbing";
          document.body.style.userSelect = "none";
        }
        const target = nearestEdge(rect, e.clientX, e.clientY);
        dropTargetRef.current = target;
        setDropTarget(target);
      }
    };

    const onMove = (e: PointerEvent) => {
      if (!drag.current && !resize.current) return;
      pending = { clientX: e.clientX, clientY: e.clientY };
      if (frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        if (pending) apply(pending);
      });
    };

    const onUp = () => {
      // Drop any queued move, so a stale coordinate cannot re-open the drop
      // preview after the pointer is already up. What was previewed is what
      // gets committed.
      if (frame) {
        cancelAnimationFrame(frame);
        frame = 0;
      }
      pending = null;
      if (resize.current) {
        resize.current = null;
        setResizing(false);
      }
      if (drag.current) {
        if (drag.current.moved && dropTargetRef.current) {
          setPrefs({ dock: dropTargetRef.current });
        }
        dropTargetRef.current = null;
        setDropTarget(null);
        drag.current = null;
      }
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
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
  }, [containerRef, setPrefs]);

  return {
    dock,
    height,
    width,
    size: isVertical(dock) ? height : width,
    resizing,
    dropTarget,
    startDockDrag,
    startResize,
  };
}
