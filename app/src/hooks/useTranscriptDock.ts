import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { capturePress, useFrameDrag } from "@/hooks/useFrameDrag";
import { isVertical, usePlayerPrefs, type Dock, type DockTab } from "@/stores/playerPrefsStore";

export { type Dock };

const MIN_H = 88;
const MIN_W = 220;
/** A higher floor while Chat is in front: replies and the composer need room. */
const CHAT_MIN_H = 260;
const CHAT_MIN_W = 300;

const minSize = (tab: DockTab, vertical: boolean) =>
  vertical ? (tab === "chat" ? CHAT_MIN_H : MIN_H) : tab === "chat" ? CHAT_MIN_W : MIN_W;

/** Room the video always keeps; also decides whether a side dock fits. */
const KEEP_H = 140;
const KEEP_W = 300;
/** Pointer travel before a header press counts as a dock drag, not a click. */
const DRAG_SLOP = 5;

/** The edge nearest the pointer; top/bottom only when `sides` won't fit. */
function nearestEdge(rect: DOMRect, x: number, y: number, sides: boolean): Dock {
  const fx = Math.min(1, Math.max(0, (x - rect.left) / rect.width));
  const fy = Math.min(1, Math.max(0, (y - rect.top) / rect.height));
  const d: [Dock, number][] = [
    ["top", fy],
    ["bottom", 1 - fy],
  ];
  if (sides) d.push(["left", fx], ["right", 1 - fx]);
  return d.reduce((a, b) => (b[1] < a[1] ? b : a))[0];
}

/**
 * Dock side + size for the lecture player's panel: drag the header to an edge,
 * drag the divider to resize. Stored in `playerPrefsStore`.
 */
export function useTranscriptDock(containerRef: RefObject<HTMLDivElement | null>) {
  const dock = usePlayerPrefs((s) => s.dock);
  const dockTab = usePlayerPrefs((s) => s.dockTab);
  const height = usePlayerPrefs((s) => s.height);
  const width = usePlayerPrefs((s) => s.width);
  const setPrefs = usePlayerPrefs((s) => s.set);

  const [dropTarget, setDropTarget] = useState<Dock | null>(null);
  // Committed from the pointer handler, not a state updater (StrictMode runs those twice).
  const dropTargetRef = useRef<Dock | null>(null);

  // State so the panel can drop its transition mid-drag.
  const [resizing, setResizing] = useState(false);

  const drag = useRef<{ x: number; y: number; moved: boolean } | null>(null);
  const resize = useRef<{ dock: Dock; x: number; y: number; size: number } | null>(
    null,
  );

  const startDockDrag = useCallback((e: React.PointerEvent) => {
    if (!capturePress(e)) return;
    drag.current = { x: e.clientX, y: e.clientY, moved: false };
  }, []);

  const [area, setArea] = useState({ width: 0, height: 0 });
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      setArea((a) => (a.width === width && a.height === height ? a : { width, height }));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [containerRef]);

  // A side dock that doesn't fit draws at the bottom; the preference is kept.
  const fitsBeside = area.width === 0 || area.width >= minSize(dockTab, false) + KEEP_W;
  const drawnDock: Dock = fitsBeside ? dock : "bottom";
  const vertical = isVertical(drawnDock);

  // The stored size clamped between the tab's floor and the container's room,
  // without rewriting the preference.
  const drawn = (() => {
    const min = minSize(dockTab, vertical);
    const measured = vertical ? area.height : area.width;
    const cap =
      measured > 0
        ? Math.max(min, measured - (vertical ? KEEP_H : KEEP_W))
        : Number.POSITIVE_INFINITY;
    return Math.min(Math.max(min, vertical ? height : width), cap);
  })();

  const startResize = useCallback(
    (e: React.PointerEvent) => {
      if (!capturePress(e)) return;
      // Resize the dock as drawn, not as stored.
      resize.current = { dock: drawnDock, x: e.clientX, y: e.clientY, size: drawn };
      setResizing(true);
      document.body.style.cursor = vertical ? "row-resize" : "col-resize";
      document.body.style.userSelect = "none";
    },
    [drawnDock, vertical, drawn],
  );

  useFrameDrag({
    active: () => !!(drag.current || resize.current),
    move: (e) => {
      const rect = containerRef.current?.getBoundingClientRect();

      if (resize.current && rect) {
        const r = resize.current;
        if (isVertical(r.dock)) {
          const min = minSize(dockTab, true);
          const dy = e.clientY - r.y;
          const next = r.size + (r.dock === "bottom" ? -dy : dy);
          setPrefs({
            height: Math.min(Math.max(min, next), Math.max(min, rect.height - KEEP_H)),
          });
        } else {
          const min = minSize(dockTab, false);
          const dx = e.clientX - r.x;
          const next = r.size + (r.dock === "right" ? -dx : dx);
          setPrefs({
            width: Math.min(Math.max(min, next), Math.max(min, rect.width - KEEP_W)),
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
        const target = nearestEdge(rect, e.clientX, e.clientY, fitsBeside);
        dropTargetRef.current = target;
        setDropTarget(target);
      }
    },
    end: () => {
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
    },
  });

  return {
    dock: drawnDock,
    height,
    width,
    size: drawn,
    resizing,
    dropTarget,
    startDockDrag,
    startResize,
  };
}
