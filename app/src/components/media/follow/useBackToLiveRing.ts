import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { IDLE_RESYNC_MS, RING_STROKE } from "@/components/media/follow/constants";

/** The "Back to live" pill's countdown ring: the pill's measured box, the
 *  stadium outline's geometry, and the animation that drains it. */
export function useBackToLiveRing(ringAnimRef: RefObject<Animation | null>) {
  const pillRef = useRef<HTMLButtonElement>(null);
  const ringRef = useRef<SVGRectElement>(null);

  // Sized from the pill's rendered box. It is faded, not unmounted, so it
  // measures while hidden.
  const [pill, setPill] = useState({ w: 0, h: 0 });
  useEffect(() => {
    const el = pillRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      const r = el.getBoundingClientRect();
      setPill((p) => (p.w === r.width && p.h === r.height ? p : { w: r.width, h: r.height }));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // A stadium's perimeter by hand: `<rect>.getTotalLength()` is SVG2.
  const ringW = Math.max(0, pill.w - RING_STROKE);
  const ringH = Math.max(0, pill.h - RING_STROKE);
  const ringLen = 2 * Math.max(0, ringW - ringH) + Math.PI * ringH;

  // Web Animations, not state: it restarts on every scroll.
  const startRing = useCallback(() => {
    ringAnimRef.current?.cancel();
    const el = ringRef.current;
    if (!el || ringLen <= 0) return;
    // Offset eats the outline over the idle window: full → bare.
    ringAnimRef.current = el.animate(
      [{ strokeDashoffset: 0 }, { strokeDashoffset: ringLen }],
      { duration: IDLE_RESYNC_MS, easing: "linear", fill: "forwards" },
    );
  }, [ringLen]);

  return { pillRef, ringRef, pill, ringW, ringH, ringLen, startRing };
}
