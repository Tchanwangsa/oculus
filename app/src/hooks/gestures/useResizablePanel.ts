import { useCallback, useEffect, useRef, useState } from "react";

interface Options {
  defaultWidth: number;
  minWidth: number;
  maxWidth: number;
  /** Docked edge; `"right"` negates the drag delta. Default `"left"`. */
  side?: "left" | "right";
  /** Snap collapsed when dragged below this. Default = minWidth / 2. */
  collapseThreshold?: number;
  /** Drawn width while collapsed: 0 hides the panel, more leaves a strip. */
  collapsedWidth?: number;
  storageKey?: string;
}

/**
 * A docked panel the user can drag wider and fold away, both remembered.
 * Collapsed is a state, not a width, so unfolding restores the last width.
 */
export function useResizablePanel({
  defaultWidth,
  minWidth,
  maxWidth,
  side = "left",
  collapseThreshold,
  collapsedWidth = 0,
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
  // State so the width transition can be disabled mid-drag.
  const [dragging, setDragging] = useState(false);

  const drag = useRef<{ x: number; w: number } | null>(null);

  const onMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    // From the visual width, so a drag out of the fold tracks the pointer.
    drag.current = { x: e.clientX, w: collapsed ? collapsedWidth : width };
    setDragging(true);
    document.body.style.cursor = "col-resize";
    document.body.style.webkitUserSelect = "none";
  }, [collapsed, collapsedWidth, width]);

  useEffect(() => {
    if (!dragging) return;
    const onMove = (e: MouseEvent) => {
      const d = drag.current;
      if (!d) return;
      const dx = side === "right" ? d.x - e.clientX : e.clientX - d.x;
      const next = Math.min(maxWidth, Math.max(0, d.w + dx));
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
      document.body.style.webkitUserSelect = "";
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
    /** What to draw: `collapsedWidth` while collapsed. */
    width: collapsed ? collapsedWidth : width,
    /** What it unfolds back to; lay content out at this so a fold clips it. */
    restWidth: width,
    collapsed,
    dragging,
    toggle,
    setCollapsed,
    onMouseDown,
  };
}
