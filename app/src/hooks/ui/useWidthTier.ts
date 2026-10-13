import { useEffect, useRef, useState, type RefObject } from "react";

export type WidthTier = "narrow" | "medium" | "wide";

/** Lower bounds, in CSS px, of the medium and wide tiers. */
export interface WidthBreakpoints {
  medium: number;
  wide: number;
}

export function widthTier(width: number, { medium, wide }: WidthBreakpoints): WidthTier {
  return width >= wide ? "wide" : width >= medium ? "medium" : "narrow";
}

/**
 * Which tier an element's own width falls in, from a ResizeObserver's content
 * box (CSS px, so page zoom doesn't skew it), not the window's — panes split.
 * Null until the first measure. The element may mount after the hook (a
 * loading state first), so each render checks which one it observes.
 */
export function useWidthTier(
  ref: RefObject<HTMLElement | null>,
  breakpoints: WidthBreakpoints,
): WidthTier | null {
  const [tier, setTier] = useState<WidthTier | null>(null);
  const { medium, wide } = breakpoints;
  const observed = useRef<{ el: HTMLElement; ro: ResizeObserver; key: string } | null>(null);
  useEffect(() => {
    const el = ref.current;
    const key = `${medium}:${wide}`;
    const cur = observed.current;
    if (cur?.el === el && cur?.key === key) return;
    cur?.ro.disconnect();
    observed.current = null;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => {
      // A zero width is a hidden ancestor, not a narrow pane.
      if (!entry || entry.contentRect.width === 0) return;
      setTier(widthTier(entry.contentRect.width, { medium, wide }));
    });
    ro.observe(el);
    observed.current = { el, ro, key };
  });
  useEffect(
    () => () => {
      observed.current?.ro.disconnect();
      observed.current = null;
    },
    [],
  );
  return tier;
}
