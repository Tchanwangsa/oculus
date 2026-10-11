import { useEffect, useRef, useState } from "react";

/** Frame widths come in steps of this, so a window resize does not redraw
 *  the stage for every pixel. */
const STEP = 80;
const MIN = 320;
/** `studio_component_frame` draws up to 4096 px; the frame is 1920 wide at
 *  most, which the stage never shows larger than. */
const MAX = 1920;

/** A ref for the stage's box and the PNG width to ask for: its width in
 *  device pixels, rounded to a step. 0 until it is measured. */
export function useStageWidth() {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => {
      const px = el.clientWidth * window.devicePixelRatio;
      setWidth(px > 0 ? Math.min(MAX, Math.max(MIN, Math.round(px / STEP) * STEP)) : 0);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  return [ref, width] as const;
}
