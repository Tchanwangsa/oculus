import { useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";

/**
 * Files dropped onto one element, from the *window's* drop events rather than
 * the page's.
 *
 * Tauri's own drag-and-drop handler sits in front of the webview, so a file
 * dragged in from Finder never reaches a React `onDrop` — the page sees
 * nothing at all. Switching that handler off would hand file drops to WebKit
 * and take in-page dragging with it, which the dock's tab reorder still needs
 * (`dataTransfer.setData` in `ViewTabs`, CLAUDE.md), so the window's events are
 * the ones to listen to.
 *
 * They arrive with a **physical** cursor position and no target, so the
 * element has to be found by arithmetic: physical pixels over
 * `devicePixelRatio`, which on this app carries both the retina scale and the
 * webview's page zoom, gives the CSS coordinates a `getBoundingClientRect`
 * can be compared against.
 *
 * `over` is true while a drag is inside the element, for whatever the caller
 * wants to draw.
 */
export function useFileDrop(
  ref: React.RefObject<HTMLElement | null>,
  onDrop: (paths: string[]) => void,
) {
  const [over, setOver] = useState(false);
  // The handler is subscribed once; a fresh closure per render would
  // re-register the listener on every keystroke in the composer.
  const latest = useRef(onDrop);
  latest.current = onDrop;

  useEffect(() => {
    let dead = false;
    let unlisten: (() => void) | null = null;

    const inside = (p: { x: number; y: number }) => {
      const el = ref.current;
      if (!el) return false;
      const r = el.getBoundingClientRect();
      const scale = window.devicePixelRatio || 1;
      const x = p.x / scale;
      const y = p.y / scale;
      return x >= r.left && x <= r.right && y >= r.top && y <= r.bottom;
    };

    // Guarded because this is the one hook that reaches for the window
    // itself: outside the app — the dev server opened in a plain browser —
    // there is no webview to ask, and an effect that throws takes the
    // composer down with it.
    try {
      getCurrentWebview()
        .onDragDropEvent((e) => {
          const p = e.payload;
          if (p.type === "leave") {
            setOver(false);
            return;
          }
          if (p.type === "enter" || p.type === "over") {
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
    } catch {
      // No drop target here; paste still works.
    }

    return () => {
      dead = true;
      unlisten?.();
    };
  }, [ref]);

  return over;
}
