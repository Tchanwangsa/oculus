import { useLayoutEffect, useRef, useState } from "react";

/** A ref for a box and whether its layout width is under `limit` px. False until measured. */
export function useNarrowBox(limit: number) {
  const ref = useRef<HTMLDivElement>(null);
  const [narrow, setNarrow] = useState(false);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => setNarrow(el.clientWidth > 0 && el.clientWidth < limit);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, [limit]);
  return [ref, narrow] as const;
}
