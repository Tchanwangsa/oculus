import { useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";

/** Pixels of slack around the target, so a near-miss still lands. */
const SLACK = 8;


/**
 * Files dropped onto one element, from Tauri's drag-and-drop events: Tauri's
 * handler sits in front of the webview, so Finder drops never reach a React
 * `onDrop`, and switching it off would hand in-page dragging to WebKit too.
 * Returns `over`: true while a drag is inside the element.
 */
export function useFileDrop(
  ref: React.RefObject<HTMLElement | null>,
  onDrop: (paths: string[]) => void,
) {
  const [over, setOver] = useState(false);
  // Subscribed once; a fresh closure per render would re-register it.
  const latest = useRef(onDrop);
  latest.current = onDrop;
  // Points → CSS pixels; 1 until measured (the unzoomed answer).
  const ratio = useRef(1);

  useEffect(() => {
    let dead = false;
    let unlisten: (() => void) | null = null;

    // Throws outside Tauri (dev server in a plain browser); don't take the
    // composer down with it.
    let view: ReturnType<typeof getCurrentWebview>;
    try {
      view = getCurrentWebview();
    } catch {
      // No drop target here; paste still works.
      return;
    }

    // The drop position is in points despite its `PhysicalPosition` type, so
    // the scale is measured, never `devicePixelRatio` (see CLAUDE.md).
    // Re-read on resize and on each drag enter, since page zoom changes it.
    const remeasure = () => {
      // `innerSize` is genuinely physical, so this is the window in points.
      Promise.all([view.window.innerSize(), view.window.scaleFactor()])
        .then(([size, scale]) => {
          const points = size.width / (scale || 1);
          if (dead || points <= 0 || window.innerWidth <= 0) return;
          ratio.current = window.innerWidth / points;
        })
        .catch(() => {});
    };
    remeasure();
    window.addEventListener("resize", remeasure);

    const inside = (p: { x: number; y: number }) => {
      const el = ref.current;
      if (!el) return false;
      // Background tabs are hidden with `visibility` (`TabPane.tsx`), so a
      // hidden composer still has a real rect under the visible one.
      if (getComputedStyle(el).visibility === "hidden") return false;
      const r = el.getBoundingClientRect();
      const x = p.x * ratio.current;
      const y = p.y * ratio.current;
      return (
        x >= r.left - SLACK &&
        x <= r.right + SLACK &&
        y >= r.top - SLACK &&
        y <= r.bottom + SLACK
      );
    };

    // Listen on the webview, not the window: a window listener is never
    // called in this app (see CLAUDE.md).
    view
      .onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === "leave") {
          setOver(false);
          return;
        }
        if (p.type === "enter") {
          // Zoom may have changed since the last drag.
          remeasure();
          setOver(inside(p.position));
          return;
        }
        if (p.type === "over") {
          setOver(inside(p.position));
          return;
        }
        setOver(false);
        if (inside(p.position)) latest.current(p.paths);
      })
      .then((un) => {
        if (dead) un();
        else unlisten = un;
      })
      .catch(() => {});

    return () => {
      dead = true;
      window.removeEventListener("resize", remeasure);
      unlisten?.();
    };
  }, [ref]);

  return over;
}
