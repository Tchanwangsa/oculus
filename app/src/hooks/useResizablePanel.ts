import { useCallback, useEffect, useRef, useState } from "react";

interface Options {
  defaultWidth: number;
  minWidth: number;
  maxWidth: number;
  /**
   * Which edge the panel is docked to. A right-docked panel grows as the
   * pointer moves *left*, so the drag delta is negated — without this the
   * handle pushes the panel away from the pointer and folds it shut.
   * Default `"left"`.
   */
  side?: "left" | "right";
  /** Snap collapsed when dragged below this. Default = minWidth / 2. */
  collapseThreshold?: number;
  storageKey?: string;
}

/**
 * A side panel the user can drag wider and fold away, with both remembered.
 *
 * Collapsed is a state, not a width: the panel keeps the width it had, so
 * folding and unfolding returns it to the size it was left at rather than to
 * the default. `width` is what to draw (0 when collapsed), `restWidth` is
 * what it goes back to — a collapse animation reads the second so the content
 * inside is clipped rather than squeezed on its way out.
 *
 * Dragging past the threshold folds the panel, and dragging back out of the
 * fold expands it again — the drag starts from 0 in that case, so the handle
 * stays under the pointer instead of jumping to the remembered width.
 */
export function useResizablePanel({
  defaultWidth,
  minWidth,
  maxWidth,
  side = "left",
  collapseThreshold,
  storageKey,
}: Options) {
  const threshold = collapseThreshold ?? Math.floor(minWidth / 2);
  const clamp = (w: number) => Math.min(maxWidth, Math.max(minWidth, w));

  const init = (): { width: number; collapsed: boolean } => {
    if (storageKey) {
      try {
        const w = Number(localStorage.getItem(storageKey));
        const c = localStorage.getItem(`${storageKey}-collapsed`) === "1";
        if (Number.isFinite(w) && w > 0) return { width: clamp(w), collapsed: c };
      } catch { /* ignore */ }
    }
    return { width: clamp(defaultWidth), collapsed: false };
  };

  const [state, setState] = useState(init);
  const { width, collapsed } = state;
  // Only for disabling the width transition mid-drag: a panel that eased
  // toward every mouse position lagged the handle by a frame and felt like
  // dragging elastic.
  const [dragging, setDragging] = useState(false);

  const drag = useRef<{ x: number; w: number } | null>(null);

  const onMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    // Start from the *visual* width, so a drag out of the fold tracks the
    // pointer from where the edge actually is.
    drag.current = { x: e.clientX, w: collapsed ? 0 : width };
    setDragging(true);
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }, [collapsed, width]);

  useEffect(() => {
    if (!dragging) return;
    const onMove = (e: MouseEvent) => {
      const d = drag.current;
      if (!d) return;
      const dx = side === "right" ? d.x - e.clientX : e.clientX - d.x;
      const next = Math.min(maxWidth, Math.max(0, d.w + dx));
      // Under the threshold the panel folds but keeps its width, so letting go
      // there and reopening does not reset it to the minimum.
      setState((prev) =>
        next < threshold
          ? prev.collapsed ? prev : { ...prev, collapsed: true }
          : { width: Math.max(minWidth, next), collapsed: false },
      );
    };
    const onUp = () => {
      drag.current = null;
      setDragging(false);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [dragging, threshold, minWidth, maxWidth, side]);

  useEffect(() => {
    if (!storageKey) return;
    localStorage.setItem(storageKey, String(width));
    localStorage.setItem(`${storageKey}-collapsed`, collapsed ? "1" : "0");
  }, [width, collapsed, storageKey]);

  const setCollapsed = useCallback(
    (next: boolean) => setState((prev) => (prev.collapsed === next ? prev : { ...prev, collapsed: next })),
    [],
  );
  const toggle = useCallback(() => setState((prev) => ({ ...prev, collapsed: !prev.collapsed })), []);

  return {
    /** What to draw: 0 while collapsed. */
    width: collapsed ? 0 : width,
    /** What it unfolds back to — the width the content is laid out at. */
    restWidth: width,
    collapsed,
    dragging,
    toggle,
    setCollapsed,
    onMouseDown,
  };
}
