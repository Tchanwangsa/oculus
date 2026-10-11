import { useLayoutEffect, useState, type RefObject } from "react";

/** The scroller's client size and the device pixel ratio, kept current. */
export function usePdfViewSize(containerRef: RefObject<HTMLDivElement | null>) {
  const [view, setView] = useState({ width: 0, height: 0 });
  const [dpr, setDpr] = useState(() => window.devicePixelRatio || 1);

  useLayoutEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const measure = () => {
      setView((v) =>
        v.width === el.clientWidth && v.height === el.clientHeight
          ? v
          : { width: el.clientWidth, height: el.clientHeight },
      );
      // A page-zoom change moves the ratio and resizes the view together.
      setDpr(window.devicePixelRatio || 1);
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [containerRef]);

  return { view, dpr };
}
