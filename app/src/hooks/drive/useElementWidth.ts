import { useLayoutEffect, useState, type RefObject } from "react";

/** An element's own width, kept current with a `ResizeObserver`: what the
 *  browser lays out by, since a split pane or page zoom makes viewport
 *  breakpoints wrong (`docs/ui.md`). */
export function useElementWidth(ref: RefObject<HTMLElement | null>): number {
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => setWidth(el.clientWidth);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, [ref]);
  return width;
}
