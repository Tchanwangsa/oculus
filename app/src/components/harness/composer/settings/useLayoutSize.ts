import { useLayoutEffect, useState } from "react";

export interface LayoutSize {
  width: number;
  height: number;
}

/**
 * An element's layout size, kept current with a `ResizeObserver`. Read from
 * `offsetWidth`/`offsetHeight`, never `getBoundingClientRect`, which page
 * zoom scales. Null until the element is measured; the first reading lands
 * before paint.
 */
export function useLayoutSize(el: HTMLElement | null): LayoutSize | null {
  const [size, setSize] = useState<LayoutSize | null>(null);
  useLayoutEffect(() => {
    if (!el) return;
    const read = () =>
      setSize((prev) => {
        const next = { width: el.offsetWidth, height: el.offsetHeight };
        return prev && prev.width === next.width && prev.height === next.height ? prev : next;
      });
    read();
    const observer = new ResizeObserver(read);
    observer.observe(el);
    return () => observer.disconnect();
  }, [el]);
  return size;
}
