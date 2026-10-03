import { useEffect, useRef, type RefObject } from "react";
import { useTabActive } from "@/components/tabs/TabContext";

/** Re-measure interval in ms, matched by a linear CSS transition so the line
 *  crawls and a seek slides. */
const TICK = 200;

export interface EntryProgressProps {
  /** The playhead as a ref — see `atRef` in `LecturePlayer.tsx`. */
  atRef: RefObject<number>;
  /** The entry's span. `end` is derived (`chapterEnds`), never stored. */
  start: number;
  end: number;
}

/**
 * How far through the playing entry the playhead is, across its card's top
 * edge. Writes its own width on an interval and never re-renders. 2px, since a
 * hairline disappears on the active card's `brand/12` wash.
 */
export function EntryProgress({ atRef, start, end }: EntryProgressProps) {
  const active = useTabActive();
  const fill = useRef<HTMLSpanElement | null>(null);

  useEffect(() => {
    if (!active) return;
    const span = Math.max(1, end - start);
    const paint = () => {
      const node = fill.current;
      if (!node) return;
      const pct = ((atRef.current - start) / span) * 100;
      node.style.width = `${Math.min(100, Math.max(0, pct))}%`;
    };
    paint();
    const t = setInterval(paint, TICK);
    return () => clearInterval(t);
  }, [atRef, start, end, active]);

  return (
    <span
      aria-hidden
      className="pointer-events-none absolute inset-x-0 top-0 h-[2px] overflow-hidden rounded-full bg-brand/20"
    >
      <span
        ref={fill}
        className="absolute inset-y-0 left-0 rounded-full bg-brand ease-linear"
        style={{ width: 0, transition: `width ${TICK}ms linear` }}
      />
    </span>
  );
}
