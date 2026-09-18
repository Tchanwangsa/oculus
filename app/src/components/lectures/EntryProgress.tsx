import { useEffect, useRef, type RefObject } from "react";

/** How often the fill is re-measured against the playhead, in ms. Paired with
 *  a linear CSS transition of the same length, so the line crawls instead of
 *  stepping — and so a seek slides rather than teleports. */
const TICK = 200;

export interface EntryProgressProps {
  /** The playhead, as a ref rather than a number. Both docks' panels are
   *  memoised against a player that re-renders four times a second, and a
   *  `currentTime` prop would throw that memo away on every frame — see
   *  `atRef` in `LecturePlayer.tsx`. */
  atRef: RefObject<number>;
  /** The entry's span. `end` is derived (`chapterEnds`), never stored. */
  start: number;
  end: number;
}

/**
 * How far through *this* entry the playhead is, drawn across the top edge of
 * the card it belongs to.
 *
 * **This replaced the strip above the scrub bar.** That strip said the same
 * thing over the video, where it had to be white-on-frame and fade with the
 * controls; here it is the brand colour, it is next to the title it is
 * measuring, and it survives the control bar fading out. The scrub bar still
 * answers "how much of the lecture is left"; this answers "how much of this
 * bit".
 *
 * Only the entry being played gets one — a track on all twelve would read as a
 * ladder rather than as a playhead, and the highlight already says which card
 * is current.
 *
 * **It writes its own width and never re-renders.** A component that set state
 * five times a second would re-render the whole list beside a decoding video
 * for one style property, so the interval writes to the node directly. 2px
 * rather than the app's usual hairline: this sits on the `brand/12` wash of the
 * active card, where a single pixel of indigo all but disappears.
 */
export function EntryProgress({ atRef, start, end }: EntryProgressProps) {
  const fill = useRef<HTMLSpanElement | null>(null);

  useEffect(() => {
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
  }, [atRef, start, end]);

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
