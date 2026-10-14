import { useCallback, useEffect, useRef, useState } from "react";

/** Sizes are along the drag axis: a width, or for `top`/`bottom` a height. */
interface Options {
  defaultWidth: number;
  minWidth: number;
  maxWidth: number;
  /** Docked edge; `"right"` and `"bottom"` negate the drag delta, `"top"`
   *  and `"bottom"` drag vertically. Default `"left"`. */
  side?: "left" | "right" | "top" | "bottom";
  /** Snap collapsed when dragged below this. Default = minWidth / 2. */
  collapseThreshold?: number;
  /** Drawn width while collapsed: 0 hides the panel, more leaves a strip. */
  collapsedWidth?: number;
  /** Folded until the user first unfolds it. Default false. */
  defaultCollapsed?: boolean;
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
  defaultCollapsed = false,
  storageKey,
}: Options) {
  const threshold = collapseThreshold ?? Math.floor(minWidth / 2);
  const vertical = side === "top" || side === "bottom";
  const clamp = (w: number) => Math.min(maxWidth, Math.max(minWidth, w));

  const init = (): { width: number; collapsed: boolean } => {
    if (storageKey) {
      try {
        const w = Number(localStorage.getItem(storageKey));
        const c = localStorage.getItem(`${storageKey}-collapsed`) === "1";
        if (Number.isFinite(w) && w > 0) return { width: clamp(w), collapsed: c };
      } catch { /* ignore */ }
    }
    return { width: clamp(defaultWidth), collapsed: defaultCollapsed };
  };

  const [state, setState] = useState(init);
  const { width, collapsed } = state;
  // State so the width transition can be disabled mid-drag.
  const [dragging, setDragging] = useState(false);

  const drag = useRef<{ x: number; w: number } | null>(null);

  const onMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    // From the visual width, so a drag out of the fold tracks the pointer.
    drag.current = { x: vertical ? e.clientY : e.clientX, w: collapsed ? collapsedWidth : width };
    setDragging(true);
    document.body.style.cursor = vertical ? "row-resize" : "col-resize";
    document.body.style.webkitUserSelect = "none";
  }, [collapsed, collapsedWidth, width, vertical]);

  useEffect(() => {
    if (!dragging) return;
    let frame = 0;
    let latest: number | null = null;
    const apply = () => {
      frame = 0;
      if (latest == null) return;
      const next = latest;
      setState((prev) => {
        if (next < threshold) return prev.collapsed ? prev : { ...prev, collapsed: true };
        const width = Math.max(minWidth, next);
        return !prev.collapsed && prev.width === width ? prev : { width, collapsed: false };
      });
    };
    const onMove = (e: MouseEvent) => {
      const d = drag.current;
      if (!d) return;
      const at = vertical ? e.clientY : e.clientX;
      const dx = side === "right" || side === "bottom" ? d.x - at : at - d.x;
      latest = Math.min(maxWidth, Math.max(0, d.w + dx));
      // Pointer samples outpace paint; reflow the neighbouring page once per frame.
      if (!frame) frame = requestAnimationFrame(apply);
    };
    const onUp = () => {
      cancelAnimationFrame(frame);
      // A release can arrive before paint: keep its final width.
      apply();
      drag.current = null;
      setDragging(false);
      document.body.style.cursor = "";
      document.body.style.webkitUserSelect = "";
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      drag.current = null;
      document.body.style.cursor = "";
      document.body.style.webkitUserSelect = "";
    };
  }, [dragging, threshold, minWidth, maxWidth, side, vertical]);

  useEffect(() => {
    // Storage is synchronous; save a drag once it ends, never on its frames.
    if (!storageKey || dragging) return;
    try {
      localStorage.setItem(storageKey, String(width));
      localStorage.setItem(`${storageKey}-collapsed`, collapsed ? "1" : "0");
    } catch { /* The live panel still works when storage is unavailable. */ }
  }, [width, collapsed, dragging, storageKey]);

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
