import { useCallback, useEffect, useRef, useState } from "react";

interface Options {
  defaultWidth: number;
  minWidth: number;
  maxWidth: number;
  /** Snap collapsed when dragged below this. Default = minWidth / 2. */
  collapseThreshold?: number;
  storageKey?: string;
}

export function useResizablePanel({
  defaultWidth,
  minWidth,
  maxWidth,
  collapseThreshold,
  storageKey,
}: Options) {
  const threshold = collapseThreshold ?? Math.floor(minWidth / 2);

  const init = (): { width: number; collapsed: boolean } => {
    if (storageKey) {
      try {
        const w = Number(localStorage.getItem(storageKey));
        const c = localStorage.getItem(`${storageKey}-collapsed`) === "1";
        if (!isNaN(w) && w > 0) return { width: w, collapsed: c };
      } catch { /* ignore */ }
    }
    return { width: defaultWidth, collapsed: false };
  };

  const initState = init();
  const [width, setWidth] = useState(initState.width);
  const [collapsed, setCollapsed] = useState(initState.collapsed);

  // Visual width: 0 when collapsed, actual width otherwise.
  const visualWidth = collapsed ? 0 : width;

  const dragging = useRef(false);
  const startX = useRef(0);
  // Start from visual width so drag from 0 expands naturally.
  const startW = useRef(0);

  const onMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    dragging.current = true;
    startX.current = e.clientX;
    startW.current = collapsed ? 0 : width;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }, [collapsed, width]);

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      if (!dragging.current) return;
      const delta = e.clientX - startX.current;
      const next = Math.min(maxWidth, Math.max(0, startW.current + delta));
      if (next < threshold) {
        setCollapsed(true);
        // Keep width at last good value for restore when dragged back out.
      } else {
        setCollapsed(false);
        setWidth(Math.max(minWidth, next));
      }
    };
    const onUp = () => {
      if (!dragging.current) return;
      dragging.current = false;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [threshold, minWidth, maxWidth]);

  useEffect(() => {
    if (!storageKey) return;
    localStorage.setItem(storageKey, String(width));
    localStorage.setItem(`${storageKey}-collapsed`, collapsed ? "1" : "0");
  }, [width, collapsed, storageKey]);

  return { width: visualWidth, collapsed, onMouseDown };
}
